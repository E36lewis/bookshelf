#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{}", network_sentence(.0))]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid: {0}")]
    Invalid(String),
    /// The journal's schema is newer than this build knows. Its own variant
    /// so each app can tell it apart from a damaged file and say "update".
    #[error(
        "this journal was made by a newer version of Bookshelf (format {found}, \
         this version understands up to {supported}). Please update Bookshelf to open it"
    )]
    NewerJournal { found: usize, supported: usize },
}

impl Error {
    /// The error as a sentence for people: "That name is already taken."
    /// `Invalid` messages are written for people already, so they're shown
    /// without the "invalid:" prefix.
    pub fn user_message(&self) -> String {
        let text = match self {
            Error::Invalid(msg) => msg.clone(),
            other => other.to_string(),
        };
        let mut chars = text.chars();
        let mut out: String = chars
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        out.push_str(chars.as_str());
        if !out.ends_with(['.', '!', '?']) {
            out.push('.');
        }
        out
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A network failure as a sentence that names the real cause. reqwest's own
/// text ("error sending request for url (…)") hides it, and its URL can
/// contain what the person searched for, so neither is shown.
fn network_sentence(e: &reqwest::Error) -> String {
    // The innermost error says what actually happened: a refused
    // connection, an untrusted certificate, a failed name lookup…
    let mut cause = None;
    let mut source = std::error::Error::source(e);
    while let Some(s) = source {
        cause = Some(s.to_string());
        source = s.source();
    }
    let detail = cause.unwrap_or_else(|| e.without_url_text());
    let lower = detail.to_lowercase();

    if e.is_timeout() {
        "Open Library took too long to answer. Please try again in a moment".into()
    } else if lower.contains("certificate") || lower.contains("issuer") {
        format!(
            "couldn't verify Open Library's secure connection ({detail}). Antivirus \
             software or a network that inspects secure connections can cause this"
        )
    } else if lower.contains("dns") || lower.contains("lookup") || lower.contains("no such host") {
        format!("couldn't find openlibrary.org ({detail}). Please check the internet connection")
    } else if e.is_connect() {
        format!(
            "couldn't connect to Open Library ({detail}). Please check the internet \
             connection, or whether a firewall is blocking Bookshelf"
        )
    } else {
        format!("couldn't reach Open Library ({detail})")
    }
}

/// reqwest's message without the URL part.
trait WithoutUrlText {
    fn without_url_text(&self) -> String;
}

impl WithoutUrlText for reqwest::Error {
    fn without_url_text(&self) -> String {
        let text = self.to_string();
        match text.find(" for url (") {
            Some(cut) => text[..cut].to_string(),
            None => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_shows_its_own_message() {
        let msg = |m: &str| Error::Invalid(m.into()).user_message();
        assert_eq!(
            msg("that name is already taken"),
            "That name is already taken."
        );
        assert_eq!(msg("Done!"), "Done!");
        assert_eq!(msg("is it?"), "Is it?");
        assert_eq!(msg("already a sentence."), "Already a sentence.");
        assert_eq!(msg("über"), "Über.");
        // Some capitals are two letters.
        assert_eq!(msg("ßtraße"), "SStraße.");
        // Nothing to say still ends a sentence.
        assert_eq!(msg(""), ".");
    }

    #[test]
    fn other_errors_show_their_kind() {
        assert_eq!(Error::NotFound.user_message(), "Not found.");
        assert_eq!(
            Error::Io(std::io::Error::other("disk full")).user_message(),
            "Io error: disk full."
        );
        assert_eq!(
            Error::Db(rusqlite::Error::QueryReturnedNoRows).user_message(),
            "Database error: Query returned no rows."
        );
        // Building a request with a bad URL fails before any network use.
        let http = reqwest::Client::new().get("not a url").build().unwrap_err();
        assert_eq!(
            Error::Http(http).user_message(),
            "Couldn't reach Open Library (relative URL without a base)."
        );
    }

    #[test]
    fn a_refused_connection_names_the_cause_and_hides_the_url() {
        // Port 9 on this machine: nothing listens there, so the connection is
        // refused at once, without touching the internet.
        let err = reqwest::blocking::Client::new()
            .get("http://127.0.0.1:9/search.json?q=my+private+search")
            .send()
            .unwrap_err();
        let msg = Error::Http(err).user_message();
        assert!(
            msg.starts_with("Couldn't connect to Open Library ("),
            "{msg}"
        );
        assert!(msg.contains("firewall"), "{msg}");
        assert!(
            !msg.contains("private"),
            "the search text must not show: {msg}"
        );
        assert!(!msg.contains("http"), "no URL: {msg}");
    }

    #[test]
    fn a_newer_journal_reads_as_it_always_did() {
        // The startup screen's sentence, word for word as it was when this
        // was an `Invalid` message.
        let err = Error::NewerJournal {
            found: 6,
            supported: 5,
        };
        let before = Error::Invalid(
            "this journal was made by a newer version of Bookshelf (format 6, \
             this version understands up to 5). Please update Bookshelf to open it"
                .into(),
        );
        assert_eq!(err.user_message(), before.user_message());
        assert_eq!(
            err.user_message(),
            "This journal was made by a newer version of Bookshelf (format 6, \
             this version understands up to 5). Please update Bookshelf to open it."
        );
    }
}
