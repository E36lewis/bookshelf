use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderValue, USER_AGENT};
use reqwest::redirect;
use serde_json::Value;

use crate::models::{Book, BookUpdate, NewBook};
use crate::thumbnails::Thumbnails;
use crate::{covers, Error, Result};

const SEARCH_ENDPOINT: &str = "https://openlibrary.org/search.json";
const BASE_URL: &str = "https://openlibrary.org";
const APP_AGENT: &str = concat!("Bookshelf/", env!("CARGO_PKG_VERSION"));

/// What a search asks for. `editions` adds the edition that best matches
/// the query, with the `editions.*` fields: Open Library files editions
/// under works, sometimes several different books under one (#20), and
/// the edition is the book that was searched for. Search has no page
/// count for an edition, so pages are always the work's.
const SEARCH_FIELDS: &str = "key,title,subtitle,author_name,first_publish_year,isbn,cover_i,\
cover_edition_key,publisher,number_of_pages_median,editions,editions.key,editions.title,\
editions.subtitle,editions.cover_i,editions.isbn,editions.publisher";

/// How many of a work's editions to look through for a cover: one page.
const MAX_EDITIONS: &str = "50";

/// Largest response we'll read. Covers are ~20 KB, JSON a few hundred KB.
const MAX_BODY_BYTES: u64 = 5 * 1024 * 1024;
const MAX_REDIRECTS: usize = 10;

/// A search-list thumbnail is a medium cover, about 180 px wide and 20 KB.
const MAX_THUMBNAIL_BYTES: u64 = 512 * 1024;
const MAX_THUMBNAIL_PIXELS: u64 = 2_000_000;

/// Blocking client: call from a worker thread, never the GTK main thread.
/// Cheap to clone (the connection pool is shared), so make one and reuse it.
/// Redirects are followed (covers.openlibrary.org -> archive.org) but only
/// to https URLs. Clones share the search thumbnails kept for the session.
#[derive(Clone)]
pub struct OpenLibrary {
    client: Client,
    user_agent: HeaderValue,
    thumbnails: Arc<Thumbnails>,
}

