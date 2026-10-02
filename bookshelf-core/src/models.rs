use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// Value of the Rails `provider` enum for Open Library.
/// TODO: confirm against the Rails Book model when we migrate data.
pub const PROVIDER_OPEN_LIBRARY: i32 = 0;

/// Default accent color: the link blue from the shared palette.
pub const DEFAULT_ACCENT: &str = "#2d71e5";

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

// ---------------------------------------------------------------- users

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
    pub color: String,
    /// Optional contact for Open Library's User-Agent. Always validated.
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            name: r.get("name")?,
            color: r.get("color")?,
            email: r.get("email")?,
            created_at: r.get("created_at")?,
            updated_at: r.get("updated_at")?,
        })
    }
}

pub fn create_user(conn: &Connection, name: &str, color: Option<&str>) -> Result<User> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("name can't be blank".into()));
    }
    let id = new_id();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO users (id, name, color, created_at, updated_at)
         VALUES (?1, ?2, COALESCE(?3, '#4f46e5'), ?4, ?4)",
        params![id, name, color, now],
    )
    .map_err(name_taken)?;
    conn.execute(
        "INSERT INTO user_settings (id, user_id, date_format, accent) VALUES (?1, ?2, 'long', ?3)",
        params![new_id(), id, DEFAULT_ACCENT],
    )?;
    get_user(conn, &id)
}

pub fn get_user(conn: &Connection, id: &str) -> Result<User> {
    conn.query_row("SELECT * FROM users WHERE id = ?1", [id], User::from_row)
        .optional()?
        .ok_or(Error::NotFound)
}

pub fn list_users(conn: &Connection) -> Result<Vec<User>> {
    let mut stmt = conn.prepare("SELECT * FROM users ORDER BY name")?;
    let rows = stmt.query_map([], User::from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn rename_user(conn: &Connection, id: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("name can't be blank".into()));
    }
    match conn
        .execute(
            "UPDATE users SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, name, Utc::now()],
        )
        .map_err(name_taken)?
    {
        0 => Err(Error::NotFound),
        _ => Ok(()),
    }
}

/// Profile names are unique; say so in words instead of a SQLite error.
fn name_taken(e: rusqlite::Error) -> Error {
    match e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Error::Invalid("that name is already taken".into())
        }
        e => e.into(),
    }
}

/// Blank means "no email". Otherwise it must look like an address and use
/// only characters that are safe inside an HTTP header.
pub fn validate_email(email: &str) -> Result<Option<String>> {
    let email = email.trim();
    if email.is_empty() {
        return Ok(None);
    }
    let invalid = || Error::Invalid("that doesn't look like an email address".into());
    let (local, domain) = email.split_once('@').ok_or_else(invalid)?;
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b"._%+-'".contains(&b);
    let ok = email.len() <= 254
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && local.bytes().all(allowed)
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if ok {
        Ok(Some(email.to_string()))
    } else {
        Err(invalid())
    }
}

pub fn set_user_email(conn: &Connection, id: &str, email: &str) -> Result<()> {
    let email = validate_email(email)?;
    match conn.execute(
        "UPDATE users SET email = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, email, Utc::now()],
    )? {
        0 => Err(Error::NotFound),
        _ => Ok(()),
    }
}

/// Removes the profile, its settings and its summaries. Books stay.
pub fn delete_user(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM users WHERE id = ?1", [id])?;
    Ok(())
}

// ------------------------------------------------------------- app settings

/// A setting for the whole app (shared by every profile).
pub fn app_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .optional()?)
}

/// `None` removes the setting, so the default applies again.
pub fn set_app_setting(conn: &Connection, key: &str, value: Option<&str>) -> Result<()> {
    match value {
        Some(v) => conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, v],
        )?,
        None => conn.execute("DELETE FROM app_settings WHERE key = ?1", [key])?,
    };
    Ok(())
}

// ------------------------------------------------------------- settings

