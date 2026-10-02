//! Distraction-free reading view of a summary. In full screen every bit of
//! chrome disappears; Esc or F11 brings you back.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::models::*;
use bookshelf_core::writer::WriterLayout;
use gtk::glib;

use crate::{editor, markdown, writer, Ctx};

pub fn reader_page(ctx: &Rc<Ctx>, summary_id: &str) -> adw::NavigationPage {
    let loaded = get_summary(&ctx.conn, summary_id)
        .and_then(|s| get_book(&ctx.conn, &s.book_id).map(|b| (s, b)));
    let (summary, book) = match loaded {
        Ok(pair) => pair,
        Err(e) => return editor::error_page(&e.to_string()),
    };
    let settings =
        get_settings(&ctx.conn, &summary.user_id).unwrap_or_else(|_| UserSettings::defaults());
    let column_width = WriterLayout::from_settings(&settings).column_width;

    // ---- the text ---------------------------------------------------------
    let title = gtk::Label::builder()
        .label(&book.title)
        .wrap(true)
        .xalign(0.0)
        .selectable(true)
        .css_classes(["book-title"])
        .build();
    let body = gtk::Label::builder()
        .use_markup(true)
        .wrap(true)
        .xalign(0.0)
        .yalign(0.0)
        .selectable(true)
        .margin_top(28)
        .css_classes(["preview-text", "reader-text"])
        .build();
    if summary.body.trim().is_empty() {
        body.set_markup("<span alpha=\"55%\"><i>Nothing written yet.</i></span>");
    } else {
        body.set_markup(&markdown::to_pango(&summary.body));
    }

    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(56)
        .margin_bottom(160)
        .margin_start(24)
        .margin_end(24)
        .build();
    column.append(&title);
    if let Some(author) = &book.author {
        column.append(
            &gtk::Label::builder()
                .label(author)
                .wrap(true)
                .xalign(0.0)
                .css_classes(["book-author"])
                .build(),
        );
    }
    column.append(&body);

    let clamp = adw::Clamp::builder()
        .maximum_size(column_width)
        .tightening_threshold(column_width - 120)
        .child(&column)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&clamp)
        .vexpand(true)
        .focusable(true) // arrow keys, space and Page Up/Down scroll right away
        .css_classes(["reader-surface"])
        .build();
    let overlay = adw::ToastOverlay::new();
    overlay.set_child(Some(&scroll));

    // ---- header -------------------------------------------------------------
    let full_btn = gtk::Button::builder()
        .icon_name("view-fullscreen-symbolic")
        .tooltip_text("Full screen (F11)")
        .build();
    let edit_btn = gtk::Button::builder()
        .icon_name("document-edit-symbolic")
        .tooltip_text("Edit this summary")
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(&book.title, "Reading")));
    header.pack_end(&full_btn);
    header.pack_end(&edit_btn);

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&overlay));
    toolbar.add_css_class("reader-page");

    let page = adw::NavigationPage::builder()
        .title(&book.title)
        .child(&toolbar)
        .build();

    {
        let ctx = ctx.clone();
        let id = summary.id.clone();
        edit_btn.connect_clicked(move |_| {
            ctx.nav.pop();
            ctx.nav.push(&writer::writer_page(&ctx, &id));
        });
    }

    // ---- full screen --------------------------------------------------------
    let toggle: Rc<dyn Fn()> = {
        let toolbar = toolbar.downgrade();
        let button = full_btn.downgrade();
        let overlay = overlay.downgrade();
        Rc::new(move || {
            let Some(toolbar) = toolbar.upgrade() else {
                return;
            };
            let Some(window) = toolbar.root().and_downcast::<gtk::Window>() else {
                return;
            };
            let entering = !window.is_fullscreen();
            if entering {
                window.fullscreen();
            } else {
                window.unfullscreen();
            }
            if let Some(button) = button.upgrade() {
                button.set_icon_name(if entering {
                    "view-restore-symbolic"
                } else {
                    "view-fullscreen-symbolic"
                });
            }
            if entering {
                if let Some(overlay) = overlay.upgrade() {
                    let hint = adw::Toast::new("Press Esc to leave full screen");
                    hint.set_timeout(2);
                    overlay.add_toast(hint);
                }
            }
        })
    };
    {
        let toggle = toggle.clone();
        full_btn.connect_clicked(move |_| toggle());
    }

    // The header disappears whenever the window is full screen, however it got
    // there (our button, F11, or the window manager), and returns afterwards.
    let sync: Rc<RefCell<Option<(gtk::Window, glib::SignalHandlerId)>>> = Rc::default();
    {
        let sync = sync.clone();
        let toolbar = toolbar.downgrade();
        page.connect_showing(move |p| {
            if sync.borrow().is_some() {
                return;
            }
            let Some(window) = p.root().and_downcast::<gtk::Window>() else {
                return;
            };
            let toolbar = toolbar.clone();
            let id = window.connect_fullscreened_notify(move |w| {
                if let Some(toolbar) = toolbar.upgrade() {
                    toolbar.set_reveal_top_bars(!w.is_fullscreen());
                }
            });
            *sync.borrow_mut() = Some((window, id));
        });
    }
    page.connect_hiding(|p| {
        // Leaving the reader leaves full screen too.
        if let Some(window) = p.root().and_downcast::<gtk::Window>() {
            window.unfullscreen();
        }
    });
    {
        let sync = sync.clone();
        page.connect_hidden(move |_| {
            if let Some((window, id)) = sync.borrow_mut().take() {
                window.disconnect(id);
            }
        });
    }

    // ---- hotkeys (same keys as the writing page) -----------------------------
    let shortcuts = gtk::ShortcutController::new();
    shortcuts.set_scope(gtk::ShortcutScope::Local);
    shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture);
    let plain = gtk::gdk::ModifierType::empty();
    writer::add_shortcut(&shortcuts, gtk::gdk::Key::F11, plain, {
        let toggle = toggle.clone();
        move || {
            toggle();
            true
        }
    });
    writer::add_shortcut(&shortcuts, gtk::gdk::Key::Escape, plain, {
        let toolbar = toolbar.downgrade();
        let toggle = toggle.clone();
        move || {
            let fullscreen = toolbar
                .upgrade()
                .and_then(|t| t.root())
                .and_downcast::<gtk::Window>()
                .is_some_and(|w| w.is_fullscreen());
            if fullscreen {
                toggle();
            }
            fullscreen // otherwise Escape behaves as usual
        }
    });
    toolbar.add_controller(shortcuts);

    {
        let scroll = scroll.clone();
        page.connect_shown(move |_| {
            scroll.grab_focus();
        });
    }
    page
}
