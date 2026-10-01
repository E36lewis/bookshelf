//! The book page: what the book is, when you read it, and your summary as a
//! finished page. Writing itself happens on the full-screen writer page.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::models::*;
use gtk::glib;

use crate::date_picker::DatePicker;
use crate::{cover_picture, friendly, markdown, plain_toast, reader, writer, Ctx, PageKeys};

pub fn summary_page(ctx: &Rc<Ctx>, summary_id: &str) -> adw::NavigationPage {
    let loaded = get_summary(&ctx.conn, summary_id)
        .and_then(|s| get_book(&ctx.conn, &s.book_id).map(|b| (s, b)));
    let (summary, book) = match loaded {
        Ok(pair) => pair,
        Err(e) => return error_page(&e.to_string()),
    };
    let settings =
        get_settings(&ctx.conn, &summary.user_id).unwrap_or_else(|_| UserSettings::defaults());

    // ---- book info block ------------------------------------------------
    let cover = cover_picture(ctx, &book, 120, 180);

    let info = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .valign(gtk::Align::Start)
        .hexpand(true)
        .build();
    info.append(
        &gtk::Label::builder()
            .label(&book.title)
            .wrap(true)
            .xalign(0.0)
            .selectable(true)
            .css_classes(["book-title"])
            .build(),
    );
    if let Some(sub) = &book.subtitle {
        info.append(&styled_label(sub, "book-subtitle"));
    }
    if let Some(author) = &book.author {
        info.append(&styled_label(author, "book-author"));
    }
    let mut meta: Vec<String> = vec![];
    if let Some(p) = &book.publisher {
        meta.push(p.clone());
    }
    if let Some(d) = &book.published_date {
        meta.push(d.clone());
    }
    if let Some(n) = book.page_count {
        meta.push(format!("{n} pages"));
    }
    if !meta.is_empty() {
        info.append(&styled_label(&meta.join(" · "), "book-meta"));
    }

    let top = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(16)
        .build();
    top.append(&cover);
    top.append(&info);

    // ---- description ----------------------------------------------------
    let about = adw::PreferencesGroup::new();
    if let Some(desc) = book.description.as_deref().filter(|d| !d.trim().is_empty()) {
        let expander = adw::ExpanderRow::builder().title("About this book").build();
        expander.add_row(
            &gtk::Label::builder()
                .label(desc)
                .wrap(true)
                .xalign(0.0)
                .selectable(true)
                .margin_top(12)
                .margin_bottom(12)
                .margin_start(12)
                .margin_end(12)
                .build(),
        );
        about.add(&expander);
    }

    // ---- dates: calendar pickers, saved the moment you pick --------------
    let overlay = adw::ToastOverlay::new();
    let dates = adw::PreferencesGroup::builder()
        .title("Reading dates")
        .build();
    dates.set_description(days_text(summary.days_to_complete).as_deref());

    let started = Rc::new(Cell::new(summary.started_on));
    let finished = Rc::new(Cell::new(summary.finished_on));

    let persist: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let id = summary.id.clone();
        let started = started.clone();
        let finished = finished.clone();
        let overlay = overlay.downgrade();
        let dates = dates.downgrade();
        Rc::new(move || {
            let result = get_summary(&ctx.conn, &id).and_then(|cur| {
                update_summary(
                    &ctx.conn,
                    &id,
                    &SummaryInput {
                        body: cur.body,
                        started_on: started.get(),
                        finished_on: finished.get(),
                    },
                )
            });
            match result {
                Ok(saved) => {
                    if let Some(group) = dates.upgrade() {
                        group.set_description(days_text(saved.days_to_complete).as_deref());
                    }
                    toast(&overlay, "Saved");
                }
                Err(e) => toast(&overlay, &e.to_string()),
            }
        })
    };

    let started_picker = DatePicker::new(
        "Started",
        summary.started_on,
        &settings.date_format,
        &settings.week_start,
        {
            // can't start after you finished
            let finished = finished.clone();
            move |d| finished.get().is_none_or(|f| d <= f)
        },
        {
            let started = started.clone();
            let persist = persist.clone();
            move |d| {
                started.set(d);
                persist();
            }
        },
    );
    let finished_picker = DatePicker::new(
        "Finished",
        summary.finished_on,
        &settings.date_format,
        &settings.week_start,
        {
            // can't finish before you started
            let started = started.clone();
            move |d| started.get().is_none_or(|s| d >= s)
        },
        {
            let finished = finished.clone();
            let persist = persist.clone();
            move |d| {
                finished.set(d);
                persist();
            }
        },
    );
    dates.add(started_picker.row());
    dates.add(finished_picker.row());

    // ---- summary, as a page ---------------------------------------------
    let summary_heading = gtk::Label::builder()
        .label("My summary")
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["section-title"])
        .build();
    let write_btn = gtk::Button::builder()
        .label("Write")
        .valign(gtk::Align::Center)
        .tooltip_text("Write or edit your summary (E)")
        .css_classes(["suggested-action"])
        .build();
    let read_btn = gtk::Button::builder()
        .label("Read")
        .valign(gtk::Align::Center)
        .tooltip_text("Distraction-free reading, with full screen (R)")
        .css_classes(["flat"])
        .build();
    let summary_head = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    summary_head.append(&summary_heading);
    summary_head.append(&read_btn);
    summary_head.append(&write_btn);

    let preview = gtk::Label::builder()
        .use_markup(true)
        .wrap(true)
        .xalign(0.0)
        .yalign(0.0)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .css_classes(["preview-text"])
        .build();
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["journal-page"])
        .build();
    card.append(&preview);
    card.set_cursor_from_name(Some("pointer"));
    refresh_preview(ctx, &summary.id, &preview, &write_btn, &read_btn);

    // ---- assemble page --------------------------------------------------
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(18)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    column.append(&top);
    column.append(&about);
    column.append(&dates);
    column.append(&summary_head);
    column.append(&card);

    let clamp = adw::Clamp::builder()
        .maximum_size(720)
        .child(&column)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&clamp)
        .focusable(true) // takes the focus when the page opens (see `shown`)
        .build();
    overlay.set_child(Some(&scroll));

    let again_btn = gtk::Button::builder()
        .label("Read it again")
        .tooltip_text("Start a new entry for this book, with its own dates and thoughts")
        .css_classes(["flat"])
        .build();
    let remove_btn = gtk::Button::builder()
        .label("Remove from my shelf")
        .css_classes(["flat"])
        .build();
    let menu_items = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    menu_items.append(&again_btn);
    menu_items.append(&remove_btn);
    let popover = gtk::Popover::builder().child(&menu_items).build();
    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .popover(&popover)
        .build();

    let header = adw::HeaderBar::new();
    header.pack_end(&menu);

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&overlay));

    let page = adw::NavigationPage::builder()
        .title(&book.title)
        .child(&toolbar)
        .build();

    // ---- open the writer -------------------------------------------------
    let open_writer: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let id = summary.id.clone();
        Rc::new(move || ctx.nav.push(&writer::writer_page(&ctx, &id)))
    };
    {
        let open = open_writer.clone();
        write_btn.connect_clicked(move |_| open());
    }
    {
        let open = open_writer.clone();
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| open());
        card.add_controller(click);
    }

    {
        let ctx = ctx.clone();
        let id = summary.id.clone();
        read_btn.connect_clicked(move |_| ctx.nav.push(&reader::reader_page(&ctx, &id)));
    }

    // E to write, R to read (only when there's something to read).
    let keys = PageKeys::new(ctx, &page);
    {
        let open = open_writer.clone();
        keys.add(
            gtk::gdk::Key::e,
            gtk::gdk::ModifierType::empty(),
            move || {
                open();
                true
            },
        );
    }
    {
        let read_btn = read_btn.clone();
        keys.add(
            gtk::gdk::Key::r,
            gtk::gdk::ModifierType::empty(),
            move || {
                if !read_btn.is_visible() {
                    return false;
                }
                read_btn.emit_clicked();
                true
            },
        );
    }
    keys.attach(&toolbar);

    // Coming back from the writer or reader: show the latest text.
    {
        let ctx = ctx.clone();
        let id = summary.id.clone();
        let preview = preview.clone();
        let write_btn = write_btn.clone();
        let read_btn = read_btn.clone();
        page.connect_showing(move |_| refresh_preview(&ctx, &id, &preview, &write_btn, &read_btn));
    }
    // Start with the focus on the page itself, not the selectable title,
    // which would show a text cursor. Arrow keys then scroll right away.
    {
        let scroll = scroll.clone();
        page.connect_shown(move |_| {
            scroll.grab_focus();
        });
    }

    // ---- read it again: a fresh entry, started today --------------------------
    {
        let ctx = ctx.clone();
        let user_id = summary.user_id.clone();
        let book_id = book.id.clone();
        let popover = popover.downgrade(); // these buttons live inside it
        let overlay = overlay.downgrade();
        again_btn.connect_clicked(move |_| {
            if let Some(p) = popover.upgrade() {
                p.popdown();
            }
            let input = SummaryInput {
                started_on: Some(chrono::Local::now().date_naive()),
                ..Default::default()
            };
            match create_summary(&ctx.conn, &user_id, &book_id, &input) {
                Ok(new) => {
                    ctx.nav.pop();
                    ctx.nav.push(&summary_page(&ctx, &new.id));
                    ctx.toasts
                        .add_toast(plain_toast("New reading started today"));
                }
                Err(e) => toast(&overlay, &friendly(&e)),
            }
        });
    }

    // ---- remove: right away, with a way back --------------------------------
    {
        let ctx = ctx.clone();
        let id = summary.id.clone();
        let title = book.title.clone();
        let popover = popover.downgrade(); // these buttons live inside it
        let overlay = overlay.downgrade();
        remove_btn.connect_clicked(move |_| {
            if let Some(p) = popover.upgrade() {
                p.popdown();
            }
            // Keep the whole entry so Undo can put it back exactly as it was.
            let removed = get_summary(&ctx.conn, &id).and_then(|s| {
                delete_summary(&ctx.conn, &id)?;
                Ok(s)
            });
            let removed = match removed {
                Ok(s) => s,
                Err(e) => return toast(&overlay, &friendly(&e)),
            };
            ctx.nav.pop();

            let undo = adw::Toast::builder()
                .title(format!("Removed “{title}”"))
                .use_markup(false)
                .button_label("Undo")
                .timeout(10)
                .build();
            let ctx_undo = ctx.clone();
            undo.connect_button_clicked(move |_| match restore_summary(&ctx_undo.conn, &removed) {
                Ok(()) => ctx_undo.refresh_home(),
                Err(e) => ctx_undo
                    .toasts
                    .add_toast(plain_toast(&format!("Couldn't undo: {}", friendly(&e)))),
            });
            ctx.toasts.add_toast(undo);
        });
    }

    page
}