impl OpenLibrary {
    pub fn new() -> Result<Self> {
        let policy = redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() != "https" {
                attempt.stop()
            } else if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else {
                attempt.follow()
            }
        });
        let client = Client::builder()
            .redirect(policy)
            .https_only(true)
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            client,
            user_agent: HeaderValue::from_static(APP_AGENT),
            thumbnails: Arc::default(),
        })
    }

    /// Same client, but requests carry the profile's contact email in the
    /// User-Agent, as Open Library asks of API users. `contact` should come
    /// from `models::validate_email`; anything unusable is left out.
    pub fn with_contact(&self, contact: Option<&str>) -> Self {
        let user_agent = contact
            .and_then(|c| HeaderValue::from_str(&format!("{APP_AGENT} (contact: {c})")).ok())
            .unwrap_or_else(|| HeaderValue::from_static(APP_AGENT));
        Self {
            client: self.client.clone(),
            user_agent,
            thumbnails: self.thumbnails.clone(),
        }
    }

    fn get(&self, url: &str) -> RequestBuilder {
        self.client
            .get(url)
            .header(USER_AGENT, self.user_agent.clone())
    }

    /// Lightweight list search: no descriptions, no cover downloads.
    pub fn search(&self, query: &str) -> Result<Vec<NewBook>> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }
        let resp = self
            .get(SEARCH_ENDPOINT)
            .query(&[("q", query), ("limit", "12"), ("fields", SEARCH_FIELDS)])
            .send()?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let json = read_json(resp)?;
        let docs = json["docs"].as_array().cloned().unwrap_or_default();
        Ok(docs
            .iter()
            .filter(|d| d["key"].as_str().is_some_and(is_work_key)) // unusable without one
            .map(parse_doc)
            .collect())
    }

    /// Long description for one work. Call once, when a result is picked.
    pub fn description_for(&self, work_key: &str) -> Result<Option<String>> {
        Ok(self.work_json(work_key)?.and_then(|j| description_of(&j)))
    }

    /// A cover for a search result whose matched edition has none: the
    /// first of the work's editions with the same title and subtitle that
    /// has one. One request, for one page of editions. Call when the
    /// result is picked.
    pub fn edition_cover(
        &self,
        work_key: &str,
        title: &str,
        subtitle: Option<&str>,
    ) -> Result<Option<i64>> {
        let Some(j) = self.editions_json(work_key)? else {
            return Ok(None);
        };
        let found = same_book(&j, title, subtitle).find_map(edition_cover_id);
        Ok(found)
    }

    /// Refetch details for a saved book. Only fields Open Library actually
    /// returned are `Some`, so nothing gets blanked out. The title and
    /// subtitle stay as saved: they're the edition's, and the work's may
    /// be another book's (#20). The page count and publisher come from the
    /// edition with the same title, subtitle and ISBN, and a book with no
    /// cover gets one as a picked search result does.
    pub fn refresh_work(&self, book: &Book) -> Result<Option<BookUpdate>> {
        let Some(work) = self.work_json(&book.external_id)? else {
            return Ok(None);
        };
        let editions = self.editions_json(&book.external_id)?;
        Ok(Some(refreshed(book, &work, editions.as_ref())))
    }

    /// A search result's cover, small, for the result list: the image
    /// bytes, or None if Open Library has no cover. Kept for the session,
    /// so searching again asks for nothing. At most a few are fetched at
    /// once; more wait their turn. Call from a worker thread.
    pub fn search_thumbnail(&self, cover_id: i64) -> Result<Option<Vec<u8>>> {
        if cover_id <= 0 {
            return Err(Error::Invalid(format!("not a cover id: {cover_id}")));
        }
        if let Some(kept) = self.thumbnails.get(cover_id) {
            return Ok(kept);
        }
        let _turn = self.thumbnails.take_turn();
        // Another thread may have fetched it while this one waited.
        if let Some(kept) = self.thumbnails.get(cover_id) {
            return Ok(kept);
        }
        let found = match self.fetch_image_capped(&cover_url(cover_id), MAX_THUMBNAIL_BYTES)? {
            Some((bytes, _)) => {
                covers::check_size(&bytes, MAX_THUMBNAIL_PIXELS)?;
                Some(bytes)
            }
            None => None,
        };
        self.thumbnails.keep(cover_id, found.clone());
        Ok(found)
    }

    /// Raw image bytes plus content type, or None on any non-2xx (including
    /// the 404 that `?default=false` gives for missing covers). Only https
    /// URLs, only images, and at most `MAX_BODY_BYTES`.
    pub fn fetch_image(&self, url: &str) -> Result<Option<(Vec<u8>, String)>> {
        self.fetch_image_capped(url, MAX_BODY_BYTES)
    }

    fn fetch_image_capped(&self, url: &str, max: u64) -> Result<Option<(Vec<u8>, String)>> {
        if !url.starts_with("https://") {
            return Err(Error::Invalid(format!("not an https url: {url}")));
        }
        let resp = self.get(url).send()?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        // Only plain raster formats: no SVG (an XML document) or anything exotic.
        let mime = content_type.split(';').next().unwrap_or("").trim();
        if !matches!(
            mime,
            "image/jpeg" | "image/png" | "image/gif" | "image/webp"
        ) {
            return Err(Error::Invalid(format!(
                "not an image ({content_type:?}): {url}"
            )));
        }
        Ok(Some((read_capped(resp, max)?, content_type)))
    }

    fn work_json(&self, key: &str) -> Result<Option<Value>> {
        if !is_work_key(key) {
            return Ok(None);
        }
        self.json(self.get(&format!("{BASE_URL}{key}.json")))
    }

    /// One page (`MAX_EDITIONS`) of a work's editions.
    fn editions_json(&self, work_key: &str) -> Result<Option<Value>> {
        if !work_key.starts_with("/works/") || !is_work_key(work_key) {
            return Ok(None);
        }
        let url = format!("{BASE_URL}{work_key}/editions.json");
        self.json(self.get(&url).query(&[("limit", MAX_EDITIONS)]))
    }

    fn json(&self, request: RequestBuilder) -> Result<Option<Value>> {
        let resp = request.send()?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        Ok(Some(read_json(resp)?))
    }
}

