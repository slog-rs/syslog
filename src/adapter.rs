//! Customize the conversion of [`slog::Record`]s to syslog messages.
//! 
//! See [`Adapter`] for more details.
//! 
//! [`Adapter`]: trait.Adapter.html
//! [`slog::Record`]: https://docs.rs/slog/2/slog/struct.Record.html

use ::{Level, Priority};
use slog::{self, KV, OwnedKVList, Record};
use std::cell::Cell;
use std::fmt::{self, Debug, Display};
use std::io;
use std::rc::Rc;
use std::sync::Arc;

/// Converts [`slog::Record`]s to syslog messages.
/// 
/// An `Adapter` has two responsibilities:
/// 
/// 1. Format structured log data into a syslog message.
/// 2. Determine the message's syslog [priority].
/// 
/// # Structured Data
/// 
/// Syslog does not support structured log data. If Slog key-value pairs are to
/// be included with log messages, they must be included as part of the
/// message. Implementations of this trait's `fmt` method determine if and
/// how this will be done.
/// 
/// # Priority
/// 
/// Each message sent to syslog has a “[priority]”, which consists of a
/// required [severity level] and an optional [facility]. This doesn't match
/// [`slog::Level`], so an implementation of the [`priority`] method of this
/// trait is used to choose a priority for each [`slog::Record`].
/// 
/// [facility]: ../enum.Facility.html
/// [priority]: ../struct.Priority.html
/// [`priority`]: #method.priority
/// [severity level]: ../enum.Level.html
/// [`slog::Level`]: https://docs.rs/slog/2/slog/enum.Level.html
/// [`slog::Record`]: https://docs.rs/slog/2/slog/struct.Record.html
pub trait Adapter: Debug {
    /// Formats a log message and its key-value pairs into the given `Formatter`.
    /// 
    /// Note that this method returns `slog::Result`, not `std::fmt::Result`.
    /// The caller of this method is responsible for handling the error,
    /// likely by storing it elsewhere and picking it up later. The free
    /// function [`format`](fn.format.html) does just that.
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result;

    /// Creates a new `Adapter` based on this one, but whose `fmt` method
    /// delegates to the provided closure.
    /// 
    /// # Example
    /// 
    /// This formatting function simply prepends `here's a message: ` to each
    /// log message:
    /// 
    /// ```
    /// use slog_syslog::adapter::{Adapter, DefaultAdapter};
    /// use slog_syslog::SyslogBuilder;
    /// 
    /// let drain = SyslogBuilder::new()
    ///     .adapter(DefaultAdapter.with_fmt(|f, record, _| {
    ///         write!(f, "here's a message: {}", record.msg())?;
    ///         Ok(())
    ///     }))
    ///     .build();
    /// ```
    /// 
    /// The [`SyslogBuilder::format`] method is a convenient shorthand:
    /// 
    /// ```
    /// use slog_syslog::SyslogBuilder;
    /// 
    /// let drain = SyslogBuilder::new()
    ///     .format(|f, record, _| {
    ///         write!(f, "here's a message: {}", record.msg())?;
    ///         Ok(())
    ///     })
    ///     .build();
    /// ```
    /// 
    /// Note the use of the `?` operator. The closure is expected to return
    /// `Result<(), slog::Error>`, not the `Result<(), std::fmt::Error>` that
    /// `write!` returns. `slog::Error` does have a conversion from
    /// `std::fmt::Error`, which the `?` operator will automatically perform.
    /// 
    /// [`SyslogBuilder::format`]: ../struct.SyslogBuilder.html#method.format
    fn with_fmt<F>(self, fmt_fn: F) -> WithFormat<Self, F>
    where
        Self: Sized,
        F: Fn(&mut fmt::Formatter, &Record, &OwnedKVList) -> slog::Result,
    {
        WithFormat {
            fmt_fn,
            inner: self,
        }
    }

