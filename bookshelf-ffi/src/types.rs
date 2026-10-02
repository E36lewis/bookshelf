//! The records and enums the journal hands the apps, and their mapping to
//! and from the core's own types.
//!
//! Conventions across the boundary: text offsets are UTF-16 code units,
//! calendar dates are `YYYY-MM-DD` strings, timestamps are Unix
//! milliseconds (`i64`) and ids are strings.

use std::path::Path;

use bookshelf_core::models::{self, Milestone, UserSettings};
use bookshelf_core::paths::{self, AppPaths};
use bookshelf_core::theme;

use crate::{CoreError, Result};

/// A count as the apps see it. Nothing here comes near `u32::MAX`, but a
/// count never wraps around if it did.
pub(crate) fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// A path as text for the apps. Bookshelf's own paths are always valid
/// Unicode on macOS and Windows.
pub(crate) fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A book's cover file as an absolute path, if it has one. The file may
/// have been deleted since; apps show no cover then.
pub(crate) fn cover_file(paths: &AppPaths, cover_path: Option<&str>) -> Option<String> {
    cover_path.map(|name| path_string(&paths.cover_file(name)))
}

/// Which build is running. A preview keeps its journal in its own folder,
/// so trying one out never touches the real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Channel {
    /// The released app.
    Stable,
    /// A test build, with a journal of its own.
    Preview,
}

impl From<Channel> for paths::Channel {
    fn from(c: Channel) -> Self {
        match c {
            Channel::Stable => paths::Channel::Stable,
            Channel::Preview => paths::Channel::Preview,
        }
    }
}

/// A profile: one person's shelves.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Profile {
    /// The profile's id.
    pub id: String,
    /// The name on the "Who's reading?" page. Unique.
    pub name: String,
    /// Optional contact sent to Open Library with searches, as it asks of
    /// API users. Always a valid address when set.
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

