//! Help: the user manual (MANUAL.md at the top of the repo, built into the
//! app) and the keyboard shortcuts window.

use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;

use crate::{markdown, Ctx};

/// The same file GitHub shows, so the two never drift apart.
pub const MANUAL: &str = include_str!("../../MANUAL.md");

/// Opens the manual (unless it's already the page showing).
pub fn open_manual(ctx: &Rc<Ctx>) {
    if ctx.nav.visible_page().and_then(|p| p.tag()).as_deref() == Some("manual") {
        return;
    }
    ctx.nav.push(&manual_page());
}

fn manual_page() -> adw::NavigationPage {
    let manual = parse_manual(MANUAL);

    // One label per section, so the Contents can scroll to each of them.
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(24)
        .margin_bottom(48)
        .margin_start(24)
        .margin_end(24)
        .build();
    column.append(&text_label(&manual.intro));
    let contents = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    contents.append(&text_label("## Contents"));
    column.append(&contents);
    let mut targets: HashMap<String, gtk::Label> = HashMap::new();
    for section in &manual.sections {
        let label = text_label(&section.markdown);
        column.append(&label);
        targets.insert(section.anchor.clone(), label);
    }

    let clamp = adw::Clamp::builder()
        .maximum_size(720)
        .child(&column)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&clamp)
        .vexpand(true)
        .focusable(true) // arrow keys and Page Up/Down scroll right away
        .build();

    // Scrolls so the section's heading sits at the top. Weak refs: the
    // buttons calling this live inside the scroller.
    let jump: Rc<dyn Fn(&str)> = {
        let scroll = scroll.downgrade();
        let clamp = clamp.downgrade();
        Rc::new(move |anchor: &str| {
            let (Some(scroll), Some(clamp)) = (scroll.upgrade(), clamp.upgrade()) else {
                return;
            };
            let Some(target) = targets.get(anchor) else {
                return;
            };
            if let Some(at) = target.compute_point(&clamp, &gtk::graphene::Point::new(0.0, 0.0)) {
                scroll.vadjustment().set_value(f64::from(at.y()) - 12.0);
                scroll.grab_focus();
            }
        })
    };

    let entry_button = |entry: &Entry| {
        let label = gtk::Label::builder()
            .label(&entry.title)
            .xalign(0.0)
            .wrap(true)
            .build();
        let button = gtk::Button::builder()
            .child(&label)
            .css_classes(["flat"])
            .build();
        let jump = jump.clone();
        let anchor = entry.anchor.clone();
        button.connect_clicked(move |_| jump(&anchor));
        button
    };
    for entry in &manual.contents {
        let button = entry_button(entry);
        if entry.nested {
            button.set_margin_start(24);
        }
        contents.append(&button);
    }

    // The same sections from the header, so there's no scrolling back up.
    let menu_items = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    let popover = gtk::Popover::builder().child(&menu_items).build();
    for entry in manual.contents.iter().filter(|e| !e.nested) {
        let button = entry_button(entry);
        let popover = popover.downgrade(); // the button lives inside it
        button.connect_clicked(move |_| {
            if let Some(p) = popover.upgrade() {
                p.popdown();
            }
        });
        menu_items.append(&button);
    }
    let contents_btn = gtk::MenuButton::builder()
        .label("Contents")
        .tooltip_text("Jump to a section")
        .popover(&popover)
        .build();
    let header = adw::HeaderBar::new();
    header.pack_end(&contents_btn);

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroll));

    let page = adw::NavigationPage::builder()
        .title("User Manual")
        .tag("manual")
        .child(&toolbar)
        .build();
    // Focus the scroller, not the text, so nothing starts out selected.
    page.connect_shown(move |_| {
        scroll.grab_focus();
    });
    page
}

fn text_label(markdown: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .use_markup(true)
        .wrap(true)
        .xalign(0.0)
        .yalign(0.0)
        .selectable(true)
        .css_classes(["preview-text", "reader-text"])
        .build();
    label.set_markup(&markdown::to_pango(markdown));
    label
}

