use std::path::{Path, PathBuf};

use directories::BaseDirs;

use crate::{Error, Result};

/// The app's id: the macOS bundle and Windows package name, and the
/// macOS data folder's name.
pub const APP_ID: &str = "io.github.e36lewis.Bookshelf";

/// The preview build's id. It's a separate app with its own journal.
pub const PREVIEW_APP_ID: &str = "io.github.e36lewis.Bookshelf.Preview";

/// Which build is running. A preview keeps its data in its own folder, so
/// trying one out never touches the real journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Stable,
    Preview,
}

impl Channel {
    pub fn app_id(self) -> &'static str {
        match self {
            Channel::Stable => APP_ID,
            Channel::Preview => PREVIEW_APP_ID,
        }
    }
}

/// The data folder this platform's app uses, without creating it:
///
/// - Linux (and other Unix): `~/.local/share/bookshelf` (in
///   `$XDG_DATA_HOME` if set), where it has always been.
/// - macOS: `~/Library/Application Support/<app id>`. In the sandbox, `~`
///   is the app's container.
/// - Windows: `%LOCALAPPDATA%\Bookshelf`. Local, not roaming: a SQLite file
///   doesn't belong in a profile that's copied between computers.
///
/// Previews use `bookshelf-preview`, the preview app id and
/// `Bookshelf Preview` respectively.
pub fn platform_data_dir(channel: Channel) -> Result<PathBuf> {
    platform_dir(channel).ok_or_else(|| Error::Invalid("no home directory".into()))
}

#[cfg(target_os = "macos")]
fn platform_dir(channel: Channel) -> Option<PathBuf> {
    Some(BaseDirs::new()?.data_dir().join(channel.app_id()))
}

#[cfg(windows)]
fn platform_dir(channel: Channel) -> Option<PathBuf> {
    let name = match channel {
        Channel::Stable => "Bookshelf",
        Channel::Preview => "Bookshelf Preview",
    };
    Some(BaseDirs::new()?.data_local_dir().join(name))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_dir(channel: Channel) -> Option<PathBuf> {
    match channel {
        // Exactly what the GTK app has always used.
        Channel::Stable => Some(
            directories::ProjectDirs::from("com", "bookshelf", "Bookshelf")?
                .data_dir()
                .to_path_buf(),
        ),
        Channel::Preview => Some(BaseDirs::new()?.data_dir().join("bookshelf-preview")),
    }
}

/// The journal's folder for this platform, created if needed.
pub fn platform_default() -> Result<AppPaths> {
    platform_default_for(Channel::Stable)
}

/// Like [`platform_default`], for a preview build's own folder.
pub fn platform_default_for(channel: Channel) -> Result<AppPaths> {
    AppPaths::in_dir(&platform_data_dir(channel)?)
}

/// Where everything lives on disk. Default: see [`platform_data_dir`].
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub covers_dir: PathBuf,
    pub backups_dir: PathBuf,
}

impl AppPaths {
    /// The stable app's folder on this platform: [`platform_default`].
    pub fn default_location() -> Result<Self> {
        platform_default()
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

    /// The journal's folder must not move when `directories` is updated, or
    /// Bookshelf would open an empty journal. Same names as
    /// `default_location`, which isn't called here: it creates the folder.
    #[test]
    fn the_default_folder_stays_put() {
        let dirs = directories::ProjectDirs::from("com", "bookshelf", "Bookshelf").unwrap();
        #[cfg(target_os = "linux")]
        {
            // ~/.local/share/bookshelf, or $XDG_DATA_HOME/bookshelf if set.
            let home = PathBuf::from(std::env::var_os("HOME").unwrap());
            let share = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(".local/share"));
            assert_eq!(dirs.data_dir(), share.join("bookshelf"));
        }
        #[cfg(target_os = "macos")]
        {
            let base = directories::BaseDirs::new().unwrap();
            assert_eq!(
                dirs.data_dir(),
                base.home_dir()
                    .join("Library/Application Support/com.bookshelf.Bookshelf")
            );
        }
        #[cfg(windows)]
        {
            let base = directories::BaseDirs::new().unwrap();
            assert_eq!(
                dirs.data_dir(),
                base.data_dir().join(r"bookshelf\Bookshelf\data")
            );
        }
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

    // These only work out the folder names: nothing is created in the real
    // home folder.

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn linux_keeps_the_folder_it_always_had() {
        let before = directories::ProjectDirs::from("com", "bookshelf", "Bookshelf").unwrap();
        let now = platform_data_dir(Channel::Stable).unwrap();
        assert_eq!(now, before.data_dir());
        if std::env::var_os("XDG_DATA_HOME").is_none() {
            assert!(now.ends_with(".local/share/bookshelf"), "{}", now.display());
        }
        let preview = platform_data_dir(Channel::Preview).unwrap();
        assert_eq!(preview, now.with_file_name("bookshelf-preview"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_uses_application_support() {
        let now = platform_data_dir(Channel::Stable).unwrap();
        assert!(
            now.ends_with("Library/Application Support/io.github.e36lewis.Bookshelf"),
            "{}",
            now.display()
        );
        let preview = platform_data_dir(Channel::Preview).unwrap();
        assert_eq!(
            preview,
            now.with_file_name("io.github.e36lewis.Bookshelf.Preview")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_local_app_data() {
        let local = BaseDirs::new().unwrap().data_local_dir().to_path_buf();
        assert_eq!(
            platform_data_dir(Channel::Stable).unwrap(),
            local.join("Bookshelf")
        );
        assert_eq!(
            platform_data_dir(Channel::Preview).unwrap(),
            local.join("Bookshelf Preview")
        );
        // Not the roaming profile.
        let roaming = BaseDirs::new().unwrap().data_dir().to_path_buf();
        assert!(!platform_data_dir(Channel::Stable)
            .unwrap()
            .starts_with(roaming));
    }

    #[test]
    fn a_preview_never_shares_the_journal() {
        assert_ne!(
            platform_data_dir(Channel::Stable).unwrap(),
            platform_data_dir(Channel::Preview).unwrap()
        );
        assert_eq!(Channel::Stable.app_id(), APP_ID);
        assert_eq!(
            Channel::Preview.app_id(),
            "io.github.e36lewis.Bookshelf.Preview"
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
