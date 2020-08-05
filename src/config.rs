//! Adapters for configuring a [`SyslogDrain`] from a configuration file using
//! [serde]. Requires Cargo feature `serde`.
//! 
//! [serde]: https://serde.rs/
//! [`SyslogDrain`]: ../struct.SyslogDrain.html

use ::{Facility, Priority, SyslogBuilder, SyslogDrain};
use adapter::{Adapter, BasicAdapter, DefaultAdapter};
use slog::{self, OwnedKVList, Record};
use std::borrow::Cow;
use std::ffi::CStr;
use std::fmt;
#[cfg(test)] use toml;

/// Deserializable configuration for a [`SyslogDrain`].
/// 
/// Call the [`build`] method to create a [`SyslogDrain`] from a
/// `SyslogConfig`.
/// 
/// [`build`]: #method.build
/// [`SyslogDrain`]: ../struct.SyslogDrain.html
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SyslogConfig {
    /// How to format syslog messages with structured data.
    /// 
    /// Possible values are `default` and `basic`.
    /// 
    /// See [`Adapter`] for more information.
    /// 
    /// [`Adapter`]: ../adapter/trait.Adapter.html
    pub format: MsgFormatConfig,

    /// The syslog facility to send logs to.
    pub facility: Facility,

    /// The name of this program, for inclusion with log messages. (POSIX calls
    /// this the “tag”.)
    /// 
    /// The string must not contain any zero (ASCII NUL) bytes.
    /// 
    /// # Default value
    /// 
    /// If a name is not given, the default behavior depends on the libc
    /// implementation in use.
    /// 
    /// BSD, GNU, and Apple libc use the actual process name. µClibc uses the
    /// constant string `syslog`. Fuchsia libc and musl libc use no name at
    /// all.
    pub ident: Option<Cow<'static, CStr>>,

    /// Include the process ID in log messages.
    pub log_pid: bool,

    /// Whether to immediately open a connection to the syslog server.
    /// 
    /// If true, a connection will be immediately opened. If false, the
    /// connection will only be opened when the first log message is submitted.
    /// 
    /// The default is platform-defined, but on most platforms, the default is
    /// `true`.
    /// 
    /// On OpenBSD 5.6 and newer, this setting has no effect, because that
    /// platform uses a dedicated system call instead of a socket for
    /// submitting syslog messages.
    pub log_delay: Option<bool>,

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
    pub log_perror: bool,

    /// Log some or all messages with the given [priority][`Priority`].
    /// 
    /// See [`Priority`] and [`Adapter::with_priority`] for more information.
    /// 
    /// [`Adapter::with_priority`]: ../adapter/trait.Adapter.html#method.with_priority
    /// [`Priority`]: ../struct.Priority.html
    pub priority: PriorityConfig,

    #[serde(skip)]
    __non_exhaustive: (),
}

impl SyslogConfig {
    /// Creates a new `SyslogConfig` with default settings.
    pub fn new() -> Self {
        Default::default()
    }

    /// Creates a new `SyslogBuilder` from the settings.
    pub fn into_builder(self) -> SyslogBuilder<ConfiguredAdapter> {
        let b = SyslogBuilder::new()
        .facility(self.facility)
        .adapter((self.format, self.priority).into());

        let b = match self.ident {
            Some(ident) => b.ident(ident),
            None => b,
        };

        let b = match self.log_pid {
            true => b.log_pid(),
            false => b,
        };

        let b = match self.log_delay {
            Some(true) => b.log_odelay(),
            Some(false) => b.log_ndelay(),
            None => b,
        };

        let b = match self.log_perror {
            true => b.log_perror(),
            false => b,
        };

        b
    }

    /// Creates a new `SyslogDrain` from the settings.
    pub fn build(self) -> SyslogDrain<ConfiguredAdapter> {
        self.into_builder().build()
    }
}

