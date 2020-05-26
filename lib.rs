//! Syslog drain for slog-rs
//! 
//! # Example
//!
//! ```
//! extern crate slog;
//! extern crate slog_syslog;
//!
//! use slog::*;
//! use slog_syslog::Facility;
//!
//! fn main() {
//!     let o = o!("build-id" => "8dfljdf");
//!
//!     // log to the local syslog daemon
//!     match slog_syslog::SyslogBuilder::new()
//!         .facility(Facility::LOG_USER)
//!         .start() {
//!         Ok(x) => {
//!             let root = Logger::root(x.fuse(), o);
//!         },
//!         Err(e) => eprintln!("Failed to start syslog. Error {:?}", e)
//!     };
//! }
//! ```
#![warn(missing_docs)]

extern crate hostname;
#[cfg_attr(test, macro_use)] // Slog macros are only used in tests.
extern crate slog;
extern crate syslog;

use slog::{Drain, Level, OwnedKVList, Record};
use std::{env, fmt, process};
use std::cell::{Cell, RefCell};
use std::error::Error as StdError;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[cfg(test)]
use std::iter;

#[cfg(not(unix))]
use std::net::Ipv4Addr;

use slog::KV;

pub use syslog::Facility;

/// Implements `Display` with a closure.
struct ClosureAsDisplay<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result>(F);
impl<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result> fmt::Display for ClosureAsDisplay<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0(f)
    }
}

/// [`Drain`] that writes log records to a [`syslog::Logger`].
/// 
/// [`SyslogBuilder`] provides a convenient API for constructing this.
/// 
/// This drain is not thread-safe (that is, it does not implement [`Sync`]). It cannot be directly used as the `Drain` underlying a [`slog::Logger`]. It must be wrapped in a [`Mutex`], a [`slog_async::Async`] (from the [slog-async] crate), or some other synchronization mechanism.
/// 
/// [`Drain`]: https://docs.rs/slog/2/slog/trait.Drain.html
/// [`Mutex`]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
/// [slog-async]: https://docs.rs/slog-async/2/slog_async/index.html
/// [`slog_async::Async`]: https://docs.rs/slog-async/2/slog_async/struct.Async.html
/// [`slog::Logger`]: https://docs.rs/slog/2/slog/struct.Logger.html
/// [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
/// [`SyslogBuilder`]: struct.SyslogBuilder.html
/// [`syslog::Logger`]: https://docs.rs/syslog/5/syslog/struct.Logger.html
pub struct Streamer3164<B: Write, F: MsgFormat3164 = BasicMsgFormat3164> {
    io: RefCell<syslog::Logger<B, syslog::Formatter3164>>,

    msg_format: F,

    level_filter: Option<Level>,
}

impl<B: Write> Streamer3164<B> {
    /// Creates a new `Streamer3164` using the given [`syslog::Logger`].
    /// 
    /// The new `Streamer3164` uses [`BasicMsgFormat3164`] for formatting log messages and associated key-value pairs. To use a different format, give it to the [`with_msg_format`] method.
    /// 
    /// [`BasicMsgFormat3164`]: struct.BasicMsgFormat3164.html
    /// [`syslog::Logger`]: https://docs.rs/syslog/5/syslog/struct.Logger.html
    /// [`with_msg_format`]: #method.with_msg_format
    pub fn new(logger: syslog::Logger<B, syslog::Formatter3164>) -> Self {
        Streamer3164 {
            io: RefCell::new(logger),
            msg_format: BasicMsgFormat3164,
            level_filter: None,
        }
    }
}

impl<B: Write, F: MsgFormat3164> Streamer3164<B, F> {
    /// Replaces this `Streamer3164` with one that uses the given [`MsgFormat3164`].
    /// 
    /// [`MsgFormat3164`]: trait.MsgFormat3164.html
    pub fn with_msg_format<F2: MsgFormat3164>(self, msg_format: F2) -> Streamer3164<B, F2> {
        Streamer3164 {
            io: self.io,
            msg_format,
            level_filter: self.level_filter,
        }
    }

