//! Per-profile settings. Every control saves and applies immediately.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::export;
use bookshelf_core::models::*;
use gtk::gio;

use crate::{email_row, plain_toast, theme, Ctx};

type State = Rc<RefCell<UserSettings>>;
type Options = &'static [(&'static str, &'static str)];

const THEMES: Options = &[("system", "Match system"), ("light", "Light"), ("dark", "Dark")];
const HEADING_FONTS: Options = &[("serif", "Serif"), ("sans", "Sans-serif")];
const WRITING_FONTS: Options = &[
    ("ia_duo", "iA Writer Duo (monospace)"),
    ("serif", "Source Serif"),
    ("sans", "System sans-serif"),
    ("mono", "System monospace"),
];
const SPACING: Options = &[("tight", "Tight"), ("normal", "Comfortable"), ("airy", "Airy")];
const WIDTHS: Options = &[("narrow", "Narrow"), ("medium", "Medium"), ("wide", "Wide")];
const DATE_FORMATS: Options = &[
    ("long", "Sep 20, 2026"),
    ("mm_dd_yyyy", "09/20/2026"),
    ("dd_mm_yyyy", "20/09/2026"),
    ("yyyy_mm_dd", "2026-09-20"),
];
const WEEK_START: Options = &[("sunday", "Sunday"), ("monday", "Monday")];
const START_TABS: Options = &[("reading", "Reading"), ("finished", "Finished"), ("eventually", "Eventually")];

/// Mutate, save, and apply in one go.
fn change(ctx: &Ctx, state: &State, f: impl FnOnce(&mut UserSettings)) {
    f(&mut state.borrow_mut());
    let s = state.borrow().clone();
    if let Err(e) = update_settings(&ctx.conn, &s) {
        eprintln!("could not save settings: {e}");
    }
    theme::apply(&s);
}

fn choice(
    ctx: &Rc<Ctx>,
    state: &State,
    title: &str,
    options: Options,
    current: &str,
    set: fn(&mut UserSettings, &str),
) -> adw::ComboRow {
    let labels: Vec<&str> = options.iter().map(|(_, label)| *label).collect();
    let row = adw::ComboRow::builder().title(title).build();
    row.set_model(Some(&gtk::StringList::new(&labels)));
    row.set_selected(options.iter().position(|(v, _)| *v == current).unwrap_or(0) as u32);

    let ctx = ctx.clone();
    let state = state.clone();
    row.connect_selected_notify(move |r| {
        if let Some((value, _)) = options.get(r.selected() as usize) {
            change(&ctx, &state, |s| set(s, value));
        }
    });
    row
}