    /// Examines a log message and determines its syslog [`Priority`].
    /// 
    /// The default implementation calls [`Level::from_slog`], which maps
    /// [`slog::Level`]s as follows:
    /// 
    /// * [`Critical`][slog critical] ⇒ [`Crit`][syslog crit]
    /// * [`Error`][slog error] ⇒ [`Err`][syslog err]
    /// * [`Warning`][slog warning] ⇒ [`Warning`][syslog warning]
    /// * [`Info`][slog info] ⇒ [`Info`][syslog info]
    /// * [`Debug`][slog debug] ⇒ [`Debug`][syslog debug]
    /// * [`Trace`][slog trace] ⇒ [`Debug`][syslog debug]
    /// 
    /// [`Level::from_slog`]: ../enum.Level.html#method.from_slog
    /// [`Priority`]: ../struct.Priority.html
    /// [`slog::Level`]: https://docs.rs/slog/2/slog/enum.Level.html
    /// [slog critical]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Critical
    /// [slog error]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Error
    /// [slog warning]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Warning
    /// [slog info]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Info
    /// [slog debug]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Debug
    /// [slog trace]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Trace
    /// [syslog crit]: ../enum.Level.html#variant.Crit
    /// [syslog err]: ../enum.Level.html#variant.Err
    /// [syslog warning]: ../enum.Level.html#variant.Warning
    /// [syslog info]: ../enum.Level.html#variant.Info
    /// [syslog debug]: ../enum.Level.html#variant.Debug
    #[allow(unused_variables)]
    fn priority(&self, record: &Record, values: &OwnedKVList) -> Priority {
        Level::from_slog(record.level()).into()
    }

    /// Creates a new `Adapter` based on this one, but whose `priority` method
    /// delegates to the provided closure.
    /// 
    /// If you want to use the default formatting and only want to control
    /// priorities, use this method.
    /// 
    /// # Example
    /// 
    /// ## Force all messages to [`Level::Err`]
    /// 
    /// This uses the default message formatting, but makes all syslog messages
    /// be [`Level::Err`]:
    /// 
    /// ```
    /// use slog_syslog::adapter::{Adapter, DefaultAdapter};
    /// use slog_syslog::SyslogBuilder;
    /// use slog_syslog::Level;
    /// 
    /// let drain = SyslogBuilder::new()
    ///     .adapter(DefaultAdapter.with_priority(|record, _| {
    ///         Level::Err.into()
    ///     }))
    ///     .build();
    /// ```
    /// 
    /// The [`SyslogBuilder::priority`] method is a convenient shorthand:
    /// 
    /// ```
    /// use slog_syslog::SyslogBuilder;
    /// use slog_syslog::Level;
    /// 
    /// let drain = SyslogBuilder::new()
    ///     .priority(|record, _| {
    ///         Level::Err.into()
    ///     })
    ///     .build();
    /// ```
    /// 
    /// Notice the use of `into()`. [`Level`] can be converted directly into
    /// [`Priority`].
    /// 
    /// ## Override level and facility
    /// 
    /// In this example, [`slog::Level::Info`] messages from the `my_app::mail`
    /// module are logged as [`Level::Notice`] instead of the default
    /// [`Level::Info`], and all messages from that module are logged with a
    /// different facility:
    /// 
    /// ```
    /// # extern crate slog;
    /// # extern crate slog_syslog;
    /// use slog_syslog::{Facility, Level, Priority, SyslogBuilder};
    /// 
    /// let drain = SyslogBuilder::new()
    ///     .facility(Facility::Daemon)
    ///     .priority(|record, _| {
    ///         Priority::new(
    ///             match record.level() {
    ///                 slog::Level::Info => Level::Notice,
    ///                 other => Level::from_slog(other),
    ///             },
    ///             match record.module() {
    ///                 "my_app::mail" => Some(Facility::Mail),
    ///                 _ => None,
    ///             },
    ///         )
    ///     })
    ///     .build();
    /// ```
    /// 
    /// [`Level`]: ../enum.Level.html
    /// [`Level::Err`]: ../enum.Level.html#variant.Err
    /// [`Level::Info`]: ../enum.Level.html#variant.Info
    /// [`Level::Notice`]: ../enum.Level.html#variant.Notice
    /// [`Priority`]: ../struct.Priority.html
    /// [`slog::Level`]: https://docs.rs/slog/2/slog/enum.Level.html
    /// [`slog::Level::Info`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Info
    /// [`SyslogBuilder::priority`]: ../struct.SyslogBuilder.html#method.priority
    fn with_priority<P>(self, priority_fn: P) -> WithPriority<Self, P>
    where
        Self: Sized,
        P: Fn(&Record, &OwnedKVList) -> Priority,
    {
        WithPriority {
            inner: self,
            priority_fn,
        }
    }
}

impl<'a, T: Adapter + ?Sized> Adapter for &'a T {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        Adapter::fmt(&**self, f, record, values)
    }
}

impl<T: Adapter + ?Sized> Adapter for Box<T> {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        Adapter::fmt(&**self, f, record, values)
    }
}

impl<T: Adapter + ?Sized> Adapter for Rc<T> {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        Adapter::fmt(&**self, f, record, values)
    }
}