    /// Borrows the [`MsgFormat3164`] being used by this `Streamer3164`.
    /// 
    /// [`MsgFormat3164`]: trait.MsgFormat3164.html
    pub fn msg_format(&self) -> &F {
        &self.msg_format
    }
}

impl<B: Write, F: MsgFormat3164> Drain for Streamer3164<B, F> {
    type Err = syslog::Error;
    type Ok = ();

    fn log(&self, info: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        if let Some(level_filter) = self.level_filter {
            if !info.level().is_at_least(level_filter) {
                return Ok(());
            }
        }

        let mut guard = self.io.borrow_mut();
        let logger = &mut *guard;

        // We need to smuggle errors out of the `Display` implementation for `msg`, below. The `Display::fmt` method takes `&self` rather than `&mut self`, so the obvious way of accomplishing this (`let mut msg_format_result: syslog::Result<()>`) will not work because the closure cannot write to any captured variable (including `msg_format_result`).
        //
        // Instead, we use a `Cell` to store the error. This is a somewhat unconventional use for `Cell` (which usually appears as a `struct` member, not a local variable like this), but it fits: it creates a mutable place in memory for us to store the error.
        let msg_format_result = Cell::<slog::Result>::new(Ok(()));

        // `syslog::Formatter3164` accepts any `T: Display` as a log message. The easy way to work with this would be to just allocate a new `String`, write both the KVs and the message into it, then submit that to the `Formatter3164`.
        //
        // But that would involve at least one and probably several unnecessary allocations. Instead, we'll give it this custom `Display` implementation, which writes the KVs and log message piece-by-piece to the output.
        //
        // This still isn't zero-copy, as the `core::fmt` module still allocates a buffer to write the entire syslog packet into, but one copy is still faster than two copies.
        let msg = ClosureAsDisplay(|f| {
            self.msg_format.fmt(f, info, values)
            .map_err(|error| {
                // `Cell::replace` returns the old value, which in this case is a `Result`. We're dropping it, but this triggers a warning by default, so we use `let _ =` to inform Rust that yes, we really do mean to drop it.
                let _ = msg_format_result.replace(Err(/*match*/ error /*{
                    // `std::fmt::Error` contains no information at all, so just pass it through.
                    slog::Error::Fmt(fmt::Error) => return fmt::Error,

                    // Any other error needs to be preserved.
                    slog::Error::Io(error) => syslog::Error::from_kind(syslog::ErrorKind::Io(error)),
                    error => {
                        let kind = syslog::ErrorKind::Msg(error.to_string());
                        syslog::Error::with_chain(error, kind)
                    }
                }*/));
                fmt::Error
            })
        });

        // Submit the message to syslog.
        let log_result = match info.level() {
            Level::Critical => logger.crit(msg),
            Level::Error => logger.err(msg),
            Level::Warning => logger.warning(msg),
            Level::Info => logger.info(msg),
            Level::Debug | Level::Trace => logger.debug(msg),
        };

        // Extract the result stored in `msg_format_result`.
        //
        // We'd use `Cell::take` instead, but `Result<(), _>` does not implement `Default`, so we use this instead. `Cell::replace` returns whatever's currently stored in the cell, same as `take`, so that works too.
        //
        // `Result<T, _>` arguably should implement `Default` for any `T: Default`, but it doesn't, so whatever.
        let msg_format_result = msg_format_result.replace(Ok(()));

        // Now, we have two results: one from `logger`, and one from `msg`. Figure out which to return.
        {
            // A significant error is one that isn't just `std::fmt::Error`, which contains no information.
            fn has_significant_error(result: &syslog::Result<()>) -> bool {
                if let Err(error) = result {
                    if let Some(source) = error.source() {
                        if !source.is::<fmt::Error>() {
                            return true;
                        }
                    }
                }

                false
            }

            // If `log_result` contains a significant error, then return it.
            if has_significant_error(&log_result) {
                log_result
            }
            // If `msg_format_result` contains an error, translate it into a `syslog::Error` and return that.
            else if let Err(msg_format_err) = msg_format_result {
                Err(match msg_format_err {
                    error @ slog::Error::Fmt(fmt::Error) => syslog::Error::with_chain(error, syslog::ErrorKind::Format),
                    slog::Error::Io(error) => syslog::Error::from_kind(syslog::ErrorKind::Io(error)),
                    error => {
                        let kind = syslog::ErrorKind::Msg(error.to_string());
                        syslog::Error::with_chain(error, kind)
                    }
                })
            }
            // Otherwise, just return the `log_result`.
            else {
                log_result
            }
        }
    }
}

