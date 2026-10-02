//! The help pages: the user manual and the keyboard shortcuts list, from
//! the same sources as the GTK app's, so every app says the same.

use bookshelf_core::{manual as core_manual, shortcuts as core_shortcuts};

/// The user manual, built in from `MANUAL.md` at the top of the repo.
const MANUAL_MD: &str = include_str!("../../MANUAL.md");

/// The user manual, split up for an in-app page.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Manual {
    /// The title and opening paragraphs, in Markdown.
    pub intro: String,
    /// The manual's own "Contents" list.
    pub contents: Vec<ManualEntry>,
    /// Every `##`/`###` heading with the Markdown under it, in order.
    pub sections: Vec<ManualSection>,
}

/// A line of the manual's Contents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ManualEntry {
    /// The section's title.
    pub title: String,
    /// The section it leads to (a `ManualSection.anchor`).
    pub anchor: String,
    /// A subsection, indented in the Contents.
    pub nested: bool,
}

/// One section of the manual.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ManualSection {
    /// The anchor GitHub gives the heading, so Contents links work in the
    /// app and on GitHub alike.
    pub anchor: String,
    /// The heading line and everything up to the next heading, in Markdown
    /// (lay it out with `render_markdown`).
    pub markdown: String,
}

/// The user manual for the platform this library was built for: the
/// Mac's in the macOS build, Windows' in the Windows build. Prefer
/// `manual_for`, which says which one it wants.
#[uniffi::export]
pub fn manual() -> Manual {
    let platform = if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    };
    manual_for(platform)
}

/// The user manual as one platform's app shows it: the text every app
/// shares plus that platform's own (its keys, menus and folders).
#[uniffi::export]
pub fn manual_for(platform: Platform) -> Manual {
    let m = core_manual::manual_for(MANUAL_MD, platform.into());
    Manual {
        intro: m.intro,
        contents: m
            .contents
            .into_iter()
            .map(|e| ManualEntry {
                title: e.title,
                anchor: e.anchor,
                nested: e.nested,
            })
            .collect(),
        sections: m
            .sections
            .into_iter()
            .map(|s| ManualSection {
                anchor: s.anchor,
                markdown: s.markdown,
            })
            .collect(),
    }
}

/// Which app's shortcuts to list, or manual to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Platform {
    /// GTK accelerators.
    Linux,
    /// Key combos with ⌘.
    Mac,
    /// Key combos with Ctrl.
    Windows,
}

impl From<Platform> for core_shortcuts::Platform {
    fn from(p: Platform) -> Self {
        match p {
            Platform::Linux => Self::Linux,
            Platform::Mac => Self::Mac,
            Platform::Windows => Self::Windows,
        }
    }
}

/// A titled group of shortcuts ("Writing").
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct ShortcutGroup {
    /// The group's title.
    pub title: String,
    /// Its shortcuts, in the order shown.
    pub items: Vec<Shortcut>,
}

/// One action and its keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Shortcut {
    /// What it does ("Bold").
    pub title: String,
    /// The keys.
    pub accel: Accel,
    /// Other keys that do the same, shown after `accel` with "or"
    /// between: Windows' Ctrl+Shift+Enter for full screen, besides F11.
    /// Usually empty.
    pub also: Vec<Accel>,
}

/// The keys for a shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Accel {
    /// A GTK accelerator string (Linux), like `<Control>n`.
    Gtk {
        /// As `gtk::accelerator_parse` reads it.
        accelerator: String,
    },
    /// A key and its modifiers (Mac and Windows).
    Keys {
        /// The key and modifiers.
        combo: KeyCombo,
    },
}

/// A key plus modifier flags, for `NSEvent.ModifierFlags` + key equivalent
/// on the Mac and `VirtualKeyModifiers` + `VirtualKey` on Windows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct KeyCombo {
    /// The key.
    pub key: ShortcutKey,
    /// ⌘ on the Mac. Never set for Windows.
    pub command: bool,
    /// ⌃ on the Mac, Ctrl on Windows.
    pub control: bool,
    /// Shift.
    pub shift: bool,
}

/// A key on the keyboard.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum ShortcutKey {
    /// The key that types this character: a lowercase letter, a digit or
    /// punctuation (`,`, `?`). For `?` the Shift it takes is implied, not
    /// in `shift`: AppKit takes "?" as the key equivalent as is; on
    /// Windows it's Shift + the `/` key (VK_OEM_2) on most layouts.
    Character {
        /// One character.
        text: String,
    },
    /// Return (Enter).
    Return,
    /// Escape.
    Escape,
    /// F1 to F12.
    Function {
        /// 1 to 12.
        number: u8,
    },
}

