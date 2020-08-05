#[macro_use]
extern crate slog;
extern crate slog_syslog;

use slog_syslog::adapter::{Adapter, DefaultAdapter};
use slog_syslog::{Facility, SyslogBuilder};
use slog::Level;

fn main() {
    let syslog = SyslogBuilder::new()
        .facility(Facility::User)
        .adapter(DefaultAdapter.with_priority(|record, values| match record.level() {
            Level::Info => slog_syslog::Level::Notice.into(),
            _ => DefaultAdapter.priority(record, values),
        }))
        .build();

    let root = slog::Logger::root(syslog, o!());

    info!(root, "Starting");

    let log = root.new(o!("who" => "slog-syslog test", "build-id" => "8dfljdf"));

    info!(log, "Message"; "x" => -1, "y" => 2);
    error!(log, "Error");
}