fn toast(overlay: &glib::WeakRef<adw::ToastOverlay>, message: &str) {
    if let Some(overlay) = overlay.upgrade() {
        overlay.add_toast(plain_toast(message));
    }
}

fn days_text(days: Option<i64>) -> Option<String> {
    days.map(|n| match n {
        0 => "Finished the same day".to_string(),
        1 => "Finished in 1 day".to_string(),
        n => format!("Finished in {n} days"),
    })
}

fn refresh_preview(
    ctx: &Ctx,
    id: &str,
    preview: &gtk::Label,
    write_btn: &gtk::Button,
    read_btn: &gtk::Button,
) {
    let body = get_summary(&ctx.conn, id)
        .map(|s| s.body)
        .unwrap_or_default();
    if body.trim().is_empty() {
        preview.set_markup(
            "<span alpha=\"55%\"><i>Nothing written yet. Tap Write to begin.</i></span>",
        );
        write_btn.set_label("Write");
        read_btn.set_visible(false); // nothing to read yet
    } else {
        preview.set_markup(&markdown::to_pango(&body));
        write_btn.set_label("Edit");
        read_btn.set_visible(true);
    }
}

fn styled_label(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .selectable(true)
        .css_classes([class])
        .build()
}

pub(crate) fn error_page(message: &str) -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(
        &adw::StatusPage::builder()
            .icon_name("dialog-error-symbolic")
            .title("Could not open this book")
            .description(glib::markup_escape_text(message)) // the description is markup
            .build(),
    ));
    adw::NavigationPage::builder()
        .title("Error")
        .child(&toolbar)
        .build()
}
