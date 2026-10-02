//! Core logs through the `log` facade, so each app decides where messages
//! go: the macOS and Windows apps route them to their own logs, GTK and the
//! CLI print them to stderr with [`log_to_stderr`].
//!
//! Never log summary text or other personal content: logs end up in bug
//! reports and system consoles.

use log::{Level, LevelFilter, Log, Metadata, Record};

struct Stderr;

impl Log for Stderr {
    fn enabled(&self, metadata: &Metadata) -> bool {
        shown(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[bookshelf] {}", record.args());
        }
    }

    fn flush(&self) {}
}

/// Only our own messages: the HTTP and TLS crates also log, and stderr
/// should read as it always has.
fn shown(target: &str, level: Level) -> bool {
    level <= Level::Info && target.starts_with("bookshelf")
}

/// Prints Bookshelf's warnings and notes to stderr as `[bookshelf] ...`.
/// Call once at startup; a later call (or another logger) is left alone.
pub fn log_to_stderr() {
    if log::set_logger(&Stderr).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_own_messages_are_printed() {
        assert!(shown("bookshelf_core::service", Level::Warn));
        assert!(shown("bookshelf_core::backup", Level::Info));
        assert!(!shown("bookshelf_core::service", Level::Debug));
        assert!(!shown("reqwest::connect", Level::Warn));
        assert!(!shown("rustls::client", Level::Info));
    }
}
