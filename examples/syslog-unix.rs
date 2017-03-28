#[macro_use]
extern crate slog;
extern crate slog_syslog;

use slog::Drain;
use slog_syslog::Facility;

fn main() {
    let syslog = slog_syslog::unix_3164(Facility::LOG_USER).unwrap();
    let root = slog::Logger::root(syslog.fuse(), o!());

    info!(root, "Starting");

    let log = root.new(o!("who" => "slog-syslog test", "build-id" => "8dfljdf"));

    info!(log, "Message"; "x" => -1, "y" => 2);
    error!(log, "Error");
}
