//! The journal as the apps use it, end to end, without the network.
//!
//! Books are added from a `SearchResult` that already has a description and
//! no `cover_url`: `service::fetch_book_details` only asks Open Library
//! when the description is missing, and a cover is only downloaded when
//! there's a URL, so nothing is fetched.

use std::path::Path;
use std::sync::Arc;

use chrono::{Datelike, Local};

use crate::*;

struct Fixture {
    _dir: tempfile::TempDir,
    journal: Arc<Journal>,
    user: Profile,
}

fn open(dir: &Path) -> Arc<Journal> {
    Journal::open_at(dir.to_string_lossy().into_owned()).unwrap()
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let journal = open(dir.path());
    let user = journal.create_profile("Avery".into(), None).unwrap();
    Fixture {
        _dir: dir,
        journal,
        user,
    }
}

/// A search result that needs nothing from the network.
fn offline_book(key: &str, title: &str) -> SearchResult {
    SearchResult {
        external_id: key.into(),
        title: title.into(),
        subtitle: None,
        author: Some("Frank Herbert".into()),
        isbn: None,
        publisher: None,
        description: Some("Sand, and the people of it.".into()),
        published_date: Some("1965".into()),
        page_count: Some(412),
        cover_url: None,
    }
}

impl Fixture {
    fn add(&self, key: &str, title: &str, shelf: Shelf) -> String {
        let book = self
            .journal
            .save_search_result(self.user.id.clone(), offline_book(key, title))
            .unwrap();
        self.journal
            .add_to_shelf(self.user.id.clone(), book.id, shelf)
            .unwrap()
    }

    fn shelf(&self, shelf: Shelf) -> ShelfView {
        self.journal
            .shelf(self.user.id.clone(), shelf, today(), 0)
            .unwrap()
    }
}

fn today() -> String {
    Local::now().date_naive().to_string()
}

fn entries(view: &ShelfView) -> Vec<&ShelfEntry> {
    view.rows
        .iter()
        .filter_map(|r| match r {
            ShelfRow::Entry { item } => Some(item),
            ShelfRow::YearHeading { .. } => None,
        })
        .collect()
}

fn is_invalid(e: &CoreError) -> bool {
    matches!(e, CoreError::Invalid { .. })
}

// ---------------------------------------------------------------- opening

#[test]
fn opening_needs_an_absolute_folder() {
    let e = Journal::open_at("relative/journal".into()).err().unwrap();
    assert!(is_invalid(&e), "{e:?}");
    assert!(Journal::open_at(String::new()).is_err());

    let dir = tempfile::tempdir().unwrap();
    let inside = dir.path().join("new").join("journal");
    let journal = open(&inside);
    assert_eq!(journal.data_dir(), inside.to_string_lossy());
    assert!(inside.join("bookshelf.sqlite3").is_file());
}

#[test]
fn a_newer_journal_is_refused_with_its_numbers() {
    let dir = tempfile::tempdir().unwrap();
    drop(open(dir.path()));
    let raw =
        bookshelf_core::rusqlite::Connection::open(dir.path().join("bookshelf.sqlite3")).unwrap();
    raw.pragma_update(None, "user_version", 99).unwrap();
    drop(raw);
    match Journal::open_at(dir.path().to_string_lossy().into_owned()) {
        Err(CoreError::NewerJournal {
            found,
            supported,
            message,
        }) => {
            assert_eq!(found, 99);
            assert!(supported < 99);
            assert!(message.contains("Please update Bookshelf"), "{message}");
        }
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("opened a journal from the future"),
    }
}

#[test]
fn close_checkpoints_and_can_be_called_twice() {
    let f = fixture();
    let wal = Path::new(&f.journal.data_dir()).join("bookshelf.sqlite3-wal");
    assert!(std::fs::metadata(&wal).unwrap().len() > 0);
    f.journal.close().unwrap();
    assert_eq!(std::fs::metadata(&wal).unwrap().len(), 0);
    f.journal.close().unwrap();
    // Still usable: a late autosave lands.
    f.journal
        .rename_profile(f.user.id.clone(), "Ave".into())
        .unwrap();
    f.journal.close().unwrap();
}

