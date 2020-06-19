#[macro_use]
extern crate slog;
extern crate slog_syslog;

use slog_syslog::{Facility, SyslogBuilder};

fn main() {
    let syslog = SyslogBuilder::new().facility(Facility::User).build();
    let root = slog::Logger::root(syslog, o!());

    info!(root, "Starting");

    let log = root.new(o!("who" => "slog-syslog test", "build-id" => "8dfljdf"));

    info!(log, "Message"; "x" => -1, "y" => 2);
    error!(log, "Error");
}
