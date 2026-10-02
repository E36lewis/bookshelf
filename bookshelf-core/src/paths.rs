use std::path::{Path, PathBuf};

use crate::Result;

/// Where everything lives on disk. Default: ~/.local/share/bookshelf/
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub covers_dir: PathBuf,
    pub backups_dir: PathBuf,
}

impl AppPaths {
    pub fn default_location() -> Result<Self> {
        let dirs = directories::ProjectDirs::from("com", "bookshelf", "Bookshelf")
            .ok_or_else(|| crate::Error::Invalid("no home directory".into()))?;
        Self::in_dir(dirs.data_dir())
    }

    /// Honors BOOKSHELF_DIR (handy for scratch data), else the default location.
    pub fn from_env() -> Result<Self> {
        match std::env::var("BOOKSHELF_DIR") {
            Ok(dir) => Self::in_dir(Path::new(&dir)),
            Err(_) => Self::default_location(),
        }
    }

    /// Also used by tests with a temp dir. The folder is made private to
    /// the current user (0700): summaries are personal.
    pub fn in_dir(dir: &Path) -> Result<Self> {
        let covers_dir = dir.join("covers");
        let backups_dir = dir.join("backups");
        std::fs::create_dir_all(&covers_dir)?;
        std::fs::create_dir_all(&backups_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self {
            data_dir: dir.to_path_buf(),
            db_path: dir.join("bookshelf.sqlite3"),
            covers_dir,
            backups_dir,
        })
    }

    /// `cover_path` in the DB is a bare filename; this resolves it. Any
    /// directory part is dropped, so the result always stays in covers_dir.
    pub fn cover_file(&self, cover_path: &str) -> PathBuf {
        let name = Path::new(cover_path).file_name().unwrap_or_default();
        self.covers_dir.join(name)
    }
}

/// A path for people: the home folder shown as `~`. The app passes its home
/// folder in, so this doesn't read the environment and tests can pick one.
pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    match home.map(|home| path.strip_prefix(home)) {
        Some(Ok(rest)) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(Ok(rest)) => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_stay_in_the_covers_folder() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path()).unwrap();
        assert_eq!(
            paths.cover_file("abc.jpg"),
            paths.covers_dir.join("abc.jpg")
        );
        assert_eq!(
            paths.cover_file("../../etc/passwd"),
            paths.covers_dir.join("passwd")
        );
    }

    #[cfg(unix)]
    #[test]
    fn home_is_shown_as_a_tilde() {
        let home = Some(Path::new("/home/ann"));
        let shown = |p: &str| display_path(Path::new(p), home);
        assert_eq!(shown("/home/ann"), "~");
        assert_eq!(shown("/home/ann/"), "~");
        assert_eq!(shown("/home/ann/Documents"), "~/Documents");
        assert_eq!(
            shown("/home/ann/.local/share/bookshelf"),
            "~/.local/share/bookshelf"
        );
        assert_eq!(shown("/tmp/bookshelf"), "/tmp/bookshelf");
        // Whole folder names only: Annabel's home isn't inside Ann's.
        assert_eq!(shown("/home/annabel/notes"), "/home/annabel/notes");
        assert_eq!(shown("/home"), "/home");
        assert_eq!(
            display_path(Path::new("/home/ann/notes"), None),
            "/home/ann/notes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn data_folder_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(dir.path()).unwrap();
        let mode = std::fs::metadata(&paths.data_dir)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}
