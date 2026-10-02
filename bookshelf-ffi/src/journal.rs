//! The journal object the apps hold: one database connection behind a lock,
//! the journal's folders, and the Open Library client.
//!
//! Every call is synchronous; the apps make them off their UI thread, one
//! at a time (Swift's `JournalWorker` actor, C#'s `JournalService` queue).
//! The lock is only ever held for database steps. Open Library calls happen
//! before or after it, and the slow file work (backups, export) uses a
//! short-lived connection of its own, so a slow network or an unplugged
//! drive never holds up an autosave.

use std::sync::{Arc, Mutex, MutexGuard};

use bookshelf_core::models::{self, SummaryInput};
use bookshelf_core::openlibrary::OpenLibrary;
use bookshelf_core::paths::{self, AppPaths};
use bookshelf_core::rusqlite::Connection;
use bookshelf_core::{backup, db, export, present, service, writer};
use chrono::NaiveDate;

use crate::types::{
    count, invalid, path_string, BackupStatus, BookInfo, Channel, EntryDetail, ExportResult,
    Profile, ProfileSettings, SaveResult, SearchResult, Shelf, ShelfView,
};
use crate::{validate, Result};

/// The folder an export makes inside the one the user picked, as in the GTK
/// app, so the files don't spill into (say) Documents.
pub const EXPORT_FOLDER: &str = "Bookshelf summaries";

/// Longest profile name. The name is a label, not a place for prose.
const MAX_NAME_CHARS: usize = 100;

/// Longest search. Open Library has nothing to say about anything longer.
const MAX_QUERY_CHARS: usize = 300;

/// One open journal: the database, its folder and the Open Library client.
#[derive(uniffi::Object)]
pub struct Journal {
    conn: Mutex<Connection>,
    paths: AppPaths,
    ol: OpenLibrary,
}

/// An entry that was just removed, kept whole (same id, dates and text) so
/// `Journal::restore_entry` can put it back: the "Undo" on the removal
/// notice. Opaque, so what was written stays inside the core. (No `Debug`,
/// so the text can't end up in a log by accident.)
#[derive(uniffi::Object)]
pub struct RemovedEntry {
    summary: models::Summary,
    title: String,
}

#[uniffi::export]
impl RemovedEntry {
    /// The removed entry's id.
    pub fn summary_id(&self) -> String {
        self.summary.id.clone()
    }

    /// The book's title, for "Removed “Dune”".
    pub fn title(&self) -> String {
        self.title.clone()
    }
}

#[uniffi::export]
impl Journal {
    /// Opens (or creates) this platform's journal: Application Support on
    /// macOS, `%LOCALAPPDATA%` on Windows. A preview build has its own.
    #[uniffi::constructor]
    pub fn open_default(channel: Channel) -> Result<Arc<Self>> {
        Self::open(paths::platform_default_for(channel.into())?)
    }

    /// Opens (or creates) the journal in `dir`, an absolute path. Tests
    /// and previews use this with a folder of their own.
    #[uniffi::constructor]
    pub fn open_at(dir: String) -> Result<Arc<Self>> {
        let dir = validate::absolute(&dir)?;
        Self::open(AppPaths::in_dir(&dir)?)
    }

    /// Call when the app quits: folds the write-ahead log back into the
    /// journal file. Safe to call twice, and the journal stays usable, so
    /// a last autosave after it still lands.
    pub fn close(&self) -> Result<()> {
        db::checkpoint(&self.conn())?;
        Ok(())
    }

    /// The folder the journal lives in.
    pub fn data_dir(&self) -> String {
        path_string(&self.paths.data_dir)
    }

    /// A number that changes whenever the journal does: writes through
    /// this object and writes by anything else (a backup, another
    /// process). A home page can skip rebuilding when it's the same as
    /// last time. Today's date is up to the app (a Reading entry's "day
    /// 12" changes at midnight).
    pub fn change_token(&self) -> Result<u64> {
        let conn = self.conn();
        // Both only ever grow, so their sum changes whenever either does.
        let others: i64 = conn.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        Ok(conn.total_changes().wrapping_add(others as u64))
    }

    // ------------------------------------------------------------ profiles