impl Default for SyslogConfig {
    fn default() -> Self {
        SyslogConfig {
            format: MsgFormatConfig::default(),
            facility: Facility::default(),
            ident: None,
            log_pid: false,
            log_delay: None,
            log_perror: false,
            priority: PriorityConfig::default(),
            __non_exhaustive: (),
        }
    }
}

/// Enumeration of built-in formatting styles.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MsgFormatConfig {
    /// RFC 5424-like formatting, using [`DefaultAdapter`](struct.DefaultAdapter.html).
    Default,

    /// Log the message only, not structured data, using [`BasicAdapter`](struct.BasicAdapter.html).
    Basic,

    #[doc(hidden)]
    __NonExhaustive,
}

impl Default for MsgFormatConfig {
    fn default() -> Self {
        MsgFormatConfig::Default
    }
}

/// Configures mapping of [`slog::Level`]s to [syslog priorities].
/// 
/// # TOML Example
/// 
/// This configuration will log [`slog::Level::Info`] messages with level
/// [`Notice`] and facility [`Daemon`], and log [`slog::Level::Critical`]
/// messages with level [`Alert`] and facility [`Mail`]:
/// 
/// ```
/// # use slog_syslog::{Facility, Level, Priority};
/// # use slog_syslog::config::{PriorityConfig, SyslogConfig};
/// #
/// # const TOML_CONFIG: &'static str = r#"
/// ident = "foo"
/// facility = "daemon"
/// 
/// [priority]
/// info = "notice"
/// critical = ["alert", "mail"]
/// # "#;
/// #
/// # let config: SyslogConfig = toml::de::from_str(TOML_CONFIG).expect("deserialization failed");
/// # assert_eq!(config.priority, {
/// #     let mut exp = PriorityConfig::new();
/// #     exp.info = Some(Priority::new(Level::Notice, None));
/// #     exp.critical = Some(Priority::new(Level::Alert, Some(Facility::Mail)));
/// #     exp
/// # });
/// ```
/// 
/// [`Alert`]: ../enum.Level.html#variant.Alert
/// [`Daemon`]: ../enum.Facility.html#variant.Daemon
/// [`Mail`]: ../enum.Facility.html#variant.Mail
/// [`Notice`]: ../enum.Level.html#variant.Notice
/// [`slog::Level`]: https://docs.rs/slog/2/slog/enum.Level.html
/// [`slog::Level::Critical`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Critical
/// [`slog::Level::Info`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Info
/// [syslog priorities]: ../struct.Priority.html
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct PriorityConfig {
    /// Default priority for all messages.
    /// 
    /// If this is not given, the [`Level`] is chosen with [`Level::from_slog`]
    /// and the [`Facility`] is taken from [`SyslogConfig::facility`].
    /// 
    /// [`Facility`]: ../enum.Facility.html
    /// [`Level`]: ../enum.Level.html
    /// [`Level::from_slog`]: ../enum.Level.html#method.from_slog
    /// [`SyslogConfig::facility`]: struct.SyslogConfig.html#structfield.facility
    pub all: Option<Priority>,

    /// Priority for [`slog::Level::Trace`].
    /// 
    /// [`slog::Level::Trace`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Trace
    pub trace: Option<Priority>,

    /// Priority for [`slog::Level::Debug`].
    /// 
    /// [`slog::Level::Debug`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Debug
    pub debug: Option<Priority>,

    /// Priority for [`slog::Level::Info`].
    /// 
    /// [`slog::Level::Info`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Info
    pub info: Option<Priority>,

    /// Priority for [`slog::Level::Warning`].
    /// 
    /// [`slog::Level::Warning`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Warning
    pub warning: Option<Priority>,

    /// Priority for [`slog::Level::Error`].
    /// 
    /// [`slog::Level::Error`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Error
    pub error: Option<Priority>,

    /// Priority for [`slog::Level::Critical`].
    /// 
    /// [`slog::Level::Critical`]: https://docs.rs/slog/2/slog/enum.Level.html#variant.Critical
    pub critical: Option<Priority>,

    #[serde(skip)]
    __non_exhaustive: (),
}

