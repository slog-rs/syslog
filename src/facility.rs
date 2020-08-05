use libc::{self, c_int};
use std::error::Error;
use std::fmt::{self, Display};
use std::str::FromStr;

/// A syslog facility. Conversions are provided to and from `c_int`.
/// 
/// Available facilities depend on the target platform. All variants of this
/// `enum` are available on all platforms, and variants not present on the
/// target platform will be mapped to a reasonable alternative.
/// 
/// The default facility is [`User`].
/// 
/// [`User`]: #variant.User
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Facility {
    /// Authentication, authorization, and other security-related matters.
    /// 
    /// Available on: all platforms
    Auth,

    /// Log messages containing sensitive information.
    /// 
    /// Available on: Linux, Emscripten, macOS, iOS, FreeBSD, DragonFly BSD,
    /// OpenBSD, NetBSD
    /// 
    /// On other platforms: becomes `Auth`
    AuthPriv,

    /// Periodic task scheduling daemons like `cron`.
    /// 
    /// Available on: Linux, Emscripten, macOS, iOS, FreeBSD, DragonFly BSD,
    /// OpenBSD, NetBSD, Solaris, illumos
    /// 
    /// On other platforms: becomes `Daemon`
    Cron,

    /// Daemons that don't fall into a more specific category.
    /// 
    /// Available on: all platforms
    Daemon,

    /// FTP server.
    /// 
    /// Available on: Linux, Emscripten, macOS, iOS, FreeBSD, DragonFly BSD,
    /// OpenBSD, NetBSD
    /// 
    /// On other platforms: becomes `Daemon`
    Ftp,

    /// Operating system kernel.
    /// 
    /// Note: Programs other than the kernel are typically not allowed to use
    /// this facility.
    /// 
    /// Available on: all platforms
    Kern,

    /// macOS installer.
    /// 
    /// Available on: macOS, iOS
    /// 
    /// On other platforms: becomes `User`
    Install,

    /// `launchd`, the macOS process supervisor.
    /// 
    /// Available on: macOS, iOS
    /// 
    /// On other platforms: becomes `Daemon`
    Launchd,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local0,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local1,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local2,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local3,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local4,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local5,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local6,

    /// Reserved for local use.
    /// 
    /// Available on: all platforms
    Local7,

    /// Print server.
    /// 
    /// Available on: all platforms
    Lpr,

    /// Mail transport and delivery agents.
    /// 
    /// Available on: all platforms
    Mail,

    /// Network Time Protocol daemon.
    /// 
    /// Available on: FreeBSD, DragonFly BSD
    /// 
    /// On other platforms: becomes `Daemon`
    Ntp,

    /// NeXT/early macOS `NetInfo` system.
    /// 
    /// Note: Obsolete on modern macOS.
    /// 
    /// Available on: macOS, iOS
    /// 
    /// On other platforms: becomes `Daemon`
    NetInfo,

    /// Usenet news system.
    /// 
    /// Available on: all platforms
    News,

    /// macOS Remote Access Service.
    /// 
    /// Available on: macOS, iOS
    /// 
    /// On other platforms: becomes `User`
    Ras,

    /// macOS remote authentication and authorization.
    /// 
    /// Available on: macOS, iOS
    /// 
    /// On other platforms: becomes `Daemon`
    RemoteAuth,

    /// Security subsystems.
    /// 
    /// Available on: FreeBSD, DragonFly BSD
    /// 
    /// On other platforms: becomes `Auth`
    Security,

    /// Messages generated internally by the syslog daemon.
    /// 
    /// Available on: all platforms
    Syslog,

    /// General user processes.
    /// 
    /// Note: This is the default facility (that is, the value returned by `Facility::default()`).
    /// 
    /// Available on: all platforms
    User,

    /// Unix-to-Unix Copy system.
    /// 
    /// Available on: all platforms
    Uucp,

    #[doc(hidden)]
    __NonExhaustive
}

