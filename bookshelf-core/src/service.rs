//! The "jobs" from the Rails app, as plain functions. The UI runs these on a
//! worker thread and sends the result back to the main thread.

use rusqlite::Connection;

use crate::models::{self, Book, NewBook};
use crate::openlibrary::OpenLibrary;
use crate::paths::AppPaths;
use crate::{covers, Result};

/// Rails `books#from_book_search`: user picked a search result. Saves the
/// book (deduplicated), fetches its description, downloads its cover.
/// Network hiccups on description/cover never fail the save.
pub fn save_book_from_search(
    conn: &Connection,
    ol: &OpenLibrary,
    paths: &AppPaths,
    mut picked: NewBook,
) -> Result<Book> {
    if picked.description.is_none() {
        picked.description = ol.description_for(&picked.external_id).unwrap_or_else(|e| {
            eprintln!("[bookshelf] description fetch failed: {e}");
            None
        });
    }
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
    if book.cover_path.is_some() {
        return Ok(());
    }
    let Some(url) = &book.cover_url else {
        return Ok(());
    };
    match covers::download(ol, paths, &book.id, url) {
        Ok(Some(file)) => models::set_cover_path(conn, &book.id, &file)?,
        Ok(None) => {}
        Err(e) => eprintln!("[bookshelf] cover download failed: {e}"),
    }
    Ok(())
}

/// Downloads covers for every book that has a cover_url but no local file.
/// Safe to re-run: books that already have a cover are skipped. Used after
/// the data import. `progress` is called once per book with (book, got_cover).
/// Returns (downloaded, missing).
pub fn fetch_missing_covers(
    conn: &Connection,
    ol: &OpenLibrary,
    paths: &AppPaths,
    mut progress: impl FnMut(&Book, bool),
) -> Result<(usize, usize)> {
    let ids: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT id FROM books WHERE cover_path IS NULL AND cover_url IS NOT NULL")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };

    let (mut downloaded, mut missing) = (0, 0);
    for id in ids {
        let book = models::get_book(conn, &id)?;
        let mut url = book.cover_url.clone().unwrap_or_default();
        // Open Library: 404 for a missing cover instead of a 1x1 placeholder.
        if url.contains("covers.openlibrary.org") && !url.contains('?') {
            url.push_str("?default=false");
        }
        let got = match covers::download(ol, paths, &book.id, &url) {
            Ok(Some(file)) => {
                models::set_cover_path(conn, &book.id, &file)?;
                true
            }
            _ => false,
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
