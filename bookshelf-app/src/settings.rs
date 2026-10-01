//! Per-profile settings. Every control saves and applies immediately.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::models::*;
use bookshelf_core::{backup, db, export};
use gtk::{gio, glib};

use crate::{display_path, email_row, friendly, plain_toast, theme, Ctx};

type State = Rc<RefCell<UserSettings>>;
type Options = &'static [(&'static str, &'static str)];

const THEMES: Options = &[
    ("system", "Match system"),
    ("light", "Light"),
    ("dark", "Dark"),
];
const HEADING_FONTS: Options = &[("serif", "Serif"), ("sans", "Sans-serif")];
const WRITING_FONTS: Options = &[
    ("ia_duo", "iA Writer Duo (monospace)"),
    ("serif", "Source Serif"),
    ("sans", "System sans-serif"),
    ("mono", "System monospace"),
];
const SPACING: Options = &[
    ("tight", "Tight"),
    ("normal", "Comfortable"),
    ("airy", "Airy"),
];
const WIDTHS: Options = &[("narrow", "Narrow"), ("medium", "Medium"), ("wide", "Wide")];
const DATE_FORMATS: Options = &[
    ("long", "Sep 20, 2026"),
    ("mm_dd_yyyy", "09/20/2026"),
    ("dd_mm_yyyy", "20/09/2026"),
    ("yyyy_mm_dd", "2026-09-20"),
];
const WEEK_START: Options = &[("sunday", "Sunday"), ("monday", "Monday")];
const START_TABS: Options = &[
    ("reading", "Reading"),
    ("finished", "Finished"),
    ("eventually", "Eventually"),
];