impl Facility {
    /// Gets the name of this `Facility`, in lowercase.
    /// 
    /// The `FromStr` implementation accepts the same names, but it is
    /// case-insensitive.
    pub fn name(&self) -> &'static str {
        #[allow(deprecated)]
        match *self {
            Facility::Auth       => "auth",
            Facility::AuthPriv   => "authpriv",
            Facility::Cron       => "cron",
            Facility::Daemon     => "daemon",
            Facility::Ftp        => "ftp",
            Facility::Kern       => "kern",
            Facility::Install    => "install",
            Facility::Launchd    => "launchd",
            Facility::Local0     => "local0",
            Facility::Local1     => "local1",
            Facility::Local2     => "local2",
            Facility::Local3     => "local3",
            Facility::Local4     => "local4",
            Facility::Local5     => "local5",
            Facility::Local6     => "local6",
            Facility::Local7     => "local7",
            Facility::Lpr        => "lpr",
            Facility::Mail       => "mail",
            Facility::Ntp        => "ntp",
            Facility::NetInfo    => "netinfo",
            Facility::News       => "news",
            Facility::Ras        => "ras",
            Facility::RemoteAuth => "remoteauth",
            Facility::Security   => "security",
            Facility::Syslog     => "syslog",
            Facility::User       => "user",
            Facility::Uucp       => "uucp",
            Facility::__NonExhaustive => panic!("Facility::__NonExhaustive used")
        }
    }

    /// Converts a `libc::LOG_*` numeric constant to a `Facility` value.
    /// 
    /// Returns `Some` if the value is a valid facility identifier, or `None`
    /// if not.
    pub fn from_int(value: c_int) -> Option<Facility> {
        match value {
            libc::LOG_AUTH => Some(Facility::Auth),
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_env = "uclibc"
            ))]
            libc::LOG_AUTHPRIV => Some(Facility::AuthPriv),
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_os = "solaris",
                target_os = "illumos",
                target_env = "uclibc"
            ))]
            libc::LOG_CRON => Some(Facility::Cron),
            libc::LOG_DAEMON => Some(Facility::Daemon),
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_env = "uclibc"
            ))]
            libc::LOG_FTP => Some(Facility::Ftp),
            libc::LOG_KERN => Some(Facility::Kern),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            libc::LOG_INSTALL => Some(Facility::Install),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            libc::LOG_LAUNCHD => Some(Facility::Launchd),
            libc::LOG_LOCAL0 => Some(Facility::Local0),
            libc::LOG_LOCAL1 => Some(Facility::Local1),
            libc::LOG_LOCAL2 => Some(Facility::Local2),
            libc::LOG_LOCAL3 => Some(Facility::Local3),
            libc::LOG_LOCAL4 => Some(Facility::Local4),
            libc::LOG_LOCAL5 => Some(Facility::Local5),
            libc::LOG_LOCAL6 => Some(Facility::Local6),
            libc::LOG_LOCAL7 => Some(Facility::Local7),
            libc::LOG_LPR => Some(Facility::Lpr),
            libc::LOG_MAIL => Some(Facility::Mail),
            #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
            libc::LOG_NTP => Some(Facility::Ntp),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            libc::LOG_NETINFO => Some(Facility::NetInfo),
            libc::LOG_NEWS => Some(Facility::News),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            libc::LOG_RAS => Some(Facility::Ras),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            libc::LOG_REMOTEAUTH => Some(Facility::RemoteAuth),
            #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
            libc::LOG_SECURITY => Some(Facility::Security),
            libc::LOG_SYSLOG => Some(Facility::Syslog),
            libc::LOG_USER => Some(Facility::User),
            libc::LOG_UUCP => Some(Facility::Uucp),
            _ => None
        }
    }
}

impl Default for Facility {
    fn default() -> Self {
        Facility::User
    }
}

