//! Syslog drain for slog-rs
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
//!     // log to a local unix sock `/var/run/syslog`
//!     match slog_syslog::SyslogBuilder::new()
//!         .facility(Facility::LOG_USER)
//!         .level(slog::Level::Debug)
//!         .unix("/var/run/syslog")
//!         .start() {
//!         Ok(x) => {
//!             let root = Logger::root(x.fuse(), o);
//!         },
//!         Err(e) => println!("Failed to start syslog on `var/run/syslog`. Error {:?}", e)
//!     };
//! }
//! ```
#![warn(missing_docs)]

extern crate nix;
extern crate slog;
extern crate syslog;

use slog::{Drain, Level, OwnedKVList, Record};
use std::{fmt, io};
use std::sync::Mutex;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::net::SocketAddr;
use std::io::{Error, ErrorKind};

use slog::KV;

pub use syslog::Facility;

thread_local! {
    static TL_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(128))
}

fn level_to_severity(level: slog::Level) -> syslog::Severity {
    match level {
        Level::Critical => syslog::Severity::LOG_CRIT,
        Level::Error => syslog::Severity::LOG_ERR,
        Level::Warning => syslog::Severity::LOG_WARNING,
        Level::Info => syslog::Severity::LOG_NOTICE,
        Level::Debug => syslog::Severity::LOG_INFO,
        Level::Trace => syslog::Severity::LOG_DEBUG,
    }
}

/// Drain formatting records and writing them to a syslog ``Logger`
///
/// Uses mutex to serialize writes.
/// TODO: Add one that does not serialize?
pub struct Streamer3164 {
    io: Mutex<Box<syslog::Logger>>,
    format: Format3164,
}

impl Streamer3164 {
    /// Create new syslog ``Streamer` using given `format`
    pub fn new(logger: Box<syslog::Logger>) -> Self {
        Streamer3164 {
            io: Mutex::new(logger),
            format: Format3164::new(),
        }
    }
}

impl Drain for Streamer3164 {
    type Err = io::Error;
    type Ok = ();

    fn log(&self, info: &Record, logger_values: &OwnedKVList) -> io::Result<()> {
        TL_BUF.with(|buf| {
            let mut buf = buf.borrow_mut();
            let res = {
                || {
                    try!(self.format.format(&mut *buf, info, logger_values));
                    let sever = level_to_severity(info.level());
                    {
                        let io = try!(
                            self.io
                                .lock()
                                .map_err(|_| Error::new(ErrorKind::Other, "locking error"))
                        );

                        let buf = String::from_utf8_lossy(&buf);
                        let buf = io.format_3164(sever, &buf).into_bytes();

                        let mut pos = 0;
                        while pos < buf.len() {
                            let n = try!(io.send_raw(&buf[pos..]));
                            if n == 0 {
                                break;
                            }

                            pos += n;
                        }
                    }

                    Ok(())
                }
            }();
            buf.clear();
            res
        })
    }
}

/// Formatter to format defined in RFC 3164
pub struct Format3164;

impl Format3164 {
    /// Create new `Format3164`
    pub fn new() -> Self {
        Format3164
    }

    fn format(
        &self,
        io: &mut io::Write,
        record: &Record,
        logger_kv: &OwnedKVList,
    ) -> io::Result<()> {
        try!(write!(io, "{}", record.msg()));

        let mut ser = KSV::new(io);
        {
            try!(logger_kv.serialize(record, &mut ser));
            try!(record.kv().serialize(record, &mut ser));
        }
        Ok(())
    }
}

/// Key-Separator-Value serializer
struct KSV<W: io::Write> {
    io: W,
}

impl<W: io::Write> KSV<W> {
    fn new(io: W) -> Self {
        KSV { io: io }
    }
}

impl<W: io::Write> slog::Serializer for KSV<W> {
    fn emit_arguments(&mut self, key: &str, val: &fmt::Arguments) -> slog::Result {
        try!(write!(self.io, ", {}: {}", key, val));
        Ok(())
    }
}

enum SyslogKind {
    Unix {
        path: PathBuf,
    },
    Tcp {
        server: SocketAddr,
        hostname: String,
    },
    Udp {
        local: SocketAddr,
        host: SocketAddr,
        hostname: String,
    },
}

/// Builder pattern for constructing a syslog
pub struct SyslogBuilder {
    facility: Option<syslog::Facility>,
    level: syslog::Severity,
    logkind: Option<SyslogKind>,
}
impl Default for SyslogBuilder {
    fn default() -> Self {
        Self {
            facility: None,
            level: syslog::Severity::LOG_DEBUG,
            logkind: None,
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

    /// Set syslog Facility
    pub fn facility(self, facility: syslog::Facility) -> Self {
        let mut s = self;
        s.facility = Some(facility);
        s
    }

    /// Filter Syslog by level
    pub fn level(self, lvl: slog::Level) -> Self {
        let mut s = self;
        s.level = level_to_severity(lvl);
        s
    }

    /// Remote UDP syslogging
    pub fn udp<S: AsRef<str>>(self, local: SocketAddr, host: SocketAddr, hostname: S) -> Self {
        let mut s = self;
        let hostname = hostname.as_ref().to_string();
        s.logkind = Some(SyslogKind::Udp {
            local,
            host,
            hostname,
        });
        s
    }

    /// Remote TCP syslogging
    pub fn tcp<S: AsRef<str>>(self, server: SocketAddr, hostname: S) -> Self {
        let mut s = self;
        let hostname = hostname.as_ref().to_string();
        s.logkind = Some(SyslogKind::Tcp { server, hostname });
        s
    }

    /// Local syslogging over a unix socket
    pub fn unix<P: AsRef<Path>>(self, path: P) -> Self {
        let mut s = self;
        let path = path.as_ref().to_path_buf();
        s.logkind = Some(SyslogKind::Unix { path });
        s
    }

    /// Start running
    pub fn start(self) -> io::Result<Streamer3164> {
        let facility = match self.facility {
            Option::Some(x) => x,
            Option::None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "facility must be provided to the builder",
                ));
            }
        };
        let logkind = match self.logkind {
            Option::Some(l) => l,
            Option::None => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "no logger kind provided, library does not know what do initialize",
                ));
            }
        };
        let log = match logkind {
            SyslogKind::Unix { path } => syslog::unix_custom(facility, path)?,
            SyslogKind::Udp {
                local,
                host,
                hostname,
            } => syslog::udp(local, host, hostname, facility)?,
            SyslogKind::Tcp { server, hostname } => syslog::tcp(server, hostname, facility)?,
        };
        Ok(Streamer3164::new(log))
    }
}

/// `Streamer` to Unix syslog using RFC 3164 format
pub fn unix_3164(facility: syslog::Facility) -> io::Result<Streamer3164> {
    syslog::unix(facility).map(Streamer3164::new)
}
