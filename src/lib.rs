//! Slog [`Drain`] that sends logs to local syslog server. Unix-like platforms
//! only. Uses the [POSIX syslog API].
//! 
//! [`Drain`]: https://docs.rs/slog/2/slog/trait.Drain.html
//! [POSIX syslog API]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/closelog.html
//! 
//! # Example
//! 
//! ```
//! #[macro_use] extern crate slog;
//! # extern crate slog_syslog;
//! 
//! use slog::Logger;
//! use slog_syslog::{Facility, SyslogBuilder};
//! use std::ffi::CStr;
//! 
//! let drain = SyslogBuilder::new()
//!     .facility(Facility::User)
//!     .ident(CStr::from_bytes_with_nul(b"example-app\0").unwrap())
//!     .build();
//! 
//! let logger = Logger::root(drain, o!());
//! 
//! info!(logger, "Hello, world! This is a test message from `slog-syslog`.");
//! ```
//! 
//! # Cargo features
//! 
//! If the Cargo feature `serde` is enabled, syslog settings can be loaded from
//! a configuration file using [`config::SyslogConfig`].
//! 
//! [`config::SyslogConfig`]: config/struct.SyslogConfig.html
//! 
//! # Concurrency issues
//! 
//! POSIX doesn't support opening more than one connection to the syslog server
//! at a time. All [`SyslogBuilder` settings] except
//! [`adapter`][`SyslogBuilder::adapter`] are stored in global variables by the
//! platform libc, are overwritten whenever the POSIX `openlog` function is
//! called (which happens when a [`SyslogDrain`] is created), and are reset
//! whenever the POSIX `closelog` function is called (which may happen when a
//! [`SyslogDrain`] is dropped).
//! 
//! For this reason, the following rules should be followed:
//! 
//! * Libraries should not construct a [`SyslogDrain`] or otherwise call
//!   `openlog` or `closelog` unless specifically told to do so by the main
//!   application.
//! * An application that uses this crate should not cause `openlog` or
//!   `closelog` to be called from anywhere else.
//! * An application should not construct more than one [`SyslogDrain`] at the
//!   same time, except when constructing a new [`SyslogDrain`] that
//!   is to replace an old one (for instance, if the application is reloading
//!   its configuration files and reinitializing its logging pipeline).
//! 
//! Failure to abide by these rules may result in `closelog` being called at
//! the wrong time. This will cause [`SyslogBuilder` settings] (except
//! [`adapter`][`SyslogBuilder::adapter`]) to be reset, and there may be a delay
//! in processing the next log message after that (because the connection to
//! the syslog server, if applicable, must be reopened).
//! 
//! [`SyslogBuilder::adapter`]: struct.SyslogBuilder.html#method.adapter
//! [`SyslogBuilder` settings]: struct.SyslogBuilder.html#impl
//! [`SyslogDrain`]: struct.SyslogDrain.html

// TODO: Some systems (including OpenBSD and Android) have reentrant versions
// of the POSIX syslog functions. These systems *do* support opening multiple
// connections to syslog, and therefore do not suffer from the above
// concurrency issues. Perhaps this crate should use the reentrant syslog API
// on those platforms.

// # Design and rationale
// 
// (This section is not part of the documentation for this crate. It's only a
// source code comment.)
// 
// This crate uses the POSIX syslog API to submit log entries to the local
// syslogd. This is unlike the `syslog` crate, which connects to `/dev/log` 
// or `/var/run/log` directly. The reasons for this approach, despite the above
// drawbacks, are as follows.
// 
// ## Portability
// 
// POSIX only specifies the `syslog` function and related functions.
// 
// POSIX does not specify that a Unix-domain socket is used for submitting log
// messages to syslogd, nor the socket's path, nor the protocol used on that
// socket. The path of the socket is different on different systems:
// 
// * `/dev/log` – original BSD, OpenBSD, Linux
// * `/var/run/log` – FreeBSD and NetBSD (but on Linux with systemd, this
//   is a folder)
// * `/var/run/syslog` – Darwin/macOS
// 
// The protocol spoken on the socket is not formally specified. It is
// whatever the system's `syslog` function writes to it, which may of course
// vary between systems. It is typically different from IETF RFCs 3164 and
// 5424.
// 
// The OpenBSD kernel has a dedicated system call for submitting log messages.
// `/dev/log` is still available, but not preferred.
// 
// On macOS, the `syslog` function submits log entries to the Apple System Log
// service. BSD-style log messages are accepted on `/var/run/syslog`, but that
// is not preferred.
// 
// ## Reliability
// 
// On every platform that has a `syslog` function, it is battle-tested and
// very definitely works.
// 
// ## Simplicity
// 
// Even in “classic” implementations of the POSIX `syslog` function, there are
// a number of details that it keeps track of:
// 
// * Opening the socket
// * Reopening the socket when necessary
// * Formatting log messages for consumption by syslogd
// * Determining the name of the process, when none is specified by the
//   application
// 
// By calling the POSIX function, we avoid needing to reimplement all this in
// Rust.

#![cfg(unix)]
#![warn(missing_docs)]

extern crate libc;

#[macro_use]
extern crate lazy_static;

#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

#[cfg_attr(test, macro_use)]
extern crate slog;

#[cfg(all(test, feature = "serde"))]
extern crate toml;

pub mod adapter;

mod builder;
pub use builder::*;

#[cfg(feature = "serde")]
pub mod config;

mod drain;
pub use drain::*;

mod facility;
pub use facility::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod level;
pub use level::*;

mod priority;
pub use priority::*;