/// Mutate, save, and apply in one go.
fn change(ctx: &Ctx, state: &State, f: impl FnOnce(&mut UserSettings)) {
    f(&mut state.borrow_mut());
    let s = state.borrow().clone();
    if let Err(e) = update_settings(&ctx.conn, &s) {
        ctx.toasts.add_toast(plain_toast(&format!(
            "Couldn't save that setting: {}",
            friendly(&e)
        )));
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
        let overlay = overlay.downgrade();
        name_row.connect_apply(move |row| {
            let result = rename_user(&ctx.conn, &id, row.text().as_str());
            report(&overlay, row, result, "Name saved");
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
        let overlay = overlay.downgrade();
        email.connect_apply(move |row| {
            let result = set_user_email(&ctx.conn, &id, row.text().as_str());
            let done = if row.text().trim().is_empty() {
                "Email removed"
            } else {
                "Email saved"
            };
            report(&overlay, row, result, done);
        });
    }
    profile.add(&email);
    page.add(&profile);

    // ---- appearance ---------------------------------------------------------
    let look = adw::PreferencesGroup::builder().title("Appearance").build();
    look.add(&choice(
        ctx,
        &state,
        "Theme",
        THEMES,
        &initial.theme,
        |s, v| s.theme = v.to_string(),
    ));
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

    writing.add(&choice(
        ctx,
        &state,
        "Line spacing",
        SPACING,
        &initial.line_spacing,
        |s, v| s.line_spacing = v.to_string(),
    ));
    writing.add(&choice(
        ctx,
        &state,
        "Page width",
        WIDTHS,
        &initial.page_width,
        |s, v| s.page_width = v.to_string(),
    ));

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
    let log = adw::PreferencesGroup::builder()
        .title("Reading log")
        .build();
    log.add(&choice(
        ctx,
        &state,
        "Date format",
        DATE_FORMATS,
        &initial.date_format,
        |s, v| s.date_format = v.to_string(),
    ));
    log.add(&choice(
        ctx,
        &state,
        "Week starts on",
        WEEK_START,
        &initial.week_start,
        |s, v| s.week_start = v.to_string(),
    ));
    log.add(&choice(
        ctx,
        &state,
        "Open to",
        START_TABS,
        &initial.start_tab,
        |s, v| s.start_tab = v.to_string(),
    ));
    page.add(&log);

    // ---- your data ------------------------------------------------------------
    let data = adw::PreferencesGroup::builder().title("Your data").build();
    let export_row = adw::ActionRow::builder()
        .title("Export my summaries")
        .subtitle("Each summary becomes a Markdown file, in a “Bookshelf summaries” folder inside the folder you choose")
        .build();
    let export_btn = gtk::Button::builder()
        .label("Export…")
        .valign(gtk::Align::Center)
        .build();
    export_row.add_suffix(&export_btn);
    export_row.set_activatable_widget(Some(&export_btn));
    data.add(&export_row);
    data.add(&backups_row(ctx, &initial.date_format, &overlay));
    page.add(&data);
    {
        let ctx = ctx.clone();
        let user_id = user.id.clone();
        let overlay = overlay.downgrade();
        export_btn.connect_clicked(move |btn| export_to_folder(&ctx, &user_id, btn, &overlay));
    }

    // ---- help -------------------------------------------------------------------
    let help_group = adw::PreferencesGroup::builder().title("Help").build();
    let help_row = |title: &str, subtitle: &str, key: &str| {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(subtitle)
            .activatable(true)
            .build();
        row.add_suffix(
            &gtk::Label::builder()
                .label(key)
                .valign(gtk::Align::Center)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        row
    };
    let shortcuts_row = help_row("Keyboard Shortcuts", "Every key, on one page", "Ctrl+?");
    shortcuts_row.connect_activated(|row| {
        crate::help::show_shortcuts(row.root().and_downcast::<gtk::Window>().as_ref());
    });
    let manual_row = help_row("User Manual", "How everything works", "F1");
    {
        let ctx = ctx.clone();
        manual_row.connect_activated(move |_| crate::help::open_manual(&ctx));
    }
    help_group.add(&shortcuts_row);
    help_group.add(&manual_row);
    page.add(&help_group);

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
            // The name may have been changed on this page since it opened.
            let name = get_user(&ctx.conn, &id).map(|u| u.name).unwrap_or_else(|_| name.clone());
            let dialog = adw::AlertDialog::builder()
                .heading(format!("Delete {name}?"))
                .body("Their summaries, dates and settings will be deleted. The books themselves stay.\n\n\
                       Export first to keep a copy of everything they wrote.")
                .build();
            dialog.add_responses(&[
                ("cancel", "Cancel"),
                ("export", "Export first…"),
                ("delete", "Delete"),
            ]);
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");

            let ctx = ctx.clone();
            let id = id.clone();
            let near = anchor.clone();
            let overlay = overlay.downgrade();
            dialog.connect_response(None, move |_, response| match response {
                "export" => export_to_folder(&ctx, &id, &near, &overlay),
                "delete" => confirm_delete_by_name(&ctx, &id, &name, &near),
                _ => {}
            });
            dialog.present(Some(&anchor));
        });
    }
    nav_page
}

/// Marks the row and says what happened, in words.
fn report(
    overlay: &glib::WeakRef<adw::ToastOverlay>,
    row: &adw::EntryRow,
    result: bookshelf_core::Result<()>,
    done: &str,
) {
    let message = match result {
        Ok(()) => {
            row.remove_css_class("error");
            done.to_string()
        }
        Err(e) => {
            row.add_css_class("error");
            friendly(&e)
        }
    };
    if let Some(overlay) = overlay.upgrade() {
        overlay.add_toast(plain_toast(&message));
    }
}

/// Pick a folder, then write every summary of `user_id` into it as Markdown.
fn export_to_folder(
    ctx: &Rc<Ctx>,
    user_id: &str,
    near: &impl IsA<gtk::Widget>,
    overlay: &glib::WeakRef<adw::ToastOverlay>,
) {
    let window = near.root().and_downcast::<gtk::Window>();
    let dialog = gtk::FileDialog::builder()
        .title("Choose where to save the summaries")
        .accept_label("Export Here")
        .build();
    let ctx = ctx.clone();
    let user_id = user_id.to_string();
    let overlay = overlay.clone();
    dialog.select_folder(window.as_ref(), gio::Cancellable::NONE, move |chosen| {
        let Ok(folder) = chosen else { return };
        let Some(path) = folder.path() else { return };
        let target = path.join("Bookshelf summaries");
        let toast = match export::export_markdown(&ctx.conn, &user_id, &target) {
            Ok(n) => {
                let what = if n == 1 {
                    "1 summary".to_string()
                } else {
                    format!("{n} summaries")
                };
                let toast = adw::Toast::builder()
                    .title(format!("Exported {what} to {}", display_path(&target)))
                    .use_markup(false)
                    .button_label("Open")
                    .timeout(8)
                    .build();
                toast.connect_button_clicked(move |_| open_folder(&target));
                toast
            }
            Err(e) => plain_toast(&format!("Export failed: {}", friendly(&e))),
        };
        if let Some(overlay) = overlay.upgrade() {
            overlay.add_toast(toast);
        }
    });
}

