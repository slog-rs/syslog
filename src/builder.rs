use Facility;
use format::{DefaultMsgFormat, MsgFormat};
use libc;
use SyslogDrain;
use std::borrow::Cow;
use std::ffi::{CStr, CString};

/// Builds a [`SyslogDrain`].
/// 
/// All settings have sensible defaults. Simply calling
/// `SyslogBuilder::new().build()` (or `SyslogDrain::new()`, which is
/// equivalent) will yield a functional, reasonable `Drain` in most 
/// situations. However, most applications will want to set the `facility`.
/// 
/// [`SyslogDrain`]: struct.SyslogDrain.html
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct SyslogBuilder<F: MsgFormat = DefaultMsgFormat> {
    pub(crate) facility: Facility,
    pub(crate) ident: Option<Cow<'static, CStr>>,
    pub(crate) option: libc::c_int,
    pub(crate) format: F,
}

impl Default for SyslogBuilder {
    fn default() -> Self {
        SyslogBuilder {
            facility: Facility::default(),
            ident: None,
            option: 0,
            format: DefaultMsgFormat,
        }
    }
}

impl SyslogBuilder {
    /// Makes a new `SyslogBuilder` instance.
    pub fn new() -> Self {
        SyslogBuilder::default()
    }
}

impl<F: MsgFormat> SyslogBuilder<F> {
    /// Sets the syslog facility to send logs to.
    /// 
    /// By default, this is [`Facility::User`].
    /// 
    /// [`Facility::User`]: enum.Facility.html#variant.User
    pub fn facility(mut self, facility: Facility) -> Self {
        self.facility = facility;
        self
    }

    /// Sets the name of this program, for inclusion with log messages.
    /// (POSIX calls this the “tag”.)
    /// 
    /// The supplied string must not contain any zero (ASCII NUL) bytes.
    /// 
    /// # Default value
    /// 
    /// If a name is not given, the default behavior depends on the libc
    /// implementation in use.
    /// 
    /// BSD, GNU, and Apple libc use the actual process name. µClibc uses the
    /// constant string `syslog`. Fuchsia libc and musl libc use no name at
    /// all.
    /// 
    /// # When to use
    /// 
    /// This method converts the given string to a C-compatible string at run
    /// time. It should only be used if the process name is obtained
    /// dynamically, such as from a configuration file.
    /// 
    /// If the process name is constant, use the `ident` method instead.
    /// 
    /// # Panics
    /// 
    /// This method panics if the supplied string contains any null bytes.
    /// 
    /// # Example
    /// 
    /// ```
    /// use slog_syslog::SyslogBuilder;
    /// 
    /// # let some_string = "hello".to_string();
    /// let my_ident: String = some_string;
    /// 
    /// let drain = SyslogBuilder::new()
    ///     .ident_str(my_ident)
    ///     .build();
    /// ```
    /// 
    /// # Data use and lifetime
    /// 
    /// This method takes an ordinary Rust string, copies it into a
    /// [`CString`] (which appends a null byte on the end), and passes that to
    /// the `ident` method.
    /// 
    /// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html
    pub fn ident_str<S: AsRef<str>>(self, ident: S) -> Self {
        let cs = CString::new(ident.as_ref())
            .expect("`SyslogBuilder::ident` called with string that contains null bytes");

        self.ident(Cow::Owned(cs))
    }