impl<B: Write> From<syslog::Logger<B, syslog::Formatter3164>> for Streamer3164<B> {
    fn from(logger: syslog::Logger<B, syslog::Formatter3164>) -> Self {
        Self::new(logger)
    }
}

/// A way to format log messages with structured data for the BSD syslog protocol.
/// 
/// The BSD syslog protocol, as described by [RFC 3164], does not support structured log data. If Slog key-value pairs are to be included with log messages, they must be included as part of the message. Implementations of this trait determine if and how this will be done.
/// 
/// [RFC 3164]: https://tools.ietf.org/html/rfc3164
pub trait MsgFormat3164 {
    /// Formats a log message and its key-value pairs into the given `Formatter`.
    /// 
    /// This method is only formatting the `CONTENT` part of a syslog message.
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result;
}

impl<T: MsgFormat3164 + ?Sized> MsgFormat3164 for &T {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        (**self).fmt(f, record, values)
    }
}

impl<T: MsgFormat3164 + ?Sized> MsgFormat3164 for Box<T> {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        (**self).fmt(f, record, values)
    }
}

impl<T: MsgFormat3164 + ?Sized> MsgFormat3164 for Rc<T> {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        (**self).fmt(f, record, values)
    }
}

impl<T: MsgFormat3164 + ?Sized> MsgFormat3164 for Arc<T> {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        (**self).fmt(f, record, values)
    }
}

/// An implementation of [`MsgFormat3164`] that discards the key-value pairs and logs only the [`msg`] part of a log [`Record`].
/// 
/// [`msg`]: https://docs.rs/slog/2/slog/struct.Record.html#method.msg
/// [`MsgFormat3164`]: trait.MsgFormat3164.html
/// [`Record`]: https://docs.rs/slog/2/slog/struct.Record.html
#[derive(Clone, Copy, Debug, Default)]
pub struct NullMsgFormat3164;
impl MsgFormat3164 for NullMsgFormat3164 {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, _: &OwnedKVList) -> slog::Result {
        write!(f, "{}", record.msg())?;
        Ok(())
    }
}

/// Copies input to output, but escapes characters as prescribed by RFC 5424 for PARAM-VALUEs.
struct Rfc5424LikeValueEscaper<W: fmt::Write>(W);

impl<W: fmt::Write> fmt::Write for Rfc5424LikeValueEscaper<W> {
    fn write_str(&mut self, mut s: &str) -> fmt::Result {
        while let Some(index) = s.find(|c| c == '\\' || c == '"' || c == ']') {
            if index != 0 {
                self.0.write_str(&s[..index])?;
            }

            // All three delimiters are ASCII characters, so this won't have bogus results.
            self.write_char(s.as_bytes()[index] as char)?;

            if s.len() >= index {
                s = &s[(index + 1)..];
            }
            else {
                s = &"";
                break;
            }
        }

        if !s.is_empty() {
            self.0.write_str(s)?;
        }

        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        match c {
            '\\' => self.0.write_str(r"\\"),
            '"' => self.0.write_str("\\\""),
            ']' => self.0.write_str("\\]"),
            _ => write!(self.0, "{}", c)
        }
    }
}

