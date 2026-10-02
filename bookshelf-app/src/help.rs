//! Help: the user manual (MANUAL.md at the top of the repo, built into the
//! app) and the keyboard shortcuts window.

use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::manual::{manual_for, Entry};
use bookshelf_core::shortcuts::{shortcuts, Accel, Platform};

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
    let manual = manual_for(MANUAL, Platform::Linux);

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

/// The standard GNOME "Keyboard Shortcuts" window, listing bookshelf-core's
/// Linux shortcuts.
pub fn show_shortcuts(parent: Option<&gtk::Window>) {
    let builder = gtk::Builder::from_string(&shortcuts_ui());
    let Some(window) = builder.object::<gtk::ShortcutsWindow>("shortcuts") else {
        return;
    };
    window.set_transient_for(parent);
    window.present();
}

/// The window's GtkBuilder XML, built from the shared Linux list.
fn shortcuts_ui() -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <interface>\n  \
         <object class=\"GtkShortcutsWindow\" id=\"shortcuts\">\n    \
         <property name=\"modal\">1</property>\n    \
         <child>\n      \
         <object class=\"GtkShortcutsSection\">\n        \
         <property name=\"section-name\">bookshelf</property>\n        \
         <property name=\"max-height\">10</property>\n",
    );
    for group in shortcuts(Platform::Linux) {
        xml.push_str(&format!(
            "        <child>\n          \
             <object class=\"GtkShortcutsGroup\">\n            \
             <property name=\"title\">{}</property>\n",
            xml_escape(&group.title)
        ));
        for item in &group.items {
            let Accel::Gtk(accel) = &item.accel else {
                continue;
            };
            xml.push_str(&format!(
                "            <child>\n              \
                 <object class=\"GtkShortcutsShortcut\">\n                \
                 <property name=\"title\">{}</property>\n                \
                 <property name=\"accelerator\">{}</property>\n              \
                 </object>\n            \
                 </child>\n",
                xml_escape(&item.title),
                xml_escape(accel)
            ));
        }
        xml.push_str("          </object>\n        </child>\n");
    }
    xml.push_str("      </object>\n    </child>\n  </object>\n</interface>\n");
    xml
}

/// Escapes text inside an XML element. Quotes may stay as they are there.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A markup mistake would make the whole manual page blank.
    #[test]
    fn the_manual_renders_as_valid_markup() {
        let markup = markdown::to_pango(&bookshelf_core::manual::platform_text(
            MANUAL,
            Platform::Linux,
        ));
        gtk::pango::parse_markup(&markup, '\0').expect("valid Pango markup");
        assert!(markup.len() > 1000, "the manual is all there");
    }

    /// Each section is its own label, so each must be valid on its own.
    /// (That every Contents entry finds its section is tested in
    /// bookshelf-core, next to the parser.)
    #[test]
    fn every_section_renders_as_valid_markup() {
        let manual = manual_for(MANUAL, Platform::Linux);
        assert!(manual.contents.len() > 10);
        for section in &manual.sections {
            let markup = markdown::to_pango(&section.markdown);
            gtk::pango::parse_markup(&markup, '\0').expect(&section.anchor);
        }
    }

    #[test]
    fn the_shortcuts_list_is_well_formed() {
        // Every accelerator in the window must parse, or GTK shows a blank key.
        let ui = shortcuts_ui();
        let mut count = 0;
        for line in ui.lines() {
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
            count += 1;
        }
        assert!(count > 10, "the accelerators are all there");
    }

    /// The window is generated from bookshelf-core's table now; it must be
    /// exactly the XML that used to be written out by hand.
    #[test]
    fn the_shortcuts_window_is_unchanged() {
        assert_eq!(shortcuts_ui(), OLD_SHORTCUTS_UI);
    }

    /// The hand-written window as it was before the table moved to
    /// bookshelf-core, frozen. Don't edit it.
    const OLD_SHORTCUTS_UI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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
}