/// Second step of deleting a profile: the name has to be typed, so a stray
/// click can't delete someone's journal.
fn confirm_delete_by_name(ctx: &Rc<Ctx>, id: &str, name: &str, near: &adw::NavigationPage) {
    let entry = gtk::Entry::builder().placeholder_text(name).build();
    entry.update_property(&[gtk::accessible::Property::Label("Profile name")]);
    let dialog = adw::AlertDialog::builder()
        .heading(format!("Delete {name} for good?"))
        .body(format!(
            "This can't be undone. All of {name}'s summaries and dates will be gone.\n\n\
             Type “{name}” to confirm."
        ))
        .extra_child(&entry)
        .build();
    dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete Forever")]);
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_response_enabled("delete", false);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    {
        let dialog = dialog.downgrade(); // the entry lives inside the dialog
        let name = name.trim().to_string();
        entry.connect_changed(move |e| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.set_response_enabled("delete", e.text().trim() == name);
            }
        });
    }
    let ctx = ctx.clone();
    let id = id.to_string();
    let name = name.to_string();
    dialog.connect_response(Some("delete"), move |_, _| {
        match delete_user(&ctx.conn, &id) {
            Ok(()) => {
                ctx.nav.pop_to_tag("profiles");
                ctx.toasts
                    .add_toast(plain_toast(&format!("Deleted {name}")));
            }
            Err(e) => ctx.toasts.add_toast(plain_toast(&friendly(&e))),
        }
    });
    dialog.present(Some(near));
    entry.grab_focus();
}

/// Opens a folder in the file manager.
fn open_folder(dir: &Path) {
    let dir = dir.to_path_buf();
    gtk::FileLauncher::new(Some(&gio::File::for_path(&dir))).launch(
        None::<&gtk::Window>,
        gio::Cancellable::NONE,
        move |result| {
            if let Err(e) = result {
                eprintln!("could not open {}: {e}", dir.display());
            }
        },
    );
}

