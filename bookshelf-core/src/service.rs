//! The "jobs" from the Rails app, as plain functions. The UI runs these on a
//! worker thread and sends the result back to the main thread.
//!
//! Each job is built from network steps, which never see the database, and
//! database steps, which never wait on the network. The macOS and Windows
//! apps share one connection behind a lock and take it only for the
//! database steps, so a slow Open Library never holds up the journal:
//!
//! - save a search result: [`fetch_book_details`], then
//!   `models::find_or_create_book`, then the cover (below).
//! - refresh: `models::get_book`, `OpenLibrary::refresh_work`, then
//!   `models::update_book_details`, then the cover.
//! - a cover: [`missing_cover_url`], [`download_cover`], then
//!   `models::set_cover_path`.
//!
//! GTK and the CLI give each worker thread its own connection, so they use
//! the combined functions here, which run the same steps in order.

use rusqlite::Connection;

use crate::models::{self, Book, NewBook};
use crate::openlibrary::OpenLibrary;
use crate::paths::AppPaths;
use crate::{covers, Result};

/// Network step: fills in the long description of a picked search result.
/// A failure is logged and the book is saved without one.
pub fn fetch_book_details(ol: &OpenLibrary, mut picked: NewBook) -> NewBook {
    if picked.description.is_none() {
        picked.description = ol.description_for(&picked.external_id).unwrap_or_else(|e| {
            log::warn!("description fetch failed: {e}");
            None
        });
    }
    picked
}

/// The cover a saved book still needs: its URL, if it has one but no file.
pub fn missing_cover_url(book: &Book) -> Option<&str> {
    match book.cover_path {
        Some(_) => None,
        None => book.cover_url.as_deref(),
    }
}

/// Network step: downloads a cover into covers_dir. Returns the filename for
/// `models::set_cover_path`, or None if there's no image. A failed download
/// is logged, never an error: a book is worth keeping without its cover.
pub fn download_cover(
    ol: &OpenLibrary,
    paths: &AppPaths,
    book_id: &str,
    url: &str,
) -> Option<String> {
    covers::download(ol, paths, book_id, url).unwrap_or_else(|e| {
        log::warn!("cover download failed: {e}");
        None
    })
}

/// Rails `books#from_book_search`: user picked a search result. Saves the
/// book (deduplicated), fetches its description, downloads its cover.
/// Network hiccups on description/cover never fail the save.
pub fn save_book_from_search(
    conn: &Connection,
    ol: &OpenLibrary,
    paths: &AppPaths,
    picked: NewBook,
) -> Result<Book> {
    let picked = fetch_book_details(ol, picked);
    let book = models::find_or_create_book(conn, &picked)?;
    attach_cover_if_missing(conn, ol, paths, &book)?;
    models::get_book(conn, &book.id)
}

/// Rails refresh job: re-pull details (and a missing cover) for one book.
pub fn refresh_book(
    conn: &Connection,
    ol: &OpenLibrary,
    paths: &AppPaths,
    book_id: &str,
) -> Result<Book> {
    let book = models::get_book(conn, book_id)?;
    if let Some(update) = ol.refresh_work(&book.external_id)? {
        models::update_book_details(conn, book_id, &update)?;
    }
    attach_cover_if_missing(conn, ol, paths, &book)?;
    models::get_book(conn, book_id)
}

fn attach_cover_if_missing(
    conn: &Connection,
    ol: &OpenLibrary,
    paths: &AppPaths,
    book: &Book,
) -> Result<()> {
    if let Some(url) = missing_cover_url(book) {
        if let Some(file) = download_cover(ol, paths, &book.id, url) {
            models::set_cover_path(conn, &book.id, &file)?;
        }
    }
    Ok(())
}

/// The URL to try in a bulk download. Open Library gives a 404 for a
/// missing cover with `?default=false`, instead of a 1x1 placeholder.
pub fn bulk_cover_url(url: &str) -> String {
    if url.contains("covers.openlibrary.org") && !url.contains('?') {
        format!("{url}?default=false")
    } else {
        url.to_owned()
    }
}