#[derive(Debug, Clone)]
pub struct UserSettings {
    pub user_id: String,
    // regional
    pub date_format: String, // long | mm_dd_yyyy | dd_mm_yyyy | yyyy_mm_dd
    pub language: String,
    pub time_format: String,
    pub timezone_name: Option<String>,
    pub week_start: String, // sunday | monday
    // look
    pub theme: String,        // system | light | dark
    pub accent: String,       // #rrggbb
    pub heading_font: String, // serif | sans
    // writing
    pub writing_font: String, // ia_duo | serif | sans | mono
    pub writing_size: i64,    // points
    pub line_spacing: String, // tight | normal | airy
    pub page_width: String,   // narrow | medium | wide
    pub focus_default: bool,
    // home
    pub start_tab: String, // reading | finished | eventually
}

impl UserSettings {
    pub fn defaults() -> Self {
        Self {
            user_id: String::new(),
            date_format: "long".into(),
            language: "en".into(),
            time_format: "12h".into(),
            timezone_name: None,
            week_start: "sunday".into(),
            theme: "system".into(),
            accent: DEFAULT_ACCENT.into(),
            heading_font: "serif".into(),
            writing_font: "ia_duo".into(),
            writing_size: 14,
            line_spacing: "normal".into(),
            page_width: "medium".into(),
            focus_default: false,
            start_tab: "reading".into(),
        }
    }
}

/// Renders a date the way the profile likes it.
pub fn format_date(setting: &str, date: NaiveDate) -> String {
    let pattern = match setting {
        "long" => "%b %-d, %Y",
        "dd_mm_yyyy" => "%d/%m/%Y",
        "yyyy_mm_dd" => "%Y-%m-%d",
        _ => "%m/%d/%Y", // mm_dd_yyyy, the Rails default
    };
    date.format(pattern).to_string()
}

pub fn get_settings(conn: &Connection, user_id: &str) -> Result<UserSettings> {
    conn.query_row(
        "SELECT user_id, date_format, language, time_format, timezone_name, week_start,
                theme, accent, heading_font, writing_font, writing_size, line_spacing,
                page_width, focus_default, start_tab
         FROM user_settings WHERE user_id = ?1",
        [user_id],
        |r| {
            Ok(UserSettings {
                user_id: r.get(0)?,
                date_format: r.get(1)?,
                language: r.get(2)?,
                time_format: r.get(3)?,
                timezone_name: r.get(4)?,
                week_start: r.get(5)?,
                theme: r.get(6)?,
                accent: r.get(7)?,
                heading_font: r.get(8)?,
                writing_font: r.get(9)?,
                writing_size: r.get(10)?,
                line_spacing: r.get(11)?,
                page_width: r.get(12)?,
                focus_default: r.get(13)?,
                start_tab: r.get(14)?,
            })
        },
    )
    .optional()?
    .ok_or(Error::NotFound)
}

pub fn update_settings(conn: &Connection, s: &UserSettings) -> Result<()> {
    conn.execute(
        "UPDATE user_settings SET
            date_format = ?2, language = ?3, time_format = ?4, timezone_name = ?5,
            week_start = ?6, theme = ?7, accent = ?8, heading_font = ?9,
            writing_font = ?10, writing_size = ?11, line_spacing = ?12,
            page_width = ?13, focus_default = ?14, start_tab = ?15
         WHERE user_id = ?1",
        params![
            s.user_id,
            s.date_format,
            s.language,
            s.time_format,
            s.timezone_name,
            s.week_start,
            s.theme,
            s.accent,
            s.heading_font,
            s.writing_font,
            s.writing_size,
            s.line_spacing,
            s.page_width,
            s.focus_default,
            s.start_tab
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- books

/// A book as returned by Open Library search (nothing saved yet).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewBook {
    pub external_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub published_date: Option<String>,
    pub page_count: Option<i64>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub published_date: Option<String>,
    pub page_count: Option<i64>,
    pub cover_url: Option<String>,
    pub cover_path: Option<String>,
    pub provider: i32,
    pub external_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Book {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            title: r.get("title")?,
            subtitle: r.get("subtitle")?,
            author: r.get("author")?,
            isbn: r.get("isbn")?,
            publisher: r.get("publisher")?,
            description: r.get("description")?,
            published_date: r.get("published_date")?,
            page_count: r.get("page_count")?,
            cover_url: r.get("cover_url")?,
            cover_path: r.get("cover_path")?,
            provider: r.get("provider")?,
            external_id: r.get("external_id")?,
            created_at: r.get("created_at")?,
            updated_at: r.get("updated_at")?,
        })
    }
}