/// Settings are stored as short strings (the schema the GTK app shares);
/// the apps see enums. Each enum maps to and from its stored strings, and a
/// value it doesn't know (hand-edited, or from a newer version) reads as
/// the default.
macro_rules! stored_enum {
    (
        $(#[$meta:meta])*
        $name:ident (default $default:ident) {
            $( $(#[$vmeta:meta])* $variant:ident = $stored:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $name {
            /// The string kept in the database.
            pub(crate) fn as_stored(self) -> &'static str {
                match self {
                    $( Self::$variant => $stored, )+
                }
            }

            /// The stored string read back; anything unknown is the default.
            pub(crate) fn from_stored(s: &str) -> Self {
                match s {
                    $( $stored => Self::$variant, )+
                    _ => Self::$default,
                }
            }
        }
    };
}

stored_enum! {
    /// One of the three shelves on the home page. A book's shelf follows
    /// from its dates: finished, started, or neither.
    Shelf (default Reading) {
        /// Started, not finished.
        Reading = "reading",
        /// Finished.
        Finished = "finished",
        /// Not started yet ("Someday").
        Eventually = "eventually",
    }
}

impl From<Shelf> for Milestone {
    fn from(s: Shelf) -> Self {
        match s {
            Shelf::Reading => Milestone::Reading,
            Shelf::Finished => Milestone::Finished,
            Shelf::Eventually => Milestone::Eventually,
        }
    }
}

impl From<Milestone> for Shelf {
    fn from(m: Milestone) -> Self {
        match m {
            Milestone::Reading => Shelf::Reading,
            Milestone::Finished => Shelf::Finished,
            Milestone::Eventually => Shelf::Eventually,
        }
    }
}

stored_enum! {
    /// Light or dark, or whatever the system uses.
    Theme (default System) {
        /// Follow the system's appearance.
        System = "system",
        /// Always light.
        Light = "light",
        /// Always dark.
        Dark = "dark",
    }
}

stored_enum! {
    /// The typeface of titles and headings.
    HeadingFont (default Serif) {
        /// Source Serif.
        Serif = "serif",
        /// The system's sans-serif.
        Sans = "sans",
    }
}

stored_enum! {
    /// The typeface of the writing page.
    WritingFont (default IaDuo) {
        /// iA Writer Duo (monospace), bundled with the app.
        IaDuo = "ia_duo",
        /// Source Serif.
        Serif = "serif",
        /// The system's sans-serif.
        Sans = "sans",
        /// The system's monospace.
        Mono = "mono",
    }
}

stored_enum! {
    /// Space between lines on the writing page (see [`PageLayout`]).
    LineSpacing (default Normal) {
        /// Close together.
        Tight = "tight",
        /// "Comfortable".
        Normal = "normal",
        /// Generous.
        Airy = "airy",
    }
}

stored_enum! {
    /// How wide the writing and reading column gets (see [`PageLayout`]).
    PageWidth (default Medium) {
        /// 600 px.
        Narrow = "narrow",
        /// 720 px.
        Medium = "medium",
        /// 900 px.
        Wide = "wide",
    }
}

stored_enum! {
    /// How dates are written in the app. The core writes them (in meta
    /// lines, for one); the apps use it for their date pickers.
    DateFormat (default Long) {
        /// "Sep 20, 2026".
        Long = "long",
        /// "09/20/2026".
        MonthDayYear = "mm_dd_yyyy",
        /// "20/09/2026".
        DayMonthYear = "dd_mm_yyyy",
        /// "2026-09-20".
        YearMonthDay = "yyyy_mm_dd",
    }
}

stored_enum! {
    /// The first day of the week in date pickers.
    WeekStart (default Sunday) {
        /// Sunday first.
        Sunday = "sunday",
        /// Monday first.
        Monday = "monday",
    }
}

/// The smallest and largest writing size, in points: the same bounds the
/// GTK app's spinner has.
const WRITING_SIZES: std::ops::RangeInclusive<u32> = 10..=28;

/// One profile's settings. Read them with `Journal::settings`, change any
/// field and pass the whole record to `Journal::update_settings`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ProfileSettings {
    /// Whose settings these are.
    pub user_id: String,
    /// Light, dark or the system's.
    pub theme: Theme,
    /// The accent color, `#rrggbb` (one of `accents()`, or any color).
    pub accent: String,
    /// The typeface of titles and headings.
    pub heading_font: HeadingFont,
    /// The typeface of the writing page.
    pub writing_font: WritingFont,
    /// The writing size in points, 10 to 28.
    pub writing_size: u32,
    /// Space between lines on the writing page.
    pub line_spacing: LineSpacing,
    /// How wide the writing column gets.
    pub page_width: PageWidth,
    /// Whether the writing page opens in focus mode.
    pub focus_default: bool,
    /// How dates are written.
    pub date_format: DateFormat,
    /// The first day of the week in date pickers.
    pub week_start: WeekStart,
    /// The shelf the home page opens on.
    pub start_shelf: Shelf,
}

impl ProfileSettings {
    pub(crate) fn from_stored(s: &UserSettings) -> Self {
        let accent = match theme::parse_hex(&s.accent) {
            Some(rgb) => theme::to_hex(rgb),
            None => models::DEFAULT_ACCENT.to_string(),
        };
        let size = s.writing_size.clamp(
            i64::from(*WRITING_SIZES.start()),
            i64::from(*WRITING_SIZES.end()),
        );
        Self {
            user_id: s.user_id.clone(),
            theme: Theme::from_stored(&s.theme),
            accent,
            heading_font: HeadingFont::from_stored(&s.heading_font),
            writing_font: WritingFont::from_stored(&s.writing_font),
            writing_size: u32::try_from(size).unwrap_or(*WRITING_SIZES.start()),
            line_spacing: LineSpacing::from_stored(&s.line_spacing),
            page_width: PageWidth::from_stored(&s.page_width),
            focus_default: s.focus_default,
            date_format: DateFormat::from_stored(&s.date_format),
            week_start: WeekStart::from_stored(&s.week_start),
            start_shelf: Shelf::from_stored(&s.start_tab),
        }
    }

    /// Refuses what the GTK app's settings page can't produce: a size out
    /// of range, or an accent that isn't a color.
    pub(crate) fn validate(&self) -> Result<()> {
        if !WRITING_SIZES.contains(&self.writing_size) {
            return Err(invalid("the text size must be between 10 and 28 points"));
        }
        if theme::parse_hex(&self.accent).is_none() {
            return Err(invalid("the accent must be a color like #2d71e5"));
        }
        Ok(())
    }

    /// Copies these settings over `stored`, leaving the fields the apps
    /// don't see (language, time format, time zone) as they were.
    pub(crate) fn apply_to(&self, stored: &mut UserSettings) {
        stored.theme = self.theme.as_stored().into();
        stored.accent = self.accent.to_ascii_lowercase();
        stored.heading_font = self.heading_font.as_stored().into();
        stored.writing_font = self.writing_font.as_stored().into();
        stored.writing_size = i64::from(self.writing_size);
        stored.line_spacing = self.line_spacing.as_stored().into();
        stored.page_width = self.page_width.as_stored().into();
        stored.focus_default = self.focus_default;
        stored.date_format = self.date_format.as_stored().into();
        stored.week_start = self.week_start.as_stored().into();
        stored.start_tab = self.start_shelf.as_stored().into();
    }

    /// The core's settings for the parts that only need the look of the
    /// page (`writer_layout`). Fields the apps don't see are the defaults.
    pub(crate) fn to_stored(&self) -> UserSettings {
        let mut s = UserSettings::defaults();
        s.user_id.clone_from(&self.user_id);
        self.apply_to(&mut s);
        s
    }
}

pub(crate) fn invalid(message: &str) -> CoreError {
    bookshelf_core::Error::Invalid(message.into()).into()
}

/// A book found on Open Library, not saved yet. Pass it back to
/// `Journal::save_search_result` to keep it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct SearchResult {
    /// Open Library's key, like `/works/OL45804W`.
    pub external_id: String,
    /// The title.
    pub title: String,
    /// The subtitle, if any.
    pub subtitle: Option<String>,
    /// The first author.
    pub author: Option<String>,
    /// One ISBN.
    pub isbn: Option<String>,
    /// The first publisher.
    pub publisher: Option<String>,
    /// The long description. Search results don't have one; it's fetched
    /// when the book is saved.
    pub description: Option<String>,
    /// The year it was first published, as Open Library writes it.
    pub published_date: Option<String>,
    /// The typical page count.
    pub page_count: Option<i64>,
    /// An https link to the cover image, for showing in the results. The
    /// cover is downloaded when the book is saved.
    pub cover_url: Option<String>,
}

