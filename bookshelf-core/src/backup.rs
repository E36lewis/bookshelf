//! Daily safety copies of the database, named `bookshelf-YYYY-MM-DD.sqlite3`.
//! They go in `<data dir>/backups` unless another folder was chosen (say a
//! USB drive or a synced folder, so a failed disk doesn't take both). The
//! choice is app-wide: a backup holds every profile. To restore one, quit
//! the app and copy it over `bookshelf.sqlite3`.

use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use rusqlite::Connection;

use crate::models::{app_setting, set_app_setting};
use crate::paths::AppPaths;
use crate::{Error, Result};

/// How many daily copies to keep.
pub const KEEP: usize = 7;

const FOLDER_KEY: &str = "backup_folder";

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
            Some(dir.to_str().ok_or_else(|| Error::Invalid("that folder's name can't be used".into()))?)
        }
        None => None,
    };
    set_app_setting(conn, FOLDER_KEY, value)
}

/// Makes today's copy if there isn't one yet, then drops all but the newest
/// `keep`. Returns the new file, or None if today's copy already existed.
pub fn daily(conn: &Connection, paths: &AppPaths, keep: usize) -> Result<Option<PathBuf>> {
    let (dir, _) = folder(conn, paths)?;
    daily_in(conn, &dir, keep, Local::now().date_naive())
}

fn daily_in(conn: &Connection, dir: &Path, keep: usize, day: NaiveDate) -> Result<Option<PathBuf>> {
    // A chosen folder that's gone (drive unplugged) is never re-created:
    // that would quietly put the backups somewhere nobody looks.
    if !dir.is_dir() {
        return Err(Error::Invalid(format!("the backup folder {} isn't available", dir.display())));
    }
    let target = dir.join(format!("bookshelf-{day}.sqlite3"));
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

    for old in list(dir)?.into_iter().skip(keep) {
        std::fs::remove_file(old)?;
    }
    Ok(Some(target))
}

/// Backups in `dir`, newest first. Only files named like ours are counted,
/// so nothing else in a chosen folder is ever touched.
pub fn list(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| date_of(p).is_some())
        .collect();
    found.sort_by(|a, b| b.cmp(a)); // ISO dates sort by name
    Ok(found)
}

/// The day a backup file was made, from its name.
pub fn date_of(path: &Path) -> Option<NaiveDate> {
    let day = path.file_name()?.to_str()?.strip_prefix("bookshelf-")?.strip_suffix(".sqlite3")?;
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

        let first = daily_in(&conn, backups, 2, day(1)).unwrap().expect("made a copy");
        assert!(daily_in(&conn, backups, 2, day(1)).unwrap().is_none(), "once per day");

        // The copy is a working database with the data in it.
        let copy = db::open(&first).unwrap();
        assert_eq!(models::list_users(&copy).unwrap().len(), 1);

        std::fs::write(backups.join("notes.txt"), "mine").unwrap(); // never touched
        daily_in(&conn, backups, 2, day(2)).unwrap();
        daily_in(&conn, backups, 2, day(3)).unwrap();
        let names: Vec<String> = list(backups)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["bookshelf-2026-10-03.sqlite3", "bookshelf-2026-10-02.sqlite3"]);
        assert!(backups.join("notes.txt").exists());
    }

    #[test]
    fn a_chosen_folder_is_used_and_never_invented() {
        let data = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(data.path()).unwrap();
        let conn = db::open(&paths.db_path).unwrap();

        assert_eq!(folder(&conn, &paths).unwrap(), (paths.backups_dir.clone(), false));
        assert!(set_folder(&conn, Some(Path::new("relative/dir"))).is_err());
        set_folder(&conn, Some(elsewhere.path())).unwrap();
        assert_eq!(folder(&conn, &paths).unwrap(), (elsewhere.path().to_path_buf(), true));

        let made = daily(&conn, &paths, KEEP).unwrap().unwrap();
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