pub fn settings_page(ctx: &Rc<Ctx>, user: &User) -> adw::NavigationPage {
    let initial = get_settings(&ctx.conn, &user.id).unwrap_or_else(|_| {
        let mut s = UserSettings::defaults();
        s.user_id = user.id.clone();
        s
    });
    let state: State = Rc::new(RefCell::new(initial.clone()));

    let page = adw::PreferencesPage::new();
    let overlay = adw::ToastOverlay::new();

    // ---- profile ------------------------------------------------------------
    let profile = adw::PreferencesGroup::builder().title("Profile").build();
    let name_row = adw::EntryRow::builder()
        .title("Name")
        .show_apply_button(true)
        .build();
    name_row.set_text(&user.name);
    name_row.connect_changed(|r| r.remove_css_class("error"));
    {
        let ctx = ctx.clone();
        let id = user.id.clone();
        name_row.connect_apply(move |row| match rename_user(&ctx.conn, &id, row.text().as_str()) {
            Ok(()) => row.remove_css_class("error"),
            Err(e) => {
                eprintln!("could not rename: {e}");
                row.add_css_class("error");
            }
        });
    }
    profile.add(&name_row);

    let email = email_row();
    email.set_show_apply_button(true);
    let current_email = get_user(&ctx.conn, &user.id).ok().and_then(|u| u.email);
    email.set_text(current_email.as_deref().unwrap_or(""));
    {
        let ctx = ctx.clone();
        let id = user.id.clone();
        email.connect_apply(move |row| match set_user_email(&ctx.conn, &id, row.text().as_str()) {
            Ok(()) => row.remove_css_class("error"),
            Err(e) => {
                eprintln!("could not save email: {e}");
                row.add_css_class("error");
            }
        });
    }
    profile.add(&email);
    page.add(&profile);

    // ---- appearance ---------------------------------------------------------
    let look = adw::PreferencesGroup::builder().title("Appearance").build();
    look.add(&choice(ctx, &state, "Theme", THEMES, &initial.theme, |s, v| s.theme = v.to_string()));
    look.add(&accent_row(ctx, &state, &initial.accent));
    look.add(&choice(
        ctx,
        &state,
        "Titles and summaries",
        HEADING_FONTS,
        &initial.heading_font,
        |s, v| s.heading_font = v.to_string(),
    ));
    page.add(&look);

    // ---- writing ------------------------------------------------------------
    let writing = adw::PreferencesGroup::builder().title("Writing").build();
    writing.add(&choice(
        ctx,
        &state,
        "Font",
        WRITING_FONTS,
        &initial.writing_font,
        |s, v| s.writing_font = v.to_string(),
    ));

    let size_row = adw::SpinRow::with_range(10.0, 28.0, 1.0);
    size_row.set_title("Text size");
    size_row.set_subtitle("Points");
    size_row.set_value(initial.writing_size as f64);
    {
        let ctx = ctx.clone();
        let state = state.clone();
        size_row.connect_value_notify(move |r| {
            let size = r.value().round() as i64;
            change(&ctx, &state, |s| s.writing_size = size);
        });
    }
    writing.add(&size_row);

    writing.add(&choice(ctx, &state, "Line spacing", SPACING, &initial.line_spacing, |s, v| {
        s.line_spacing = v.to_string()
    }));
    writing.add(&choice(ctx, &state, "Page width", WIDTHS, &initial.page_width, |s, v| {
        s.page_width = v.to_string()
    }));

    let focus_row = adw::SwitchRow::builder()
        .title("Start in focus mode")
        .subtitle("Dim everything except the sentence you're writing")
        .active(initial.focus_default)
        .build();
    {
        let ctx = ctx.clone();
        let state = state.clone();
        focus_row.connect_active_notify(move |r| {
            let on = r.is_active();
            change(&ctx, &state, |s| s.focus_default = on);
        });
    }
    writing.add(&focus_row);
    page.add(&writing);

    // ---- live preview -------------------------------------------------------
    let preview_group = adw::PreferencesGroup::builder().title("Preview").build();
    let preview = gtk::Label::builder()
        .label("The quiet hours between chapters are where the best thoughts arrive.")
        .wrap(true)
        .xalign(0.0)
        .margin_top(14)
        .margin_bottom(14)
        .margin_start(16)
        .margin_end(16)
        .css_classes(["writer-preview"])
        .build();
    let card = gtk::Box::builder().css_classes(["journal-page"]).build();
    card.append(&preview);
    preview_group.add(&card);
    page.add(&preview_group);

    // ---- reading log --------------------------------------------------------
    let log = adw::PreferencesGroup::builder().title("Reading log").build();
    log.add(&choice(ctx, &state, "Date format", DATE_FORMATS, &initial.date_format, |s, v| {
        s.date_format = v.to_string()
    }));
    log.add(&choice(ctx, &state, "Week starts on", WEEK_START, &initial.week_start, |s, v| {
        s.week_start = v.to_string()
    }));
    log.add(&choice(ctx, &state, "Open to", START_TABS, &initial.start_tab, |s, v| {
        s.start_tab = v.to_string()
    }));
    page.add(&log);

    // ---- your data ------------------------------------------------------------
    let data = adw::PreferencesGroup::builder().title("Your data").build();
    let export_row = adw::ActionRow::builder()
        .title("Export my summaries")
        .subtitle("Saves each one as a Markdown file in a folder you choose")
        .build();
    let export_btn = gtk::Button::builder()
        .label("Export…")
        .valign(gtk::Align::Center)
        .build();
    export_row.add_suffix(&export_btn);
    export_row.set_activatable_widget(Some(&export_btn));
    data.add(&export_row);
    page.add(&data);
    {
        let ctx = ctx.clone();
        let user_id = user.id.clone();
        let overlay = overlay.downgrade();
        export_btn.connect_clicked(move |btn| {
            let window = btn.root().and_downcast::<gtk::Window>();
            let dialog = gtk::FileDialog::builder().title("Export summaries to…").build();
            let ctx = ctx.clone();
            let user_id = user_id.clone();
            let overlay = overlay.clone();
            dialog.select_folder(window.as_ref(), gio::Cancellable::NONE, move |chosen| {
                let Ok(folder) = chosen else { return };
                let Some(path) = folder.path() else { return };
                let message =
                    match export::export_markdown(&ctx.conn, &user_id, &path.join("Bookshelf summaries")) {
                        Ok(n) => format!("Exported {n} summaries"),
                        Err(e) => format!("Export failed: {e}"),
                    };
                if let Some(overlay) = overlay.upgrade() {
                    overlay.add_toast(plain_toast(&message));
                }
            });
        });
    }

    // ---- danger zone --------------------------------------------------------
    let danger = adw::PreferencesGroup::new();
    let delete_btn = gtk::Button::builder()
        .label("Delete this profile…")
        .halign(gtk::Align::Start)
        .css_classes(["destructive-action"])
        .build();
    danger.add(&delete_btn);
    page.add(&danger);

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar.add_top_bar(&adw::HeaderBar::new());
    overlay.set_child(Some(&page));
    toolbar.set_content(Some(&overlay));

    let nav_page = adw::NavigationPage::builder()
        .title("Settings")
        .child(&toolbar)
        .build();

    {
        let ctx = ctx.clone();
        let id = user.id.clone();
        let name = user.name.clone();
        let anchor = nav_page.clone();
        delete_btn.connect_clicked(move |_| {
            let dialog = adw::AlertDialog::builder()
                .heading(format!("Delete {name}?"))
                .body("Their summaries, dates and settings will be deleted. The books themselves stay.")
                .build();
            dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete")]);
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");

            let ctx = ctx.clone();
            let id = id.clone();
            dialog.connect_response(None, move |_, response| {
                if response == "delete" && delete_user(&ctx.conn, &id).is_ok() {
                    ctx.nav.pop_to_tag("profiles");
                }
            });
            dialog.present(Some(&anchor));
        });
    }
    nav_page
}

