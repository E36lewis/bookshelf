//! What the home shelves show: each entry's dated line and excerpt, the
//! Finished shelf's year headings, the counts, and the shelf search. Plain
//! strings and numbers, so every app's home page reads the same; drawing
//! them is left to the app.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};

use crate::models::{format_date, Milestone, SummaryWithBook};

/// Shown in place of an excerpt on the Reading and Finished shelves.
/// Eventually books haven't been started, so an empty page is expected there.
pub const NOTHING_WRITTEN: &str = "Nothing written yet.";

/// One shelf, ready to draw, in display order.
#[derive(Debug, Clone)]
pub struct ShelfView<'a> {
    pub rows: Vec<ShelfRow<'a>>,
    /// Books on the shelf.
    pub total: usize,
    /// Books finished this calendar year (only counted on the Finished shelf).
    pub this_year: usize,
    /// "12 books · 3 this year", under the shelf's heading.
    pub count_line: String,
}

#[derive(Debug, Clone)]
pub enum ShelfRow<'a> {
    /// "2026 · 12 books" between the finished entries.
    Year(YearHeading),
    Entry(ShelfEntry<'a>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearHeading {
    pub year: i32,
    pub count: usize,
    /// "12 books"
    pub count_label: String,
}

#[derive(Debug, Clone)]
pub struct ShelfEntry<'a> {
    pub item: &'a SummaryWithBook,
    /// "Finished Sep 5, 2026 · 10 days"; empty when there's nothing to say.
    pub meta: String,
    /// A plain-text taste of what was written; may be empty.
    pub excerpt: String,
    /// What to show when `excerpt` is empty, if anything.
    pub empty_note: Option<&'static str>,
    /// What the shelf search looks in (see [`haystack`]).
    pub haystack: String,
}

/// Lays out one shelf. `entries` come in the order `list_summaries` gives
/// them. `today` and `tz` are parameters so tests don't depend on the
/// clock: apps pass today's local date and `&chrono::Local`.
pub fn shelf_view<'a, Tz: TimeZone>(
    entries: &'a [SummaryWithBook],
    milestone: Milestone,
    date_format: &str,
    today: NaiveDate,
    tz: &Tz,
) -> ShelfView<'a> {
    let by_year = milestone == Milestone::Finished;
    let year_of = |item: &SummaryWithBook| item.summary.finished_on.map(|d| d.year());
    let mut per_year: HashMap<i32, usize> = HashMap::new();
    if by_year {
        for year in entries.iter().filter_map(year_of) {
            *per_year.entry(year).or_default() += 1;
        }
    }

    let mut rows = Vec::with_capacity(entries.len() + per_year.len());
    let mut current = None;
    for item in entries {
        // A heading wherever the year changes. Entries arrive newest first,
        // so each year gets one heading; one without a date gets none.
        if by_year && year_of(item) != current {
            current = year_of(item);
            if let Some(year) = current {
                let count = per_year[&year];
                rows.push(ShelfRow::Year(YearHeading {
                    year,
                    count,
                    count_label: books(count),
                }));
            }
        }
        let excerpt = excerpt(&item.summary.body);
        let empty_note =
            (excerpt.is_empty() && milestone != Milestone::Eventually).then_some(NOTHING_WRITTEN);
        rows.push(ShelfRow::Entry(ShelfEntry {
            item,
            meta: meta_line(item, milestone, date_format, today, tz),
            excerpt,
            empty_note,
            haystack: haystack(item),
        }));
    }

    let total = entries.len();
    let this_year = per_year.get(&today.year()).copied().unwrap_or(0);
    ShelfView {
        rows,
        total,
        this_year,
        count_line: count_line(total, this_year),
    }
}

/// "1 book", "12 books".
pub fn books(n: usize) -> String {
    if n == 1 {
        "1 book".to_string()
    } else {
        format!("{n} books")
    }
}

/// The line under a shelf's heading: "12 books", or "12 books · 3 this year"
/// once something has been finished this year.
pub fn count_line(total: usize, this_year: usize) -> String {
    if this_year > 0 {
        format!("{} · {this_year} this year", books(total))
    } else {
        books(total)
    }
}