impl PriorityConfig {
    /// Creates a new `PriorityConfig` with default settings.
    pub fn new() -> Self {
        Default::default()
    }
}

/// Implements [`Adapter`] based on the settings in an [`MsgFormatConfig`] and
/// [`PriorityConfig`].
/// 
/// This is the type of [`Adapter`] used by [`SyslogDrain`]s constructed from
/// a [`SyslogConfig`].
/// 
/// [`Adapter`]: ../adapter/trait.Adapter.html
/// [`MsgFormatConfig`]: enum.MsgFormatConfig.html
/// [`PriorityConfig`]: struct.PriorityConfig.html
/// [`SyslogConfig`]: struct.SyslogConfig.html
/// [`SyslogDrain`]: ../struct.SyslogDrain.html
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ConfiguredAdapter {
    format: MsgFormatConfig,
    priority: PriorityConfig,
}

impl Adapter for ConfiguredAdapter {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        match self.format {
            MsgFormatConfig::Basic => BasicAdapter.fmt(f, record, values),
            MsgFormatConfig::Default => DefaultAdapter.fmt(f, record, values),
            MsgFormatConfig::__NonExhaustive => panic!("MsgFormatConfig::__NonExhaustive used")
        }
    }

    fn priority(&self, record: &Record, values: &OwnedKVList) -> Priority {
        let priority = match record.level() {
            slog::Level::Critical => self.priority.critical,
            slog::Level::Error => self.priority.error,
            slog::Level::Warning => self.priority.warning,
            slog::Level::Debug => self.priority.debug,
            slog::Level::Trace => self.priority.trace,
            _ => self.priority.info,
        };

        match (priority, self.priority.all) {
            (Some(priority), Some(priority_all)) => priority.overlay(priority_all),
            (None, Some(priority_all)) => priority_all,
            (Some(priority), None) => priority,
            (None, None) => DefaultAdapter.priority(record, values),
        }
    }
}

impl From<MsgFormatConfig> for ConfiguredAdapter {
    fn from(config: MsgFormatConfig) -> Self {
        ConfiguredAdapter {
            format: config,
            priority: PriorityConfig::default(),
        }
    }
}

impl From<PriorityConfig> for ConfiguredAdapter {
    fn from(priority: PriorityConfig) -> Self {
        ConfiguredAdapter {
            format: MsgFormatConfig::Default,
            priority,
        }
    }
}

impl From<(Option<MsgFormatConfig>, Option<PriorityConfig>)> for ConfiguredAdapter {
    fn from((format_opt, priority_opt): (Option<MsgFormatConfig>, Option<PriorityConfig>)) -> Self {
        ConfiguredAdapter {
            format: format_opt.unwrap_or(MsgFormatConfig::Default),
            priority: priority_opt.unwrap_or(PriorityConfig::default()),
        }
    }
}

impl From<(MsgFormatConfig, PriorityConfig)> for ConfiguredAdapter {
    fn from((format, priority): (MsgFormatConfig, PriorityConfig)) -> Self {
        ConfiguredAdapter { format, priority }
    }
}

#[test]
fn test_config() {
    use Level;

    const TOML_CONFIG: &'static str = r#"
format = "basic"
ident = "foo"
facility = "daemon"
log_pid = true
log_perror = true

[priority]
info = "notice"
critical = ["alert", "mail"]
"#;

    let config: SyslogConfig = toml::de::from_str(TOML_CONFIG).expect("deserialization failed");

    let builder = config.into_builder();

    assert_eq!(
        builder,
        SyslogBuilder::new()
        .adapter(ConfiguredAdapter::from((
            MsgFormatConfig::Basic,
            PriorityConfig {
                info: Some(Priority::new(Level::Notice, None)),
                critical: Some(Priority::new(Level::Alert, Some(Facility::Mail))),
                ..PriorityConfig::default()
            }
        )))
        .ident_str("foo")
        .facility(Facility::Daemon)
        .log_pid()
        .log_perror()
    );
}