/// The manual, split up for the in-app page.
struct Manual {
    /// The title and opening paragraphs.
    intro: String,
    /// The "Contents" list, read from the manual itself.
    contents: Vec<Entry>,
    /// Every `##`/`###` heading with the text under it (Contents left out).
    sections: Vec<Section>,
}

struct Entry {
    title: String,
    anchor: String,
    nested: bool,
}

struct Section {
    anchor: String,
    markdown: String,
}

fn parse_manual(md: &str) -> Manual {
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
/// means one Contents list works on GitHub and in the app.
fn github_anchor(title: &str, seen: &mut HashMap<String, usize>) -> String {
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

/// The standard GNOME "Keyboard Shortcuts" window. Keep it in step with the
/// handlers in main.rs (home, help), editor.rs, writer.rs and reader.rs.
pub fn show_shortcuts(parent: Option<&gtk::Window>) {
    let builder = gtk::Builder::from_string(SHORTCUTS_UI);
    let Some(window) = builder.object::<gtk::ShortcutsWindow>("shortcuts") else {
        return;
    };
    window.set_transient_for(parent);
    window.present();
}

const SHORTCUTS_UI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<interface>
  <object class="GtkShortcutsWindow" id="shortcuts">
    <property name="modal">1</property>
    <child>
      <object class="GtkShortcutsSection">
        <property name="section-name">bookshelf</property>
        <property name="max-height">10</property>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title">Your shelves</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Reading</property>
                <property name="accelerator">1</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Finished</property>
                <property name="accelerator">2</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Eventually</property>
                <property name="accelerator">3</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Add a book</property>
                <property name="accelerator">&lt;Control&gt;n</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Search your shelves</property>
                <property name="accelerator">&lt;Control&gt;f</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Settings</property>
                <property name="accelerator">&lt;Control&gt;comma</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title">A book's page</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Write or edit your summary</property>
                <property name="accelerator">e</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Read your summary</property>
                <property name="accelerator">r</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title">Writing</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Save now (it also saves as you type)</property>
                <property name="accelerator">&lt;Control&gt;s</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Bold</property>
                <property name="accelerator">&lt;Control&gt;b</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Italic</property>
                <property name="accelerator">&lt;Control&gt;i</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Focus mode</property>
                <property name="accelerator">&lt;Control&gt;f</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title">Writing and reading</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Full screen</property>
                <property name="accelerator">F11</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Leave full screen</property>
                <property name="accelerator">Escape</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title">Help</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">User manual</property>
                <property name="accelerator">F1</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="title">Keyboard shortcuts</property>
                <property name="accelerator">&lt;Control&gt;question</property>
              </object>
            </child>
          </object>
        </child>
      </object>
    </child>
  </object>
</interface>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// A markup mistake would make the whole manual page blank.
    #[test]
    fn the_manual_renders_as_valid_markup() {
        let markup = markdown::to_pango(MANUAL);
        gtk::pango::parse_markup(&markup, '\0').expect("valid Pango markup");
        assert!(markup.len() > 1000, "the manual is all there");
    }

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
        for section in &manual.sections {
            let markup = markdown::to_pango(&section.markdown);
            gtk::pango::parse_markup(&markup, '\0').expect(&section.anchor);
        }
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

    #[test]
    fn the_shortcuts_list_is_well_formed() {
        // Every accelerator in the window must parse, or GTK shows a blank key.
        for line in SHORTCUTS_UI.lines() {
            let Some(rest) = line.trim().strip_prefix(r#"<property name="accelerator">"#) else {
                continue;
            };
            let accel = rest.trim_end_matches("</property>");
            let name = accel.strip_prefix("&lt;Control&gt;").unwrap_or(accel);
            assert!(!name.contains("&lt;"), "only Ctrl is used: {accel}");
            assert!(
                gtk::gdk::Key::from_name(name).is_some(),
                "unknown key {name:?}"
            );
        }
    }
}
