use libc::{self, c_int};
use slog;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

/// A syslog severity level. Conversions are provided to and from `c_int`. Not
/// to be confused with [`slog::Level`].
/// 
/// Available levels are platform-independent. They were originally defined by
/// BSD, are specified by POSIX, and this author is not aware of any system
/// that has a different set of log severities.
/// 
/// [`slog::Level`]: https://docs.rs/slog/2/slog/enum.Level.html
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Level {
    /// Verbose debugging messages.
    Debug,

    /// Normal informational messages. A program that's starting up might log
    /// its version number at this level.
    Info,

    /// The situation is not an error, but it probably needs attention.
    Notice,

    /// Warning. Something has probably gone wrong.
    #[cfg_attr(feature = "serde", serde(alias = "warn"))]
    Warning,

    /// Error. Something has definitely gone wrong.
    #[cfg_attr(feature = "serde", serde(alias = "error"))]
    Err,

    /// Critical error. Hardware failures fall under this level.
    Crit,
    
    /// Something has happened that requires immediate action.
    Alert,
    
    /// The system has failed. This level is for kernel panics and similar
    /// system-wide failures.
    #[cfg_attr(feature = "serde", serde(alias = "panic"))]
    Emerg,
}

impl Level {
    /// Gets the name of this `Level`, like `emerg` or `notice`.
    /// 
    /// The `FromStr` implementation accepts the same names, but it is
    /// case-insensitive.
    pub fn name(&self) -> &'static str {
        match *self {
            Level::Emerg => "emerg",
            Level::Alert => "alert",
            Level::Crit => "crit",
            Level::Err => "err",
            Level::Warning => "warning",
            Level::Notice => "notice",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }

    /// Converts a `libc::LOG_*` numeric constant to a `Level` value.
    /// 
    /// Returns `Some` if the value is a valid level, or `None` if not.
    pub fn from_int(value: c_int) -> Option<Level> {
        match value {
            libc::LOG_EMERG => Some(Level::Emerg),
            libc::LOG_ALERT => Some(Level::Alert),
            libc::LOG_CRIT => Some(Level::Crit),
            libc::LOG_ERR => Some(Level::Err),
            libc::LOG_WARNING => Some(Level::Warning),
            libc::LOG_NOTICE => Some(Level::Notice),
            libc::LOG_INFO => Some(Level::Info),
            libc::LOG_DEBUG => Some(Level::Debug),
            _ => None,
        }
    }

    /// Maps a [`slog::Level`] to a syslog level.
    /// 
    /// Mappings are as follows:
    /// 
    /// * [`Critical`][slog critical] ⇒ [`Crit`][syslog crit]
    /// * [`Error`][slog error] ⇒ [`Err`][syslog err]
    /// * [`Warning`][slog warning] ⇒ [`Warning`][syslog warning]
    /// * [`Info`][slog info] ⇒ [`Info`][syslog info]
    /// * [`Debug`][slog debug] ⇒ [`Debug`][syslog debug]
    /// * [`Trace`][slog trace] ⇒ [`Debug`][syslog debug]
    /// 
    /// This is used by the default implementation of [`Adapter::priority`].
    /// 
    /// [`Adapter::priority`]: adapter/trait.Adapter.html#method.priority
    /// [`Priority`]: struct.Priority.html
    /// [`slog::Level`]: https://docs.rs/slog/2/slog/enum.Level.html
    /// [slog critical]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Critical
    /// [slog error]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Error
    /// [slog warning]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Warning
    /// [slog info]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Info
    /// [slog debug]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Debug
    /// [slog trace]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Trace
    /// [syslog crit]: #variant.Crit
    /// [syslog err]: #variant.Err
    /// [syslog warning]: #variant.Warning
    /// [syslog info]: #variant.Info
    /// [syslog debug]: #variant.Debug
    pub fn from_slog(level: slog::Level) -> Self {
        match level {
            slog::Level::Critical => Level::Crit.into(),
            slog::Level::Error => Level::Err.into(),
            slog::Level::Warning => Level::Warning.into(),
            slog::Level::Debug | slog::Level::Trace => Level::Debug.into(),

            // `slog::Level` isn't non-exhaustive, so adding any more levels
            // would be a breaking change. That is highly unlikely to ever
            // happen. Still, we'll handle the possibility here, just in case.
            _ => Level::Info.into()
        }
    }
}

impl Display for Level {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl From<Level> for c_int {
    fn from(level: Level) -> Self {
        match level {
            Level::Emerg => libc::LOG_EMERG,
            Level::Alert => libc::LOG_ALERT,
            Level::Crit => libc::LOG_CRIT,
            Level::Err => libc::LOG_ERR,
            Level::Warning => libc::LOG_WARNING,
            Level::Notice => libc::LOG_NOTICE,
            Level::Info => libc::LOG_INFO,
            Level::Debug => libc::LOG_DEBUG,
        }
    }
}

impl FromStr for Level {
    type Err = UnknownLevelError;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        let s = s.to_ascii_lowercase();

        match &*s {
            "emerg" | "panic" => Ok(Level::Emerg),
            "alert" => Ok(Level::Alert),
            "crit" => Ok(Level::Crit),
            "err" | "error" => Ok(Level::Err),
            "warning" | "warn" => Ok(Level::Warning),
            "notice" => Ok(Level::Notice),
            "info" => Ok(Level::Info),
            "debug" => Ok(Level::Debug),
            _ => Err(UnknownLevelError {
                name: s,
            })
        }
    }
}

/// Indicates that `<Level as FromStr>::from_str` was called with an unknown
/// level name.
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct UnknownLevelError {
    name: String,
}

impl UnknownLevelError {
    /// The unrecognized level name.
    pub fn name(&self) -> &str {
        &*self.name
    }
}

impl Display for UnknownLevelError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unrecognized syslog level name `{}`", self.name)
    }
}

impl Error for UnknownLevelError {
    #[allow(deprecated)] // Old versions of Rust require this.
    fn description(&self) -> &str {
        "unrecognized syslog level name"
    }
}

#[test]
fn test_level_from_str() {
    assert_eq!(Level::from_str("notice"), Ok(Level::Notice));
    assert_eq!(Level::from_str("foobar"), Err(UnknownLevelError { name: "foobar".to_string() }));
    assert_eq!(Level::from_str("foobar").unwrap_err().to_string(), "unrecognized syslog level name `foobar`");
}

#[test]
fn test_level_ordering() {
    assert!(Level::Debug < Level::Emerg);
}
