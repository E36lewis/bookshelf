use std::path::Path;

use rusqlite::Connection;

use crate::{Error, Result};

/// Ordered migrations. Index + 1 == schema version (stored in PRAGMA user_version).
/// Never edit an existing entry; append a new one.
const MIGRATIONS: &[&str] = &[
    // 1: initial schema (ported from the Rails app; UUIDs stored as TEXT)
    r#"
    CREATE TABLE users (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        color TEXT NOT NULL DEFAULT '#4f46e5',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE user_settings (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
        date_format TEXT NOT NULL DEFAULT 'mm_dd_yyyy',
        language TEXT NOT NULL DEFAULT 'en',
        time_format TEXT NOT NULL DEFAULT '12h',
        timezone_name TEXT
    );

    CREATE TABLE books (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        subtitle TEXT,
        author TEXT,
        isbn TEXT,
        publisher TEXT,
        description TEXT,
        published_date TEXT,          -- free text: Open Library gives a year
        page_count INTEGER,
        cover_url TEXT,
        cover_path TEXT,              -- filename inside covers_dir
        provider INTEGER NOT NULL DEFAULT 0,
        external_id TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        UNIQUE (provider, external_id)
    );
    CREATE INDEX idx_books_title ON books(title);
    CREATE INDEX idx_books_author ON books(author);

    CREATE TABLE summaries (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
        body TEXT NOT NULL DEFAULT '',   -- replaces Action Text
        started_on TEXT,
        finished_on TEXT,
        days_to_complete INTEGER,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE INDEX idx_summaries_user ON summaries(user_id);
    CREATE INDEX idx_summaries_book ON summaries(book_id);
    "#,
    // 2: per-profile look & feel
    r#"
    ALTER TABLE user_settings ADD COLUMN theme TEXT NOT NULL DEFAULT 'system';
    ALTER TABLE user_settings ADD COLUMN accent TEXT NOT NULL DEFAULT '#b4532a';
    ALTER TABLE user_settings ADD COLUMN heading_font TEXT NOT NULL DEFAULT 'serif';
    ALTER TABLE user_settings ADD COLUMN writing_font TEXT NOT NULL DEFAULT 'ia_duo';
    ALTER TABLE user_settings ADD COLUMN writing_size INTEGER NOT NULL DEFAULT 14;
    ALTER TABLE user_settings ADD COLUMN line_spacing TEXT NOT NULL DEFAULT 'normal';
    ALTER TABLE user_settings ADD COLUMN page_width TEXT NOT NULL DEFAULT 'medium';
    ALTER TABLE user_settings ADD COLUMN focus_default INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE user_settings ADD COLUMN start_tab TEXT NOT NULL DEFAULT 'reading';
    ALTER TABLE user_settings ADD COLUMN week_start TEXT NOT NULL DEFAULT 'sunday';
    -- The untouched Rails default becomes the journal-style "Sep 20, 2026".
    UPDATE user_settings SET date_format = 'long' WHERE date_format = 'mm_dd_yyyy';
    "#,
    // 3: the default accent is now the blue from the shared palette
    r#"
    UPDATE user_settings SET accent = '#2d71e5' WHERE accent = '#b4532a';
    "#,
    // 4: optional contact email, sent to Open Library in the User-Agent
    r#"
    ALTER TABLE users ADD COLUMN email TEXT;
    "#,
    // 5: settings for the whole app (not per profile), e.g. the backup folder
    r#"
    CREATE TABLE app_settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
];

pub fn open(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Folds the write-ahead log back into the main file and empties it. Call
/// on quit: a leftover log would otherwise be replayed onto whatever file
/// sits at the same path next time, such as a restored backup.
pub fn checkpoint(conn: &Connection) -> Result<()> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    // A newer Bookshelf made this journal; running on a schema we don't
    // know could lose whatever the newer version stores.
    if current > MIGRATIONS.len() {
        return Err(Error::Invalid(format!(
            "this journal was made by a newer version of Bookshelf (format {current}, \
             this version understands up to {}). Please update Bookshelf to open it",
            MIGRATIONS.len()
        )));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_journal_from_a_newer_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("j.sqlite3");
        drop(open(&path).unwrap());
        let raw = Connection::open(&path).unwrap();
        raw.pragma_update(None, "user_version", (MIGRATIONS.len() + 1) as i64).unwrap();
        drop(raw);
        let err = open(&path).unwrap_err().to_string();
        assert!(err.contains("newer version"), "{err}");
    }

    #[test]
    fn checkpoint_empties_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("j.sqlite3");
        let conn = open(&path).unwrap();
        crate::models::create_user(&conn, "Reader", None).unwrap();
        let wal = dir.path().join("j.sqlite3-wal");
        assert!(std::fs::metadata(&wal).unwrap().len() > 0);
        checkpoint(&conn).unwrap();
        assert_eq!(std::fs::metadata(&wal).unwrap().len(), 0);
    }
}
