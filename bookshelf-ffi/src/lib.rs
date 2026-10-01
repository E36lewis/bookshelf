//! The Rust side of the macOS and Windows apps. UniFFI generates their Swift
//! and C# bindings from what's exported here (see `scripts/gen-bindings.sh`).
//!
//! Every call is synchronous; the apps make them off their UI thread. Keep
//! this layer thin: logic belongs in `bookshelf-core`, where all three apps
//! share it and it's tested.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use bookshelf_core::paths::AppPaths;
use bookshelf_core::rusqlite::Connection;
use bookshelf_core::text::highlight;
use bookshelf_core::{db, models};

uniffi::setup_scaffolding!();

/// Errors as the apps see them. Messages are written for people.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("{message}")]
    Database { message: String },
    #[error("{message}")]
    Network { message: String },
    #[error("{message}")]
    Io { message: String },
    #[error("not found")]
    NotFound,
    #[error("{message}")]
    Invalid { message: String },
}

impl From<bookshelf_core::Error> for CoreError {
    fn from(e: bookshelf_core::Error) -> Self {
        use bookshelf_core::Error as E;
        match e {
            E::Db(e) => CoreError::Database {
                message: e.to_string(),
            },
            E::Http(e) => CoreError::Network {
                message: e.to_string(),
            },
            E::Io(e) => CoreError::Io {
                message: e.to_string(),
            },
            E::NotFound => CoreError::NotFound,
            E::Invalid(message) => CoreError::Invalid { message },
        }
    }
}

type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Clone, uniffi::Record)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
}

impl From<models::User> for Profile {
    fn from(u: models::User) -> Self {
        Self {
            id: u.id,
            name: u.name,
            email: u.email,
        }
    }
}

/// One open journal: the database and its folder.
#[derive(uniffi::Object)]
pub struct Journal {
    conn: Mutex<Connection>,
    paths: AppPaths,
}

#[uniffi::export]
impl Journal {
    /// Opens (or creates) the journal in `dir`.
    #[uniffi::constructor]
    pub fn open_at(dir: String) -> Result<Arc<Self>> {
        let paths = AppPaths::in_dir(Path::new(&dir))?;
        let conn = db::open(&paths.db_path)?;
        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
            paths,
        }))
    }

    /// The folder the journal lives in.
    pub fn data_dir(&self) -> String {
        self.paths.data_dir.to_string_lossy().into_owned()
    }

    pub fn profiles(&self) -> Result<Vec<Profile>> {
        Ok(models::list_users(&self.conn())?
            .into_iter()
            .map(Profile::from)
            .collect())
    }

    pub fn create_profile(&self, name: String) -> Result<Profile> {
        Ok(models::create_user(&self.conn(), &name, None)?.into())
    }
}

impl Journal {
    fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves the connection itself fine.
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum StyleKind {
    Heading,
    Bold,
    Italic,
    Quote,
    Code,
    Strike,
    Syntax,
}

/// `start..end` in UTF-16 code units, as NSTextView and RichEditBox count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct StyleSpan {
    pub kind: StyleKind,
    pub start: u32,
    pub end: u32,
}

/// Markdown highlighting for the writing page (see `core::text::highlight`).
#[uniffi::export]
pub fn markdown_spans(text: String) -> Vec<StyleSpan> {
    use highlight::StyleKind as K;
    highlight::spans_utf16(&text)
        .into_iter()
        .map(|s| StyleSpan {
            kind: match s.kind {
                K::Heading => StyleKind::Heading,
                K::Bold => StyleKind::Bold,
                K::Italic => StyleKind::Italic,
                K::Quote => StyleKind::Quote,
                K::Code => StyleKind::Code,
                K::Strike => StyleKind::Strike,
                K::Syntax => StyleKind::Syntax,
            },
            start: s.start as u32,
            end: s.end as u32,
        })
        .collect()
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
    fn a_journal_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::open_at(dir.path().to_string_lossy().into_owned()).unwrap();
        assert!(journal.profiles().unwrap().is_empty());
        journal.create_profile("Avery".into()).unwrap();
        let names: Vec<String> = journal
            .profiles()
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Avery"]);
        let err = journal.create_profile("Avery".into()).unwrap_err();
        assert!(matches!(err, CoreError::Invalid { .. }), "{err:?}");
    }

    #[test]
    fn spans_cross_in_utf16() {
        let s = markdown_spans("📚 **b**".into());
        assert_eq!(
            s[0],
            StyleSpan {
                kind: StyleKind::Bold,
                start: 5,
                end: 6
            }
        );
    }
}