impl<T: Adapter + ?Sized> Adapter for Arc<T> {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        Adapter::fmt(&**self, f, record, values)
    }
}

// This helper structure provides a convenient way to implement
// `Display` with a closure.
struct ClosureAsDisplay<A: Fn(&mut fmt::Formatter) -> fmt::Result>(A);
impl<A: Fn(&mut fmt::Formatter) -> fmt::Result> Display for ClosureAsDisplay<A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0(f)
    }
}

/// Formats a log message and its key-value pairs into the given writer using
/// the given adapter.
/// 
/// # Errors
/// 
/// This method can fail if the [`Adapter::fmt`] method fails, as well as if
/// the `writer` encounters an I/O error.
/// 
/// [`Adapter::fmt`]: trait.Adapter.html#tymethod.fmt
pub fn format<A: Adapter, W: io::Write>(adapter: A, mut writer: W, record: &Record, values: &OwnedKVList) -> slog::Result<()> {
    // If there is an error calling `adapter.fmt`, it will be stored here. We
    // have to use `Cell` because the `Display::fmt` method doesn't get a
    // mutable reference to `self`.
    let result: Cell<Option<slog::Error>> = Cell::new(None);

    // Construct our `Display` implementation…
    let displayable = ClosureAsDisplay(|f| {
        // Do the formatting.
        if let Err(e) = Adapter::fmt(&adapter, f, record, values) {
            // If there's an error, smuggle it out.
            result.set(Some(e));
        }
        // Pretend to succeed, even if there was an error. The real error will
        // be picked up later.
        Ok(())
    });

    // …and use it to write into the given writer.
    let outer_result: io::Result<()> = write!(writer, "{}", displayable);

    // If there was an I/O error, fail with that. This takes precedence over
    // the `result`, because if an I/O error happened, `result` probably
    // contains a `slog::Error::Fmt` that resulted from the I/O error.
    if let Err(e) = outer_result {
        Err(slog::Error::Io(e))
    }
    // If there was a formatter/serializer error other than one caused by I/O,
    // fail with that.
    else if let Some(e) = result.take() {
        Err(e)
    }
    // No error. Yay!
    else {
        Ok(())
    }
}

/// An implementation of [`Adapter`] that discards the key-value pairs and
/// logs only the [`msg`] part of a log [`Record`].
/// 
/// [`msg`]: https://docs.rs/slog/2/slog/struct.Record.html#method.msg
/// [`Adapter`]: trait.Adapter.html
/// [`Record`]: https://docs.rs/slog/2/slog/struct.Record.html
#[derive(Clone, Copy, Debug, Default)]
pub struct BasicAdapter;
impl Adapter for BasicAdapter {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, _: &OwnedKVList) -> slog::Result {
        write!(f, "{}", record.msg()).map_err(From::from)
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
    use std::iter;

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

/// An implementation of [`Adapter`] that formats the key-value pairs of a
/// log [`Record`] similarly to [RFC 5424].
/// 
/// # Not really RFC 5424
/// 
/// This does not actually generate conformant RFC 5424 STRUCTURED-DATA. The
/// differences are:
/// 
/// * All key-value pairs are placed into a single SD-ELEMENT.
/// * The SD-ELEMENT does not contain an SD-ID, only SD-PARAMs.
/// * PARAM-NAMEs are encoded in UTF-8, not ASCII.
/// * Forbidden characters in PARAM-NAMEs are not filtered out, nor is an error
///   raised if a key contains such characters.
/// 
/// # Example output
/// 
/// Given a log message `Hello, world!`, where the key `key1` has the value
/// `value1` and `key2` has the value `value2`, the formatted message will be
/// `Hello, world! [key1="value1" key2="value2"]` (possibly with `key2` first
/// instead of `key1`).
/// 
/// [`Adapter`]: trait.Adapter.html
/// [`Record`]: https://docs.rs/slog/2/slog/struct.Record.html
/// [RFC 5424]: https://tools.ietf.org/html/rfc5424
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultAdapter;
impl Adapter for DefaultAdapter {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        struct SerializerImpl<'a, 'b: 'a> {
            f: &'a mut fmt::Formatter<'b>,
            is_first_kv: bool,
        }

        impl<'a, 'b> SerializerImpl<'a, 'b> {
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
        
