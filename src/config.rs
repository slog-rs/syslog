//! Adapters for configuring a [`SyslogDrain`] from a configuration file using
//! [serde]. Requires Cargo feature `serde`.
//! 
//! [serde]: https://serde.rs/
//! [`SyslogDrain`]: ../struct.SyslogDrain.html

use Facility;
use format::{BasicMsgFormat, DefaultMsgFormat, MsgFormat};
use slog::{self, OwnedKVList, Record};
use std::borrow::Cow;
use std::ffi::CStr;
use std::fmt;
use SyslogBuilder;
use SyslogDrain;
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
    /// See [`MsgFormat`] for more information.
    /// 
    /// [`MsgFormat`]: ../format/trait.MsgFormat.html
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

    #[serde(skip)]
    __non_exhaustive: (),
}

impl SyslogConfig {
    /// Creates a new `SyslogConfig` with default settings.
    pub fn new() -> Self {
        Default::default()
    }

    /// Creates a new `SyslogBuilder` from the settings.
    pub fn into_builder(self) -> SyslogBuilder<ConfiguredMsgFormat> {
        let b = SyslogBuilder::new()
        .facility(self.facility)
        .format(self.format.into());

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
    pub fn build(self) -> SyslogDrain<ConfiguredMsgFormat> {
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
            __non_exhaustive: (),
        }
    }
}

/// Enumeration of built-in `MsgFormat`s.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MsgFormatConfig {
    /// [`DefaultMsgFormat`](struct.DefaultMsgFormat.html).
    Default,

    /// [`BasicMsgFormat`](struct.BasicMsgFormat.html).
    Basic,

    #[doc(hidden)]
    __NonExhaustive,
}

impl Default for MsgFormatConfig {
    fn default() -> Self {
        MsgFormatConfig::Default
    }
}

/// Implements [`MsgFormat`] based on the settings in a [`MsgFormatConfig`].
/// 
/// This is the type of [`MsgFormat`] used by [`SyslogDrain`]s constructed from
/// a [`SyslogConfig`].
/// 
/// [`MsgFormat`]: ../format/trait.MsgFormat.html
/// [`MsgFormatConfig`]: enum.MsgFormatConfig.html
/// [`SyslogConfig`]: struct.SyslogConfig.html
/// [`SyslogDrain`]: ../struct.SyslogDrain.html
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ConfiguredMsgFormat {
    config: MsgFormatConfig,
}

impl MsgFormat for ConfiguredMsgFormat {
    fn fmt(&self, f: &mut fmt::Formatter, record: &Record, values: &OwnedKVList) -> slog::Result {
        match self.config {
            MsgFormatConfig::Basic => BasicMsgFormat.fmt(f, record, values),
            MsgFormatConfig::Default => DefaultMsgFormat.fmt(f, record, values),
            MsgFormatConfig::__NonExhaustive => panic!("MsgFormatConfig::__NonExhaustive used")
        }
    }
}

impl From<MsgFormatConfig> for ConfiguredMsgFormat {
    fn from(config: MsgFormatConfig) -> Self {
        ConfiguredMsgFormat {
            config
        }
    }
}

#[test]
fn test_config() {
    const TOML_CONFIG: &'static str = r#"
format = "basic"
ident = "foo"
facility = "daemon"
log_pid = true
log_perror = true
"#;

    let config: SyslogConfig = toml::de::from_str(TOML_CONFIG).expect("deserialization failed");

    let builder = config.into_builder();

    assert_eq!(
        builder,
        SyslogBuilder::new()
        .format(ConfiguredMsgFormat::from(MsgFormatConfig::Basic))
        .ident_str("foo")
        .facility(Facility::Daemon)
        .log_pid()
        .log_perror()
    );
}