impl From<core_shortcuts::Accel> for Accel {
    fn from(a: core_shortcuts::Accel) -> Self {
        use core_shortcuts::Key as K;
        match a {
            core_shortcuts::Accel::Gtk(accelerator) => Accel::Gtk { accelerator },
            core_shortcuts::Accel::Keys(k) => Accel::Keys {
                combo: KeyCombo {
                    key: match k.key {
                        K::Char(c) => ShortcutKey::Character {
                            text: c.to_string(),
                        },
                        K::Return => ShortcutKey::Return,
                        K::Escape => ShortcutKey::Escape,
                        K::F(number) => ShortcutKey::Function { number },
                    },
                    command: k.command,
                    control: k.control,
                    shift: k.shift,
                },
            },
        }
    }
}

/// The keyboard shortcuts list for one platform, in the order it's shown.
#[uniffi::export]
pub fn shortcuts(platform: Platform) -> Vec<ShortcutGroup> {
    core_shortcuts::shortcuts(platform.into())
        .into_iter()
        .map(|g| ShortcutGroup {
            title: g.title,
            items: g
                .items
                .into_iter()
                .map(|s| Shortcut {
                    title: s.title,
                    accel: s.accel.into(),
                    also: s.also.into_iter().map(Accel::from).collect(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manual_is_built_in() {
        let m = manual();
        assert!(m.intro.starts_with("# Bookshelf"));
        assert!(m.contents.len() > 10);
        for entry in &m.contents {
            assert!(
                m.sections.iter().any(|s| s.anchor == entry.anchor),
                "{}",
                entry.anchor
            );
        }
        assert!(m.contents.iter().any(|e| e.nested));
    }

    #[test]
    fn each_platform_has_its_own_manual() {
        let text = |m: &Manual| {
            let mut all = m.intro.clone();
            for s in &m.sections {
                all.push_str(&s.markdown);
            }
            all
        };
        let mac = manual_for(Platform::Mac);
        let windows = manual_for(Platform::Windows);
        let linux = manual_for(Platform::Linux);
        for m in [&mac, &windows, &linux] {
            assert!(m.intro.starts_with("# Bookshelf"));
            assert!(!text(m).contains("<!--"));
            assert_eq!(m.contents, linux.contents);
        }
        assert!(text(&mac).contains("**⌘N**"));
        assert!(!text(&mac).contains("Ctrl+"));
        assert!(text(&windows).contains("**Ctrl+E**"));
        assert!(text(&windows).contains("%LOCALAPPDATA%"));
        assert!(text(&linux).contains("~/.local/share/bookshelf"));
        // manual() picks the platform it was built for (Linux, in tests here).
        let built_for = if cfg!(target_os = "macos") {
            mac
        } else if cfg!(windows) {
            windows
        } else {
            linux
        };
        assert_eq!(manual(), built_for);
    }

    fn find(platform: Platform, title: &str) -> Accel {
        shortcuts(platform)
            .into_iter()
            .flat_map(|g| g.items)
            .find(|s| s.title == title)
            .map(|s| s.accel)
            .unwrap_or_else(|| panic!("no {title}"))
    }

    fn combo(key: ShortcutKey, command: bool, control: bool, shift: bool) -> Accel {
        Accel::Keys {
            combo: KeyCombo {
                key,
                command,
                control,
                shift,
            },
        }
    }

    fn ch(c: &str) -> ShortcutKey {
        ShortcutKey::Character { text: c.into() }
    }

    #[test]
    fn shortcuts_per_platform() {
        assert_eq!(
            find(Platform::Linux, "Bold"),
            Accel::Gtk {
                accelerator: "<Control>b".into()
            }
        );
        assert_eq!(
            find(Platform::Mac, "Bold"),
            combo(ch("b"), true, false, false)
        );
        assert_eq!(
            find(Platform::Windows, "Bold"),
            combo(ch("b"), false, true, false)
        );
        assert_eq!(
            find(Platform::Mac, "Focus mode"),
            combo(ch("f"), true, false, true)
        );
        assert_eq!(
            find(Platform::Mac, "Write or edit your summary"),
            combo(ShortcutKey::Return, true, false, false)
        );
        assert_eq!(
            find(Platform::Windows, "Full screen"),
            combo(ShortcutKey::Function { number: 11 }, false, false, false)
        );
        let full_screen = shortcuts(Platform::Windows)
            .into_iter()
            .flat_map(|g| g.items)
            .find(|s| s.title == "Full screen")
            .unwrap();
        assert_eq!(
            full_screen.also,
            [combo(ShortcutKey::Return, false, true, true)]
        );
        assert!(shortcuts(Platform::Mac)
            .iter()
            .chain(&shortcuts(Platform::Linux))
            .flat_map(|g| &g.items)
            .all(|s| s.also.is_empty()));
        assert_eq!(
            find(Platform::Windows, "Leave full screen"),
            combo(ShortcutKey::Escape, false, false, false)
        );
        assert_eq!(
            find(Platform::Mac, "User manual"),
            combo(ch("?"), true, false, false)
        );
        // The Mac lists its shortcuts in the menus instead.
        let mac = shortcuts(Platform::Mac);
        assert!(mac
            .iter()
            .flat_map(|g| &g.items)
            .all(|s| s.title != "Keyboard shortcuts"));
        assert_eq!(mac[0].title, "Your shelves");
    }
}