        impl<'a, 'b> slog::Serializer for SerializerImpl<'a, 'b> {
            fn emit_arguments(&mut self, key: slog::Key, val: &fmt::Arguments) -> slog::Result {
                use std::fmt::Write;

                self.f.write_str(if self.is_first_kv {" ["} else {" "})?;
                self.is_first_kv = false;

                // Write the key unaltered, but escape the value.
                //
                // RFC 5424 does not allow space, ']', '"', or '\' to
                // appear in PARAM-NAMEs, and does not allow such
                // characters to be escaped.
                write!(self.f, "{}=\"", key)?;
                write!(Rfc5424LikeValueEscaper(&mut self.f), "{}", val)?;
                self.f.write_char('"')?;
                Ok(())
            }
        }

        write!(f, "{}", record.msg())?;

        {
            let mut serializer = SerializerImpl::new(f);

            values.serialize(record, &mut serializer)?;
            record.kv().serialize(record, &mut serializer)?;
            serializer.finish()?;
        }

        Ok(())
    }
}

/// Makes sure the example output for `DefaultAdapter` is what it actually
/// generates.
#[test]
fn test_default_adapter_fmt() {
    use slog::Level;

    let mut buf = Vec::new();

    format(
        DefaultAdapter,
        &mut buf,
        &record!(
            Level::Info,
            "",
            &format_args!("Hello, world!"),
            b!("key1" => "value1")
        ),
        &o!("key2" => "value2").into(),
    ).expect("formatting failed");

    let result = String::from_utf8(buf).expect("invalid UTF-8");

    assert!(
        // The KVs' order is not well-defined, so they might get reversed.
        result == "Hello, world! [key1=\"value1\" key2=\"value2\"]" ||
        result == "Hello, world! [key2=\"value2\" key1=\"value1\"]"
    );
}

/// An [`Adapter`] implementation that calls a closure to perform custom
/// formatting.
/// 
/// # Example
/// 
/// ```
/// use slog_syslog::adapter::{Adapter, DefaultAdapter};
/// use slog_syslog::SyslogBuilder;
/// 
/// let drain = SyslogBuilder::new()
///     .format(|f, record, _| {
///         write!(f, "here's a message: {}", record.msg())?;
///         Ok(())
///     })
///     .build();
/// ```
/// 
/// Note the use of the `?` operator. The closure is expected to return
/// `Result<(), slog::Error>`, not the `Result<(), std::fmt::Error>` that
/// `write!` returns. `slog::Error` does have a conversion from
/// `std::fmt::Error`, which the `?` operator will automatically perform.
/// 
/// [`Adapter`]: trait.Adapter.html
#[derive(Clone, Copy)]
pub struct WithFormat<A, F>
where
    A: Adapter,
    F: Fn(&mut fmt::Formatter, &Record, &OwnedKVList) -> slog::Result,
{
    fmt_fn: F,
    inner: A,
}

impl<A, F> Adapter for WithFormat<A, F>
where
    A: Adapter,
    F: Fn(&mut fmt::Formatter, &Record, &OwnedKVList) -> slog::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        (self.fmt_fn)(f, record, values)
    }

    fn priority(&self, record: &Record, values: &OwnedKVList) -> Priority {
        self.inner.priority(record, values)
    }
}

impl<A, F> Debug for WithFormat<A, F>
where
    A: Adapter,
    F: Fn(&mut fmt::Formatter, &Record, &OwnedKVList) -> slog::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WithFormat")
            .field("inner", &self.inner)
            .finish()
    }
}

/// An [`Adapter`] that calls a closure to decide the [`Priority`] of each log
/// message.
/// 
/// This is created by the [`Adapter::with_priority`] method.
/// 
/// [`Adapter`]: trait.Adapter.html
/// [`Adapter::with_priority`]: trait.Adapter.html#method.with_priority
/// [`Priority`]: ../struct.Priority.html
#[derive(Clone, Copy)]
pub struct WithPriority<A, P>
where
    A: Adapter,
    P: Fn(&Record, &OwnedKVList) -> Priority,
{
    inner: A,
    priority_fn: P,
}

impl<A, P> Adapter for WithPriority<A, P>
where
    A: Adapter,
    P: Fn(&Record, &OwnedKVList) -> Priority,
{
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        Adapter::fmt(&self.inner, f, record, values)
    }

    fn priority(&self, record: &Record, values: &OwnedKVList) -> Priority {
        (self.priority_fn)(record, values)
    }
}

impl<A, P> Debug for WithPriority<A, P>
where
    A: Adapter,
    P: Fn(&Record, &OwnedKVList) -> Priority,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("WithPriority")
            .field("inner", &self.inner)
            .finish()
    }
}