/// The dated line on an entry: when it was finished and how long it took,
/// which day of reading this is, or when it was added. `milestone` is the
/// shelf it's shown on. "Added" is the local date the entry was made, as
/// `tz` sees it, so a late-evening entry isn't dated tomorrow.
pub fn meta_line<Tz: TimeZone>(
    item: &SummaryWithBook,
    milestone: Milestone,
    date_format: &str,
    today: NaiveDate,
    tz: &Tz,
) -> String {
    let s = &item.summary;
    let mut parts: Vec<String> = vec![];
    match milestone {
        Milestone::Finished => {
            if let Some(d) = s.finished_on {
                parts.push(format!("Finished {}", format_date(date_format, d)));
            }
            if let Some(n) = s.days_to_complete {
                parts.push(if n == 1 {
                    "1 day".into()
                } else {
                    format!("{n} days")
                });
            }
        }
        Milestone::Reading => {
            if let Some(d) = s.started_on {
                parts.push(format!("Started {}", format_date(date_format, d)));
                // The start day is day 1. A start date still to come gets no day.
                let day = (today - d).num_days() + 1;
                if day >= 1 {
                    parts.push(format!("day {day}"));
                }
            }
        }
        Milestone::Eventually => {
            let added = local_date(s.created_at, tz);
            parts.push(format!("Added {}", format_date(date_format, added)));
        }
    }
    parts.join(" · ")
}

fn local_date<Tz: TimeZone>(at: DateTime<Utc>, tz: &Tz) -> NaiveDate {
    at.with_timezone(tz).date_naive()
}

/// A plain-text taste of the summary for the list: markdown marks removed,
/// whitespace flattened, trimmed to about two sentences' worth.
pub fn excerpt(body: &str) -> String {
    let cleaned: String = body
        .chars()
        .filter(|c| !matches!(c, '*' | '#' | '>' | '`' | '\\'))
        .collect();
    let flat = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 160 {
        let cut: String = flat.chars().take(160).collect();
        format!("{}…", cut.trim_end())
    } else {
        flat
    }
}

/// The lowercased text an entry can be found by: title, author, and what
/// was written.
pub fn haystack(item: &SummaryWithBook) -> String {
    let text = format!(
        "{} {} {}",
        item.book.title,
        item.book.author.as_deref().unwrap_or(""),
        item.summary.body
    );
    text.to_lowercase()
}

