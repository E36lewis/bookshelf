//! The Rust side of the macOS and Windows apps. UniFFI generates their Swift
//! and C# bindings from what's exported here (see `scripts/gen-bindings.sh`).
//!
//! Every call is synchronous; the apps make them off their UI thread. Keep
//! this layer thin: logic belongs in `bookshelf-core`, where all three apps
//! share it and it's tested. What this layer adds is the boundary: plain
//! records and enums, checks on what comes in, and errors as sentences.
//!
//! Across the boundary, text offsets are UTF-16 code units, calendar dates
//! are `YYYY-MM-DD` strings, timestamps are Unix milliseconds and ids are
//! strings.

use std::path::Path;

use bookshelf_core::{models, paths, present};

mod help;
mod journal;
mod logging;
mod look;
mod text;
mod types;
mod validate;

#[cfg(test)]
mod journal_tests;

pub use help::*;
pub use journal::*;
pub use logging::*;
pub use look::*;
pub use text::*;
pub use types::*;

uniffi::setup_scaffolding!();

/// Errors as the apps see them. Every `message` is a sentence for people,
/// ready to show as is ("That name is already taken.").
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    /// The journal file couldn't be read or written.
    #[error("{message}")]
    Database {
        /// The sentence to show.
        message: String,
    },
    /// Open Library couldn't be reached, or answered with an error.
    #[error("{message}")]
    Network {
        /// The sentence to show.
        message: String,
    },
    /// A file or folder couldn't be read or written.
    #[error("{message}")]
    Io {
        /// The sentence to show.
        message: String,
    },
    /// No such profile, entry or book (it may have just been deleted).
    #[error("{message}")]
    NotFound {
        /// The sentence to show.
        message: String,
    },
    /// Something passed in was refused: a blank or taken name, a bad
    /// email or date, a folder that isn't one.
    #[error("{message}")]
    Invalid {
        /// The sentence to show.
        message: String,
    },
    /// The journal was made by a newer Bookshelf, with a format (`found`)
    /// newer than this one understands (`supported`). Ask for an update.
    #[error("{message}")]
    NewerJournal {
        /// The journal's format version.
        found: u32,
        /// The newest format this build understands.
        supported: u32,
        /// The sentence to show.
        message: String,
    },
}

impl From<bookshelf_core::Error> for CoreError {
    fn from(e: bookshelf_core::Error) -> Self {
        use bookshelf_core::Error as E;
        let message = e.user_message();
        match e {
            E::Db(_) => CoreError::Database { message },
            E::Http(_) => CoreError::Network { message },
            E::Io(_) => CoreError::Io { message },
            E::NotFound => CoreError::NotFound { message },
            E::Invalid(_) => CoreError::Invalid { message },
            E::NewerJournal { found, supported } => CoreError::NewerJournal {
                found: u32::try_from(found).unwrap_or(u32::MAX),
                supported: u32::try_from(supported).unwrap_or(u32::MAX),
                message,
            },
        }
    }
}

impl From<bookshelf_core::rusqlite::Error> for CoreError {
    fn from(e: bookshelf_core::rusqlite::Error) -> Self {
        bookshelf_core::Error::from(e).into()
    }
}

type Result<T> = std::result::Result<T, CoreError>;

/// The words of a shelf search, lowercased. No words means no search.
#[uniffi::export]
pub fn query_terms(query: String) -> Vec<String> {
    present::query_terms(&query)
}

/// Whether a shelf entry matches a search: every word of `terms` (from
/// `query_terms`) appears somewhere in its `haystack`, in any order.
#[uniffi::export]
pub fn matches(haystack: String, terms: Vec<String>) -> bool {
    present::matches(&haystack, &terms)
}

/// Checks an email address as typed: `None` for blank, the trimmed address
/// if it's usable, or an `Invalid` error to show under the field.
#[uniffi::export]
pub fn validate_email(email: String) -> Result<Option<String>> {
    Ok(models::validate_email(&email)?)
}

/// A path for people, with the home folder shown as `~` (`home` is the
/// user's home folder, if the app wants that).
#[uniffi::export]
pub fn display_path(path: String, home: Option<String>) -> String {
    paths::display_path(Path::new(&path), home.as_deref().map(Path::new))
}

/// The core's version, so an app can show what it's running on.
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_sentences() {
        let e: CoreError =
            bookshelf_core::Error::Invalid("that name is already taken".into()).into();
        assert_eq!(e.to_string(), "That name is already taken.");
        assert!(matches!(e, CoreError::Invalid { .. }));
        let e: CoreError = bookshelf_core::Error::NotFound.into();
        assert!(matches!(&e, CoreError::NotFound { message } if message == "Not found."));
        let e: CoreError = bookshelf_core::Error::Io(std::io::Error::other("disk full")).into();
        assert!(matches!(&e, CoreError::Io { message } if message == "Io error: disk full."));
        let e: CoreError =
            bookshelf_core::Error::Db(bookshelf_core::rusqlite::Error::QueryReturnedNoRows).into();
        assert!(matches!(e, CoreError::Database { .. }));
    }

    #[test]
    fn a_newer_journal_says_so() {
        let e: CoreError = bookshelf_core::Error::NewerJournal {
            found: 9,
            supported: 5,
        }
        .into();
        let CoreError::NewerJournal {
            found,
            supported,
            message,
        } = e
        else {
            panic!("{e:?}")
        };
        assert_eq!((found, supported), (9, 5));
        assert!(message.starts_with("This journal was made by a newer version"));
        assert!(message.ends_with("Please update Bookshelf to open it."));
    }

    #[test]
    fn search_helpers() {
        let terms = query_terms("  Frank HERBERT ".into());
        assert_eq!(terms, ["frank", "herbert"]);
        assert!(matches("dune frank herbert".into(), terms.clone()));
        assert!(!matches("dune".into(), terms));
        assert!(matches("anything".into(), query_terms(" ".into())));
    }

    #[test]
    fn emails_as_typed() {
        assert_eq!(validate_email("  ".into()).unwrap(), None);
        assert_eq!(
            validate_email(" me@example.com ".into())
                .unwrap()
                .as_deref(),
            Some("me@example.com")
        );
        let e = validate_email("me@example.com\r\nX: y".into()).unwrap_err();
        assert_eq!(e.to_string(), "That doesn't look like an email address.");
    }

    #[cfg(unix)]
    #[test]
    fn paths_for_people() {
        assert_eq!(
            display_path("/home/ann/notes".into(), Some("/home/ann".into())),
            "~/notes"
        );
        assert_eq!(
            display_path("/tmp/x".into(), Some("/home/ann".into())),
            "/tmp/x"
        );
        assert_eq!(display_path("/home/ann".into(), None), "/home/ann");
    }

    #[test]
    fn version() {
        assert_eq!(core_version(), env!("CARGO_PKG_VERSION"));
    }
}
