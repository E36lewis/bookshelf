//! Daily safety copies of the database, named
//! `bookshelf-<journal id>-YYYY-MM-DD.sqlite3`. The id is random, made once
//! per journal and stored inside it, so two computers sharing one backup
//! folder never mistake each other's copies for their own.
//!
//! They go in `<data dir>/backups` unless another folder was chosen (say a
//! USB drive or a synced folder, so a failed disk doesn't take both). The
//! choice is app-wide: a backup holds every profile.
//!
//! To restore: quit Bookshelf, delete `bookshelf.sqlite3-wal` and
//! `bookshelf.sqlite3-shm` from the data folder if they're there, then copy
//! the backup over `bookshelf.sqlite3`.

use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use rusqlite::Connection;

use crate::models::{app_setting, set_app_setting};
use crate::paths::AppPaths;
use crate::{Error, Result};

/// How many daily copies to keep.
pub const KEEP: usize = 7;

const FOLDER_KEY: &str = "backup_folder";
const JOURNAL_ID_KEY: &str = "journal_id";

/// This journal's id: 8 hex characters, made the first time it's needed.
pub fn journal_id(conn: &Connection) -> Result<String> {
    if let Some(id) = app_setting(conn, JOURNAL_ID_KEY)? {
        return Ok(id);
    }
    let id = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
    set_app_setting(conn, JOURNAL_ID_KEY, Some(&id))?;
    Ok(id)
}

/// The date of this journal's newest backup, if it has one where backups go now.
pub fn latest(conn: &Connection, paths: &AppPaths) -> Result<Option<NaiveDate>> {
    let (dir, _) = folder(conn, paths)?;
    if !dir.is_dir() {
        return Ok(None);
    }
    let id = journal_id(conn)?;
    Ok(list(&dir, &id)?.first().and_then(|p| date_of(p, &id)))
}

/// Where backups go, and whether that's a folder the user chose.
pub fn folder(conn: &Connection, paths: &AppPaths) -> Result<(PathBuf, bool)> {
    Ok(match app_setting(conn, FOLDER_KEY)? {
        Some(chosen) => (PathBuf::from(chosen), true),
        None => (paths.backups_dir.clone(), false),
    })
}

/// Use `chosen` for backups from now on, or the default folder for `None`.
pub fn set_folder(conn: &Connection, chosen: Option<&Path>) -> Result<()> {
    let value = match chosen {
        Some(dir) => {
            if !dir.is_absolute() || !dir.is_dir() {
                return Err(Error::Invalid(format!("{} isn't a folder", dir.display())));
            }
            Some(
                dir.to_str()
                    .ok_or_else(|| Error::Invalid("that folder's name can't be used".into()))?,
            )
        }
        None => None,
    };
    set_app_setting(conn, FOLDER_KEY, value)
}

/// Makes today's copy if there isn't one yet, then drops all but the newest
/// `keep`. Returns the new file, or None if today's copy already existed.
pub fn daily(conn: &Connection, paths: &AppPaths, keep: usize) -> Result<Option<PathBuf>> {
    let (dir, _) = folder(conn, paths)?;
    let id = journal_id(conn)?;
    daily_in(conn, &dir, &id, keep, Local::now().date_naive())
}

fn daily_in(
    conn: &Connection,
    dir: &Path,
    id: &str,
    keep: usize,
    day: NaiveDate,
) -> Result<Option<PathBuf>> {
    // A chosen folder that's gone (drive unplugged) is never re-created:
    // that would quietly put the backups somewhere nobody looks.
    if !dir.is_dir() {
        return Err(Error::Invalid(format!(
            "the backup folder {} isn't available",
            dir.display()
        )));
    }
    let target = dir.join(format!("bookshelf-{id}-{day}.sqlite3"));
    if target.exists() {
        return Ok(None);
    }
    // Write under a temporary name and rename at the end, so a crash midway
    // never leaves something that looks like a good backup.
    let part = target.with_extension("sqlite3.part");
    let _ = std::fs::remove_file(&part); // left over from an interrupted run
    let part_str = part
        .to_str()
        .ok_or_else(|| Error::Invalid("backup path isn't valid UTF-8".into()))?;
    conn.execute("VACUUM INTO ?1", [part_str])?;
    // Readable only by you, even in a shared or synced folder.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&part, &target)?;

    for old in list(dir, id)?.into_iter().skip(keep) {
        std::fs::remove_file(old)?;
    }
    Ok(Some(target))
}