pub fn get_book(conn: &Connection, id: &str) -> Result<Book> {
    conn.query_row("SELECT * FROM books WHERE id = ?1", [id], Book::from_row)
        .optional()?
        .ok_or(Error::NotFound)
}

/// Returns the existing row for (provider, external_id), or inserts one.
pub fn find_or_create_book(conn: &Connection, nb: &NewBook) -> Result<Book> {
    let existing = conn
        .query_row(
            "SELECT * FROM books WHERE provider = ?1 AND external_id = ?2",
            params![PROVIDER_OPEN_LIBRARY, nb.external_id],
            Book::from_row,
        )
        .optional()?;
    if let Some(book) = existing {
        return Ok(book);
    }

    let id = new_id();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO books (id, title, subtitle, author, isbn, publisher, description,
                            published_date, page_count, cover_url, provider, external_id,
                            created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13)",
        params![
            id,
            nb.title,
            nb.subtitle,
            nb.author,
            nb.isbn,
            nb.publisher,
            nb.description,
            nb.published_date,
            nb.page_count,
            nb.cover_url,
            PROVIDER_OPEN_LIBRARY,
            nb.external_id,
            now
        ],
    )?;
    get_book(conn, &id)
}

/// Fields from a refresh. `None` means "leave what we have", so a partial
/// response from Open Library never blanks out existing data.
#[derive(Debug, Clone, Default)]
pub struct BookUpdate {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub published_date: Option<String>,
    pub page_count: Option<i64>,
    pub publisher: Option<String>,
}

pub fn update_book_details(conn: &Connection, id: &str, u: &BookUpdate) -> Result<()> {
    conn.execute(
        "UPDATE books SET
            title = COALESCE(?2, title),
            subtitle = COALESCE(?3, subtitle),
            description = COALESCE(?4, description),
            published_date = COALESCE(?5, published_date),
            page_count = COALESCE(?6, page_count),
            publisher = COALESCE(?7, publisher),
            updated_at = ?8
         WHERE id = ?1",
        params![
            id,
            u.title,
            u.subtitle,
            u.description,
            u.published_date,
            u.page_count,
            u.publisher,
            Utc::now()
        ],
    )?;
    Ok(())
}

/// Books with a cover to download: a cover_url but no local file yet.
pub fn books_missing_covers(conn: &Connection) -> Result<Vec<Book>> {
    let mut stmt =
        conn.prepare("SELECT * FROM books WHERE cover_path IS NULL AND cover_url IS NOT NULL")?;
    let rows = stmt.query_map([], Book::from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn set_cover_path(conn: &Connection, id: &str, cover_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE books SET cover_path = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, cover_path, Utc::now()],
    )?;
    Ok(())
}

// ------------------------------------------------------------ summaries

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Milestone {
    Finished,
    Reading,
    Eventually,
}

