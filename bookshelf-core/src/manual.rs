//! The user manual (MANUAL.md at the top of the repo), split up for the
//! in-app manual page: the intro, the Contents list and one section per
//! heading, so a view can jump to each. The apps build the file in and pass
//! its text here, so GitHub and every app show the same manual.
//!
//! One file serves three apps, so what differs per platform (keys, menus,
//! folders) sits in platform blocks, which are HTML comments and so
//! invisible on GitHub:
//!
//! ```markdown
//! <!-- platform: mac -->
//! Press **⌘N**.
//! <!-- /platform -->
//! <!-- platform: linux, windows -->
//! Press **Ctrl+N**.
//! <!-- /platform -->
//! ```
//!
//! [`manual_for`] keeps the shared text and that platform's blocks, and
//! drops every other block and every comment line. GitHub shows all of it.
//! Headings stay outside blocks (see [`check_markup`]), so every platform
//! has the same Contents and the same anchors as GitHub.

use std::collections::HashMap;

use crate::shortcuts::{self, Platform};

/// The manual, split up for the in-app page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manual {
    /// The title and opening paragraphs.
    pub intro: String,
    /// The "Contents" list, read from the manual itself.
    pub contents: Vec<Entry>,
    /// Every `##`/`###` heading with the text under it (Contents left out).
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    pub anchor: String,
    /// A subsection, indented in the Contents.
    pub nested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub anchor: String,
    /// The heading line and everything up to the next heading.
    pub markdown: String,
}

/// The Linux manual, as the GTK app shows it: [`manual_for`] with
/// [`Platform::Linux`].
pub fn parse_manual(md: &str) -> Manual {
    manual_for(md, Platform::Linux)
}

/// The manual as one platform's app shows it: its own blocks kept, the
/// other platforms' blocks and all comment lines dropped, then split up.
pub fn manual_for(md: &str, platform: Platform) -> Manual {
    split(&platform_text(md, platform))
}

/// The manual's Markdown for one platform, before it's split up (see the
/// module docs). Lenient: a mistake in the markup never loses the manual;
/// [`check_markup`] is what catches it, in the tests.
pub fn platform_text(md: &str, platform: Platform) -> String {
    let mut out = String::new();
    let mut keep = true;
    // Something was left out since the last line kept: a blank line next
    // would double up the one before it.
    let mut skipped = false;
    for line in md.lines() {
        match marker(line) {
            Some(Marker::Open(names)) => {
                keep = names.contains(&platform_name(platform));
                skipped = true;
                continue;
            }
            Some(Marker::Close) => {
                keep = true;
                skipped = true;
                continue;
            }
            Some(Marker::Comment) => {
                skipped = true;
                continue;
            }
            None if !keep => {
                skipped = true;
                continue;
            }
            None => {}
        }
        let blank = line.trim().is_empty();
        if blank && skipped && (out.is_empty() || out.ends_with("\n\n")) {
            continue;
        }
        skipped &= blank;
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The name a platform has in `<!-- platform: … -->`.
fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Linux => "linux",
        Platform::Mac => "mac",
        Platform::Windows => "windows",
    }
}

const PLATFORM_NAMES: [&str; 3] = ["linux", "mac", "windows"];

enum Marker<'a> {
    /// `<!-- platform: mac, windows -->`
    Open(Vec<&'a str>),
    /// `<!-- /platform -->`
    Close,
    /// Any other comment on a line of its own: a note for whoever edits
    /// the manual, never shown in an app.
    Comment,
}

fn marker(line: &str) -> Option<Marker<'_>> {
    let inner = line
        .trim()
        .strip_prefix("<!--")?
        .strip_suffix("-->")?
        .trim();
    if inner == "/platform" {
        return Some(Marker::Close);
    }
    Some(match inner.strip_prefix("platform:") {
        Some(names) => Marker::Open(names.split(',').map(str::trim).collect()),
        None => Marker::Comment,
    })
}