#[test]
fn the_change_token_moves_with_every_write() {
    let f = fixture();
    let a = f.journal.change_token().unwrap();
    assert_eq!(
        f.journal.change_token().unwrap(),
        a,
        "reading changes nothing"
    );
    f.journal.profiles().unwrap();
    assert_eq!(f.journal.change_token().unwrap(), a);

    let id = f.add("/works/OL1W", "Dune", Shelf::Reading);
    let b = f.journal.change_token().unwrap();
    assert_ne!(a, b);

    // A write on another connection (as a backup or another process makes).
    let other =
        bookshelf_core::db::open(&Path::new(&f.journal.data_dir()).join("bookshelf.sqlite3"))
            .unwrap();
    bookshelf_core::models::update_summary_body(&other, &id, "Elsewhere.").unwrap();
    let c = f.journal.change_token().unwrap();
    assert_ne!(b, c);
    assert_eq!(f.journal.change_token().unwrap(), c);
}

#[test]
fn the_default_folder_comes_from_the_platform() {
    // Only works out the folder: opening it would touch the real one.
    let stable = bookshelf_core::paths::platform_data_dir(Channel::Stable.into()).unwrap();
    let preview = bookshelf_core::paths::platform_data_dir(Channel::Preview.into()).unwrap();
    assert_ne!(stable, preview);
}

// --------------------------------------------------------------- profiles

#[test]
fn profiles_with_and_without_email() {
    let f = fixture();
    let sam = f
        .journal
        .create_profile("Sam".into(), Some(" sam@example.com ".into()))
        .unwrap();
    assert_eq!(sam.email.as_deref(), Some("sam@example.com"));
    let blank = f
        .journal
        .create_profile("Blank".into(), Some("  ".into()))
        .unwrap();
    assert_eq!(blank.email, None);

    // A bad email makes nothing.
    let e = f
        .journal
        .create_profile("Kim".into(), Some("nope".into()))
        .unwrap_err();
    assert_eq!(e.to_string(), "That doesn't look like an email address.");
    let names: Vec<String> = f
        .journal
        .profiles()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Avery", "Blank", "Sam"]);

    let e = f.journal.create_profile("Sam".into(), None).unwrap_err();
    assert_eq!(e.to_string(), "That name is already taken.");
    assert!(is_invalid(
        &f.journal.create_profile(" ".into(), None).unwrap_err()
    ));
    assert!(is_invalid(
        &f.journal.create_profile("x".repeat(101), None).unwrap_err()
    ));
}