/// Longest title or key accepted back from an app. Open Library's own
/// fields are clipped well below this.
const MAX_FIELD_CHARS: usize = 2000;

impl SearchResult {
    pub(crate) fn validate(&self) -> Result<()> {
        let blank_or_long =
            |s: &str| s.trim().is_empty() || s.chars().nth(MAX_FIELD_CHARS).is_some();
        if blank_or_long(&self.external_id) {
            return Err(invalid("that book has no Open Library key"));
        }
        if blank_or_long(&self.title) {
            return Err(invalid("that book has no title"));
        }
        Ok(())
    }
}

impl From<models::NewBook> for SearchResult {
    fn from(b: models::NewBook) -> Self {
        Self {
            external_id: b.external_id,
            title: b.title,
            subtitle: b.subtitle,
            author: b.author,
            isbn: b.isbn,
            publisher: b.publisher,
            description: b.description,
            published_date: b.published_date,
            page_count: b.page_count,
            cover_url: b.cover_url,
        }
    }
}

impl From<SearchResult> for models::NewBook {
    fn from(b: SearchResult) -> Self {
        Self {
            external_id: b.external_id,
            title: b.title,
            subtitle: b.subtitle,
            author: b.author,
            isbn: b.isbn,
            publisher: b.publisher,
            description: b.description,
            published_date: b.published_date,
            page_count: b.page_count,
            cover_url: b.cover_url,
        }
    }
}

/// A saved book. Books are shared by every profile; entries are not.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct BookInfo {
    /// The book's id.
    pub id: String,
    /// The title.
    pub title: String,
    /// The subtitle, if any.
    pub subtitle: Option<String>,
    /// The author.
    pub author: Option<String>,
    /// One ISBN.
    pub isbn: Option<String>,
    /// The publisher.
    pub publisher: Option<String>,
    /// The long description ("About this book").
    pub description: Option<String>,
    /// The year it was first published, as Open Library writes it.
    pub published_date: Option<String>,
    /// The typical page count.
    pub page_count: Option<i64>,
    /// The cover image's absolute path, if one was downloaded.
    pub cover_path: Option<String>,
}

impl BookInfo {
    pub(crate) fn new(b: models::Book, paths: &AppPaths) -> Self {
        Self {
            cover_path: cover_file(paths, b.cover_path.as_deref()),
            id: b.id,
            title: b.title,
            subtitle: b.subtitle,
            author: b.author,
            isbn: b.isbn,
            publisher: b.publisher,
            description: b.description,
            published_date: b.published_date,
            page_count: b.page_count,
        }
    }
}

/// One entry: a profile's reading of a book, with what they wrote.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct EntryDetail {
    /// The entry's id (a "summary" in the database).
    pub summary_id: String,
    /// Whose entry it is.
    pub user_id: String,
    /// The book.
    pub book: BookInfo,
    /// When reading started, `YYYY-MM-DD`.
    pub started: Option<String>,
    /// When it was finished, `YYYY-MM-DD`.
    pub finished: Option<String>,
    /// Days from start to finish, when both are set.
    pub days: Option<i64>,
    /// What was written, in Markdown, with `\n` line ends.
    pub body: String,
    /// The shelf it's on, from its dates.
    pub shelf: Shelf,
    /// When the entry was made, in Unix milliseconds.
    pub created_at_ms: i64,
    /// When it last changed, in Unix milliseconds.
    pub updated_at_ms: i64,
}