/// Downloads covers for every book that has a cover_url but no local file.
/// Safe to re-run: books that already have a cover are skipped. Used after
/// the data import. `progress` is called once per book with (book, got_cover).
/// Returns (downloaded, missing). The steps: `models::books_missing_covers`,
/// then per book [`bulk_cover_url`], [`download_cover`] and
/// `models::set_cover_path`.
pub fn fetch_missing_covers(
    conn: &Connection,
    ol: &OpenLibrary,
    paths: &AppPaths,
    mut progress: impl FnMut(&Book, bool),
) -> Result<(usize, usize)> {
    let (mut downloaded, mut missing) = (0, 0);
    for book in models::books_missing_covers(conn)? {
        let url = bulk_cover_url(book.cover_url.as_deref().unwrap_or_default());
        let got = match download_cover(ol, paths, &book.id, &url) {
            Some(file) => {
                models::set_cover_path(conn, &book.id, &file)?;
                true
            }
            None => false,
        };
        if got {
            downloaded += 1;
        } else {
            missing += 1;
        }
        progress(&book, got);
        std::thread::sleep(std::time::Duration::from_millis(150)); // be polite
    }
    Ok((downloaded, missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    // None of these reach the network: each input is one Open Library
    // would refuse before sending a request (or never needs to ask about).

    fn new_book(key: &str, cover_url: Option<&str>) -> NewBook {
        NewBook {
            external_id: key.into(),
            title: "Dune".into(),
            description: Some("Sand.".into()),
            cover_url: cover_url.map(str::to_owned),
            ..NewBook::default()
        }
    }

    fn setup() -> (tempfile::TempDir, AppPaths, Connection, OpenLibrary) {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path()).unwrap();
        let conn = db::open(&paths.db_path).unwrap();
        (dir, paths, conn, OpenLibrary::new().unwrap())
    }

    #[test]
    fn a_known_description_is_kept() {
        let ol = OpenLibrary::new().unwrap();
        let picked = fetch_book_details(&ol, new_book("/works/OL1W", None));
        assert_eq!(picked.description.as_deref(), Some("Sand."));
    }

    #[test]
    fn saving_twice_gives_the_same_book() {
        let (_dir, paths, conn, ol) = setup();
        let a = save_book_from_search(&conn, &ol, &paths, new_book("/works/OL1W", None)).unwrap();
        let b = save_book_from_search(&conn, &ol, &paths, new_book("/works/OL1W", None)).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.description.as_deref(), Some("Sand."));
    }

    #[test]
    fn a_failed_cover_never_fails_the_job() {
        let (_dir, paths, conn, ol) = setup();
        // Not https, so the download is refused before any request.
        let cover = Some("http://example.invalid/c.jpg");
        let book = save_book_from_search(&conn, &ol, &paths, new_book("x", cover)).unwrap();
        assert_eq!(book.cover_path, None);
        assert_eq!(missing_cover_url(&book), cover);

        // Not a work key, so there's nothing to ask Open Library either.
        let again = refresh_book(&conn, &ol, &paths, &book.id).unwrap();
        assert_eq!(again.title, "Dune");

        let mut seen = vec![];
        let counts = fetch_missing_covers(&conn, &ol, &paths, |b, got| {
            seen.push((b.id.clone(), got));
        })
        .unwrap();
        assert_eq!(counts, (0, 1));
        assert_eq!(seen, [(book.id.clone(), false)]);
        assert_eq!(download_cover(&ol, &paths, "../x", "https://a.b/c"), None);
    }

    #[test]
    fn books_with_a_cover_are_left_alone() {
        let (_dir, _paths, conn, _ol) = setup();
        let book =
            models::find_or_create_book(&conn, &new_book("x", Some("https://c/1.jpg"))).unwrap();
        assert_eq!(models::books_missing_covers(&conn).unwrap().len(), 1);
        models::set_cover_path(&conn, &book.id, "x.jpg").unwrap();
        let book = models::get_book(&conn, &book.id).unwrap();
        assert_eq!(missing_cover_url(&book), None);
        assert!(models::books_missing_covers(&conn).unwrap().is_empty());
    }

    #[test]
    fn bulk_downloads_ask_for_a_404_over_a_placeholder() {
        let ol = "https://covers.openlibrary.org/b/id/1-M.jpg";
        assert_eq!(bulk_cover_url(ol), format!("{ol}?default=false"));
        let asked = format!("{ol}?default=false");
        assert_eq!(bulk_cover_url(&asked), asked);
        assert_eq!(bulk_cover_url("https://x/c.jpg"), "https://x/c.jpg");
    }
}
