//! Throwaway terminal front end for exercising bookshelf-core.
//! Set BOOKSHELF_DIR to use a scratch data directory instead of
//! ~/.local/share/Bookshelf.

use std::env;
use std::error::Error;

use bookshelf_core::db;
use bookshelf_core::models::*;
use bookshelf_core::openlibrary::OpenLibrary;
use bookshelf_core::paths::AppPaths;
use bookshelf_core::rusqlite::Connection;
use bookshelf_core::service;
use chrono::NaiveDate;

type Res<T> = Result<T, Box<dyn Error>>;

const USAGE: &str = "\
usage:
  bookshelf user add <name> [email]        email is optional; sent to Open Library as contact
  bookshelf user list
  bookshelf search <query...>
  bookshelf add <user> <n> <query...>      save the n-th search result for <user>
  bookshelf list <user> [finished|reading|eventually]
  bookshelf update <summary-id> [--started YYYY-MM-DD] [--finished YYYY-MM-DD] [--body TEXT]
  bookshelf delete <summary-id>
  bookshelf refresh <book-id>
  bookshelf import <export-dir>           import CSV export from the Rails app (empty data dir only;
                                          needs a build with --features rails-import)
  bookshelf fetch-covers                  download any missing cover images";

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Res<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let paths = AppPaths::from_env()?;
    let conn = db::open(&paths.db_path)?;
    let ol = OpenLibrary::new()?;

    match args.as_slice() {
        ["user", "add", name, email @ ..] if email.len() <= 1 => {
            let email = validate_email(email.first().copied().unwrap_or(""))?;
            let u = create_user(&conn, name, None)?;
            if let Some(e) = &email {
                set_user_email(&conn, &u.id, e)?;
            }
            println!("created {} ({})", u.name, u.id);
        }
        ["user", "list"] => {
            for u in list_users(&conn)? {
                println!("{}  {}", u.id, u.name);
            }
        }
        ["search", query @ ..] if !query.is_empty() => {
            for (i, b) in ol.search(&query.join(" "))?.iter().enumerate() {
                println!(
                    "{:>2}. {} — {} ({}) {}",
                    i + 1,
                    b.title,
                    b.author.as_deref().unwrap_or("?"),
                    b.published_date.as_deref().unwrap_or("?"),
                    if b.cover_url.is_some() { "[cover]" } else { "" }
                );
            }
        }
        ["add", user, n, query @ ..] if !query.is_empty() => {
            let user = find_user(&conn, user)?;
            let ol = ol.with_contact(user.email.as_deref());
            let n: usize = n.parse()?;
            let results = ol.search(&query.join(" "))?;
            let picked = results
                .get(n.checked_sub(1).ok_or("n starts at 1")?)
                .ok_or("no such result")?
                .clone();
            let book = service::save_book_from_search(&conn, &ol, &paths, picked)?;
            let s = create_summary(&conn, &user.id, &book.id, &SummaryInput::default())?;
            println!("saved \"{}\"", book.title);
            println!("  book id:    {}", book.id);
            println!("  summary id: {}", s.id);
            println!(
                "  description: {}",
                if book.description.is_some() {
                    "yes"
                } else {
                    "no"
                }
            );
            match &book.cover_path {
                Some(p) => println!("  cover:      {}", paths.cover_file(p).display()),
                None => println!("  cover:      none"),
            }
        }
        ["list", user] => {
            let user = find_user(&conn, user)?;
            for (label, m) in MILESTONES {
                print_list(&conn, &user.id, label, m)?;
            }
        }
        ["list", user, which] => {
            let user = find_user(&conn, user)?;
            let (label, m) = MILESTONES
                .iter()
                .find(|(l, _)| l == which)
                .ok_or("use finished, reading or eventually")?;
            print_list(&conn, &user.id, label, *m)?;
        }
        ["update", id, flags @ ..] => {
            let existing = get_summary(&conn, id)?;
            let mut input = SummaryInput {
                body: existing.body,
                started_on: existing.started_on,
                finished_on: existing.finished_on,
            };
            let mut it = flags.iter();
            while let Some(flag) = it.next() {
                let value = it.next().ok_or("flag needs a value")?;
                match *flag {
                    "--started" => input.started_on = Some(parse_date(value)?),
                    "--finished" => input.finished_on = Some(parse_date(value)?),
                    "--body" => input.body = value.to_string(),
                    other => return Err(format!("unknown flag {other}").into()),
                }
            }
            let s = update_summary(&conn, id, &input)?;
            println!(
                "updated: {:?}, days_to_complete {:?}",
                s.milestone(),
                s.days_to_complete
            );
        }
        ["delete", id] => {
            delete_summary(&conn, id)?;
            println!("deleted");
        }
        ["refresh", book_id] => {
            let b = service::refresh_book(&conn, &ol, &paths, book_id)?;
            println!(
                "refreshed \"{}\": pages {:?}, description {}",
                b.title,
                b.page_count,
                if b.description.is_some() { "yes" } else { "no" }
            );
        }
        #[cfg(not(feature = "rails-import"))]
        ["import", _] => {
            return Err("this build has no importer; rebuild with --features rails-import".into())
        }
        #[cfg(feature = "rails-import")]
        ["import", dir] => {
            let r = bookshelf_core::import::import_rails_export(&conn, std::path::Path::new(dir))?;
            println!(
                "imported {} users, {} books, {} summaries",
                r.users, r.books, r.summaries
            );
            println!("summaries with no text: {}", r.summaries_without_text);
            println!(
                "rich text names found on summaries: {:?}",
                r.rich_text_names
            );
            println!("next: bookshelf fetch-covers");
        }
        ["fetch-covers"] => {
            let (ok, missing) = service::fetch_missing_covers(&conn, &ol, &paths, |b, got| {
                println!("{}  {}", if got { "ok     " } else { "missing" }, b.title);
            })?;
            println!("done: {ok} downloaded, {missing} unavailable");
        }
        _ => println!("{USAGE}"),
    }
    Ok(())
}

const MILESTONES: [(&str, Milestone); 3] = [
    ("finished", Milestone::Finished),
    ("reading", Milestone::Reading),
    ("eventually", Milestone::Eventually),
];

fn print_list(conn: &Connection, user_id: &str, label: &str, m: Milestone) -> Res<()> {
    println!("== {label} ==");
    for row in list_summaries(conn, user_id, m)? {
        let days = row
            .summary
            .days_to_complete
            .map(|d| format!(" [{d} days]"))
            .unwrap_or_default();
        println!(
            "{}  {} — {}{}{}",
            row.summary.id,
            row.book.title,
            row.book.author.as_deref().unwrap_or("?"),
            days,
            if row.book.cover_path.is_some() {
                " [cover]"
            } else {
                ""
            }
        );
    }
    Ok(())
}

fn find_user(conn: &Connection, name: &str) -> Res<User> {
    list_users(conn)?
        .into_iter()
        .find(|u| u.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("no user named {name:?} (try: bookshelf user add {name})").into())
}

fn parse_date(s: &str) -> Res<NaiveDate> {
    Ok(NaiveDate::parse_from_str(s, "%Y-%m-%d")?)
}
