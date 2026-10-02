//! Bookshelf core: database, models, Open Library client, cover storage.
//! No UI code lives here. The GTK front end calls into this crate.
//!
//! Threading: `rusqlite::Connection` is not `Sync`. Each thread (UI thread,
//! each worker thread) should call `db::open` for its own connection. WAL
//! mode makes that safe.

pub mod backup;
pub mod covers;
pub mod db;
pub mod error;
pub mod export;
pub mod logging;
pub mod manual;
pub mod mdedit;
pub mod models;
pub mod openlibrary;
pub mod paths;
pub mod present;
pub mod render;
pub mod service;
pub mod shortcuts;
pub mod text;
pub mod theme;
pub mod writer;

#[cfg(windows)]
mod winacl;

// The one-off import from the old Rails app. Only built with
// `--features rails-import`, so it never ships in the app.
#[cfg(feature = "rails-import")]
pub mod html_to_markdown;
#[cfg(feature = "rails-import")]
pub mod import;

pub use error::{Error, Result};
pub use rusqlite;
