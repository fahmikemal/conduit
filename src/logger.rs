//! Minimal error-log writer for the `log` facade: one line per record on stderr.
//! Pingora points stderr at `server.error_log` when daemonized.

use std::io::Write;

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::timefmt;

/// Pingora logs this as an error when a pooled upstream connection is still held by its
/// idle watcher, a task on another worker thread that is just letting go of it (more
/// likely with `work_stealing: false`). Nothing fails: the request opens a new connection.
/// It is counted (conduit_upstream_reuse_lost_total) and logged at debug instead.
fn benign(record: &Record<'_>) -> bool {
    record.level() == Level::Error
        && record.target() == "pingora_core::connectors"
        && record.args().as_str() == Some("failed to acquire reusable stream")
}

/// Pingora's cache asks for a dictionary to compress the headers of cached responses
/// (useful for its disk storage); conduit's cache keeps them in memory as they are. Logged
/// once, at the first cached response, at debug instead of warn.
fn chatty(record: &Record<'_>) -> bool {
    record.target() == "pingora_cache::meta"
        && record
            .args()
            .as_str()
            .is_some_and(|m| m.starts_with("no header compression dictionary loaded"))
}

struct Logger {
    level: LevelFilter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let mut level = record.level();
        let race = benign(record);
        if race {
            crate::metrics::UPSTREAM_REUSE_LOST.inc();
        }
        if race || chatty(record) {
            if self.level < LevelFilter::Debug {
                return;
            }
            level = Level::Debug;
        }
        let line = format!(
            "{} {:<5} {}: {}\n",
            timefmt::rfc3339(timefmt::now_secs()),
            level,
            record.target(),
            record.args()
        );
        let _ = std::io::stderr().lock().write_all(line.as_bytes());
    }

    fn flush(&self) {}
}

/// Level from `CONDUIT_LOG` (error, warn, info, debug, trace); `info` by default.
pub fn init() {
    let level = std::env::var("CONDUIT_LOG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(LevelFilter::Info);
    let logger: &'static Logger = Box::leak(Box::new(Logger { level }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_pool_race_is_benign() {
        let check = |level, target, args: std::fmt::Arguments<'_>| {
            benign(
                &Record::builder()
                    .level(level)
                    .target(target)
                    .args(args)
                    .build(),
            )
        };
        let msg = "failed to acquire reusable stream";
        assert!(check(
            Level::Error,
            "pingora_core::connectors",
            format_args!("failed to acquire reusable stream")
        ));
        assert!(!check(
            Level::Error,
            "pingora_core::connectors",
            format_args!("{msg} now")
        ));
        assert!(!check(
            Level::Error,
            "conduit::proxy",
            format_args!("failed to acquire reusable stream")
        ));
        assert!(!check(
            Level::Warn,
            "pingora_core::connectors",
            format_args!("failed to acquire reusable stream")
        ));
        let dict = |target| {
            chatty(
                &Record::builder()
                    .level(Level::Warn)
                    .target(target)
                    .args(format_args!("no header compression dictionary loaded - use set_compression_dict_content() or set_compression_dict_path() to set one"))
                    .build(),
            )
        };
        assert!(dict("pingora_cache::meta"));
        assert!(!dict("conduit::cache"));
    }
}