impl Milestone {
    fn filter_and_order(self) -> (&'static str, &'static str) {
        match self {
            // created_at breaks ties, so same-day entries keep their order.
            Milestone::Finished => (
                "finished_on IS NOT NULL",
                "finished_on DESC, created_at DESC",
            ),
            Milestone::Reading => (
                "started_on IS NOT NULL AND finished_on IS NULL",
                "started_on DESC, created_at DESC",
            ),
            Milestone::Eventually => (
                "started_on IS NULL AND finished_on IS NULL",
                "created_at DESC, id",
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub id: String,
    pub user_id: String,
    pub book_id: String,
    pub body: String,
    pub started_on: Option<NaiveDate>,
    pub finished_on: Option<NaiveDate>,
    pub days_to_complete: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Summary {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            user_id: r.get("user_id")?,
            book_id: r.get("book_id")?,
            body: r.get("body")?,
            started_on: r.get("started_on")?,
            finished_on: r.get("finished_on")?,
            days_to_complete: r.get("days_to_complete")?,
            created_at: r.get("created_at")?,
            updated_at: r.get("updated_at")?,
        })
    }

    pub fn milestone(&self) -> Milestone {
        match (self.started_on, self.finished_on) {
            (_, Some(_)) => Milestone::Finished,
            (Some(_), None) => Milestone::Reading,
            (None, None) => Milestone::Eventually,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SummaryInput {
    pub body: String,
    pub started_on: Option<NaiveDate>,
    pub finished_on: Option<NaiveDate>,
}

fn days_between(input: &SummaryInput) -> Result<Option<i64>> {
    match (input.started_on, input.finished_on) {
        (Some(s), Some(f)) if f < s => Err(Error::Invalid(
            "finished date is before started date".into(),
        )),
        (Some(s), Some(f)) => Ok(Some((f - s).num_days())),
        _ => Ok(None),
    }
}

pub fn create_summary(
    conn: &Connection,
    user_id: &str,
    book_id: &str,
    input: &SummaryInput,
) -> Result<Summary> {
    let days = days_between(input)?;
    let id = new_id();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO summaries (id, user_id, book_id, body, started_on, finished_on,
                                days_to_complete, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        params![
            id,
            user_id,
            book_id,
            input.body,
            input.started_on,
            input.finished_on,
            days,
            now
        ],
    )?;
    get_summary(conn, &id)
}

pub fn update_summary(conn: &Connection, id: &str, input: &SummaryInput) -> Result<Summary> {
    let days = days_between(input)?;
    let changed = conn.execute(
        "UPDATE summaries SET body = ?2, started_on = ?3, finished_on = ?4,
                days_to_complete = ?5, updated_at = ?6
         WHERE id = ?1",
        params![
            id,
            input.body,
            input.started_on,
            input.finished_on,
            days,
            Utc::now()
        ],
    )?;
    if changed == 0 {
        return Err(Error::NotFound);
    }
    get_summary(conn, id)
}

pub fn delete_summary(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM summaries WHERE id = ?1", [id])?;
    Ok(())
}

/// Puts back a summary exactly as it was (same id, dates and text). This is
/// the "Undo" after removing a book from the shelf.
pub fn restore_summary(conn: &Connection, s: &Summary) -> Result<()> {
    conn.execute(
        "INSERT INTO summaries (id, user_id, book_id, body, started_on, finished_on,
                                days_to_complete, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            s.id,
            s.user_id,
            s.book_id,
            s.body,
            s.started_on,
            s.finished_on,
            s.days_to_complete,
            s.created_at,
            s.updated_at
        ],
    )?;
    Ok(())
}

pub fn get_summary(conn: &Connection, id: &str) -> Result<Summary> {
    conn.query_row(
        "SELECT * FROM summaries WHERE id = ?1",
        [id],
        Summary::from_row,
    )
    .optional()?
    .ok_or(Error::NotFound)
}

#[derive(Debug, Clone)]
pub struct SummaryWithBook {
    pub summary: Summary,
    pub book: Book,
}

/// The three home-screen lists (finished / reading / eventually) for one user.
pub fn list_summaries(
    conn: &Connection,
    user_id: &str,
    milestone: Milestone,
) -> Result<Vec<SummaryWithBook>> {
    let (filter, order) = milestone.filter_and_order();
    let sql = format!("SELECT * FROM summaries WHERE user_id = ?1 AND {filter} ORDER BY {order}");
    let mut stmt = conn.prepare(&sql)?;
    let summaries: Vec<Summary> = stmt
        .query_map([user_id], Summary::from_row)?
        .collect::<rusqlite::Result<_>>()?;

    // All their books in one query, rather than one query per summary.
    let mut stmt = conn.prepare(&format!(
        "SELECT * FROM books WHERE id IN
           (SELECT book_id FROM summaries WHERE user_id = ?1 AND {filter})"
    ))?;
    let books: HashMap<String, Book> = stmt
        .query_map([user_id], Book::from_row)?
        .map(|b| b.map(|b| (b.id.clone(), b)))
        .collect::<rusqlite::Result<_>>()?;

    summaries
        .into_iter()
        .map(|summary| {
            let book = books
                .get(&summary.book_id)
                .cloned()
                .ok_or(Error::NotFound)?;
            Ok(SummaryWithBook { summary, book })
        })
        .collect()
}

/// Everyone's summaries for a book (the book "show" page).
pub fn summaries_for_book(conn: &Connection, book_id: &str) -> Result<Vec<Summary>> {
    let mut stmt =
        conn.prepare("SELECT * FROM summaries WHERE book_id = ?1 ORDER BY created_at DESC")?;
    let rows = stmt.query_map([book_id], Summary::from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The user's most recent summary for a book, if they have one.
pub fn find_user_summary_for_book(
    conn: &Connection,
    user_id: &str,
    book_id: &str,
) -> Result<Option<Summary>> {
    Ok(conn
        .query_row(
            "SELECT * FROM summaries WHERE user_id = ?1 AND book_id = ?2
             ORDER BY created_at DESC LIMIT 1",
            params![user_id, book_id],
            Summary::from_row,
        )
        .optional()?)
}

/// Autosave path for the writing page: only the text changes.
pub fn update_summary_body(conn: &Connection, id: &str, body: &str) -> Result<()> {
    let changed = conn.execute(
        "UPDATE summaries SET body = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, body, Utc::now()],
    )?;
    if changed == 0 {
        return Err(Error::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn book(key: &str) -> NewBook {
        NewBook {
            external_id: key.into(),
            title: "Dune".into(),
            ..Default::default()
        }
    }

    #[test]
    fn milestones_and_days() {
        let conn = db::open_in_memory().unwrap();
        let user = create_user(&conn, "Avery", None).unwrap();
        let b = find_or_create_book(&conn, &book("/works/OL1W")).unwrap();
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();

        let s = create_summary(&conn, &user.id, &b.id, &SummaryInput::default()).unwrap();
        assert_eq!(s.milestone(), Milestone::Eventually);

        let s = update_summary(
            &conn,
            &s.id,
            &SummaryInput {
                body: "Sand.".into(),
                started_on: Some(d(9, 1)),
                finished_on: None,
            },
        )
        .unwrap();
        assert_eq!(s.milestone(), Milestone::Reading);
        assert_eq!(
            list_summaries(&conn, &user.id, Milestone::Reading)
                .unwrap()
                .len(),
            1
        );

        let s = update_summary(
            &conn,
            &s.id,
            &SummaryInput {
                body: "Sand.".into(),
                started_on: Some(d(9, 1)),
                finished_on: Some(d(9, 11)),
            },
        )
        .unwrap();
        assert_eq!(s.milestone(), Milestone::Finished);
        assert_eq!(s.days_to_complete, Some(10));

        assert!(update_summary(
            &conn,
            &s.id,
            &SummaryInput {
                body: String::new(),
                started_on: Some(d(9, 11)),
                finished_on: Some(d(9, 1)),
            }
        )
        .is_err());
    }

    #[test]
    fn settings_round_trip() {
        let conn = db::open_in_memory().unwrap();
        let user = create_user(&conn, "Avery", None).unwrap();

        let mut s = get_settings(&conn, &user.id).unwrap();
        assert_eq!(s.date_format, "long");
        assert_eq!(s.theme, "system");
        assert_eq!(s.writing_font, "ia_duo");

        s.theme = "dark".into();
        s.accent = "#2f6f9f".into();
        s.writing_size = 18;
        s.focus_default = true;
        update_settings(&conn, &s).unwrap();

        let back = get_settings(&conn, &user.id).unwrap();
        assert_eq!(back.theme, "dark");
        assert_eq!(back.accent, "#2f6f9f");
        assert_eq!(back.writing_size, 18);
        assert!(back.focus_default);
    }

    #[test]
    fn dates_follow_the_setting() {
        let d = NaiveDate::from_ymd_opt(2026, 9, 5).unwrap();
        assert_eq!(format_date("long", d), "Sep 5, 2026");
        assert_eq!(format_date("mm_dd_yyyy", d), "09/05/2026");
        assert_eq!(format_date("dd_mm_yyyy", d), "05/09/2026");
        assert_eq!(format_date("yyyy_mm_dd", d), "2026-09-05");
    }

    #[test]
    fn rename_and_delete_profile() {
        let conn = db::open_in_memory().unwrap();
        let a = create_user(&conn, "Avery", None).unwrap();
        create_user(&conn, "Sam", None).unwrap();
        assert!(rename_user(&conn, &a.id, "Sam").is_err()); // taken
        let taken = create_user(&conn, "Sam", None).unwrap_err().to_string();
        assert!(taken.contains("already taken"), "{taken}");
        rename_user(&conn, &a.id, "Ave").unwrap();
        assert_eq!(get_user(&conn, &a.id).unwrap().name, "Ave");
        delete_user(&conn, &a.id).unwrap();
        assert!(get_user(&conn, &a.id).is_err());
        assert!(get_settings(&conn, &a.id).is_err()); // cascaded
    }

    #[test]
    fn lists_pair_each_summary_with_its_book() {
        let conn = db::open_in_memory().unwrap();
        let user = create_user(&conn, "Avery", None).unwrap();
        let other = create_user(&conn, "Sam", None).unwrap();
        let dune = find_or_create_book(&conn, &book("/works/OL1W")).unwrap();
        let mut nb = book("/works/OL2W");
        nb.title = "Emma".into();
        let emma = find_or_create_book(&conn, &nb).unwrap();
        create_summary(&conn, &user.id, &dune.id, &SummaryInput::default()).unwrap();
        create_summary(&conn, &user.id, &emma.id, &SummaryInput::default()).unwrap();
        create_summary(&conn, &other.id, &emma.id, &SummaryInput::default()).unwrap();

        let list = list_summaries(&conn, &user.id, Milestone::Eventually).unwrap();
        assert_eq!(list.len(), 2);
        for item in &list {
            assert_eq!(item.summary.book_id, item.book.id);
            assert_eq!(item.summary.user_id, user.id);
        }
        let mut titles: Vec<_> = list.iter().map(|i| i.book.title.as_str()).collect();
        titles.sort();
        assert_eq!(titles, ["Dune", "Emma"]);
    }

    #[test]
    fn emails_are_validated() {
        assert_eq!(validate_email("  ").unwrap(), None);
        assert_eq!(
            validate_email(" me@example.com ").unwrap().as_deref(),
            Some("me@example.com")
        );
        assert_eq!(
            validate_email("o'neil+books@mail.example.co")
                .unwrap()
                .as_deref(),
            Some("o'neil+books@mail.example.co")
        );
        for bad in [
            "me",
            "@example.com",
            "me@example",
            "me@.com",
            "a b@example.com",
            "me@example.com\r\nX: y",
            "(me)@example.com",
            "me@exa mple.com",
        ] {
            assert!(validate_email(bad).is_err(), "{bad:?} should be rejected");
        }

        let conn = db::open_in_memory().unwrap();
        let user = create_user(&conn, "Avery", None).unwrap();
        assert_eq!(user.email, None);
        set_user_email(&conn, &user.id, "me@example.com").unwrap();
        assert_eq!(
            get_user(&conn, &user.id).unwrap().email.as_deref(),
            Some("me@example.com")
        );
        assert!(set_user_email(&conn, &user.id, "nope").is_err());
        set_user_email(&conn, &user.id, "").unwrap();
        assert_eq!(get_user(&conn, &user.id).unwrap().email, None);
    }

    #[test]
    fn removed_summary_can_be_restored() {
        let conn = db::open_in_memory().unwrap();
        let user = create_user(&conn, "Avery", None).unwrap();
        let b = find_or_create_book(&conn, &book("/works/OL1W")).unwrap();
        let s = create_summary(
            &conn,
            &user.id,
            &b.id,
            &SummaryInput {
                body: "Sand.".into(),
                ..Default::default()
            },
        )
        .unwrap();
        delete_summary(&conn, &s.id).unwrap();
        assert!(get_summary(&conn, &s.id).is_err());
        restore_summary(&conn, &s).unwrap();
        let back = get_summary(&conn, &s.id).unwrap();
        assert_eq!((back.body, back.created_at), (s.body, s.created_at));
    }

    #[test]
    fn same_book_is_deduplicated() {
        let conn = db::open_in_memory().unwrap();
        let a = find_or_create_book(&conn, &book("/works/OL1W")).unwrap();
        let b = find_or_create_book(&conn, &book("/works/OL1W")).unwrap();
        assert_eq!(a.id, b.id);
    }
}
