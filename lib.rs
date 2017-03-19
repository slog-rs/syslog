//! Syslog drain for slog-rs
//!
//! WARNING: This crate needs some improvements.
//!
//! ```
//! extern crate slog;
//! extern crate slog_syslog;
//!
//! use slog::*;
//! use slog_syslog::Facility;
//!
//! fn main() {
//!     let drain = slog_syslog::unix_3164(
//!                 Facility::LOG_USER,
//!                 );
//!     let root = Logger::root(drain.fuse(), o!("build-id" => "8dfljdf"));
//! }
//! ```
#![warn(missing_docs)]

extern crate slog;
extern crate syslog;
extern crate nix;

use slog::{Drain, Level, Record, OwnedKVList};
use std::{io, fmt};
use std::sync::Mutex;
use std::cell::RefCell;

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
                        let io = try!(self.io
                            .lock()
                            .map_err(|_| io::Error::new(io::ErrorKind::Other, "locking error")));

                        let buf = String::from_utf8_lossy(&buf);
                        let buf = io.format_3164(sever, &buf).into_bytes();

                        let mut pos = 0;
                        while pos < buf.len() {
                            let n = try!(io.send_raw(&buf[pos..]));
                            if n == 0 {
                                break
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

    fn format(&self,
              io: &mut io::Write,
              record: &Record,
              logger_kv: &OwnedKVList)
              -> io::Result<()> {
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
        KSV {
            io: io,
        }
    }
}

impl<W: io::Write> slog::Serializer for KSV<W> {
    fn emit_arguments(&mut self, key: &str, val: &fmt::Arguments) -> slog::Result {
        try!(write!(self.io, ", {}: {}", key, val));
        Ok(())
    }
}
/// `Streamer` to Unix syslog using RFC 3164 format
pub fn unix_3164(facility: syslog::Facility) -> Streamer3164 {
    Streamer3164::new(syslog::unix(facility).unwrap())
}
