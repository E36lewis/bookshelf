//! Bookshelf core: database, models, Open Library client, cover storage.
//! No UI code lives here. The GTK front end calls into this crate.
//!
//! Threading: `rusqlite::Connection` is not `Sync`. Each thread (UI thread,
//! each worker thread) should call `db::open` for its own connection. WAL
//! mode makes that safe.

pub mod covers;
pub mod db;
pub mod error;
pub mod export;
pub mod html_to_markdown;
pub mod import;
pub mod mdedit;
pub mod models;
pub mod openlibrary;
pub mod paths;
pub mod service;

pub use error::{Error, Result};
pub use rusqlite;
