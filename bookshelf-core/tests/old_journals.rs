//! Journals written by earlier Bookshelf builds (rusqlite 0.32, SQLite
//! 3.46) must keep opening and reading the same after a rusqlite update.
//! The dates and times here are SQL literals in the format those builds
//! wrote, not values written by the code under test.

use bookshelf_core::db;
use bookshelf_core::models;
use bookshelf_core::rusqlite::{params, Connection};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};

fn utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// Bookshelf's first journal format (user_version 1), frozen.
const FORMAT_1: &str = r#"
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
        published_date TEXT,
        page_count INTEGER,
        cover_url TEXT,
        cover_path TEXT,
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
        body TEXT NOT NULL DEFAULT '',
        started_on TEXT,
        finished_on TEXT,
        days_to_complete INTEGER,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE INDEX idx_summaries_user ON summaries(user_id);
    CREATE INDEX idx_summaries_book ON summaries(book_id);
    PRAGMA user_version = 1;

    -- Times as rusqlite 0.32 wrote DateTime<Utc> ("%F %T%.f%:z": nanoseconds
    -- from Utc::now, microseconds from the Rails import, or none at all)
    -- and dates as it wrote NaiveDate ("%F").
    INSERT INTO users VALUES ('u1', 'Ann', '#4f46e5',
        '2024-07-26 17:11:20.123456789+00:00', '2024-07-26 17:11:20+00:00');
    INSERT INTO user_settings (id, user_id) VALUES ('s1', 'u1');
    INSERT INTO books (id, title, author, external_id, created_at, updated_at)
        VALUES ('b1', 'Dune', 'Frank Herbert', 'OL1W',
        '2024-07-01 08:00:00.5+00:00', '2024-07-02 09:30:15.250000+00:00');
    INSERT INTO summaries VALUES ('m1', 'u1', 'b1', 'Spice.', '2024-07-01',
        '2024-07-26', 25, '2024-07-01 08:00:01.000000001+00:00',
        '2024-07-26 23:59:59.999999999+00:00');
    INSERT INTO summaries VALUES ('m2', 'u1', 'b1', '', NULL, NULL, NULL,
        '2024-07-01 08:00:02+00:00', '2024-07-01 08:00:02+00:00');
"#;

#[test]
fn a_journal_in_the_first_format_is_migrated_and_reads_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bookshelf.sqlite3");
    Connection::open(&path)
        .unwrap()
        .execute_batch(FORMAT_1)
        .unwrap();

    let conn = db::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 5);

    let user = models::get_user(&conn, "u1").unwrap();
    assert_eq!(user.name, "Ann");
    assert_eq!(user.email, None);
    assert_eq!(user.created_at, utc("2024-07-26T17:11:20.123456789+00:00"));
    assert_eq!(user.updated_at, utc("2024-07-26T17:11:20+00:00"));

    // Migrations 2 and 3 ran on the old settings row.
    let settings = models::get_settings(&conn, "u1").unwrap();
    assert_eq!(settings.date_format, "long");
    assert_eq!(settings.accent, "#2d71e5");
    assert_eq!(settings.writing_size, 14);

    let book = models::get_book(&conn, "b1").unwrap();
    assert_eq!(book.created_at, utc("2024-07-01T08:00:00.5+00:00"));
    assert_eq!(book.updated_at, utc("2024-07-02T09:30:15.25+00:00"));

    let s = models::get_summary(&conn, "m1").unwrap();
    assert_eq!(s.started_on, Some(day(2024, 7, 1)));
    assert_eq!(s.finished_on, Some(day(2024, 7, 26)));
    assert_eq!(s.days_to_complete, Some(25));
    assert_eq!(s.created_at, utc("2024-07-01T08:00:01.000000001+00:00"));
    assert_eq!(s.updated_at, utc("2024-07-26T23:59:59.999999999+00:00"));
    let s = models::get_summary(&conn, "m2").unwrap();
    assert_eq!((s.started_on, s.finished_on), (None, None));

    // Newest first, by the stored text.
    let ids: Vec<String> = models::summaries_for_book(&conn, "b1")
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, ["m2", "m1"]);

    // Opening again is a no-op.
    drop(conn);
    let conn = db::open(&path).unwrap();
    assert_eq!(models::get_user(&conn, "u1").unwrap().name, "Ann");
}

#[test]
fn dates_and_times_are_written_as_before() {
    let conn = db::open_in_memory().unwrap();
    let whole = Utc.with_ymd_and_hms(2026, 9, 20, 14, 3, 7).unwrap();
    let nanos = whole + Duration::nanoseconds(123_456_789);
    let micros = whole + Duration::microseconds(250_000);
    let written: Vec<String> = conn
        .query_row(
            "SELECT ?1, ?2, ?3, ?4, typeof(?1), typeof(?4)",
            params![whole, nanos, micros, day(2026, 9, 1)],
            |r| (0..6).map(|i| r.get(i)).collect(),
        )
        .unwrap();
    assert_eq!(
        written,
        [
            "2026-09-20 14:03:07+00:00",
            "2026-09-20 14:03:07.123456789+00:00",
            "2026-09-20 14:03:07.250+00:00",
            "2026-09-01",
            "text",
            "text",
        ]
    );

    // And through the models, as the apps save them.
    let user = models::create_user(&conn, "Ann", None).unwrap();
    let book = models::find_or_create_book(
        &conn,
        &models::NewBook {
            external_id: "OL1W".into(),
            title: "Dune".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let input = models::SummaryInput {
        body: "Spice.".into(),
        started_on: Some(day(2026, 9, 1)),
        finished_on: Some(day(2026, 9, 20)),
    };
    let s = models::create_summary(&conn, &user.id, &book.id, &input).unwrap();
    let (started, finished, created): (String, String, String) = conn
        .query_row(
            "SELECT started_on, finished_on, created_at FROM summaries WHERE id = ?1",
            [&s.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (started.as_str(), finished.as_str()),
        ("2026-09-01", "2026-09-20")
    );
    // "YYYY-MM-DD HH:MM:SS[.fraction]+00:00", as before.
    assert_eq!(&created[10..11], " ");
    assert!(created.ends_with("+00:00"), "{created}");
    assert_eq!(utc(&created.replacen(' ', "T", 1)), s.created_at);
}

#[test]
fn a_journal_file_is_opened_with_the_same_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bookshelf.sqlite3");
    let conn = db::open(&path).unwrap();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    let busy: i64 = conn
        .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
        .unwrap();
    assert_eq!((mode.as_str(), foreign_keys, busy), ("wal", 1, 5000));

    // data_version moves when another connection writes, and
    // total_changes counts this connection's own writes.
    let data_version = |c: &Connection| -> i64 {
        c.query_row("PRAGMA data_version", [], |r| r.get(0))
            .unwrap()
    };
    let before = (data_version(&conn), conn.total_changes());
    let other = db::open(&path).unwrap();
    models::create_user(&other, "Ann", None).unwrap();
    assert_ne!(data_version(&conn), before.0);
    assert_eq!(conn.total_changes(), before.1);
    models::create_user(&conn, "Bea", None).unwrap();
    assert!(conn.total_changes() > before.1);
}

#[test]
fn a_negative_journal_format_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bookshelf.sqlite3");
    Connection::open(&path)
        .unwrap()
        .execute_batch("PRAGMA user_version = -1")
        .unwrap();
    assert!(db::open(&path).is_err());
}