/// This journal's backups in `dir`, newest first. Only files named exactly
/// like ours, with our id, are counted, so nothing else in a chosen folder
/// (another computer's backups included) is ever touched.
fn list(dir: &Path, id: &str) -> Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| date_of(p, id).is_some())
        .collect();
    found.sort_by(|a, b| b.cmp(a)); // same prefix, ISO dates: sorts by name
    Ok(found)
}

/// The day a backup of journal `id` was made, from its name.
fn date_of(path: &Path, id: &str) -> Option<NaiveDate> {
    let name = path.file_name()?.to_str()?;
    let day = name
        .strip_prefix("bookshelf-")?
        .strip_prefix(id)?
        .strip_prefix('-')?;
    let day = day.strip_suffix(".sqlite3")?;
    // Exactly YYYY-MM-DD (chrono alone would also take 2026-1-5).
    if day.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, models};

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    #[test]
    fn one_copy_a_day_and_old_ones_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path()).unwrap();
        let conn = db::open(&paths.db_path).unwrap();
        models::create_user(&conn, "Avery", None).unwrap();
        let backups = &paths.backups_dir;

        let id = journal_id(&conn).unwrap();
        assert_eq!(id, journal_id(&conn).unwrap(), "made once, then kept");
        let first = daily_in(&conn, backups, &id, 2, day(1))
            .unwrap()
            .expect("made a copy");
        assert!(
            daily_in(&conn, backups, &id, 2, day(1)).unwrap().is_none(),
            "once per day"
        );

        // The copy is a working database with the data in it.
        let copy = db::open(&first).unwrap();
        assert_eq!(models::list_users(&copy).unwrap().len(), 1);
        drop(copy); // Windows can't delete a file that's still open

        // Never touched: unrelated files, and another computer's backups.
        std::fs::write(backups.join("notes.txt"), "mine").unwrap();
        std::fs::write(
            backups.join("bookshelf-0000beef-2026-09-01.sqlite3"),
            "theirs",
        )
        .unwrap();
        daily_in(&conn, backups, &id, 2, day(2)).unwrap();
        daily_in(&conn, backups, &id, 2, day(3)).unwrap();
        let names: Vec<String> = list(backups, &id)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                format!("bookshelf-{id}-2026-10-03.sqlite3"),
                format!("bookshelf-{id}-2026-10-02.sqlite3")
            ]
        );
        assert!(backups.join("notes.txt").exists());
        assert!(backups
            .join("bookshelf-0000beef-2026-09-01.sqlite3")
            .exists());
        assert_eq!(latest(&conn, &paths).unwrap(), Some(day(3)));
    }

    #[test]
    fn a_chosen_folder_is_used_and_never_invented() {
        let data = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(data.path()).unwrap();
        let conn = db::open(&paths.db_path).unwrap();

        assert_eq!(
            folder(&conn, &paths).unwrap(),
            (paths.backups_dir.clone(), false)
        );
        assert!(set_folder(&conn, Some(Path::new("relative/dir"))).is_err());
        set_folder(&conn, Some(elsewhere.path())).unwrap();
        assert_eq!(
            folder(&conn, &paths).unwrap(),
            (elsewhere.path().to_path_buf(), true)
        );

        // Another journal's copy from today doesn't count as ours.
        let today = Local::now().date_naive();
        std::fs::write(
            elsewhere
                .path()
                .join(format!("bookshelf-0000beef-{today}.sqlite3")),
            "x",
        )
        .unwrap();
        let made = daily(&conn, &paths, KEEP)
            .unwrap()
            .expect("our own copy is still made");
        assert_eq!(made.parent().unwrap(), elsewhere.path());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&made).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // Unplugged drive: an error, and no folder quietly re-created.
        let gone = elsewhere.path().to_path_buf();
        drop(elsewhere);
        assert!(daily(&conn, &paths, KEEP).is_err());
        assert!(!gone.exists());

        set_folder(&conn, None).unwrap();
        assert!(!folder(&conn, &paths).unwrap().1);
    }
}