/// Checks the platform markup, so a slip can't hide text from an app or
/// show a marker: blocks open and close on lines of their own, don't nest,
/// name only known platforms, and hold no headings (which would give the
/// platforms different Contents and anchors from GitHub's). Comments must
/// be whole lines too, so none is left half in the text.
pub fn check_markup(md: &str) -> Result<(), String> {
    let mut open: Option<usize> = None;
    for (i, line) in md.lines().enumerate() {
        let n = i + 1;
        match marker(line) {
            Some(Marker::Open(names)) => {
                if let Some(at) = open {
                    return Err(format!("line {n}: block inside the block from line {at}"));
                }
                if names.is_empty() || names.iter().any(|n| !PLATFORM_NAMES.contains(n)) {
                    return Err(format!(
                        "line {n}: platforms are {}: {line}",
                        PLATFORM_NAMES.join(", ")
                    ));
                }
                open = Some(n);
            }
            Some(Marker::Close) => {
                if open.take().is_none() {
                    return Err(format!("line {n}: closes a block that isn't open"));
                }
            }
            Some(Marker::Comment) if line.matches("<!--").count() > 1 => {
                return Err(format!("line {n}: one comment per line"));
            }
            Some(Marker::Comment) => {}
            None if line.contains("<!--") || line.contains("-->") => {
                return Err(format!("line {n}: comments go on lines of their own"));
            }
            None if open.is_some() && heading_title(line).is_some() => {
                return Err(format!("line {n}: headings go outside platform blocks"));
            }
            None => {}
        }
    }
    match open {
        Some(at) => Err(format!("line {at}: block never closed")),
        None => Ok(()),
    }
}

/// The keyboard shortcuts list as the manual writes it for one platform:
/// a bold title per group and a line per shortcut, from the same table the
/// apps' own lists come from. The manual's Keyboard shortcuts section must
/// contain this exactly (a test checks), so the two can't disagree.
pub fn shortcuts_markdown(platform: Platform) -> String {
    let mut out = String::new();
    for group in shortcuts::shortcuts(platform) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("**{}**\n\n", group.title));
        for item in &group.items {
            let keys = shortcuts::key_label(platform, &item.accel);
            out.push_str(&format!("- **{keys}**: {}\n", item.title));
        }
    }
    out
}

/// A `##` or `###` heading's title, the levels the manual is split at.
fn heading_title(line: &str) -> Option<&str> {
    line.strip_prefix("## ")
        .or_else(|| line.strip_prefix("### "))
}

/// Splits one platform's manual into the intro, Contents and sections.
fn split(md: &str) -> Manual {
    let mut intro = String::new();
    let mut contents_md = String::new();
    let mut sections: Vec<Section> = vec![];
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut in_contents = false;

    for line in md.lines() {
        if let Some(title) = heading_title(line) {
            let anchor = github_anchor(title, &mut seen);
            in_contents = line.starts_with("## ") && title.trim() == "Contents";
            if in_contents {
                continue;
            }
            sections.push(Section {
                anchor,
                markdown: String::new(),
            });
        }
        let target = if in_contents {
            &mut contents_md
        } else if let Some(section) = sections.last_mut() {
            &mut section.markdown
        } else {
            &mut intro
        };
        target.push_str(line);
        target.push('\n');
    }

    // "- [Title](#anchor)", indented when it's a subsection.
    let contents = contents_md
        .lines()
        .filter_map(|line| {
            let item = line.trim_start();
            let (title, rest) = item.strip_prefix("- [")?.split_once("](#")?;
            Some(Entry {
                title: title.to_string(),
                anchor: rest.strip_suffix(')')?.to_string(),
                nested: item.len() < line.len(),
            })
        })
        .collect();
    Manual {
        intro,
        contents,
        sections,
    }
}