    /// Every profile, by name.
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        Ok(models::list_users(&self.conn())?
            .into_iter()
            .map(Profile::from)
            .collect())
    }

    /// Makes a profile. `email` is optional (blank means none). A bad email
    /// is refused before anything is made.
    pub fn create_profile(&self, name: String, email: Option<String>) -> Result<Profile> {
        check_name(&name)?;
        let email = match email {
            Some(e) => models::validate_email(&e)?,
            None => None,
        };
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let user = models::create_user(&tx, &name, None)?;
        if let Some(email) = &email {
            models::set_user_email(&tx, &user.id, email)?;
        }
        let user = models::get_user(&tx, &user.id)?;
        tx.commit()?;
        Ok(user.into())
    }

    /// Renames a profile. Names are unique.
    pub fn rename_profile(&self, user_id: String, name: String) -> Result<Profile> {
        validate::id(&user_id)?;
        check_name(&name)?;
        let conn = self.conn();
        models::rename_user(&conn, &user_id, &name)?;
        Ok(models::get_user(&conn, &user_id)?.into())
    }

    /// Sets or clears (`None` or blank) a profile's contact email.
    pub fn set_profile_email(&self, user_id: String, email: Option<String>) -> Result<Profile> {
        validate::id(&user_id)?;
        let conn = self.conn();
        models::set_user_email(&conn, &user_id, email.as_deref().unwrap_or(""))?;
        Ok(models::get_user(&conn, &user_id)?.into())
    }

    /// Deletes a profile with its settings and entries. Books stay, since
    /// other profiles may have them. There's no undo: apps ask first.
    pub fn delete_profile(&self, user_id: String) -> Result<()> {
        validate::id(&user_id)?;
        models::delete_user(&self.conn(), &user_id)?;
        Ok(())
    }

    // ------------------------------------------------------------ settings

    /// A profile's settings.
    pub fn settings(&self, user_id: String) -> Result<ProfileSettings> {
        validate::id(&user_id)?;
        let stored = models::get_settings(&self.conn(), &user_id)?;
        Ok(ProfileSettings::from_stored(&stored))
    }

    /// Saves a profile's settings (the profile is `settings.user_id`).
    pub fn update_settings(&self, settings: ProfileSettings) -> Result<()> {
        validate::id(&settings.user_id)?;
        settings.validate()?;
        let conn = self.conn();
        // Read and write under one lock, so the fields the apps don't see
        // keep their values.
        let mut stored = models::get_settings(&conn, &settings.user_id)?;
        settings.apply_to(&mut stored);
        models::update_settings(&conn, &stored)?;
        Ok(())
    }

    // ------------------------------------------------------------ shelves

    /// One shelf of the home page, laid out. `today` is the local date
    /// (`YYYY-MM-DD`) and `utc_offset_minutes` the local offset from UTC
    /// right now (UTC+2 is 120): the "day 12" of a Reading entry counts
    /// from `today`, and an Eventually entry's "Added" date is the day it
    /// was added at that offset. Passing them in keeps the core off the
    /// system clock, so a shelf drawn just before midnight and its refresh
    /// agree, and tests can pick the day.
    pub fn shelf(
        &self,
        user_id: String,
        shelf: Shelf,
        today: String,
        utc_offset_minutes: i32,
    ) -> Result<ShelfView> {
        validate::id(&user_id)?;
        let today = validate::date(&today)?;
        let tz = validate::utc_offset(utc_offset_minutes)?;
        let (entries, date_format) = {
            let conn = self.conn();
            let date_format = models::get_settings(&conn, &user_id)?.date_format;
            let entries = models::list_summaries(&conn, &user_id, shelf.into())?;
            (entries, date_format)
        };
        // Laid out after the lock is let go.
        let view = present::shelf_view(&entries, shelf.into(), &date_format, today, &tz);
        Ok(ShelfView::new(view, &self.paths))
    }

    /// One entry, with its book: the book page and the writing page.
    pub fn entry(&self, summary_id: String) -> Result<EntryDetail> {
        validate::id(&summary_id)?;
        let (summary, book) = {
            let conn = self.conn();
            let summary = models::get_summary(&conn, &summary_id)?;
            let book = models::get_book(&conn, &summary.book_id)?;
            (summary, book)
        };
        Ok(EntryDetail::new(summary, book, &self.paths))
    }

    /// The profile's latest entry for a book, if it has one: after picking
    /// a search result, "open my entry" or "read it again".
    pub fn existing_entry(&self, user_id: String, book_id: String) -> Result<Option<String>> {
        validate::id(&user_id)?;
        validate::id(&book_id)?;
        let found = models::find_user_summary_for_book(&self.conn(), &user_id, &book_id)?;
        Ok(found.map(|s| s.id))
    }

    /// Puts a saved book on one of the profile's shelves, dated today
    /// (local): Reading starts today, Finished finishes today, Eventually
    /// has no dates. Returns the new entry's id.
    pub fn add_to_shelf(&self, user_id: String, book_id: String, shelf: Shelf) -> Result<String> {
        validate::id(&user_id)?;
        validate::id(&book_id)?;
        let today = local_today();
        let input = match shelf {
            Shelf::Eventually => SummaryInput::default(),
            Shelf::Reading => SummaryInput {
                started_on: Some(today),
                ..SummaryInput::default()
            },
            Shelf::Finished => SummaryInput {
                finished_on: Some(today),
                ..SummaryInput::default()
            },
        };
        let conn = self.conn();
        // Not found, rather than a foreign key error.
        models::get_user(&conn, &user_id)?;
        models::get_book(&conn, &book_id)?;
        Ok(models::create_summary(&conn, &user_id, &book_id, &input)?.id)
    }

    /// Sets (or clears, with `None`) when reading started and finished,
    /// as `YYYY-MM-DD`. Finishing before starting is refused. The entry
    /// moves shelf to match; the result says where it is now.
    pub fn set_dates(
        &self,
        summary_id: String,
        started: Option<String>,
        finished: Option<String>,
    ) -> Result<EntryDetail> {
        validate::id(&summary_id)?;
        let started_on = validate::optional_date(started.as_deref())?;
        let finished_on = validate::optional_date(finished.as_deref())?;
        let (saved, book) = {
            let conn = self.conn();
            let current = models::get_summary(&conn, &summary_id)?;
            let input = SummaryInput {
                body: current.body,
                started_on,
                finished_on,
            };
            let saved = models::update_summary(&conn, &summary_id, &input)?;
            let book = models::get_book(&conn, &saved.book_id)?;
            (saved, book)
        };
        Ok(EntryDetail::new(saved, book, &self.paths))
    }

    /// Saves the writing page's text (autosave, and Save now). Line ends
    /// are stored as `\n`, whatever the text view uses (`\r` in WinUI's
    /// RichEditBox, `\r\n` from a paste).
    pub fn save_body(&self, summary_id: String, body: String) -> Result<SaveResult> {
        validate::id(&summary_id)?;
        let body = unix_newlines(body);
        models::update_summary_body(&self.conn(), &summary_id, &body)?;
        Ok(SaveResult {
            words: count(writer::word_count(&body)),
        })
    }

    /// The last resort when `save_body` fails: writes the text to
    /// `<data dir>/recovery/<title>-<time>.md` so it isn't lost, and
    /// returns that file's path to tell the user.
    pub fn rescue_body(&self, title: String, body: String) -> Result<String> {
        let saved = export::save_recovery_copy(&self.paths, &title, &unix_newlines(body))?;
        Ok(path_string(&saved))
    }

    /// Logs another reading of the entry's book: a new entry on the
    /// Reading shelf, started today (local). Returns its id.
    pub fn read_again(&self, summary_id: String) -> Result<String> {
        validate::id(&summary_id)?;
        let input = SummaryInput {
            started_on: Some(local_today()),
            ..SummaryInput::default()
        };
        let conn = self.conn();
        let current = models::get_summary(&conn, &summary_id)?;
        Ok(models::create_summary(&conn, &current.user_id, &current.book_id, &input)?.id)
    }

    /// Removes an entry right away. Keep the result to offer "Undo" with
    /// `restore_entry`.
    pub fn remove_entry(&self, summary_id: String) -> Result<Arc<RemovedEntry>> {
        validate::id(&summary_id)?;
        let conn = self.conn();
        let summary = models::get_summary(&conn, &summary_id)?;
        let title = models::get_book(&conn, &summary.book_id)?.title;
        models::delete_summary(&conn, &summary_id)?;
        Ok(Arc::new(RemovedEntry { summary, title }))
    }

    /// Puts a removed entry back exactly as it was. Restoring one that's
    /// already back does nothing, so a double-clicked Undo is harmless.
    pub fn restore_entry(&self, removed: Arc<RemovedEntry>) -> Result<()> {
        let conn = self.conn();
        match models::get_summary(&conn, &removed.summary.id) {
            Ok(_) => Ok(()),
            Err(bookshelf_core::Error::NotFound) => {
                models::restore_summary(&conn, &removed.summary)?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    // ------------------------------------------------------- Open Library

    /// Searches Open Library (a network call; nothing is saved). Requests
    /// carry the profile's contact email, if it set one.
    pub fn search_books(&self, user_id: String, query: String) -> Result<Vec<SearchResult>> {
        validate::id(&user_id)?;
        let query = query.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }
        if query.chars().nth(MAX_QUERY_CHARS).is_some() {
            return Err(invalid("that search is too long"));
        }
        let ol = self.client_for(&user_id)?;
        Ok(ol
            .search(query)?
            .into_iter()
            .map(SearchResult::from)
            .collect())
    }

    /// Saves a picked search result as a book (the same book again if it
    /// was saved before), fetching its description and downloading its
    /// cover. Neither of those failing fails the save. Add it to a shelf
    /// with `add_to_shelf`, after `existing_entry`.
    pub fn save_search_result(&self, user_id: String, result: SearchResult) -> Result<BookInfo> {
        validate::id(&user_id)?;
        result.validate()?;
        let ol = self.client_for(&user_id)?;
        // Network, without the lock. A result that already has its
        // description (a fixture, or a re-save) asks for nothing.
        let picked = service::fetch_book_details(&ol, result.into());
        let mut book = models::find_or_create_book(&self.conn(), &picked)?;
        // Network again, without the lock. No cover URL, no request.
        let cover = service::missing_cover_url(&book)
            .and_then(|url| service::download_cover(&ol, &self.paths, &book.id, url));
        if let Some(file) = cover {
            models::set_cover_path(&self.conn(), &book.id, &file)?;
            book.cover_path = Some(file);
        }
        Ok(BookInfo::new(book, &self.paths))
    }

    // ------------------------------------------------------------- data

    /// Where backups go and the newest one's date. Looks at the backup
    /// folder without taking the journal's lock: it may be a slow drive.
    pub fn backup_status(&self) -> Result<BackupStatus> {
        let conn = self.own_connection()?;
        let (folder, is_custom) = backup::folder(&conn, &self.paths)?;
        let available = folder.is_dir();
        let latest = if available {
            backup::latest(&conn, &self.paths)?.map(|d| d.to_string())
        } else {
            None
        };
        Ok(BackupStatus {
            folder: path_string(&folder),
            is_custom,
            available,
            latest,
            keep: count(backup::KEEP),
        })
    }

    /// Keeps backups in `folder` (an absolute path to an existing folder)
    /// from now on, or in the default folder again with `None`. Follow it
    /// with `back_up_now` so the new place has a copy right away.
    pub fn set_backup_folder(&self, folder: Option<String>) -> Result<()> {
        let chosen = folder.as_deref().map(validate::existing_dir).transpose()?;
        let conn = self.own_connection()?;
        backup::set_folder(&conn, chosen.as_deref())?;
        Ok(())
    }

    /// Makes today's backup if there isn't one yet, and drops all but the
    /// newest few. Returns the new file, or `None` if today's was already
    /// made. Uses a connection of its own, never the journal's lock, so
    /// call it at startup without holding anything up.
    pub fn back_up_now(&self) -> Result<Option<String>> {
        let conn = self.own_connection()?;
        let made = backup::daily(&conn, &self.paths, backup::KEEP)?;
        Ok(made.as_deref().map(path_string))
    }

    /// Writes each of the profile's entries as a Markdown file into a
    /// "Bookshelf summaries" folder inside `parent_dir` (an absolute path
    /// to an existing folder). Files already there are never overwritten.
    pub fn export_markdown(&self, user_id: String, parent_dir: String) -> Result<ExportResult> {
        validate::id(&user_id)?;
        let folder = validate::existing_dir(&parent_dir)?.join(EXPORT_FOLDER);
        // Its own connection: writing files to a chosen folder (a USB
        // stick, say) shouldn't hold up the journal.
        let conn = self.own_connection()?;
        models::get_user(&conn, &user_id)?;
        let written = export::export_markdown(&conn, &user_id, &folder)?;
        Ok(ExportResult {
            count: count(written),
            folder: path_string(&folder),
        })
    }
}

impl Journal {
    fn open(paths: AppPaths) -> Result<Arc<Self>> {
        let conn = db::open(&paths.db_path)?;
        let ol = OpenLibrary::new()?;
        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
            paths,
            ol,
        }))
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves the connection itself fine.
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A short-lived connection for work that waits on slow files. WAL
    /// mode lets it run next to the shared one.
    fn own_connection(&self) -> Result<Connection> {
        Ok(db::open(&self.paths.db_path)?)
    }

    /// The Open Library client, sending the profile's contact email. The
    /// lock is held just long enough to read it.
    fn client_for(&self, user_id: &str) -> Result<OpenLibrary> {
        let email = models::get_user(&self.conn(), user_id)?.email;
        Ok(self.ol.with_contact(email.as_deref()))
    }
}

/// Today's date where the user is. chrono asks the system for the time zone
/// on macOS and Windows.
fn local_today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn check_name(name: &str) -> Result<()> {
    if name.trim().chars().nth(MAX_NAME_CHARS).is_some() {
        return Err(invalid("that name is too long"));
    }
    Ok(())
}

/// `\r\n` and lone `\r` as `\n`. Text without a `\r` is passed through
/// untouched, without a copy.
pub(crate) fn unix_newlines(text: String) -> String {
    if !text.contains('\r') {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            chars.next_if_eq(&'\n');
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}