/// The words of a shelf search, lowercased. No words means no search.
pub fn query_terms(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Whether an entry matches: every word appears somewhere in its haystack,
/// in any order, even inside a longer word.
pub fn matches(haystack: &str, terms: &[String]) -> bool {
    terms.iter().all(|word| haystack.contains(word.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Book, Summary};
    use chrono::FixedOffset;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn utc(y: i32, m: u32, day: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, day, h, min, 0).unwrap()
    }

    fn entry(id: &str, started: Option<NaiveDate>, finished: Option<NaiveDate>) -> SummaryWithBook {
        let t = utc(2026, 3, 4, 12, 0);
        SummaryWithBook {
            summary: Summary {
                id: id.into(),
                user_id: "u".into(),
                book_id: "b".into(),
                body: String::new(),
                started_on: started,
                finished_on: finished,
                days_to_complete: None,
                created_at: t,
                updated_at: t,
            },
            book: Book {
                id: "b".into(),
                title: "Dune".into(),
                subtitle: None,
                author: None,
                isbn: None,
                publisher: None,
                description: None,
                published_date: None,
                page_count: None,
                cover_url: None,
                cover_path: None,
                provider: 0,
                external_id: "/works/OL1W".into(),
                created_at: t,
                updated_at: t,
            },
        }
    }

    fn finished(id: &str, on: Option<NaiveDate>, days: Option<i64>) -> SummaryWithBook {
        let mut e = entry(id, None, on);
        e.summary.days_to_complete = days;
        e
    }

    fn today() -> NaiveDate {
        d(2026, 10, 1)
    }

    fn meta(item: &SummaryWithBook, milestone: Milestone, format: &str) -> String {
        meta_line(item, milestone, format, today(), &Utc)
    }

    #[test]
    fn finished_line() {
        let f = Milestone::Finished;
        let on = Some(d(2026, 9, 5));
        assert_eq!(
            meta(&finished("a", on, Some(10)), f, "long"),
            "Finished Sep 5, 2026 · 10 days"
        );
        assert_eq!(
            meta(&finished("a", on, Some(1)), f, "long"),
            "Finished Sep 5, 2026 · 1 day"
        );
        // Started and finished the same day.
        assert_eq!(
            meta(&finished("a", on, Some(0)), f, "yyyy_mm_dd"),
            "Finished 2026-09-05 · 0 days"
        );
        assert_eq!(
            meta(&finished("a", on, None), f, "dd_mm_yyyy"),
            "Finished 05/09/2026"
        );
        assert_eq!(
            meta(&finished("a", on, None), f, "mm_dd_yyyy"),
            "Finished 09/05/2026"
        );
        assert_eq!(meta(&finished("a", None, Some(3)), f, "long"), "3 days");
        assert_eq!(meta(&finished("a", None, None), f, "long"), "");
    }

    #[test]
    fn reading_line_counts_days_from_one() {
        let r = Milestone::Reading;
        let line = |started| meta(&entry("a", Some(started), None), r, "long");
        assert_eq!(line(d(2026, 9, 20)), "Started Sep 20, 2026 · day 12");
        assert_eq!(line(d(2026, 9, 30)), "Started Sep 30, 2026 · day 2");
        assert_eq!(line(today()), "Started Oct 1, 2026 · day 1");
        // Across a year end.
        assert_eq!(line(d(2025, 12, 31)), "Started Dec 31, 2025 · day 275");
        // A start date still to come: no day yet.
        assert_eq!(line(d(2026, 10, 2)), "Started Oct 2, 2026");
        assert_eq!(meta(&entry("a", None, None), r, "long"), "");
    }

    #[test]
    fn added_line_uses_the_local_date() {
        let added = |created_at, hours: i32| {
            let mut item = entry("a", None, None);
            item.summary.created_at = created_at;
            let tz = FixedOffset::east_opt(hours * 3600).unwrap();
            meta_line(&item, Milestone::Eventually, "long", today(), &tz)
        };
        let late = utc(2026, 3, 4, 23, 30);
        assert_eq!(added(late, 0), "Added Mar 4, 2026");
        assert_eq!(added(late, 1), "Added Mar 5, 2026");
        assert_eq!(added(late, -5), "Added Mar 4, 2026");
        let early = utc(2026, 3, 5, 2, 0);
        assert_eq!(added(early, 0), "Added Mar 5, 2026");
        assert_eq!(added(early, -5), "Added Mar 4, 2026");
    }

    #[test]
    fn line_follows_the_shelf_not_the_dates() {
        let item = finished("a", Some(d(2026, 9, 5)), Some(4));
        assert_eq!(meta(&item, Milestone::Reading, "long"), "");
        assert_eq!(
            meta(&item, Milestone::Eventually, "long"),
            "Added Mar 4, 2026"
        );
    }

    #[test]
    fn excerpt_strips_marks_and_flattens() {
        assert_eq!(excerpt(""), "");
        assert_eq!(excerpt("  \n\t "), "");
        assert_eq!(
            excerpt("# Title\n\n**Bold** and `code` > quote\\"),
            "Title Bold and code quote"
        );
        // Only those five marks go; the rest of markdown stays as typed.
        assert_eq!(
            excerpt("_under_ [link](url) C# a>b"),
            "_under_ [link](url) C ab"
        );
    }

    #[test]
    fn excerpt_cuts_at_160_characters() {
        let a = |n| "a".repeat(n);
        assert_eq!(excerpt(&a(160)), a(160));
        assert_eq!(excerpt(&a(161)), format!("{}…", a(160)));
        // A cut just after a space doesn't leave the space before the "…".
        assert_eq!(excerpt(&format!("{} bcd", a(159))), format!("{}…", a(159)));
        // Marks don't count towards the length.
        assert_eq!(excerpt(&format!("**{}**", a(160))), a(160));
        // Characters, not bytes.
        assert_eq!(excerpt(&"é".repeat(160)), "é".repeat(160));
        assert_eq!(excerpt(&"é".repeat(161)), format!("{}…", "é".repeat(160)));
        assert_eq!(excerpt(&"📚".repeat(200)), format!("{}…", "📚".repeat(160)));
    }

    #[test]
    fn counts() {
        assert_eq!(books(0), "0 books");
        assert_eq!(books(1), "1 book");
        assert_eq!(books(2), "2 books");
        assert_eq!(count_line(0, 0), "0 books");
        assert_eq!(count_line(1, 0), "1 book");
        assert_eq!(count_line(12, 3), "12 books · 3 this year");
        assert_eq!(count_line(1, 1), "1 book · 1 this year");
    }

    fn headings(view: &ShelfView) -> Vec<String> {
        view.rows
            .iter()
            .map(|row| match row {
                ShelfRow::Year(y) => format!("{} {}", y.year, y.count_label),
                ShelfRow::Entry(e) => e.item.summary.id.clone(),
            })
            .collect()
    }

    #[test]
    fn finished_shelf_is_grouped_by_year() {
        let rows = vec![
            finished("a", Some(d(2026, 9, 5)), None),
            finished("b", Some(d(2026, 1, 2)), None),
            finished("c", Some(d(2025, 12, 31)), None),
            finished("d", Some(d(2023, 6, 1)), None),
        ];
        let view = shelf_view(&rows, Milestone::Finished, "long", today(), &Utc);
        assert_eq!(
            headings(&view),
            [
                "2026 2 books",
                "a",
                "b",
                "2025 1 book",
                "c",
                "2023 1 book",
                "d"
            ]
        );
        assert_eq!(view.total, 4);
        assert_eq!(view.this_year, 2);
        assert_eq!(view.count_line, "4 books · 2 this year");
    }

    #[test]
    fn nothing_finished_this_year() {
        let rows = vec![finished("a", Some(d(2025, 9, 5)), None)];
        let view = shelf_view(&rows, Milestone::Finished, "long", today(), &Utc);
        assert_eq!(view.this_year, 0);
        assert_eq!(view.count_line, "1 book");
        let view = shelf_view(&[], Milestone::Finished, "long", today(), &Utc);
        assert!(view.rows.is_empty());
        assert_eq!(view.count_line, "0 books");
    }

    #[test]
    fn missing_finished_date_gets_no_heading() {
        // The Finished query never returns one, but the layout copes. The
        // entry sits without a heading, and the year after it starts again,
        // counted in full each time.
        let rows = vec![
            finished("a", Some(d(2026, 9, 5)), None),
            finished("b", None, None),
            finished("c", Some(d(2026, 1, 2)), None),
        ];
        let view = shelf_view(&rows, Milestone::Finished, "long", today(), &Utc);
        assert_eq!(
            headings(&view),
            ["2026 2 books", "a", "b", "2026 2 books", "c"]
        );
        assert_eq!(view.count_line, "3 books · 2 this year");
    }

    #[test]
    fn other_shelves_have_no_headings() {
        // Even with finished dates, which these shelves never have.
        let rows = vec![
            finished("a", Some(d(2026, 9, 5)), None),
            entry("b", Some(d(2026, 9, 1)), None),
        ];
        for milestone in [Milestone::Reading, Milestone::Eventually] {
            let view = shelf_view(&rows, milestone, "long", today(), &Utc);
            assert_eq!(headings(&view), ["a", "b"]);
            assert_eq!(view.this_year, 0);
            assert_eq!(view.count_line, "2 books");
        }
    }

    #[test]
    fn entries_carry_their_text() {
        let mut item = entry("a", Some(d(2026, 9, 30)), None);
        item.summary.body = "**Spice** must flow.".into();
        item.book.author = Some("Frank Herbert".into());
        let blank = entry("b", Some(d(2026, 9, 30)), None);
        let rows = vec![item, blank];

        let view = shelf_view(&rows, Milestone::Reading, "long", today(), &Utc);
        let ShelfRow::Entry(e) = &view.rows[0] else {
            panic!("expected an entry")
        };
        assert_eq!(e.meta, "Started Sep 30, 2026 · day 2");
        assert_eq!(e.excerpt, "Spice must flow.");
        assert_eq!(e.empty_note, None);
        assert_eq!(e.haystack, "dune frank herbert **spice** must flow.");
        let ShelfRow::Entry(e) = &view.rows[1] else {
            panic!("expected an entry")
        };
        assert_eq!(e.excerpt, "");
        assert_eq!(e.empty_note, Some("Nothing written yet."));

        // Not yet started: an empty page is expected.
        let view = shelf_view(&rows, Milestone::Eventually, "long", today(), &Utc);
        let ShelfRow::Entry(e) = &view.rows[1] else {
            panic!("expected an entry")
        };
        assert_eq!(e.empty_note, None);
    }

    #[test]
    fn haystack_without_author() {
        let mut item = entry("a", None, None);
        item.book.title = "The Left Hand of Darkness".into();
        item.summary.body = "Genly Ai".into();
        // A missing author leaves two spaces, which no search can notice.
        assert_eq!(haystack(&item), "the left hand of darkness  genly ai");
    }

    #[test]
    fn search_words() {
        assert_eq!(query_terms(""), Vec::<String>::new());
        assert_eq!(query_terms("   "), Vec::<String>::new());
        assert_eq!(query_terms("  Frank\tHERBERT \n"), ["frank", "herbert"]);
        assert_eq!(query_terms("Élan"), ["élan"]);
    }

    #[test]
    fn search_matches_every_word_anywhere() {
        let hay = "dune frank herbert spice must flow.";
        let m = |q: &str| matches(hay, &query_terms(q));
        assert!(m(""));
        assert!(m("dune"));
        assert!(m("DUNE"));
        assert!(m("herbert dune"));
        assert!(m("spi"));
        assert!(m("flow."));
        assert!(!m("dune asimov"));
        assert!(!m("dune frank herbert spice must flow. extra"));
        // Words, not phrases: the space between them isn't required.
        assert!(m("must spice"));
    }
}