#[test]
fn test_rfc_5424_like_value_escaper() {
    fn case(input: &str, expected_output: &str) {
        let mut e = Rfc5424LikeValueEscaper(String::new());
        fmt::Write::write_str(&mut e, input).unwrap();
        assert_eq!(e.0, expected_output);
    }

    // Test that each character is properly escaped.
    for c in &['\\', '"', ']'] {
        let ec = format!("\\{}", c);

        {
            let input = format!("{}", c);
            case(&*input, &*ec);
        }

        for at_start_count in 0..=2 {
        for at_mid_count in 0..=2 {
        for at_end_count in 0..=2 {
            // First, we assemble the input and expected output strings.
            let mut input = String::new();
            let mut expected_output = String::new();

            // Place the symbol(s) at the beginning of the strings.
            input.extend(iter::repeat(c).take(at_start_count));
            expected_output.extend(iter::repeat(&*ec).take(at_start_count));

            // First plain text.
            input.push_str("foo");
            expected_output.push_str("foo");

            // Middle symbol(s).
            input.extend(iter::repeat(c).take(at_mid_count));
            expected_output.extend(iter::repeat(&*ec).take(at_mid_count));

            // Second plain text.
            input.push_str("bar");
            expected_output.push_str("bar");

            // End symbol(s).
            input.extend(iter::repeat(c).take(at_end_count));
            expected_output.extend(iter::repeat(&*ec).take(at_end_count));

            // Finally, test this combination.
            case(&*input, &*expected_output);
        }}}
    }

    case("", "");
    case("foo", "foo");
    case("[foo]", "[foo\\]");
    case("\\\"]", "\\\\\\\"\\]"); // \"] ⇒ \\\"\]
}

/// An implementation of [`MsgFormat3164`] that formats the key-value pairs of a log [`Record`] similarly to [RFC 5424].
/// 
/// # Not really RFC 5424
/// 
/// This does not actually generate conformant RFC 5424 STRUCTURED-DATA. The differences are:
/// 
/// * All key-value pairs are placed into a single SD-ELEMENT.
/// * The SD-ELEMENT does not contain an SD-ID, only SD-PARAMs.
/// * PARAM-NAMEs are encoded in UTF-8, not ASCII.
/// * Forbidden characters in PARAM-NAMEs are not filtered out, nor is an error raised if a key contains such characters.
/// 
/// # Example output
/// 
/// Given a log message `Hello, world!`, where the key `key1` has the value `value1` and `key2` has the value `value2`, the formatted message will be `Hello, world! [key1="value1" key2="value2"]` (possibly with `key2` first instead of `key1`).
/// 
/// [`MsgFormat3164`]: trait.MsgFormat3164.html
/// [`Record`]: https://docs.rs/slog/2/slog/struct.Record.html
/// [RFC 5424]: https://tools.ietf.org/html/rfc5424
#[derive(Clone, Copy, Debug, Default)]
pub struct BasicMsgFormat3164;
impl MsgFormat3164 for BasicMsgFormat3164 {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        struct Basic3164Serializer<'a, 'b> {
            f: &'a mut fmt::Formatter<'b>,
            is_first_kv: bool,
        }
        
        impl<'a, 'b> Basic3164Serializer<'a, 'b> {
            fn new(f: &'a mut fmt::Formatter<'b>) -> Self {
                Self { f, is_first_kv: true }
            }
        
            fn finish(&mut self) -> slog::Result {
                if !self.is_first_kv {
                    write!(self.f, "]")?;
                }
                Ok(())
            }
        }
        
        impl<'a, 'b> slog::Serializer for Basic3164Serializer<'a, 'b> {
            fn emit_arguments(&mut self, key: slog::Key, val: &fmt::Arguments) -> slog::Result {
                use fmt::Write;

                self.f.write_str(if self.is_first_kv {" ["} else {" "})?;
                self.is_first_kv = false;

                // Write the key unaltered, but escape the value.
                //
                // RFC 5424 does not allow space, ']', '"', or '\' to appear in PARAM-NAMEs, and does not allow such characters to be escaped.
                write!(self.f, "{}=\"", key)?;
                write!(Rfc5424LikeValueEscaper(&mut self.f), "{}", val)?;
                self.f.write_char('"')?;
                Ok(())
            }
        }

        write!(f, "{}", record.msg())?;

        {
            let mut serializer = Basic3164Serializer::new(f);

            values.serialize(record, &mut serializer)?;
            record.kv().serialize(record, &mut serializer)?;
            serializer.finish()?;
        }

        Ok(())
    }
}