/// The anchor GitHub gives a heading: lowercase, spaces to dashes,
/// punctuation dropped, and "-1", "-2", ... on repeats. Using the same rule
/// means one Contents list works on GitHub and in the app. `seen` carries
/// the repeats from one heading to the next.
pub fn github_anchor(title: &str, seen: &mut HashMap<String, usize>) -> String {
    let base: String = title
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect();
    let n = seen.entry(base.clone()).or_insert(0);
    let anchor = if *n == 0 { base } else { format!("{base}-{n}") };
    *n += 1;
    anchor
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANUAL: &str = include_str!("../../MANUAL.md");

    const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Mac, Platform::Windows];

    /// Everything, as GitHub shows it: every block, no markers.
    fn github_view(md: &str) -> String {
        md.lines()
            .filter(|l| marker(l).is_none())
            .map(|l| format!("{l}\n"))
            .collect()
    }

    #[test]
    fn every_contents_entry_leads_to_its_section() {
        for platform in PLATFORMS {
            contents_lead_to_sections(&platform_text(MANUAL, platform));
        }
        contents_lead_to_sections(&github_view(MANUAL));
    }

    fn contents_lead_to_sections(md: &str) {
        let manual = split(md);
        assert!(manual.contents.len() > 10);
        assert!(manual.intro.starts_with("# Bookshelf"));
        let anchors: Vec<&str> = manual.sections.iter().map(|s| s.anchor.as_str()).collect();
        for entry in &manual.contents {
            assert!(
                anchors.contains(&entry.anchor.as_str()),
                "no section for {:?}",
                entry.anchor
            );
            // and it's the heading the entry names, not a same-named one elsewhere
            let section = manual
                .sections
                .iter()
                .find(|s| s.anchor == entry.anchor)
                .unwrap();
            assert!(
                section
                    .markdown
                    .lines()
                    .next()
                    .unwrap()
                    .ends_with(&entry.title),
                "{}",
                entry.title
            );
        }
        // Contents itself isn't repeated in the sections, and nothing is lost.
        assert!(!anchors.contains(&"contents"));
        let headings = md.lines().filter(|l| heading_title(l).is_some()).count();
        assert_eq!(manual.sections.len(), headings - 1);
    }

    #[test]
    fn the_manual_markup_is_valid() {
        check_markup(MANUAL).unwrap();
    }

    #[test]
    fn markup_mistakes_are_caught() {
        let bad = [
            ("<!-- platform: mac -->\ntext", "never closed"),
            ("<!-- /platform -->", "isn't open"),
            (
                "<!-- platform: mac -->\n<!-- platform: linux -->\n<!-- /platform -->",
                "inside the block",
            ),
            (
                "<!-- platform: macos -->\n<!-- /platform -->",
                "platforms are",
            ),
            ("<!-- platform: -->\n<!-- /platform -->", "platforms are"),
            (
                "<!-- platform: mac -->\n## Mac only\n<!-- /platform -->",
                "headings go outside",
            ),
            ("Press <!-- platform: mac -->⌘N", "lines of their own"),
            ("<!-- a --> text <!-- b -->", "one comment per line"),
            ("<!-- a\nlong note -->", "lines of their own"),
        ];
        for (md, why) in bad {
            let err = check_markup(md).unwrap_err();
            assert!(err.contains(why), "{md:?}: {err}");
        }
        assert_eq!(
            check_markup(
                "## Shared\n  <!-- platform: mac, windows -->  \n#### Small heading\n\
                 <!-- /platform -->\n<!-- a note -->"
            ),
            Ok(())
        );
    }

    #[test]
    fn each_platform_keeps_its_own_blocks() {
        let md = "# Title\n\nShared.\n\n<!-- platform: mac -->\nMac.\n<!-- /platform -->\n\
                  <!-- platform: linux, windows -->\nPC.\n<!-- /platform -->\n\n\
                  <!-- platform: windows -->\n\nWindows.\n\n<!-- /platform -->\n\
                  <!-- TODO: a note -->\n\nAfter.\n";
        assert_eq!(
            platform_text(md, Platform::Mac),
            "# Title\n\nShared.\n\nMac.\n\nAfter.\n"
        );
        assert_eq!(
            platform_text(md, Platform::Linux),
            "# Title\n\nShared.\n\nPC.\n\nAfter.\n"
        );
        assert_eq!(
            platform_text(md, Platform::Windows),
            "# Title\n\nShared.\n\nPC.\n\nWindows.\n\nAfter.\n"
        );
        // Lines in a list stay one list once the markers go.
        let list = "1. One\n<!-- platform: mac -->\n2. Mac\n<!-- /platform -->\n\
                    <!-- platform: linux -->\n2. Linux\n<!-- /platform -->\n3. Three\n";
        assert_eq!(
            platform_text(list, Platform::Mac),
            "1. One\n2. Mac\n3. Three\n"
        );
        assert_eq!(platform_text(list, Platform::Windows), "1. One\n3. Three\n");
    }

    #[test]
    fn each_platforms_manual_is_whole_and_unmarked() {
        let github = split(&github_view(MANUAL));
        for platform in PLATFORMS {
            let manual = manual_for(MANUAL, platform);
            assert!(manual.intro.starts_with("# Bookshelf"), "{platform:?}");
            // The same Contents and anchors everywhere, GitHub's included.
            assert_eq!(manual.contents, github.contents, "{platform:?}");
            let anchors = |m: &Manual| -> Vec<String> {
                m.sections.iter().map(|s| s.anchor.clone()).collect()
            };
            assert_eq!(anchors(&manual), anchors(&github), "{platform:?}");
            let text = platform_text(MANUAL, platform);
            assert!(
                !text.contains("<!--") && !text.contains("-->"),
                "{platform:?}"
            );
            assert!(
                !text.contains("\n\n\n"),
                "{platform:?}: doubled blank lines"
            );
            for section in &manual.sections {
                assert!(
                    section.markdown.lines().count() > 1,
                    "{platform:?}: {} is empty",
                    section.anchor
                );
            }
        }
        assert_eq!(parse_manual(MANUAL), manual_for(MANUAL, Platform::Linux));
    }

    #[test]
    fn each_platform_is_told_its_own_keys_and_folders() {
        let linux = platform_text(MANUAL, Platform::Linux);
        let mac = platform_text(MANUAL, Platform::Mac);
        let windows = platform_text(MANUAL, Platform::Windows);
        for word in ["⌘", "%LOCALAPPDATA%", "Library/Containers", "Finder"] {
            assert!(!linux.contains(word), "Linux: {word}");
        }
        for word in [
            "Ctrl",
            "F11",
            "**F1**",
            "%LOCALAPPDATA%",
            ".local/share",
            "AppImage",
        ] {
            assert!(!mac.contains(word), "Mac: {word}");
        }
        for word in [
            "⌘",
            "Library/Containers",
            ".local/share",
            "AppImage",
            "Finder",
        ] {
            assert!(!windows.contains(word), "Windows: {word}");
        }
        assert!(linux.contains("`~/.local/share/bookshelf`"));
        assert!(windows.contains(r"`%LOCALAPPDATA%\Bookshelf`"));
        assert!(windows.contains(r"`%LOCALAPPDATA%\Bookshelf Preview`"));
        // The Mac's folder is in its sandbox container, named after the app id.
        use crate::paths::{APP_ID, PREVIEW_APP_ID};
        let container =
            format!("`~/Library/Containers/{APP_ID}/Data/Library/Application Support/{APP_ID}`");
        assert!(mac.contains(&container), "{container}");
        assert!(mac.contains(&format!("`{PREVIEW_APP_ID}`")));
        // Restoring a backup: every platform clears the WAL files first.
        for text in [&linux, &mac, &windows] {
            let restore = &text[text.find("**Restoring a backup:**").unwrap()..];
            let wal = restore.find("bookshelf.sqlite3-wal").unwrap();
            let copy = restore.find("over `bookshelf.sqlite3`").unwrap();
            assert!(wal < copy);
        }
    }

    #[test]
    fn the_shortcuts_section_matches_the_table() {
        for platform in PLATFORMS {
            let manual = manual_for(MANUAL, platform);
            let section = manual
                .sections
                .iter()
                .find(|s| s.anchor == "keyboard-shortcuts")
                .unwrap();
            let list = shortcuts_markdown(platform);
            assert!(
                section.markdown.contains(&list),
                "{platform:?}: the manual's Keyboard shortcuts must list, word for word:\n\n{list}"
            );
        }
    }

    #[test]
    fn the_shortcuts_list_is_written_per_platform() {
        let mac = shortcuts_markdown(Platform::Mac);
        assert!(mac.starts_with("**Your shelves**\n\n- **⌘1**: Reading\n"));
        assert!(mac.contains("- **⇧⌘F**: Focus mode\n"));
        assert!(!mac.contains("Keyboard shortcuts"));
        let windows = shortcuts_markdown(Platform::Windows);
        assert!(windows.contains("- **Ctrl+E**: Write or edit your summary\n"));
        assert!(windows.contains("\n\n**Help**\n\n- **F1**: User manual\n"));
        let linux = shortcuts_markdown(Platform::Linux);
        assert!(linux.contains("- **Ctrl+,**: Settings\n"));
        assert!(!linux.contains("Link"));
    }

    #[test]
    fn anchors_match_github() {
        let mut seen = HashMap::new();
        assert_eq!(github_anchor("A book's page", &mut seen), "a-books-page");
        assert_eq!(
            github_anchor("Your data, backups and privacy", &mut seen),
            "your-data-backups-and-privacy"
        );
        assert_eq!(github_anchor("Writing", &mut seen), "writing");
        assert_eq!(github_anchor("Writing", &mut seen), "writing-1");
    }
}
