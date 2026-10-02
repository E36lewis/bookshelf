//! The user manual (MANUAL.md at the top of the repo), split up for the
//! in-app manual page: the intro, the Contents list and one section per
//! heading, so a view can jump to each. The apps build the file in and pass
//! its text here, so GitHub and every app show the same manual.

use std::collections::HashMap;

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

pub fn parse_manual(md: &str) -> Manual {
    let mut intro = String::new();
    let mut contents_md = String::new();
    let mut sections: Vec<Section> = vec![];
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut in_contents = false;

    for line in md.lines() {
        let heading = line
            .strip_prefix("## ")
            .or_else(|| line.strip_prefix("### "));
        if let Some(title) = heading {
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

    #[test]
    fn every_contents_entry_leads_to_its_section() {
        let manual = parse_manual(MANUAL);
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
        let headings = MANUAL
            .lines()
            .filter(|l| l.starts_with("## ") || l.starts_with("### "))
            .count();
        assert_eq!(manual.sections.len(), headings - 1);
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