impl EntryDetail {
    pub(crate) fn new(s: models::Summary, book: models::Book, paths: &AppPaths) -> Self {
        Self {
            shelf: s.milestone().into(),
            summary_id: s.id,
            user_id: s.user_id,
            book: BookInfo::new(book, paths),
            started: s.started_on.map(|d| d.to_string()),
            finished: s.finished_on.map(|d| d.to_string()),
            days: s.days_to_complete,
            body: s.body,
            created_at_ms: s.created_at.timestamp_millis(),
            updated_at_ms: s.updated_at.timestamp_millis(),
        }
    }
}

/// One shelf of the home page, ready to draw, in display order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ShelfView {
    /// Year headings (Finished shelf only) and entries, in order.
    pub rows: Vec<ShelfRow>,
    /// Books on the shelf.
    pub total: u32,
    /// Books finished this calendar year (Finished shelf only, else 0).
    pub this_year: u32,
    /// "12 books · 3 this year", under the shelf's heading.
    pub count_line: String,
}

/// A row on a shelf.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum ShelfRow {
    /// "2026 · 12 books" between the Finished shelf's entries.
    YearHeading {
        /// The year.
        year: i32,
        /// Books finished that year.
        count: u32,
        /// "12 books".
        label: String,
    },
    /// One book on the shelf.
    Entry {
        /// What the row shows.
        item: ShelfEntry,
    },
}

/// One book on a shelf.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ShelfEntry {
    /// The entry's id, for `Journal::entry` and the rest.
    pub summary_id: String,
    /// The book's id.
    pub book_id: String,
    /// The book's title.
    pub title: String,
    /// The book's author.
    pub author: Option<String>,
    /// The cover image's absolute path, if there is one.
    pub cover_path: Option<String>,
    /// "Finished Sep 5, 2026 · 10 days"; empty when there's nothing to say.
    pub meta: String,
    /// A plain-text taste of what was written; may be empty.
    pub excerpt: String,
    /// What to show when `excerpt` is empty ("Nothing written yet."), if
    /// anything.
    pub empty_note: Option<String>,
    /// The lowercased text the shelf search looks in: pass it to
    /// `matches` with the `query_terms` of the search.
    pub haystack: String,
}

impl ShelfView {
    pub(crate) fn new(view: bookshelf_core::present::ShelfView<'_>, paths: &AppPaths) -> Self {
        use bookshelf_core::present::ShelfRow as Row;
        let rows = view
            .rows
            .into_iter()
            .map(|row| match row {
                Row::Year(y) => ShelfRow::YearHeading {
                    year: y.year,
                    count: count(y.count),
                    label: y.count_label,
                },
                Row::Entry(e) => {
                    let (book, summary) = (&e.item.book, &e.item.summary);
                    ShelfRow::Entry {
                        item: ShelfEntry {
                            summary_id: summary.id.clone(),
                            book_id: book.id.clone(),
                            title: book.title.clone(),
                            author: book.author.clone(),
                            cover_path: cover_file(paths, book.cover_path.as_deref()),
                            meta: e.meta,
                            excerpt: e.excerpt,
                            empty_note: e.empty_note.map(str::to_owned),
                            haystack: e.haystack,
                        },
                    }
                }
            })
            .collect();
        Self {
            rows,
            total: count(view.total),
            this_year: count(view.this_year),
            count_line: view.count_line,
        }
    }
}

/// What saving the writing page's text gives back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct SaveResult {
    /// The word count of what was saved, for the status line.
    pub words: u32,
}

/// Where backups go, and how they're doing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct BackupStatus {
    /// The folder backups go to (absolute).
    pub folder: String,
    /// Whether that's a folder the user chose, rather than the default.
    pub is_custom: bool,
    /// Whether the folder is there right now. A chosen folder on a drive
    /// that's unplugged isn't, and backups wait until it's back.
    pub available: bool,
    /// The date of this journal's newest backup there, `YYYY-MM-DD`.
    pub latest: Option<String>,
    /// How many daily copies are kept.
    pub keep: u32,
}

/// What an export wrote.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ExportResult {
    /// How many Markdown files were written.
    pub count: u32,
    /// The folder they're in (absolute), to offer to open it.
    pub folder: String,
}