impl Display for Facility {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl From<Facility> for c_int {
    fn from(facility: Facility) -> Self {
        #[allow(deprecated)]
        match facility {
            Facility::Auth => libc::LOG_AUTH,
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_env = "uclibc"
            ))]
            Facility::AuthPriv => libc::LOG_AUTHPRIV,
            #[cfg(not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_env = "uclibc"
            )))]
            Facility::AuthPriv => libc::LOG_AUTH,
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_os = "solaris",
                target_os = "illumos",
                target_env = "uclibc"
            ))]
            Facility::Cron => libc::LOG_CRON,
            #[cfg(not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_os = "solaris",
                target_os = "illumos",
                target_env = "uclibc"
            )))]
            Facility::Cron => libc::LOG_DAEMON,
            Facility::Daemon => libc::LOG_DAEMON,
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_env = "uclibc"
            ))]
            Facility::Ftp => libc::LOG_FTP,
            #[cfg(not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "emscripten",
                target_os = "macos",
                target_os = "ios",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "openbsd",
                target_os = "netbsd",
                target_env = "uclibc"
            )))]
            Facility::Ftp => libc::LOG_DAEMON,
            Facility::Kern => libc::LOG_KERN,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            Facility::Install => libc::LOG_INSTALL,
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            Facility::Install => libc::LOG_USER,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            Facility::Launchd => libc::LOG_LAUNCHD,
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            Facility::Launchd => libc::LOG_DAEMON,
            Facility::Local0 => libc::LOG_LOCAL0,
            Facility::Local1 => libc::LOG_LOCAL1,
            Facility::Local2 => libc::LOG_LOCAL2,
            Facility::Local3 => libc::LOG_LOCAL3,
            Facility::Local4 => libc::LOG_LOCAL4,
            Facility::Local5 => libc::LOG_LOCAL5,
            Facility::Local6 => libc::LOG_LOCAL6,
            Facility::Local7 => libc::LOG_LOCAL7,
            Facility::Lpr => libc::LOG_LPR,
            Facility::Mail => libc::LOG_MAIL,
            #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
            Facility::Ntp => libc::LOG_NTP,
            #[cfg(not(any(target_os = "freebsd", target_os = "dragonfly")))]
            Facility::Ntp => libc::LOG_DAEMON,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            Facility::NetInfo => libc::LOG_NETINFO,
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            Facility::NetInfo => libc::LOG_DAEMON,
            Facility::News => libc::LOG_NEWS,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            Facility::Ras => libc::LOG_RAS,
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            Facility::Ras => libc::LOG_USER,
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            Facility::RemoteAuth => libc::LOG_REMOTEAUTH,
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            Facility::RemoteAuth => libc::LOG_DAEMON,
            #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
            Facility::Security => libc::LOG_SECURITY,
            #[cfg(not(any(target_os = "freebsd", target_os = "dragonfly")))]
            Facility::Security => libc::LOG_AUTH,
            Facility::Syslog => libc::LOG_SYSLOG,
            Facility::User => libc::LOG_USER,
            Facility::Uucp => libc::LOG_UUCP,
            Facility::__NonExhaustive => panic!("Facility::__NonExhaustive used")
        }
    }
}

impl FromStr for Facility {
    type Err = UnknownFacilityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();

        match &*s {
            "auth"       => Ok(Facility::Auth),
            "authpriv"   => Ok(Facility::AuthPriv),
            "cron"       => Ok(Facility::Cron),
            "daemon"     => Ok(Facility::Daemon),
            "ftp"        => Ok(Facility::Ftp),
            "kern"       => Ok(Facility::Kern),
            "install"    => Ok(Facility::Install),
            "launchd"    => Ok(Facility::Launchd),
            "local0"     => Ok(Facility::Local0),
            "local1"     => Ok(Facility::Local1),
            "local2"     => Ok(Facility::Local2),
            "local3"     => Ok(Facility::Local3),
            "local4"     => Ok(Facility::Local4),
            "local5"     => Ok(Facility::Local5),
            "local6"     => Ok(Facility::Local6),
            "local7"     => Ok(Facility::Local7),
            "lpr"        => Ok(Facility::Lpr),
            "mail"       => Ok(Facility::Mail),
            "ntp"        => Ok(Facility::Ntp),
            "netinfo"    => Ok(Facility::NetInfo),
            "news"       => Ok(Facility::News),
            "ras"        => Ok(Facility::Ras),
            "remoteauth" => Ok(Facility::RemoteAuth),
            "security"   => Ok(Facility::Security),
            "syslog"     => Ok(Facility::Syslog),
            "user"       => Ok(Facility::User),
            "uucp"       => Ok(Facility::Uucp),
            _ => Err(UnknownFacilityError {
                name: s,
            })
        }
    }
}

/// Indicates that `<Facility as FromStr>::from_str` was called with an unknown
/// facility name.
#[derive(Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct UnknownFacilityError {
    name: String,
}

impl UnknownFacilityError {
    /// The unrecognized facility name.
    pub fn name(&self) -> &str {
        &*self.name
    }
}

impl Display for UnknownFacilityError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unrecognized syslog facility name `{}`", self.name)
    }
}

impl Error for UnknownFacilityError {
    #[allow(deprecated)] // Old versions of Rust require this.
    fn description(&self) -> &str {
        "unrecognized syslog facility name"
    }
}

#[test]
fn test_facility_from_str() {
    assert_eq!(Facility::from_str("daemon"), Ok(Facility::Daemon));
    assert_eq!(Facility::from_str("foobar"), Err(UnknownFacilityError { name: "foobar".to_string() }));
    assert_eq!(Facility::from_str("foobar").unwrap_err().to_string(), "unrecognized syslog facility name `foobar`");
}