/// Open Library keys look like `/works/OL45804W` or `/books/OL7353617M`.
/// Anything else is refused, so a key can never point the request at
/// another path or host (`@evil.example/...`).
fn is_work_key(key: &str) -> bool {
    let id = key
        .strip_prefix("/works/")
        .or_else(|| key.strip_prefix("/books/"));
    id.is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// An edition's key, like `/books/OL7353617M`, of a sane length.
fn is_edition_key(key: &str) -> bool {
    key.len() <= 64 && key.starts_with("/books/") && is_work_key(key)
}

/// Reads the body, refusing anything over `max` bytes.
fn read_capped(resp: Response, max: u64) -> Result<Vec<u8>> {
    if resp.content_length().is_some_and(|n| n > max) {
        return Err(Error::Invalid("response too large".into()));
    }
    let mut body = Vec::new();
    resp.take(max + 1).read_to_end(&mut body)?;
    if body.len() as u64 > max {
        return Err(Error::Invalid("response too large".into()));
    }
    Ok(body)
}

fn read_json(resp: Response) -> Result<Value> {
    serde_json::from_slice(&read_capped(resp, MAX_BODY_BYTES)?)
        .map_err(|e| Error::Invalid(format!("bad response from Open Library: {e}")))
}

fn str_field(j: &Value, key: &str) -> Option<String> {
    j[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| clip(s, MAX_FIELD_CHARS))
}

/// Titles and such: anything longer is surely junk.
const MAX_FIELD_CHARS: usize = 500;
/// Book descriptions: generous, but a multi-megabyte one would bog down the page.
const MAX_DESCRIPTION_CHARS: usize = 20_000;

fn clip(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &s[..cut]),
        None => s.to_owned(),
    }
}

/// `description` is either a plain string or `{ "type": ..., "value": ... }`.
fn description_of(j: &Value) -> Option<String> {
    match &j["description"] {
        Value::String(s) => Some(clip(s, MAX_DESCRIPTION_CHARS)),
        Value::Object(o) => o
            .get("value")
            .and_then(|v| v.as_str())
            .map(|s| clip(s, MAX_DESCRIPTION_CHARS)),
        _ => None,
    }
    .filter(|s| !s.trim().is_empty())
}

/// The work's fields, overridden by its matched edition's where it has
/// them: the edition is the book that was searched for. Every text field
/// is clipped: the response is untrusted, and these go straight into the
/// database and onto the page.
fn parse_doc(doc: &Value) -> NewBook {
    let edition = doc["editions"]["docs"].get(0).filter(|e| e.is_object());
    let either = |field: fn(&Value, &str) -> Option<String>, key: &str| {
        edition
            .and_then(|e| field(e, key))
            .or_else(|| field(doc, key))
    };
    let cover_id = cover_of(doc, edition);

    NewBook {
        external_id: doc["key"].as_str().unwrap_or_default().to_owned(),
        title: either(str_field, "title").unwrap_or_else(|| "Untitled".into()),
        subtitle: either(str_field, "subtitle"),
        author: join_strs(doc, "author_name"),
        isbn: either(first_str, "isbn"),
        publisher: either(first_str, "publisher"),
        description: None,
        published_date: doc["first_publish_year"].as_i64().map(|y| y.to_string()),
        page_count: doc["number_of_pages_median"].as_i64(),
        cover_url: cover_id.map(cover_url),
        cover_id,
        edition_key: edition
            .and_then(|e| e["key"].as_str())
            .filter(|k| is_edition_key(k))
            .map(str::to_owned),
    }
}

/// `refresh_work`'s update, from the work and a page of its editions.
fn refreshed(book: &Book, work: &Value, editions: Option<&Value>) -> BookUpdate {
    let mut update = BookUpdate {
        description: description_of(work),
        published_date: str_field(work, "first_publish_date"),
        ..BookUpdate::default()
    };
    let Some(editions) = editions else {
        return update;
    };
    let same = || same_book(editions, &book.title, book.subtitle.as_deref());
    if let Some(isbn) = book.isbn.as_deref() {
        if let Some(ed) = same().find(|ed| has_isbn(ed, isbn)) {
            update.page_count = ed["number_of_pages"].as_i64().filter(|&n| n > 0);
            update.publisher = first_str(ed, "publishers");
        }
    }
    if book.cover_url.is_none() {
        update.cover_url = same().find_map(edition_cover_id).map(cover_url);
    }
    update
}

/// The cover to show, by the rule "never the wrong one" (#20). A work's
/// cover is one of its editions', and Open Library sometimes files other
/// books under the same work. So: the matched edition's own cover; else
/// the work's, but only if it's the matched edition's (`cover_edition_key`)
/// or no edition was matched; else none (`OpenLibrary::edition_cover`
/// looks further when the book is saved).
fn cover_of(doc: &Value, edition: Option<&Value>) -> Option<i64> {
    let work_cover = positive(&doc["cover_i"]);
    let Some(edition) = edition else {
        return work_cover;
    };
    let bare = |key: &str| key.strip_prefix("/books/").unwrap_or(key).to_owned();
    let is_cover_edition = match (edition["key"].as_str(), doc["cover_edition_key"].as_str()) {
        (Some(matched), Some(cover)) => bare(matched) == bare(cover),
        _ => false,
    };
    positive(&edition["cover_i"]).or(work_cover.filter(|_| is_cover_edition))
}