#[test]
fn renaming_emailing_and_deleting_profiles() {
    let f = fixture();
    let id = f.user.id.clone();
    let p = f
        .journal
        .rename_profile(id.clone(), " Ave ".into())
        .unwrap();
    assert_eq!(p.name, "Ave");
    let p = f
        .journal
        .set_profile_email(id.clone(), Some("ave@example.com".into()))
        .unwrap();
    assert_eq!(p.email.as_deref(), Some("ave@example.com"));
    assert!(is_invalid(
        &f.journal
            .set_profile_email(id.clone(), Some("ave@".into()))
            .unwrap_err()
    ));
    let p = f.journal.set_profile_email(id.clone(), None).unwrap();
    assert_eq!(p.email, None);

    f.journal.create_profile("Sam".into(), None).unwrap();
    assert!(is_invalid(
        &f.journal
            .rename_profile(id.clone(), "Sam".into())
            .unwrap_err()
    ));
    let missing = "00000000-0000-0000-0000-000000000000".to_string();
    assert!(matches!(
        f.journal.rename_profile(missing.clone(), "X".into()),
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        f.journal.set_profile_email(missing, None),
        Err(CoreError::NotFound { .. })
    ));

    f.add("/works/OL1W", "Dune", Shelf::Reading);
    f.journal.delete_profile(id.clone()).unwrap();
    assert!(f.journal.profiles().unwrap().iter().all(|p| p.id != id));
    assert!(matches!(
        f.journal.settings(id.clone()),
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        f.journal.shelf(id, Shelf::Reading, today(), 0),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn ids_are_checked_at_the_door() {
    let f = fixture();
    let bad = "x'; DROP TABLE users; --".to_string();
    assert!(is_invalid(&f.journal.entry(bad.clone()).unwrap_err()));
    assert!(is_invalid(
        &f.journal.delete_profile(bad.clone()).unwrap_err()
    ));
    assert!(is_invalid(&f.journal.settings(bad.clone()).unwrap_err()));
    assert!(is_invalid(
        &f.journal.save_body(bad.clone(), "x".into()).unwrap_err()
    ));
    assert!(is_invalid(
        &f.journal
            .existing_entry(f.user.id.clone(), bad.clone())
            .unwrap_err()
    ));
    assert!(is_invalid(
        &f.journal
            .search_books(bad.clone(), "dune".into())
            .unwrap_err()
    ));
    assert!(is_invalid(
        &f.journal.export_markdown(bad, "/".into()).unwrap_err()
    ));
    assert_eq!(f.journal.profiles().unwrap().len(), 1);
}

// --------------------------------------------------------------- settings

#[test]
fn settings_round_trip_through_the_enums() {
    let f = fixture();
    let id = f.user.id.clone();
    let s = f.journal.settings(id.clone()).unwrap();
    assert_eq!(
        s,
        ProfileSettings {
            user_id: id.clone(),
            theme: Theme::System,
            accent: "#2d71e5".into(),
            heading_font: HeadingFont::Serif,
            writing_font: WritingFont::IaDuo,
            writing_size: 14,
            line_spacing: LineSpacing::Normal,
            page_width: PageWidth::Medium,
            focus_default: false,
            date_format: DateFormat::Long,
            week_start: WeekStart::Sunday,
            start_shelf: Shelf::Reading,
        }
    );

    let changed = ProfileSettings {
        theme: Theme::Dark,
        accent: "#2A7F7A".into(),
        heading_font: HeadingFont::Sans,
        writing_font: WritingFont::Mono,
        writing_size: 18,
        line_spacing: LineSpacing::Airy,
        page_width: PageWidth::Wide,
        focus_default: true,
        date_format: DateFormat::YearMonthDay,
        week_start: WeekStart::Monday,
        start_shelf: Shelf::Eventually,
        ..s
    };
    f.journal.update_settings(changed.clone()).unwrap();
    let back = f.journal.settings(id.clone()).unwrap();
    assert_eq!(
        back,
        ProfileSettings {
            accent: "#2a7f7a".into(),
            ..changed.clone()
        }
    );

    // Stored as the strings the GTK app reads, and the fields the apps
    // don't see are left alone.
    let conn =
        bookshelf_core::db::open(&Path::new(&f.journal.data_dir()).join("bookshelf.sqlite3"))
            .unwrap();
    let mut stored = bookshelf_core::models::get_settings(&conn, &id).unwrap();
    assert_eq!(
        (
            stored.theme.as_str(),
            stored.heading_font.as_str(),
            stored.writing_font.as_str(),
            stored.line_spacing.as_str(),
            stored.page_width.as_str(),
            stored.date_format.as_str(),
            stored.week_start.as_str(),
            stored.start_tab.as_str(),
        ),
        (
            "dark",
            "sans",
            "mono",
            "airy",
            "wide",
            "yyyy_mm_dd",
            "monday",
            "eventually"
        )
    );
    assert_eq!(
        (stored.language.as_str(), stored.time_format.as_str()),
        ("en", "12h")
    );

    // Values from a newer version (or a hand edit) read as the defaults.
    stored.theme = "sepia".into();
    stored.writing_font = "comic".into();
    stored.date_format = "julian".into();
    stored.start_tab = "wishlist".into();
    stored.accent = "blue".into();
    stored.writing_size = 99;
    bookshelf_core::models::update_settings(&conn, &stored).unwrap();
    let read = f.journal.settings(id.clone()).unwrap();
    assert_eq!(read.theme, Theme::System);
    assert_eq!(read.writing_font, WritingFont::IaDuo);
    assert_eq!(read.date_format, DateFormat::Long);
    assert_eq!(read.start_shelf, Shelf::Reading);
    assert_eq!(read.accent, "#2d71e5");
    assert_eq!(read.writing_size, 28);
    assert_eq!(read.line_spacing, LineSpacing::Airy, "known values stay");

    // Refused: what Settings can't produce.
    for bad in [
        ProfileSettings {
            writing_size: 9,
            ..changed.clone()
        },
        ProfileSettings {
            writing_size: 29,
            ..changed.clone()
        },
        ProfileSettings {
            accent: "red".into(),
            ..changed.clone()
        },
        ProfileSettings {
            accent: "#ééé".into(),
            ..changed.clone()
        },
    ] {
        assert!(is_invalid(&f.journal.update_settings(bad).unwrap_err()));
    }
    let missing = ProfileSettings {
        user_id: "00000000-0000-0000-0000-000000000000".into(),
        ..changed
    };
    assert!(matches!(
        f.journal.update_settings(missing),
        Err(CoreError::NotFound { .. })
    ));
}

// ---------------------------------------------------------------- shelves

#[test]
fn adding_books_to_each_shelf() {
    let f = fixture();
    let reading = f.add("/works/OL1W", "Dune", Shelf::Reading);
    let finished = f.add("/works/OL2W", "Emma", Shelf::Finished);
    let someday = f.add("/works/OL3W", "Middlemarch", Shelf::Eventually);

    let r = f.journal.entry(reading.clone()).unwrap();
    assert_eq!(r.shelf, Shelf::Reading);
    assert_eq!(r.started, Some(today()));
    assert_eq!(r.finished, None);
    assert_eq!(r.user_id, f.user.id);
    assert_eq!(r.book.title, "Dune");
    assert_eq!(
        r.book.description.as_deref(),
        Some("Sand, and the people of it.")
    );
    assert_eq!(r.book.page_count, Some(412));
    assert_eq!(r.book.cover_path, None);
    assert_eq!(r.body, "");
    assert!(r.created_at_ms > 1_700_000_000_000);
    assert!(r.updated_at_ms >= r.created_at_ms);

    let done = f.journal.entry(finished).unwrap();
    assert_eq!(done.shelf, Shelf::Finished);
    assert_eq!((done.started, done.finished), (None, Some(today())));
    let later = f.journal.entry(someday).unwrap();
    assert_eq!(later.shelf, Shelf::Eventually);
    assert_eq!((later.started, later.finished), (None, None));

    // Unknown profile or book: not found, not a database error.
    let missing = "00000000-0000-0000-0000-000000000000".to_string();
    assert!(matches!(
        f.journal
            .add_to_shelf(f.user.id.clone(), missing.clone(), Shelf::Reading),
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        f.journal
            .add_to_shelf(missing.clone(), r.book.id.clone(), Shelf::Reading),
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        f.journal.entry(missing),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn saving_a_book_twice_gives_the_same_book() {
    let f = fixture();
    let a = f
        .journal
        .save_search_result(f.user.id.clone(), offline_book("/works/OL1W", "Dune"))
        .unwrap();
    let b = f
        .journal
        .save_search_result(f.user.id.clone(), offline_book("/works/OL1W", "Dune"))
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(
        f.journal
            .existing_entry(f.user.id.clone(), a.id.clone())
            .unwrap(),
        None
    );
    let id = f
        .journal
        .add_to_shelf(f.user.id.clone(), a.id.clone(), Shelf::Reading)
        .unwrap();
    assert_eq!(
        f.journal
            .existing_entry(f.user.id.clone(), a.id.clone())
            .unwrap(),
        Some(id)
    );
    // Someone else's entry isn't theirs.
    let sam = f.journal.create_profile("Sam".into(), None).unwrap();
    assert_eq!(f.journal.existing_entry(sam.id, a.id).unwrap(), None);
}

#[test]
fn search_results_are_checked() {
    let f = fixture();
    let user = f.user.id.clone();
    let mut blank_title = offline_book("/works/OL1W", " ");
    assert!(is_invalid(
        &f.journal
            .save_search_result(user.clone(), blank_title.clone())
            .unwrap_err()
    ));
    blank_title.title = "x".repeat(2001);
    assert!(f
        .journal
        .save_search_result(user.clone(), blank_title)
        .is_err());
    assert!(is_invalid(
        &f.journal
            .save_search_result(user.clone(), offline_book("", "Dune"))
            .unwrap_err()
    ));
    let missing = "00000000-0000-0000-0000-000000000000".to_string();
    assert!(matches!(
        f.journal
            .save_search_result(missing.clone(), offline_book("/works/OL1W", "Dune")),
        Err(CoreError::NotFound { .. })
    ));
    // Nothing to search for: no request.
    assert!(f
        .journal
        .search_books(user.clone(), "   ".into())
        .unwrap()
        .is_empty());
    assert!(is_invalid(
        &f.journal.search_books(user, "a".repeat(301)).unwrap_err()
    ));
    assert!(matches!(
        f.journal.search_books(missing, "dune".into()),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn a_cover_that_cant_be_fetched_is_logged_not_fatal() {
    let f = fixture();
    let seen = crate::logging::tests::captured();
    let mut book = offline_book("/works/OL9W", "Dune");
    // Not https, so it's refused before any request is made.
    book.cover_url = Some("http://example.invalid/cover.jpg".into());
    let saved = f
        .journal
        .save_search_result(f.user.id.clone(), book)
        .unwrap();
    assert_eq!(saved.cover_path, None);
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .any(|(level, _, m)| *level == LogLevel::Warn && m.contains("cover download failed")));
}

#[test]
fn the_shelf_view() {
    let f = fixture();
    let a = f.add("/works/OL1W", "Dune", Shelf::Finished);
    f.add("/works/OL2W", "Emma", Shelf::Finished);
    f.add("/works/OL3W", "Middlemarch", Shelf::Reading);
    f.journal
        .set_dates(a.clone(), None, Some("2024-03-04".into()))
        .unwrap();
    f.journal
        .save_body(a.clone(), "**Spice** must flow.".into())
        .unwrap();

    let view = f.shelf(Shelf::Finished);
    let year = Local::now().year();
    assert_eq!(view.total, 2);
    assert_eq!(view.this_year, 1);
    assert_eq!(view.count_line, "2 books · 1 this year");
    assert_eq!(
        view.rows[0],
        ShelfRow::YearHeading {
            year,
            count: 1,
            label: "1 book".into()
        }
    );
    assert!(matches!(&view.rows[1], ShelfRow::Entry { item } if item.title == "Emma"));
    assert_eq!(
        view.rows[2],
        ShelfRow::YearHeading {
            year: 2024,
            count: 1,
            label: "1 book".into()
        }
    );
    let ShelfRow::Entry { item: dune } = &view.rows[3] else {
        panic!("{:?}", view.rows[3])
    };
    assert_eq!(dune.summary_id, a);
    assert_eq!(dune.author.as_deref(), Some("Frank Herbert"));
    assert_eq!(dune.meta, "Finished Mar 4, 2024");
    assert_eq!(dune.excerpt, "Spice must flow.");
    assert_eq!(dune.empty_note, None);
    assert_eq!(dune.haystack, "dune frank herbert **spice** must flow.");
    assert_eq!(dune.cover_path, None);
    let ShelfRow::Entry { item: emma } = &view.rows[1] else {
        panic!()
    };
    assert_eq!(emma.empty_note.as_deref(), Some("Nothing written yet."));

    let reading = f.shelf(Shelf::Reading);
    assert_eq!(reading.count_line, "1 book");
    let r = entries(&reading);
    assert_eq!(r.len(), 1);
    assert!(r[0].meta.ends_with("· day 1"), "{}", r[0].meta);
    assert!(f.shelf(Shelf::Eventually).rows.is_empty());

    // The date follows the profile's setting.
    let mut s = f.journal.settings(f.user.id.clone()).unwrap();
    s.date_format = DateFormat::YearMonthDay;
    f.journal.update_settings(s).unwrap();
    let view = f.shelf(Shelf::Finished);
    assert_eq!(entries(&view)[1].meta, "Finished 2024-03-04");

    // Search with the shelf's own haystacks.
    let terms = query_terms("SPICE herbert".into());
    let found: Vec<&str> = entries(&view)
        .into_iter()
        .filter(|e| matches(e.haystack.clone(), terms.clone()))
        .map(|e| e.title.as_str())
        .collect();
    assert_eq!(found, ["Dune"]);
}

#[test]
fn the_shelf_checks_today_and_the_offset() {
    let f = fixture();
    let user = f.user.id.clone();
    assert!(is_invalid(
        &f.journal
            .shelf(user.clone(), Shelf::Reading, "today".into(), 0)
            .unwrap_err()
    ));
    assert!(is_invalid(
        &f.journal
            .shelf(user.clone(), Shelf::Reading, today(), 24 * 60)
            .unwrap_err()
    ));
    // "Day N" counts from the day passed in.
    f.add("/works/OL1W", "Dune", Shelf::Reading);
    let tomorrow = (Local::now().date_naive() + chrono::Days::new(1)).to_string();
    let view = f
        .journal
        .shelf(user.clone(), Shelf::Reading, tomorrow, -300)
        .unwrap();
    assert!(entries(&view)[0].meta.ends_with("· day 2"));
    // "Added" is the local date at the offset passed in.
    f.add("/works/OL2W", "Emma", Shelf::Eventually);
    for offset in [-720, 0, 840] {
        let view = f
            .journal
            .shelf(user.clone(), Shelf::Eventually, today(), offset)
            .unwrap();
        assert!(entries(&view)[0].meta.starts_with("Added "));
    }
}

// ---------------------------------------------------------------- entries

#[test]
fn dates_are_validated_and_move_the_entry() {
    let f = fixture();
    let id = f.add("/works/OL1W", "Dune", Shelf::Eventually);
    let e = f
        .journal
        .set_dates(id.clone(), Some("2026-09-01".into()), None)
        .unwrap();
    assert_eq!(e.shelf, Shelf::Reading);
    assert_eq!(e.days, None);
    let e = f
        .journal
        .set_dates(
            id.clone(),
            Some("2026-09-01".into()),
            Some("2026-09-11".into()),
        )
        .unwrap();
    assert_eq!(e.shelf, Shelf::Finished);
    assert_eq!(e.days, Some(10));
    assert_eq!(e.started.as_deref(), Some("2026-09-01"));
    assert_eq!(e.finished.as_deref(), Some("2026-09-11"));

    let e = f
        .journal
        .set_dates(
            id.clone(),
            Some("2026-09-11".into()),
            Some("2026-09-01".into()),
        )
        .unwrap_err();
    assert_eq!(e.to_string(), "Finished date is before started date.");
    for bad in ["2026-02-30", "09/01/2026", "2026-9-1", ""] {
        assert!(is_invalid(
            &f.journal
                .set_dates(id.clone(), Some(bad.into()), None)
                .unwrap_err()
        ));
        assert!(is_invalid(
            &f.journal
                .set_dates(id.clone(), None, Some(bad.into()))
                .unwrap_err()
        ));
    }
    // Refused changes leave the entry as it was.
    assert_eq!(f.journal.entry(id.clone()).unwrap().days, Some(10));

    // The text is kept when the dates change.
    f.journal.save_body(id.clone(), "Kept.".into()).unwrap();
    let e = f.journal.set_dates(id.clone(), None, None).unwrap();
    assert_eq!(e.shelf, Shelf::Eventually);
    assert_eq!(e.body, "Kept.");

    assert!(matches!(
        f.journal
            .set_dates("00000000-0000-0000-0000-000000000000".into(), None, None),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn saving_normalises_line_ends() {
    let f = fixture();
    let id = f.add("/works/OL1W", "Dune", Shelf::Reading);
    let saved = f
        .journal
        .save_body(id.clone(), "# One\r\ntwo three\rfour\n\r\nfive 📚".into())
        .unwrap();
    assert_eq!(saved.words, 7);
    assert_eq!(
        f.journal.entry(id.clone()).unwrap().body,
        "# One\ntwo three\nfour\n\nfive 📚"
    );
    let saved = f.journal.save_body(id.clone(), String::new()).unwrap();
    assert_eq!(saved, SaveResult { words: 0 });
    assert!(matches!(
        f.journal
            .save_body("00000000-0000-0000-0000-000000000000".into(), "x".into()),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn line_ends() {
    use crate::journal::unix_newlines;
    assert_eq!(unix_newlines("a\r\nb\rc\nd".into()), "a\nb\nc\nd");
    assert_eq!(unix_newlines("\r\r\n\n\r".into()), "\n\n\n\n");
    assert_eq!(unix_newlines("plain\n".into()), "plain\n");
    assert_eq!(unix_newlines("é\r📚".into()), "é\n📚");
}

#[test]
fn rescued_text_lands_in_the_recovery_folder() {
    let f = fixture();
    let path = f
        .journal
        .rescue_body("Dune: Messiah".into(), "Line\r\nLine 2".into())
        .unwrap();
    let path = Path::new(&path);
    assert!(path.starts_with(Path::new(&f.journal.data_dir()).join("recovery")));
    assert!(path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("dune-messiah-"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "Line\nLine 2");
}

#[test]
fn reading_again_starts_a_new_entry_today() {
    let f = fixture();
    let first = f.add("/works/OL1W", "Dune", Shelf::Finished);
    let again = f.journal.read_again(first.clone()).unwrap();
    assert_ne!(again, first);
    let e = f.journal.entry(again.clone()).unwrap();
    assert_eq!(e.shelf, Shelf::Reading);
    assert_eq!(e.started, Some(today()));
    assert_eq!(e.book, f.journal.entry(first).unwrap().book);
    assert_eq!(
        f.journal
            .existing_entry(f.user.id.clone(), e.book.id)
            .unwrap(),
        Some(again)
    );
}

#[test]
fn removed_entries_can_be_restored() {
    let f = fixture();
    let id = f.add("/works/OL1W", "Dune", Shelf::Finished);
    f.journal
        .set_dates(
            id.clone(),
            Some("2026-01-02".into()),
            Some("2026-01-05".into()),
        )
        .unwrap();
    f.journal.save_body(id.clone(), "Sand.".into()).unwrap();
    let before = f.journal.entry(id.clone()).unwrap();

    let removed = f.journal.remove_entry(id.clone()).unwrap();
    assert_eq!(removed.summary_id(), id);
    assert_eq!(removed.title(), "Dune");
    assert!(matches!(
        f.journal.entry(id.clone()),
        Err(CoreError::NotFound { .. })
    ));
    assert_eq!(f.shelf(Shelf::Finished).total, 0);
    assert!(matches!(
        f.journal.remove_entry(id.clone()),
        Err(CoreError::NotFound { .. })
    ));

    f.journal.restore_entry(removed.clone()).unwrap();
    assert_eq!(f.journal.entry(id.clone()).unwrap(), before);
    // A second Undo is harmless.
    f.journal.restore_entry(removed).unwrap();
    assert_eq!(f.shelf(Shelf::Finished).total, 1);
}

// ------------------------------------------------------------------- data

#[test]
fn backups_in_the_default_and_a_chosen_folder() {
    let f = fixture();
    let status = f.journal.backup_status().unwrap();
    let default = Path::new(&f.journal.data_dir()).join("backups");
    assert_eq!(Path::new(&status.folder), default);
    assert!(!status.is_custom);
    assert!(status.available);
    assert_eq!(status.latest, None);
    assert_eq!(status.keep, 7);

    let made = f.journal.back_up_now().unwrap().unwrap();
    assert!(Path::new(&made).starts_with(&default));
    assert_eq!(f.journal.back_up_now().unwrap(), None, "one a day");
    assert_eq!(f.journal.backup_status().unwrap().latest, Some(today()));

    let chosen = tempfile::tempdir().unwrap();
    let chosen_path = chosen.path().to_string_lossy().into_owned();
    f.journal
        .set_backup_folder(Some(chosen_path.clone()))
        .unwrap();
    let status = f.journal.backup_status().unwrap();
    assert_eq!(status.folder, chosen_path);
    assert!(status.is_custom);
    assert_eq!(status.latest, None, "none there yet");
    let made = f.journal.back_up_now().unwrap().unwrap();
    assert!(Path::new(&made).starts_with(chosen.path()));
    assert!(Path::new(&made).is_file());
    assert_eq!(f.journal.backup_status().unwrap().latest, Some(today()));

    // Refused: not absolute, not there, not a folder.
    for bad in [
        "backups".to_string(),
        String::new(),
        chosen.path().join("gone").to_string_lossy().into_owned(),
        made.clone(),
    ] {
        assert!(is_invalid(
            &f.journal.set_backup_folder(Some(bad)).unwrap_err()
        ));
    }
    assert!(f.journal.backup_status().unwrap().is_custom);

    // A chosen folder that goes away (a drive unplugged) pauses backups.
    drop(chosen);
    let status = f.journal.backup_status().unwrap();
    assert!(!status.available);
    assert_eq!(status.latest, None);
    assert!(f.journal.back_up_now().is_err());

    f.journal.set_backup_folder(None).unwrap();
    let status = f.journal.backup_status().unwrap();
    assert!(!status.is_custom);
    assert_eq!(Path::new(&status.folder), default);
}

#[test]
fn export_writes_markdown_files() {
    let f = fixture();
    let a = f.add("/works/OL1W", "Dune", Shelf::Finished);
    f.add("/works/OL2W", "Emma", Shelf::Eventually);
    f.journal.save_body(a, "Sand.".into()).unwrap();
    let other = f.journal.create_profile("Sam".into(), None).unwrap();

    let out = tempfile::tempdir().unwrap();
    let parent = out.path().to_string_lossy().into_owned();
    let result = f
        .journal
        .export_markdown(f.user.id.clone(), parent.clone())
        .unwrap();
    assert_eq!(result.count, 2);
    let folder = out.path().join(EXPORT_FOLDER);
    assert_eq!(Path::new(&result.folder), folder);
    let dune = std::fs::read_to_string(folder.join("dune.md")).unwrap();
    assert!(dune.contains("title: \"Dune\""), "{dune}");
    assert!(dune.ends_with("Sand.\n"), "{dune}");
    assert!(folder.join("emma.md").is_file());

    // Again: nothing is overwritten.
    let again = f
        .journal
        .export_markdown(f.user.id.clone(), parent.clone())
        .unwrap();
    assert_eq!(again.count, 2);
    assert!(folder.join("dune-2.md").is_file());

    assert_eq!(
        f.journal
            .export_markdown(other.id, parent.clone())
            .unwrap()
            .count,
        0
    );
    assert!(matches!(
        f.journal
            .export_markdown("00000000-0000-0000-0000-000000000000".into(), parent),
        Err(CoreError::NotFound { .. })
    ));
    assert!(is_invalid(
        &f.journal
            .export_markdown(f.user.id.clone(), "relative".into())
            .unwrap_err()
    ));
    assert!(is_invalid(
        &f.journal
            .export_markdown(
                f.user.id.clone(),
                out.path().join("missing").to_string_lossy().into_owned()
            )
            .unwrap_err()
    ));
}

#[test]
fn the_page_layout_from_a_profiles_settings() {
    let f = fixture();
    let s = f.journal.settings(f.user.id.clone()).unwrap();
    assert_eq!(writer_layout(s).column_width, 720);
}