/// Makes sure the example output for `BasicMsgFormat3164` is what it actually generates.
#[test]
fn test_basic_msg_format_3164() {
    let result = ClosureAsDisplay(|f| {
        BasicMsgFormat3164.fmt(
            f,
            &record!(
                Level::Info,
                "",
                &format_args!("Hello, world!"),
                b!("key1" => "value1", "key2" => "value2")
            ),
            &o!().into()
        ).unwrap();
        Ok(())
    }).to_string();

    assert!(
        // The KVs' order is not well-defined, so they might get reversed.
        result == "Hello, world! [key1=\"value1\" key2=\"value2\"]" ||
        result == "Hello, world! [key2=\"value2\" key1=\"value1\"]"
    );
}

#[doc(hidden)]
#[deprecated(since = "0.13.0", note = "no longer used")]
pub struct Format3164;

#[allow(deprecated)]
#[doc(hidden)]
impl Format3164 {
    pub fn new() -> Self {
        Format3164
    }
}

#[derive(Clone, Debug)]
enum SyslogKind {
    UnixDefault,
    Unix {
        path: PathBuf,
    },
    Tcp {
        server: SocketAddr,
    },
    Udp {
        local: SocketAddr,
        host: SocketAddr,
    },
}
impl Default for SyslogKind {
    #[cfg(unix)]
    fn default() -> Self {
        SyslogKind::UnixDefault
    }

    #[cfg(not(unix))]
    fn default() -> Self {
        SyslogKind::Udp {
            local: SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 0),
            host: SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 514)
        }
    }
}

/// Builder pattern for constructing a syslog drain that uses RFC 3164 (BSD) style.
/// 
/// All settings have default values. `SyslogBuilder::new().start()` will give you a sensibly configured log drain, but you might especially want to customize the `facility`.
/// 
/// Default settings are:
/// 
/// * Facility: `LOG_USER`
/// * Level: all
/// * Transport (Unix-like platforms): Unix socket `/dev/log` or `/var/run/log`
/// * Transport (other platforms): UDP to 127.0.0.1:514
/// * Message format: [`BasicMsgFormat3164`]
/// * Process name: the file name portion of [`std::env::current_exe()`]
/// * PID: [`std::process::id()`]
/// * Hostname: [`hostname::get()`]
/// 
/// [`BasicMsgFormat3164`]: struct.BasicMsgFormat3164.html
/// [`std::env::current_exe()`]: https://doc.rust-lang.org/std/env/fn.current_exe.html
/// [`std::process::id()`]: https://doc.rust-lang.org/std/process/fn.id.html
/// [`hostname::get()`]: https://docs.rs/hostname/0.3.0/hostname/fn.get.html
#[derive(Clone, Debug)]
pub struct SyslogBuilder<F: MsgFormat3164 = BasicMsgFormat3164> {
    facility: Option<syslog::Facility>,
    hostname: Option<String>,
    level: Option<Level>,
    logkind: SyslogKind,
    msg_format: F,
    pid: Option<i32>,
    process: Option<String>,
}
impl Default for SyslogBuilder {
    fn default() -> Self {
        SyslogBuilder {
            facility: None,
            hostname: None,
            level: None,
            logkind: SyslogKind::default(),
            msg_format: BasicMsgFormat3164,
            pid: None,
            process: None,
        }
    }
}
impl SyslogBuilder {
    /// Build a default logger
    ///
    /// By default this will attempt to connect to (in order)
    pub fn new() -> SyslogBuilder {
        Self::default()
    }
}
impl<F: MsgFormat3164> SyslogBuilder<F> {
    /// Set syslog Facility
    /// 
    /// The default facility, as per [POSIX], is `LOG_USER`.
    /// 
    /// [POSIX]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/closelog.html
    pub fn facility(self, facility: syslog::Facility) -> Self {
        let mut s = self;
        s.facility = Some(facility);
        s
    }

    /// Filter Syslog by level
    pub fn level(self, lvl: slog::Level) -> Self {
        let mut s = self;
        s.level = Some(lvl);
        s
    }

