//! The keyboard shortcuts list, per platform, in one table so the three
//! apps' lists stay in step. Each app still wires up its own key handlers;
//! this is what they show the user (GTK's Keyboard Shortcuts window is
//! generated from the Linux list).
//!
//! Linux accelerators are GTK accelerator strings, exactly as
//! `gtk::accelerator_parse` reads them (`<Control>n`, `F11`, `1`). Mac and
//! Windows ones are a [`KeyCombo`]: a key plus modifier flags, which the
//! apps map onto `NSEvent.ModifierFlags` + key equivalent and WinUI's
//! `VirtualKeyModifiers` + `VirtualKey`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Mac,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutGroup {
    pub title: String,
    pub items: Vec<Shortcut>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    pub title: String,
    pub accel: Accel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accel {
    /// GTK accelerator syntax (Linux).
    Gtk(String),
    /// A key and its modifiers (Mac and Windows).
    Keys(KeyCombo),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCombo {
    pub key: Key,
    /// ⌘ on the Mac. Never set for Windows.
    pub command: bool,
    /// ⌃ on the Mac, Ctrl on Windows.
    pub control: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// The key that types this character: a lowercase letter, a digit or
    /// punctuation (`,`, `?`). For `?` the Shift it takes is implied, not
    /// in `shift`: AppKit takes "?" as the key equivalent as is; on Windows
    /// it's Shift + the `/` key (VK_OEM_2) on most layouts.
    Char(char),
    Return,
    Escape,
    /// F1 to F12.
    F(u8),
}

/// The shortcuts list for one platform, in the order it's shown.
pub fn shortcuts(platform: Platform) -> Vec<ShortcutGroup> {
    TABLE
        .iter()
        .map(|(title, rows)| ShortcutGroup {
            title: title.to_string(),
            items: rows
                .iter()
                .filter_map(|row| {
                    let accel = match platform {
                        Platform::Linux => Accel::Gtk(row.linux?.to_string()),
                        Platform::Mac => Accel::Keys(row.mac?),
                        Platform::Windows => Accel::Keys(row.windows?),
                    };
                    Some(Shortcut {
                        title: row.title.to_string(),
                        accel,
                    })
                })
                .collect(),
        })
        .filter(|group| !group.items.is_empty())
        .collect()
}

/// One action; `None` where a platform has no shortcut for it (or doesn't
/// list it).
struct Row {
    title: &'static str,
    linux: Option<&'static str>,
    mac: Option<KeyCombo>,
    windows: Option<KeyCombo>,
}

const fn row(
    title: &'static str,
    linux: Option<&'static str>,
    mac: Option<KeyCombo>,
    windows: Option<KeyCombo>,
) -> Row {
    Row {
        title,
        linux,
        mac,
        windows,
    }
}

const fn combo(key: Key, command: bool, control: bool, shift: bool) -> KeyCombo {
    KeyCombo {
        key,
        command,
        control,
        shift,
    }
}

/// ⌘ + key.
const fn cmd(key: Key) -> Option<KeyCombo> {
    Some(combo(key, true, false, false))
}

/// Ctrl + key.
const fn ctrl(key: Key) -> Option<KeyCombo> {
    Some(combo(key, false, true, false))
}

/// The key alone.
const fn bare(key: Key) -> Option<KeyCombo> {
    Some(combo(key, false, false, false))
}

const fn ch(c: char) -> Key {
    Key::Char(c)
}

// Keep the Linux column in step with the handlers in main.rs (home, help),
// editor.rs, writer.rs and reader.rs of bookshelf-app.
const TABLE: &[(&str, &[Row])] = &[
    (
        "Your shelves",
        &[
            row("Reading", Some("1"), cmd(ch('1')), ctrl(ch('1'))),
            row("Finished", Some("2"), cmd(ch('2')), ctrl(ch('2'))),
            row("Eventually", Some("3"), cmd(ch('3')), ctrl(ch('3'))),
            row(
                "Add a book",
                Some("<Control>n"),
                cmd(ch('n')),
                ctrl(ch('n')),
            ),
            row(
                "Search your shelves",
                Some("<Control>f"),
                cmd(ch('f')),
                ctrl(ch('f')),
            ),
            row(
                "Settings",
                Some("<Control>comma"),
                cmd(ch(',')),
                ctrl(ch(',')),
            ),
        ],
    ),
    (
        "A book's page",
        &[
            row(
                "Write or edit your summary",
                Some("e"),
                cmd(Key::Return),
                ctrl(ch('e')),
            ),
            row("Read your summary", Some("r"), cmd(ch('r')), ctrl(ch('r'))),
        ],
    ),
    (
        "Writing",
        &[
            row(
                "Save now (it also saves as you type)",
                Some("<Control>s"),
                cmd(ch('s')),
                ctrl(ch('s')),
            ),
            row("Bold", Some("<Control>b"), cmd(ch('b')), ctrl(ch('b'))),
            row("Italic", Some("<Control>i"), cmd(ch('i')), ctrl(ch('i'))),
            // Not on Linux yet.
            row("Link", None, cmd(ch('k')), ctrl(ch('k'))),
            row(
                "Focus mode",
                Some("<Control>f"),
                Some(combo(ch('f'), true, false, true)),
                Some(combo(ch('f'), false, true, true)),
            ),
        ],
    ),
    (
        "Writing and reading",
        &[
            row(
                "Full screen",
                Some("F11"),
                Some(combo(ch('f'), true, true, false)),
                bare(Key::F(11)),
            ),
            row(
                "Leave full screen",
                Some("Escape"),
                bare(Key::Escape),
                bare(Key::Escape),
            ),
        ],
    ),
    (
        "Help",
        &[
            row("User manual", Some("F1"), cmd(ch('?')), bare(Key::F(1))),
            // The Mac lists its shortcuts in the menus instead.
            row(
                "Keyboard shortcuts",
                Some("<Control>question"),
                None,
                ctrl(ch('?')),
            ),
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn find(platform: Platform, title: &str) -> Option<Accel> {
        shortcuts(platform)
            .into_iter()
            .flat_map(|g| g.items)
            .find(|s| s.title == title)
            .map(|s| s.accel)
    }

    fn keys(platform: Platform, title: &str) -> KeyCombo {
        match find(platform, title) {
            Some(Accel::Keys(k)) => k,
            other => panic!("{title}: {other:?}"),
        }
    }

    #[test]
    fn linux_uses_gtk_accelerators() {
        let groups = shortcuts(Platform::Linux);
        let titles: Vec<&str> = groups.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Your shelves",
                "A book's page",
                "Writing",
                "Writing and reading",
                "Help"
            ]
        );
        assert_eq!(
            find(Platform::Linux, "Settings"),
            Some(Accel::Gtk("<Control>comma".into()))
        );
        assert_eq!(find(Platform::Linux, "Link"), None);
        for item in groups.iter().flat_map(|g| &g.items) {
            assert!(matches!(item.accel, Accel::Gtk(_)), "{}", item.title);
        }
    }

    #[test]
    fn mac_uses_command() {
        assert_eq!(
            keys(Platform::Mac, "Write or edit your summary"),
            combo(Key::Return, true, false, false)
        );
        assert_eq!(
            keys(Platform::Mac, "Focus mode"),
            combo(ch('f'), true, false, true)
        );
        assert_eq!(
            keys(Platform::Mac, "Full screen"),
            combo(ch('f'), true, true, false)
        );
        assert_eq!(
            keys(Platform::Mac, "User manual"),
            combo(ch('?'), true, false, false)
        );
        assert_eq!(find(Platform::Mac, "Keyboard shortcuts"), None);
        assert_eq!(
            keys(Platform::Mac, "Link"),
            combo(ch('k'), true, false, false)
        );
    }

    #[test]
    fn windows_uses_ctrl_and_function_keys() {
        assert_eq!(
            keys(Platform::Windows, "Write or edit your summary"),
            combo(ch('e'), false, true, false)
        );
        assert_eq!(
            keys(Platform::Windows, "Full screen"),
            combo(Key::F(11), false, false, false)
        );
        assert_eq!(
            keys(Platform::Windows, "User manual"),
            combo(Key::F(1), false, false, false)
        );
        assert_eq!(
            keys(Platform::Windows, "Keyboard shortcuts"),
            combo(ch('?'), false, true, false)
        );
        // Windows has no ⌘.
        for item in shortcuts(Platform::Windows).iter().flat_map(|g| &g.items) {
            assert!(
                matches!(item.accel, Accel::Keys(k) if !k.command),
                "{}",
                item.title
            );
        }
    }

    #[test]
    fn every_platform_lists_the_same_actions_in_the_same_order() {
        let titles = |p| -> Vec<String> {
            shortcuts(p)
                .into_iter()
                .flat_map(|g| g.items)
                .map(|s| s.title)
                .filter(|t| t != "Link" && t != "Keyboard shortcuts")
                .collect()
        };
        assert_eq!(titles(Platform::Linux), titles(Platform::Mac));
        assert_eq!(titles(Platform::Linux), titles(Platform::Windows));
    }
}