/// A row of color swatches plus a custom color button.
fn accent_row(ctx: &Rc<Ctx>, state: &State, current: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title("Accent color").build();
    let holder = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .valign(gtk::Align::Center)
        .build();

    let marks: Rc<RefCell<Vec<gtk::Image>>> = Rc::new(RefCell::new(vec![]));
    let selected = theme::ACCENTS
        .iter()
        .position(|(_, hex)| hex.eq_ignore_ascii_case(current));

    // custom color (created first so the swatches can refer to it)
    let dialog = gtk::ColorDialog::new();
    dialog.set_with_alpha(false);
    let custom = gtk::ColorDialogButton::new(Some(dialog));
    custom.set_tooltip_text(Some("Custom color"));
    custom.set_valign(gtk::Align::Center);
    if let Ok(rgba) = gtk::gdk::RGBA::parse(current) {
        custom.set_rgba(&rgba);
    }

    for (i, (name, hex)) in theme::ACCENTS.iter().enumerate() {
        let mark = gtk::Image::from_icon_name("object-select-symbolic");
        mark.set_opacity(if selected == Some(i) { 1.0 } else { 0.0 });
        marks.borrow_mut().push(mark.clone());

        let btn = gtk::Button::builder()
            .child(&mark)
            .tooltip_text(*name)
            .css_classes(["circular", "swatch", &format!("swatch-{i}")])
            .build();
        let ctx = ctx.clone();
        let state = state.clone();
        let marks = marks.clone();
        let hex = hex.to_string();
        let custom = custom.clone();
        btn.connect_clicked(move |_| {
            for (j, m) in marks.borrow().iter().enumerate() {
                m.set_opacity(if j == i { 1.0 } else { 0.0 });
            }
            if let Ok(rgba) = gtk::gdk::RGBA::parse(hex.as_str()) {
                custom.set_rgba(&rgba);
            }
            let hex = hex.clone();
            change(&ctx, &state, |s| s.accent = hex);
        });
        holder.append(&btn);
    }

    {
        let ctx = ctx.clone();
        let state = state.clone();
        let marks = marks.clone();
        custom.connect_rgba_notify(move |b| {
            let c = b.rgba();
            let hex = format!(
                "#{:02x}{:02x}{:02x}",
                (c.red() * 255.0).round() as u8,
                (c.green() * 255.0).round() as u8,
                (c.blue() * 255.0).round() as u8
            );
            let already = state.borrow().accent.eq_ignore_ascii_case(&hex);
            if already {
                return; // the swatches set this same color; nothing to do
            }
            for m in marks.borrow().iter() {
                m.set_opacity(0.0);
            }
            change(&ctx, &state, |s| s.accent = hex);
        });
    }
    holder.append(&custom);
    row.add_suffix(&holder);
    row
}