    /// Sets the name of this program, for inclusion with log messages.
    /// (POSIX calls this the “tag”.)
    /// 
    /// # Default value
    /// 
    /// If a name is not given, the default behavior depends on the libc
    /// implementation in use.
    /// 
    /// BSD, GNU, and Apple libc use the actual process name. µClibc uses the
    /// constant string `syslog`. Fuchsia libc and musl libc use no name at
    /// all.
    /// 
    /// # When to use
    /// 
    /// This method should be used if you already have a C-compatible string to
    /// use for the process name, or if the process name is constant (as
    /// opposed to taken from a configuration file or command line parameter).
    /// 
    /// # Data use and lifetime
    /// 
    /// This method takes a C-compatible string, either owned or with the
    /// `'static` lifetime. This ensures that the string remains available for
    /// the entire time that the system libc might need it (until `closelog` is
    /// called, which happens when the `SyslogDrain` is dropped).
    /// 
    /// # Example
    /// 
    /// ```
    /// use slog_syslog::SyslogBuilder;
    /// use std::ffi::CStr;
    /// 
    /// let drain = SyslogBuilder::new()
    ///     .ident(CStr::from_bytes_with_nul("example-app\0".as_bytes()).unwrap())
    ///     .build();
    /// ```
    pub fn ident<S: Into<Cow<'static, CStr>>>(mut self, ident: S) -> Self {
        self.ident = Some(ident.into());
        self
    }

    // The `log_*` flag methods are all `#[inline]` because, in theory, the
    // optimizer could collapse several flag method calls into a single store
    // operation, which would be much faster…but it can only do that if the
    // calls are all inlined.

    /// Include the process ID in log messages.
    #[inline]
    pub fn log_pid(mut self) -> Self {
        self.option |= libc::LOG_PID;
        self
    }

    /// Immediately open a connection to the syslog server, instead of waiting
    /// until the first log message is sent.
    /// 
    /// `log_ndelay` and `log_odelay` are mutually exclusive, and one of them
    /// is the default. Exactly which one is the default depends on the
    /// platform, but on most platforms, `log_odelay` is the default.
    /// 
    /// On OpenBSD 5.6 and newer, this setting has no effect, because that
    /// platform uses a dedicated system call instead of a socket for
    /// submitting syslog messages.
    #[inline]
    pub fn log_ndelay(mut self) -> Self {
        self.option = (self.option & !libc::LOG_ODELAY) | libc::LOG_NDELAY;
        self
    }

    /// *Don't* immediately open a connection to the syslog server. Wait until
    /// the first log message is sent before connecting.
    /// 
    /// `log_ndelay` and `log_odelay` are mutually exclusive, and one of them
    /// is the default. Exactly which one is the default depends on the
    /// platform, but on most platforms, `log_odelay` is the default.
    /// 
    /// On OpenBSD 5.6 and newer, this setting has no effect, because that
    /// platform uses a dedicated system call instead of a socket for
    /// submitting syslog messages.
    #[inline]
    pub fn log_odelay(mut self) -> Self {
        self.option = (self.option & !libc::LOG_NDELAY) | libc::LOG_ODELAY;
        self
    }

    /// If a child process is created to send a log message, don't wait for
    /// that child process to exit.
    /// 
    /// This option is highly unlikely to have any effect on any modern system.
    /// On a modern system, spawning a child process for every single log
    /// message would be extremely slow. This option only ever existed as a
    /// [workaround for limitations of the 2.11BSD kernel][2.11BSD wait call],
    /// and was already [deprecated as of 4.4BSD][4.4BSD deprecation notice].
    /// It is included here only for completeness because, unfortunately,
    /// [POSIX defines it].
    /// 
    /// [2.11BSD wait call]: https://www.retro11.de/ouxr/211bsd/usr/src/lib/libc/gen/syslog.c.html#n:176
    /// [4.4BSD deprecation notice]: https://github.com/sergev/4.4BSD-Lite2/blob/50587b00e922225c62f1706266587f435898126d/usr/src/sys/sys/syslog.h#L164
    /// [POSIX defines it]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/closelog.html
    #[inline]
    pub fn log_nowait(mut self) -> Self {
        self.option |= libc::LOG_NOWAIT;
        self
    }

    /// Also emit log messages on `stderr` (**see warning**).
    /// 
    /// # Warning
    /// 
    /// The libc `syslog` function is not subject to the global mutex that
    /// Rust uses to synchronize access to `stderr`. As a result, if one thread
    /// writes to `stderr` at the same time as another thread emits a log
    /// message with this option, the log message may appear in the middle of
    /// the other thread's output.
    /// 
    /// Note that this problem is not specific to Rust or this crate. Any
    /// program in any language that writes to `stderr` in one thread and logs
    /// to `syslog` with `LOG_PERROR` in another thread at the same time will
    /// have the same problem.
    /// 
    /// The exception is the `syslog` implementation in GNU libc, which
    /// implements this option by writing to `stderr` through the C `stdio`
    /// API (as opposed to the `write` system call), which has its own mutex.
    /// As long as all threads write to `stderr` using the C `stdio` API, log
    /// messages on this platform will never appear in the middle of other
    /// `stderr` output. However, Rust does not use the C `stdio` API for
    /// writing to `stderr`, so even on GNU libc, using this option may result 
    /// in garbled output.
    #[inline]
    pub fn log_perror(mut self) -> Self {
        self.option |= libc::LOG_PERROR;
        self
    }

    /// Set a format for log messages and structured data.
    /// 
    /// The default is [`DefaultMsgFormat`].
    /// 
    /// # Example
    /// 
    /// ```
    /// use slog_syslog::SyslogBuilder;
    /// use slog_syslog::format::BasicMsgFormat;
    /// 
    /// let logger = SyslogBuilder::new()
    ///     .format(BasicMsgFormat)
    ///     .build();
    /// ```
    /// 
    /// [`DefaultMsgFormat`]: format/struct.DefaultMsgFormat.html
    pub fn format<F2: MsgFormat>(self, format: F2) -> SyslogBuilder<F2> {
        SyslogBuilder {
            facility: self.facility,
            ident: self.ident,
            option: self.option,
            format,
        }
    }

    /// Builds a `SyslogDrain` from the settings provided.
    pub fn build(self) -> SyslogDrain<F> {
        SyslogDrain::from_builder(self)
    }
}