/// The image of cover `id`, medium size. `default=false` makes Open
/// Library 404 instead of serving a 1x1 placeholder.
pub fn cover_url(id: i64) -> String {
    format!("https://covers.openlibrary.org/b/id/{id}-M.jpg?default=false")
}

/// A cover id; Open Library uses -1 for a removed cover.
fn positive(v: &Value) -> Option<i64> {
    v.as_i64().filter(|&id| id > 0)
}

/// The editions in an `editions.json` page with this title and subtitle,
/// as `clip`ped, ignoring case and spacing. No subtitle matches only no
/// subtitle.
fn same_book<'a>(
    editions: &'a Value,
    title: &str,
    subtitle: Option<&str>,
) -> impl Iterator<Item = &'a Value> {
    let fold = |s: Option<&str>| -> String {
        clip(s.unwrap_or_default(), MAX_FIELD_CHARS)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let wanted = (fold(Some(title)), fold(subtitle));
    editions["entries"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(move |ed| (fold(ed["title"].as_str()), fold(ed["subtitle"].as_str())) == wanted)
}

/// An `editions.json` edition's first cover.
fn edition_cover_id(edition: &Value) -> Option<i64> {
    edition["covers"].as_array()?.iter().find_map(positive)
}

fn has_isbn(edition: &Value, isbn: &str) -> bool {
    ["isbn_13", "isbn_10"].iter().any(|key| {
        edition[key]
            .as_array()
            .is_some_and(|all| all.iter().any(|v| v.as_str() == Some(isbn)))
    })
}

/// The strings in an array, joined with commas and clipped.
fn join_strs(j: &Value, key: &str) -> Option<String> {
    let parts: Vec<&str> = j[key]
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    (!parts.is_empty()).then(|| clip(&parts.join(", "), MAX_FIELD_CHARS))
}

/// The first of an array of strings, clipped.
fn first_str(j: &Value, key: &str) -> Option<String> {
    j[key]
        .as_array()?
        .first()?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| clip(s, MAX_FIELD_CHARS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_keys_are_fetched() {
        assert!(is_work_key("/works/OL45804W"));
        assert!(is_work_key("/books/OL7353617M"));
        assert!(!is_work_key(""));
        assert!(!is_work_key("/works/"));
        assert!(!is_work_key("@evil.example/x"));
        assert!(!is_work_key("/works/OL1W/../../x"));
        assert!(!is_work_key("/works/OL1W?x=1"));
        assert!(!is_work_key("/authors/OL1A"));
    }

    #[test]
    fn long_text_is_clipped_on_a_character_boundary() {
        assert_eq!(clip("Dune", 10), "Dune");
        assert_eq!(clip("Éowyn's tale", 5), "Éowyn…");
    }

    #[test]
    fn every_field_of_a_search_result_is_clipped() {
        // A doc with a few hundred authors and a book-length "ISBN".
        let authors: Vec<Value> = (0..300).map(|i| format!("Author {i}").into()).collect();
        let doc = serde_json::json!({
            "key": "/works/OL1W",
            "title": "x".repeat(2000),
            "subtitle": "y".repeat(2000),
            "author_name": authors,
            "isbn": ["9".repeat(100_000)],
            "publisher": ["p".repeat(100_000), "second"],
        });
        let book = parse_doc(&doc);
        let len = |s: &Option<String>| s.as_deref().map_or(0, |s| s.chars().count());
        assert_eq!(book.title.chars().count(), MAX_FIELD_CHARS + 1); // plus "…"
        assert_eq!(len(&book.subtitle), MAX_FIELD_CHARS + 1);
        assert_eq!(len(&book.author), MAX_FIELD_CHARS + 1);
        assert!(book
            .author
            .as_deref()
            .unwrap()
            .starts_with("Author 0, Author 1, "));
        assert_eq!(len(&book.isbn), MAX_FIELD_CHARS + 1);
        assert_eq!(len(&book.publisher), MAX_FIELD_CHARS + 1);
        // Short fields are left exactly as they came.
        let doc = serde_json::json!({
            "key": "/works/OL1W",
            "title": "Dune",
            "author_name": ["Frank Herbert"],
            "isbn": ["9780441013593"],
            "publisher": ["Chilton Books"],
        });
        let book = parse_doc(&doc);
        assert_eq!(book.author.as_deref(), Some("Frank Herbert"));
        assert_eq!(book.isbn.as_deref(), Some("9780441013593"));
        assert_eq!(book.publisher.as_deref(), Some("Chilton Books"));
    }

    #[test]
    fn every_field_of_a_matched_edition_is_clipped() {
        let doc = serde_json::json!({
            "key": "/works/OL1W",
            "title": "Dune",
            "editions": { "docs": [{
                "key": format!("/books/OL{}M", "1".repeat(5000)),
                "title": "x".repeat(2000),
                "subtitle": "y".repeat(2000),
                "isbn": ["9".repeat(100_000)],
                "publisher": ["p".repeat(100_000)],
            }]},
        });
        let book = parse_doc(&doc);
        let len = |s: &Option<String>| s.as_deref().map_or(0, |s| s.chars().count());
        assert_eq!(book.title.chars().count(), MAX_FIELD_CHARS + 1);
        assert_eq!(len(&book.subtitle), MAX_FIELD_CHARS + 1);
        assert_eq!(len(&book.isbn), MAX_FIELD_CHARS + 1);
        assert_eq!(len(&book.publisher), MAX_FIELD_CHARS + 1);
        // A key is used whole or not at all.
        assert_eq!(book.edition_key, None);
    }

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    /// Open Library has merged several Halo novels into one work, whose
    /// cover is Bad Blood's. A search for New Blood finds that work, and
    /// its New Blood edition (#20).
    #[test]
    fn a_search_result_is_the_edition_that_matched() {
        let json = fixture("halo-search.json");
        let docs = json["docs"].as_array().unwrap();
        let halo = parse_doc(&docs[0]);
        assert_eq!(
            halo.external_id, "/works/OL21163022W",
            "the work stays the key"
        );
        assert_eq!(halo.edition_key.as_deref(), Some("/books/OL48159580M"));
        assert_eq!(halo.title, "Halo");
        assert_eq!(halo.subtitle.as_deref(), Some("New Blood"));
        assert_eq!(halo.isbn.as_deref(), Some("9781785650635"));
        assert_eq!(halo.publisher.as_deref(), Some("Titan Books Limited"));
        assert_eq!(halo.author.as_deref(), Some("Matt Forbeck"));
        assert_eq!(halo.published_date.as_deref(), Some("2015"));
        assert_eq!(
            halo.page_count,
            Some(352),
            "editions have no pages in search"
        );
        // Not Bad Blood's cover: none, until saving finds New Blood's.
        assert_eq!(halo.cover_id, None);
        assert_eq!(halo.cover_url, None);

        // A matched edition with a cover of its own shows it.
        let machines = parse_doc(&docs[2]);
        assert_eq!(machines.cover_id, Some(14532481));
        assert_eq!(
            machines.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/14532481-M.jpg?default=false")
        );
        assert!(machines.subtitle.unwrap().starts_with("how to get ahead"));
        // And no cover anywhere is no cover.
        assert_eq!(parse_doc(&docs[1]).cover_id, None);
    }

    #[test]
    fn the_works_cover_is_used_only_when_nothing_contradicts_it() {
        let doc = |editions: Value| {
            let mut doc = serde_json::json!({
                "key": "/works/OL1W",
                "title": "Halo",
                "cover_i": 10329162,
                "cover_edition_key": "OL28650849M",
            });
            if !editions.is_null() {
                doc["editions"] = serde_json::json!({ "docs": [editions] });
            }
            parse_doc(&doc).cover_id
        };
        // No matched edition (or an older response without them).
        assert_eq!(doc(Value::Null), Some(10329162));
        // The matched edition is the one the work's cover is from.
        assert_eq!(
            doc(serde_json::json!({"key": "/books/OL28650849M"})),
            Some(10329162)
        );
        // Another edition, without a cover of its own: none.
        assert_eq!(doc(serde_json::json!({"key": "/books/OL48159580M"})), None);
        assert_eq!(doc(serde_json::json!({"title": "Halo"})), None);
        // Another edition with its own cover: that one.
        let own = serde_json::json!({"key": "/books/OL32049972M", "cover_i": 10716003});
        assert_eq!(doc(own), Some(10716003));
        // Open Library's -1 is no cover.
        let removed = serde_json::json!({"key": "/books/OL28650849M", "cover_i": -1});
        assert_eq!(doc(removed), Some(10329162));
        let doc = serde_json::json!({"key": "/works/OL1W", "title": "x", "cover_i": -1});
        assert_eq!(parse_doc(&doc).cover_id, None);
    }

    #[test]
    fn a_missing_cover_comes_from_an_edition_with_the_same_title() {
        let editions = fixture("halo-editions.json");
        let cover = |title: &str, subtitle: Option<&str>| {
            same_book(&editions, title, subtitle).find_map(edition_cover_id)
        };
        assert_eq!(cover("Halo", Some("New Blood")), Some(10716003));
        assert_eq!(cover("HALO ", Some(" new  blood")), Some(10716003));
        assert_eq!(cover("Halo", Some("Bad Blood")), Some(10329162));
        assert_eq!(cover("Halo", Some("Legacy of Onyx")), None, "none has one");
        assert_eq!(cover("Halo", None), None, "every Halo here has a subtitle");
        assert_eq!(cover("Halo", Some("New")), None);
        assert_eq!(same_book(&Value::Null, "Halo", None).count(), 0);
    }

    fn saved(title: &str, subtitle: Option<&str>, isbn: Option<&str>) -> Book {
        Book {
            id: "b".into(),
            title: title.into(),
            subtitle: subtitle.map(str::to_owned),
            author: None,
            isbn: isbn.map(str::to_owned),
            publisher: None,
            description: None,
            published_date: None,
            page_count: None,
            cover_url: None,
            cover_path: None,
            provider: 0,
            external_id: "/works/OL21163022W".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn a_refresh_keeps_to_the_saved_edition() {
        let work = serde_json::json!({
            "title": "Halo",
            "subtitle": "Bad Blood",
            "first_publish_date": "2015",
            "description": {"type": "/type/text", "value": "Spartans."},
        });
        let editions = fixture("halo-editions.json");
        let book = saved("Halo", Some("New Blood"), Some("9781476796703"));
        let update = refreshed(&book, &work, Some(&editions));
        // Never the work's title or subtitle: they may be another book's.
        assert_eq!(update.title, None);
        assert_eq!(update.subtitle, None);
        assert_eq!(update.description.as_deref(), Some("Spartans."));
        assert_eq!(update.published_date.as_deref(), Some("2015"));
        // The edition with this ISBN, and a cover from one with this title.
        assert_eq!(update.page_count, Some(200));
        assert_eq!(
            update.publisher.as_deref(),
            Some("Simon & Schuster, Limited")
        );
        assert_eq!(update.cover_url, Some(cover_url(10716003)));

        // A book saved before this (the work's ISBN, no subtitle) matches
        // no edition, so only the work's own fields change.
        let old = Book {
            cover_url: Some(cover_url(10329162)),
            ..saved("Halo", None, Some("1501132628"))
        };
        let update = refreshed(&old, &work, Some(&editions));
        assert_eq!(
            (update.page_count, update.publisher, update.cover_url),
            (None, None, None)
        );
        assert!(update.description.is_some());
        assert!(refreshed(&book, &work, None).cover_url.is_none());
    }

    #[test]
    fn edition_lookups_need_a_work_key() {
        // Refused before any request.
        let ol = OpenLibrary::new().unwrap();
        for key in ["/books/OL1M", "", "/works/OL1W/../x", "@evil.example/x"] {
            assert_eq!(ol.edition_cover(key, "Dune", None).unwrap(), None, "{key}");
        }
    }

    #[test]
    fn thumbnails_are_kept_for_every_clone() {
        let ol = OpenLibrary::new().unwrap();
        assert!(ol.search_thumbnail(0).is_err());
        assert!(ol.search_thumbnail(-1).is_err());
        // Answers kept earlier are given without a request.
        ol.thumbnails.keep(7, Some(vec![1, 2, 3]));
        ol.thumbnails.keep(8, None);
        let other = ol.with_contact(Some("me@example.com"));
        assert_eq!(other.search_thumbnail(7).unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(other.search_thumbnail(8).unwrap(), None);
    }

    #[test]
    fn contact_goes_in_the_user_agent() {
        let ol = OpenLibrary::new().unwrap();
        assert_eq!(ol.user_agent, APP_AGENT);
        let with = ol.with_contact(Some("me@example.com"));
        assert_eq!(
            with.user_agent,
            format!("{APP_AGENT} (contact: me@example.com)").as_str()
        );
        // a header-breaking value is dropped, not sent
        assert_eq!(ol.with_contact(Some("a@b.c\r\nX: y")).user_agent, APP_AGENT);
    }
}