/// Where backups go (with a way to change it), and when the last one was made.
fn backups_row(ctx: &Rc<Ctx>, date_format: &str, overlay: &adw::ToastOverlay) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title("Daily backups")
        .use_markup(false)
        .build();
    let reset_btn = gtk::Button::builder()
        .icon_name("edit-undo-symbolic")
        .tooltip_text("Use the default folder again")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let open_btn = gtk::Button::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text(format!(
            "Open the backup folder.\n\nTo restore a backup: quit Bookshelf, delete \
             bookshelf.sqlite3-wal and bookshelf.sqlite3-shm from {} if they're there, \
             then copy the backup over bookshelf.sqlite3 in that same folder.",
            display_path(&ctx.paths.data_dir)
        ))
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    let change_btn = gtk::Button::builder()
        .label("Change…")
        .tooltip_text(
            "Keep backups somewhere else, like a USB drive or a synced folder, \
                       so they survive if this computer's disk fails",
        )
        .valign(gtk::Align::Center)
        .build();
    row.add_suffix(&reset_btn);
    row.add_suffix(&open_btn);
    row.add_suffix(&change_btn);

    let refresh: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let row = row.downgrade();
        let reset_btn = reset_btn.downgrade();
        let date_format = date_format.to_string();
        Rc::new(move || {
            let (Some(row), Some(reset_btn)) = (row.upgrade(), reset_btn.upgrade()) else {
                return;
            };
            let Ok((dir, chosen)) = backup::folder(&ctx.conn, &ctx.paths) else {
                return;
            };
            reset_btn.set_visible(chosen);
            let place = display_path(&dir);
            let subtitle = if !dir.is_dir() {
                format!(
                    "Saved to {place}\nThat folder isn't available right now (is the drive \
                     plugged in?). Backups are paused until it's back, or until you choose \
                     another folder."
                )
            } else {
                let latest = backup::latest(&ctx.conn, &ctx.paths).ok().flatten();
                let when = match latest {
                    Some(d) => format!("Latest: {}", format_date(&date_format, d)),
                    None => "The first one is made the next time Bookshelf starts.".to_string(),
                };
                format!(
                    "Saved to {place}\nOne copy a day of every profile; the last {} are kept. {when}",
                    backup::KEEP
                )
            };
            row.set_subtitle(&subtitle);
        })
    };
    refresh();

    {
        let ctx = ctx.clone();
        let overlay = overlay.downgrade();
        open_btn.connect_clicked(move |_| match backup::folder(&ctx.conn, &ctx.paths) {
            Ok((dir, _)) if dir.is_dir() => open_folder(&dir),
            _ => toast(&overlay, "That folder isn't available right now."),
        });
    }
    {
        let ctx = ctx.clone();
        let overlay = overlay.downgrade();
        let refresh = refresh.clone();
        change_btn.connect_clicked(move |btn| {
            let window = btn.root().and_downcast::<gtk::Window>();
            let dialog = gtk::FileDialog::builder()
                .title("Choose where to keep backups")
                .accept_label("Keep Backups Here")
                .build();
            if let Ok((dir, _)) = backup::folder(&ctx.conn, &ctx.paths) {
                if dir.is_dir() {
                    dialog.set_initial_folder(Some(&gio::File::for_path(&dir)));
                }
            }
            let ctx = ctx.clone();
            let overlay = overlay.clone();
            let refresh = refresh.clone();
            dialog.select_folder(window.as_ref(), gio::Cancellable::NONE, move |chosen| {
                let Some(dir) = chosen.ok().and_then(|f| f.path()) else {
                    return;
                };
                match backup::set_folder(&ctx.conn, Some(&dir)) {
                    Ok(()) => back_up_now(
                        &ctx,
                        overlay,
                        refresh,
                        format!("Backups now go to {}", display_path(&dir)),
                    ),
                    Err(e) => toast(&overlay, &friendly(&e)),
                }
            });
        });
    }
    {
        let ctx = ctx.clone();
        let overlay = overlay.downgrade();
        reset_btn.connect_clicked(move |_| match backup::set_folder(&ctx.conn, None) {
            Ok(()) => back_up_now(
                &ctx,
                overlay.clone(),
                refresh.clone(),
                "Backups are back in the default folder".to_string(),
            ),
            Err(e) => toast(&overlay, &friendly(&e)),
        });
    }
    row
}

/// Makes today's backup in the (new) folder right away, on a worker thread.
fn back_up_now(
    ctx: &Ctx,
    overlay: glib::WeakRef<adw::ToastOverlay>,
    refresh: Rc<dyn Fn()>,
    done: String,
) {
    let paths = ctx.paths.clone();
    glib::spawn_future_local(async move {
        let made = gio::spawn_blocking(move || {
            db::open(&paths.db_path)
                .and_then(|conn| backup::daily(&conn, &paths, backup::KEEP))
                .map_err(|e| friendly(&e))
        })
        .await;
        refresh();
        let message = match made {
            Ok(Ok(_)) => done,
            Ok(Err(e)) => format!("Couldn't make a backup: {e}"),
            Err(_) => "Couldn't make a backup.".to_string(),
        };
        toast(&overlay, &message);
    });
}

fn toast(overlay: &glib::WeakRef<adw::ToastOverlay>, message: &str) {
    if let Some(overlay) = overlay.upgrade() {
        overlay.add_toast(plain_toast(message));
    }
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
            // Save first: moving the color button below fires its own
            // handler, which must see this color as already chosen.
            let chosen = hex.clone();
            change(&ctx, &state, |s| s.accent = chosen);
            if let Ok(rgba) = gtk::gdk::RGBA::parse(hex.as_str()) {
                custom.set_rgba(&rgba);
            }
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
