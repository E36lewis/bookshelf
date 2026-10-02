//! Writes every summary of one profile out as a Markdown file.

use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::models::{self, Milestone, SummaryWithBook};
use crate::paths::AppPaths;
use crate::Result;

/// Returns how many files were written. Existing files are never overwritten;
/// a number is added to the name instead.
pub fn export_markdown(conn: &Connection, user_id: &str, dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(dir)?;
    let mut count = 0;
    for milestone in [
        Milestone::Finished,
        Milestone::Reading,
        Milestone::Eventually,
    ] {
        for item in models::list_summaries(conn, user_id, milestone)? {
            write_new(dir, &slug(&item.book.title), &render(&item))?;
            count += 1;
        }
    }
    Ok(count)
}

/// Last resort when a summary can't be saved to the database: the text goes
/// to `<data dir>/recovery/<title>-<time>.md` so it isn't lost.
pub fn save_recovery_copy(paths: &AppPaths, title: &str, body: &str) -> Result<PathBuf> {
    let dir = paths.data_dir.join("recovery");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
    write_new(&dir, &format!("{}-{stamp}", slug(title)), body)
}

fn render(item: &SummaryWithBook) -> String {
    let (b, s) = (&item.book, &item.summary);
    let mut out = String::from("---\n");
    out.push_str(&format!("title: {}\n", yaml(&b.title)));
    if let Some(a) = &b.author {
        out.push_str(&format!("author: {}\n", yaml(a)));
    }
    if let Some(d) = s.started_on {
        out.push_str(&format!("started: {d}\n"));
    }
    if let Some(d) = s.finished_on {
        out.push_str(&format!("finished: {d}\n"));
    }
    if let Some(n) = s.days_to_complete {
        out.push_str(&format!("days_to_complete: {n}\n"));
    }
    out.push_str("---\n\n");
    out.push_str(s.body.trim_end());
    out.push('\n');
    out
}

fn yaml(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A title as a file name: lowercase letters and digits joined by dashes.
/// Never one of the names Windows reserves for devices, which can't be
/// used as a file name there whatever the extension ("con.md").
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(80).collect();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else if is_windows_device_name(&trimmed) {
        format!("{trimmed}-book")
    } else {
        trimmed
    }
}

/// CON, PRN, AUX, NUL, COM0-9 and LPT0-9, including the superscript digits
/// Windows also treats as port numbers. `name` is already lowercase.
fn is_windows_device_name(name: &str) -> bool {
    if matches!(name, "con" | "prn" | "aux" | "nul") {
        return true;
    }
    let port = name
        .strip_prefix("com")
        .or_else(|| name.strip_prefix("lpt"));
    port.is_some_and(|n| {
        let mut chars = n.chars();
        matches!(
            (chars.next(), chars.next()),
            (Some('0'..='9' | '¹' | '²' | '³'), None)
        )
    })
}

/// Writes `<stem>.md`, or `<stem>-2.md`, ... if taken. `create_new` makes
/// the "is it free?" check and the create one step, and never follows a
/// symlink that sits where the file would go.
fn write_new(dir: &Path, stem: &str, contents: &str) -> Result<PathBuf> {
    for n in 1.. {
        let name = if n == 1 {
            format!("{stem}.md")
        } else {
            format!("{stem}-{n}.md")
        };
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(contents.as_bytes())?;
                file.sync_all()?;
                return Ok(path);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::{slug, write_new};

    #[test]
    fn slugs() {
        assert_eq!(
            slug("The Hobbit: Or, There & Back!"),
            "the-hobbit-or-there-back"
        );
        assert_eq!(slug("???"), "untitled");
    }

    #[test]
    fn slugs_are_never_windows_device_names() {
        assert_eq!(slug("Con"), "con-book");
        assert_eq!(slug("PRN"), "prn-book");
        assert_eq!(slug("Aux!"), "aux-book");
        assert_eq!(slug("nul."), "nul-book");
        assert_eq!(slug("COM1"), "com1-book");
        assert_eq!(slug("com9"), "com9-book");
        assert_eq!(slug("Com0"), "com0-book");
        assert_eq!(slug("LPT1"), "lpt1-book");
        assert_eq!(slug("LPT9"), "lpt9-book");
        assert_eq!(slug("COM¹"), "com¹-book");
        assert_eq!(slug("lpt³"), "lpt³-book");
        // Everything else is as it always was.
        assert_eq!(slug("Console"), "console");
        assert_eq!(slug("Con Air"), "con-air");
        assert_eq!(slug("COM10"), "com10");
        assert_eq!(slug("LPT"), "lpt");
        assert_eq!(slug("Com 1"), "com-1");
        assert_eq!(slug("COM⁴"), "com⁴");
    }

    #[test]
    fn recovery_copies_keep_the_text() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::AppPaths::in_dir(dir.path()).unwrap();
        let a = super::save_recovery_copy(&paths, "Dune", "Sand.").unwrap();
        let b = super::save_recovery_copy(&paths, "Dune", "More sand.").unwrap();
        assert_ne!(a, b, "a second copy in the same second gets its own file");
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "Sand.");
        assert!(a.starts_with(dir.path().join("recovery")));
    }

    #[test]
    fn existing_files_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        write_new(dir.path(), "dune", "one").unwrap();
        write_new(dir.path(), "dune", "two").unwrap();
        let read = |n: &str| std::fs::read_to_string(dir.path().join(n)).unwrap();
        assert_eq!(read("dune.md"), "one");
        assert_eq!(read("dune-2.md"), "two");
    }
}
