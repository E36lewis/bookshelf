use std::io::Read;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderValue, USER_AGENT};
use reqwest::redirect;
use serde_json::Value;

use crate::models::{BookUpdate, NewBook};
use crate::{Error, Result};

const SEARCH_ENDPOINT: &str = "https://openlibrary.org/search.json";
const BASE_URL: &str = "https://openlibrary.org";
const APP_AGENT: &str = concat!("Bookshelf/", env!("CARGO_PKG_VERSION"));

/// Largest response we'll read. Covers are ~20 KB, JSON a few hundred KB.
const MAX_BODY_BYTES: u64 = 5 * 1024 * 1024;
const MAX_REDIRECTS: usize = 10;

/// Blocking client: call from a worker thread, never the GTK main thread.
/// Cheap to clone (the connection pool is shared), so make one and reuse it.
/// Redirects are followed (covers.openlibrary.org -> archive.org) but only
/// to https URLs.
#[derive(Clone)]
pub struct OpenLibrary {
    client: Client,
    user_agent: HeaderValue,
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
            .query(&[
                ("q", query),
                ("limit", "12"),
                ("fields", "key,title,subtitle,author_name,first_publish_year,isbn,cover_i,publisher,number_of_pages_median"),
            ])
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

    /// Refetch details for a saved book. Only fields Open Library actually
    /// returned are `Some`, so nothing gets blanked out.
    pub fn refresh_work(&self, work_key: &str) -> Result<Option<BookUpdate>> {
        let Some(j) = self.work_json(work_key)? else {
            return Ok(None);
        };
        Ok(Some(BookUpdate {
            title: str_field(&j, "title"),
            subtitle: str_field(&j, "subtitle"),
            description: description_of(&j),
            published_date: str_field(&j, "first_publish_date"),
            page_count: j["number_of_pages_median"].as_i64(),
            publisher: j["publisher"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .map(str::to_owned),
        }))
    }

    /// Raw image bytes plus content type, or None on any non-2xx (including
    /// the 404 that `?default=false` gives for missing covers). Only https
    /// URLs, only images, and at most `MAX_BODY_BYTES`.
    pub fn fetch_image(&self, url: &str) -> Result<Option<(Vec<u8>, String)>> {
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
        Ok(Some((read_capped(resp)?, content_type)))
    }

    fn work_json(&self, key: &str) -> Result<Option<Value>> {
        if !is_work_key(key) {
            return Ok(None);
        }
        let resp = self.get(&format!("{BASE_URL}{key}.json")).send()?;
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

/// Reads the body, refusing anything over `MAX_BODY_BYTES`.
fn read_capped(resp: Response) -> Result<Vec<u8>> {
    if resp.content_length().is_some_and(|n| n > MAX_BODY_BYTES) {
        return Err(Error::Invalid("response too large".into()));
    }
    let mut body = Vec::new();
    resp.take(MAX_BODY_BYTES + 1).read_to_end(&mut body)?;
    if body.len() as u64 > MAX_BODY_BYTES {
        return Err(Error::Invalid("response too large".into()));
    }
    Ok(body)
}

fn read_json(resp: Response) -> Result<Value> {
    serde_json::from_slice(&read_capped(resp)?)
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

fn parse_doc(doc: &Value) -> NewBook {
    let join_strs = |key: &str| -> Option<String> {
        let parts: Vec<&str> = doc[key]
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    };
    let first_str =
        |key: &str| -> Option<String> { doc[key].as_array()?.first()?.as_str().map(str::to_owned) };

    NewBook {
        external_id: doc["key"].as_str().unwrap_or_default().to_owned(),
        title: clip(doc["title"].as_str().unwrap_or("Untitled"), MAX_FIELD_CHARS),
        subtitle: str_field(doc, "subtitle"),
        author: join_strs("author_name"),
        isbn: first_str("isbn"),
        publisher: first_str("publisher"),
        description: None,
        published_date: doc["first_publish_year"].as_i64().map(|y| y.to_string()),
        page_count: doc["number_of_pages_median"].as_i64(),
        // default=false makes Open Library 404 instead of serving a 1x1 placeholder
        cover_url: doc["cover_i"]
            .as_i64()
            .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-M.jpg?default=false")),
    }
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
