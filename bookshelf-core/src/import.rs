//! One-shot import of the old Rails/Postgres data from CSV exports
//! (one file per table, exported with psql \copy ... WITH (FORMAT csv, HEADER)).
//!
//! Files read from the export directory:
//!   users.csv, user_settings.csv, books.csv, summaries.csv,
//!   action_text_rich_texts.csv (optional: holds the summary text)
//!
//! Runs in a single transaction: either everything imports or nothing does.
//! Refuses to run if the database already has data.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rusqlite::{params, Connection};

use crate::html_to_markdown::html_to_markdown;
use crate::{Error, Result};

#[derive(Debug, Default)]
pub struct ImportReport {
    pub users: usize,
    pub books: usize,
    pub summaries: usize,
    pub summaries_without_text: usize,
    /// Distinct Action Text `name` values seen on Summary rows.
    pub rich_text_names: Vec<String>,
}

struct Table {
    headers: HashMap<String, usize>,
    rows: Vec<csv::StringRecord>,
}

impl Table {
    fn read(path: &Path) -> Result<Self> {
        let mut rdr = csv::Reader::from_path(path)
            .map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
        let headers = rdr
            .headers()
            .map_err(csv_err)?
            .iter()
            .enumerate()
            .map(|(i, h)| (h.to_string(), i))
            .collect();
        let rows = rdr.records().collect::<std::result::Result<Vec<_>, _>>().map_err(csv_err)?;
        Ok(Self { headers, rows })
    }

    fn empty() -> Self {
        Self { headers: HashMap::new(), rows: vec![] }
    }

    /// Blank cells (Postgres NULLs) come back as None.
    fn get<'a>(&self, row: &'a csv::StringRecord, col: &str) -> Option<&'a str> {
        let i = *self.headers.get(col)?;
        row.get(i).filter(|v| !v.is_empty())
    }

    fn req<'a>(&self, row: &'a csv::StringRecord, col: &str) -> Result<&'a str> {
        self.get(row, col)
            .ok_or_else(|| Error::Invalid(format!("row is missing required column {col:?}")))
    }
}

fn csv_err(e: csv::Error) -> Error {
    Error::Invalid(format!("csv: {e}"))
}

fn ts(s: &str) -> Result<DateTime<Utc>> {
    let s = s.trim_end_matches("+00");
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
        .map(|n| n.and_utc())
        .map_err(|e| Error::Invalid(format!("bad timestamp {s:?}: {e}")))
}

fn date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| Error::Invalid(format!("bad date {s:?}: {e}")))
}

pub fn import_rails_export(conn: &Connection, dir: &Path) -> Result<ImportReport> {
    for table in ["users", "books", "summaries"] {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
        if n > 0 {
            return Err(Error::Invalid(format!(
                "the database already has data in `{table}`; import into an empty data directory"
            )));
        }
    }

    let users = Table::read(&dir.join("users.csv"))?;
    let settings = Table::read(&dir.join("user_settings.csv"))?;
    let books = Table::read(&dir.join("books.csv"))?;
    let summaries = Table::read(&dir.join("summaries.csv"))?;
    let rich_path = dir.join("action_text_rich_texts.csv");
    let rich = if rich_path.exists() { Table::read(&rich_path)? } else { Table::empty() };

    // summary id -> markdown body
    let mut bodies: HashMap<String, String> = HashMap::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for r in &rich.rows {
        if rich.get(r, "record_type") == Some("Summary") {
            names.insert(rich.get(r, "name").unwrap_or("").to_string());
            let id = rich.req(r, "record_id")?.to_string();
            let html = rich.get(r, "body").unwrap_or("");
            bodies.entry(id).or_insert_with(|| html_to_markdown(html));
        }
    }

    let mut report = ImportReport {
        rich_text_names: names.into_iter().collect(),
        ..Default::default()
    };
    let tx = conn.unchecked_transaction()?;

    for r in &users.rows {
        tx.execute(
            "INSERT INTO users (id, name, color, created_at, updated_at) VALUES (?1,?2,?3,?4,?5)",
            params![
                users.req(r, "id")?,
                users.req(r, "name")?,
                users.get(r, "color").unwrap_or("#4f46e5"),
                ts(users.req(r, "created_at")?)?,
                ts(users.req(r, "updated_at")?)?,
            ],
        )?;
        report.users += 1;
    }

    for r in &settings.rows {
        tx.execute(
            "INSERT INTO user_settings (id, user_id, date_format, language, time_format, timezone_name)
             VALUES (?1, ?2, COALESCE(?3,'long'), COALESCE(?4,'en'), COALESCE(?5,'12h'), ?6)",
            params![
                settings.req(r, "id")?,
                settings.req(r, "user_id")?,
                settings.get(r, "date_format"),
                settings.get(r, "language"),
                settings.get(r, "time_format"),
                settings.get(r, "timezone_name"),
            ],
        )?;
    }
    // Any user without a settings row gets defaults.
    tx.execute(
        "INSERT INTO user_settings (id, user_id)
         SELECT lower(hex(randomblob(16))), id FROM users
         WHERE id NOT IN (SELECT user_id FROM user_settings)",
        [],
    )?;

    for r in &books.rows {
        tx.execute(
            "INSERT INTO books (id, title, subtitle, author, isbn, publisher, description,
                                published_date, page_count, cover_url, provider, external_id,
                                created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                books.req(r, "id")?,
                books.req(r, "title")?,
                books.get(r, "subtitle"),
                books.get(r, "author"),
                books.get(r, "isbn"),
                books.get(r, "publisher"),
                books.get(r, "description"),
                books.get(r, "published_date"),
                books.get(r, "page_count").and_then(|s| s.parse::<i64>().ok()),
                books.get(r, "cover_url"),
                books.get(r, "provider").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0),
                books.req(r, "external_id")?,
                ts(books.req(r, "created_at")?)?,
                ts(books.req(r, "updated_at")?)?,
            ],
        )?;
        report.books += 1;
    }

    for r in &summaries.rows {
        let id = summaries.req(r, "id")?;
        let started = summaries.get(r, "started_on").map(date).transpose()?;
        let finished = summaries.get(r, "finished_on").map(date).transpose()?;
        let days = summaries
            .get(r, "days_to_complete")
            .and_then(|s| s.parse::<i64>().ok())
            .or_else(|| match (started, finished) {
                (Some(s), Some(f)) => Some((f - s).num_days()),
                _ => None,
            });
        let body = bodies.get(id).cloned().unwrap_or_default();
        if body.is_empty() {
            report.summaries_without_text += 1;
        }
        tx.execute(
            "INSERT INTO summaries (id, user_id, book_id, body, started_on, finished_on,
                                    days_to_complete, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                id,
                summaries.req(r, "user_id")?,
                summaries.req(r, "book_id")?,
                body,
                started,
                finished,
                days,
                ts(summaries.req(r, "created_at")?)?,
                ts(summaries.req(r, "updated_at")?)?,
            ],
        )?;
        report.summaries += 1;
    }

    tx.commit()?;
    Ok(report)
}
