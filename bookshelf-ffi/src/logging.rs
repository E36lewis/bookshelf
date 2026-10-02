//! Routes the core's log messages to the app: `os.Logger` on the Mac, a
//! log file on Windows. Only Bookshelf's own warnings and notes are passed
//! on (not the HTTP and TLS crates'), and the core never logs what anyone
//! wrote, so nothing personal reaches a log.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{PoisonError, RwLock};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// How serious a log message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum LogLevel {
    /// Something failed.
    Error,
    /// Something went wrong but was worked around (a cover that didn't
    /// download, say).
    Warn,
    /// A note.
    Info,
}

/// Where the core's log messages go. Implemented by the app.
///
/// `log` is called on whatever thread the core is working on, sometimes
/// while the journal is busy, so it must be quick and must never call back
/// into the journal.
#[uniffi::export(callback_interface)]
pub trait Logger: Send + Sync {
    /// One message. `target` is the part of the core it came from
    /// (`bookshelf_core::backup`). Messages never hold what anyone wrote.
    fn log(&self, level: LogLevel, target: String, message: String);
}

/// The app's logger, once it has set one.
static SINK: RwLock<Option<Box<dyn Logger>>> = RwLock::new(None);

/// What the `log` crate sees: hands each message to the app's logger.
struct Forward;

static FORWARD: Forward = Forward;

impl Log for Forward {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info && metadata.target().starts_with("bookshelf")
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = match record.level() {
            Level::Error => LogLevel::Error,
            Level::Warn => LogLevel::Warn,
            _ => LogLevel::Info,
        };
        let sink = SINK.read().unwrap_or_else(PoisonError::into_inner);
        if let Some(sink) = sink.as_ref() {
            deliver(
                sink.as_ref(),
                level,
                record.target(),
                record.args().to_string(),
            );
        }
    }

    fn flush(&self) {}
}

/// Hands one message to the app. A logger that throws must not turn a
/// logged warning into a failed call: the message is dropped instead.
fn deliver(sink: &dyn Logger, level: LogLevel, target: &str, message: String) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        sink.log(level, target.to_owned(), message);
    }));
}

/// Sends the core's log messages to `logger` from now on. Call once at
/// startup; calling again replaces the logger.
#[uniffi::export]
pub fn set_logger(logger: Box<dyn Logger>) {
    *SINK.write().unwrap_or_else(PoisonError::into_inner) = Some(logger);
    // Only the first call installs it; the `log` crate allows one logger.
    if log::set_logger(&FORWARD).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    pub(crate) type Seen = Mutex<Vec<(LogLevel, String, String)>>;

    /// Every message logged in this test run. The logger is process-wide,
    /// so it's set once, here, for every test that checks a message.
    pub(crate) fn captured() -> &'static Seen {
        static SEEN: OnceLock<&'static Seen> = OnceLock::new();
        SEEN.get_or_init(|| {
            let seen: &'static Seen = Box::leak(Box::default());
            set_logger(Box::new(Collect(seen)));
            seen
        })
    }

    struct Collect(&'static Seen);

    impl Logger for Collect {
        fn log(&self, level: LogLevel, target: String, message: String) {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((level, target, message));
        }
    }

    struct Throws;

    impl Logger for Throws {
        fn log(&self, _: LogLevel, _: String, _: String) {
            panic!("the app's logger failed");
        }
    }

    fn metadata(target: &str, level: Level) -> Metadata<'_> {
        Metadata::builder().target(target).level(level).build()
    }

    #[test]
    fn only_our_messages_reach_the_app() {
        assert!(FORWARD.enabled(&metadata("bookshelf_core::service", Level::Warn)));
        assert!(FORWARD.enabled(&metadata("bookshelf_core::backup", Level::Info)));
        assert!(!FORWARD.enabled(&metadata("bookshelf_core::service", Level::Debug)));
        assert!(!FORWARD.enabled(&metadata("reqwest::connect", Level::Warn)));
    }

    #[test]
    fn messages_reach_the_app() {
        let seen = captured();
        log::warn!(target: "bookshelf_core::test", "kept {}", 1);
        log::error!(target: "bookshelf_ffi::test", "also kept");
        log::info!(target: "reqwest::connect", "not ours");
        log::debug!(target: "bookshelf_core::test", "too chatty");
        let seen = seen.lock().unwrap();
        let has = |level, target: &str, message: &str| {
            seen.contains(&(level, target.into(), message.into()))
        };
        assert!(has(LogLevel::Warn, "bookshelf_core::test", "kept 1"));
        assert!(has(LogLevel::Error, "bookshelf_ffi::test", "also kept"));
        assert!(seen
            .iter()
            .all(|(_, target, message)| target != "reqwest::connect" && message != "too chatty"));
    }

    #[test]
    fn a_failing_logger_is_harmless() {
        deliver(
            &Throws,
            LogLevel::Warn,
            "bookshelf_core::test",
            "dropped".into(),
        );
    }
}