    /// Set a custom hostname, instead of detecting it.
    pub fn hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = Some(hostname.into());
        self
    }

    /// Set a custom process ID, instead of detecting it.
    pub fn pid(mut self, pid: i32) -> Self {
        self.pid = Some(pid);
        self
    }

    /// Set the name of this process, instead of detecting it.
    pub fn process(mut self, process: impl Into<String>) -> Self {
        self.process = Some(process.into());
        self
    }

    /// Set the `MsgFormat3164` to use for formatting key-value pairs in log messages.
    pub fn msg_format<F2: MsgFormat3164>(self, msg_format: F2) -> SyslogBuilder<F2> {
        // This changes the `F` type parameter of this `SyslogBuilder`, so we can't just change the `msg_format` field. We have to make a whole new `SyslogBuilder` with the new `msg_format`.
        SyslogBuilder {
            facility: self.facility,
            hostname: self.hostname,
            level: self.level,
            logkind: self.logkind,
            msg_format,
            pid: self.pid,
            process: self.process,
        }
    }

    /// Remote UDP syslogging
    pub fn udp(self, local: SocketAddr, host: SocketAddr) -> Self {
        let mut s = self;
        s.logkind = SyslogKind::Udp {
            local,
            host,
        };
        s
    }

    /// Remote TCP syslogging
    pub fn tcp(self, server: SocketAddr) -> Self {
        let mut s = self;
        s.logkind = SyslogKind::Tcp { server };
        s
    }

    /// Local syslogging over a unix socket
    pub fn unix(self, path: impl Into<PathBuf>) -> Self {
        let mut s = self;
        let path = path.into();
        s.logkind = SyslogKind::Unix { path };
        s
    }

    /// Start running
    /// 
    /// This method wraps the created `Streamer3164` in a `Mutex`. (For an explanation of why, see the `Streamer3164` documentation.) To get a `Streamer3164` without a `Mutex` wrapper, use the `start_single_threaded` method instead.
    pub fn start(self) -> syslog::Result<Mutex<Streamer3164<syslog::LoggerBackend, F>>> {
        self.start_single_threaded().map(|streamer| Mutex::new(streamer))
    }

    /// Start running, without wrapping the `Streamer3164` in a `Mutex`.
    /// 
    /// Use this if you plan to use [slog-async] or some other synchronization mechanism. Otherwise, use the `start` method instead.
    /// 
    /// [slog-async]: https://docs.rs/slog-async/2/slog_async/index.html
    pub fn start_single_threaded(self) -> syslog::Result<Streamer3164<syslog::LoggerBackend, F>> {
        let formatter = syslog::Formatter3164 {
            facility: self.facility.unwrap_or(Facility::LOG_USER),
            hostname: match self.hostname {
                some @ Some(_) => some,
                None => match hostname::get() {
                    Ok(hostname) => Some(hostname.to_string_lossy().to_string()),
                    Err(_) => None
                }
            },
            pid: self.pid.unwrap_or_else(|| process::id() as i32),
            process: match self.process {
                Some(process) => process,
                None => {
                    let exe = syslog::ResultExt::chain_err(env::current_exe(), || syslog::ErrorKind::Initialization)?;

                    let file_name = exe.file_name().ok_or_else(|| {
                        let error_msg: Box<dyn StdError + Send + Sync + 'static> = Box::from("couldn't get name of this process");

                        syslog::Error::with_boxed_chain(error_msg, syslog::ErrorKind::Initialization)
                    })?;

                    file_name.to_string_lossy().to_string()
                }
            },
        };
        let log = match self.logkind {
            SyslogKind::UnixDefault => syslog::unix(formatter)?,
            SyslogKind::Unix { path } => syslog::unix_custom(formatter, path)?,
            SyslogKind::Udp {
                local,
                host,
            } => syslog::udp(formatter, local, host)?,
            SyslogKind::Tcp { server } => syslog::tcp(formatter, server)?,
        };
        Ok(Streamer3164::new(log).with_msg_format(self.msg_format))
    }
}

/// `Streamer3164` to local syslog daemon using RFC 3164 format
/// 
/// For more control over the created `Streamer3164`, use [`SyslogBuilder`].
/// 
/// [`SyslogBuilder`]: struct.SyslogBuilder.html
pub fn unix_3164(facility: syslog::Facility) -> syslog::Result<Mutex<Streamer3164<syslog::LoggerBackend>>> {
    SyslogBuilder::new().facility(facility).start()
}
