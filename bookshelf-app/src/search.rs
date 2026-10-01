use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::db;
use bookshelf_core::models::*;
use bookshelf_core::openlibrary::OpenLibrary;
use bookshelf_core::service;
use gtk::{gio, glib};

use crate::{editor, plain_toast, Ctx};

/// Everything the result rows need, cloned cheaply into callbacks.
#[derive(Clone)]
struct Ui {
    ctx: Rc<Ctx>,
    user: User,
    ol: OpenLibrary,
    list: gtk::ListBox,
    spinner: gtk::Spinner,
    overlay: adw::ToastOverlay,
}

pub fn search_page(ctx: &Rc<Ctx>, user: &User) -> adw::NavigationPage {
    let entry = gtk::SearchEntry::builder()
        .placeholder_text("Search by title, author or ISBN")
        .hexpand(true)
        .search_delay(400)
        .build();
    let spinner = gtk::Spinner::new();

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&entry));
    header.pack_end(&spinner);

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .valign(gtk::Align::Start)
        .build();
    list.set_placeholder(Some(
        &adw::StatusPage::builder()
            .icon_name("system-search-symbolic")
            .title("Find a book")
            .description("Results come from Open Library.")
            .build(),
    ));

    let clamp = adw::Clamp::builder()
        .maximum_size(720)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .child(&list)
        .build();
    let scroll = gtk::ScrolledWindow::builder().child(&clamp).build();
    let overlay = adw::ToastOverlay::new();
    overlay.set_child(Some(&scroll));

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&overlay));

    let page = adw::NavigationPage::builder()
        .title("Add a book")
        .child(&toolbar)
        .build();

    // Read fresh: the email may have been changed in settings since `user` was loaded.
    let contact = get_user(&ctx.conn, &user.id).ok().and_then(|u| u.email);
    let ui = Ui {
        ctx: ctx.clone(),
        user: user.clone(),
        ol: ctx.ol.with_contact(contact.as_deref()),
        list,
        spinner,
        overlay,
    };
    let generation = Rc::new(Cell::new(0u64));

    entry.connect_search_changed(move |entry| {
        let query = entry.text().trim().to_string();
        let gen = generation.get() + 1;
        generation.set(gen);

        if query.is_empty() {
            ui.list.remove_all();
            ui.spinner.set_spinning(false);
            return;
        }
        ui.spinner.set_spinning(true);

        let ui = ui.clone();
        let generation = generation.clone();
        glib::spawn_future_local(async move {
            // Network work happens on a worker thread, never the UI thread.
            let ol = ui.ol.clone();
            let result = gio::spawn_blocking(move || ol.search(&query).map_err(|e| e.to_string()))
                .await;

            if generation.get() != gen {
                return; // a newer search superseded this one
            }
            ui.spinner.set_spinning(false);
            ui.list.remove_all();
            match result {
                Ok(Ok(books)) => {
                    for book in books {
                        ui.list.append(&result_row(&ui, book));
                    }
                }
                Ok(Err(msg)) => ui.overlay.add_toast(plain_toast(&format!("Search failed: {msg}"))),
                Err(_) => {}
            }
        });
    });

    page
}

fn result_row(ui: &Ui, book: NewBook) -> adw::ActionRow {
    let subtitle = [book.author.clone(), book.published_date.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let row = adw::ActionRow::builder()
        .title(&book.title)
        .subtitle(&subtitle)
        .use_markup(false)
        .activatable(true)
        .build();
    row.add_suffix(&gtk::Image::from_icon_name("list-add-symbolic"));

    let ui = ui.clone();
    row.connect_activated(move |_| pick(&ui, book.clone()));
    row
}

/// User picked a result: save it (description + cover) on a worker thread,
/// then open its summary page. Books you haven't started land in "Eventually".
fn pick(ui: &Ui, picked: NewBook) {
    ui.list.set_sensitive(false);
    ui.spinner.set_spinning(true);

    let paths = ui.ctx.paths.clone();
    let ol = ui.ol.clone();
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let saved = gio::spawn_blocking(move || -> Result<Book, String> {
            let conn = db::open(&paths.db_path).map_err(|e| e.to_string())?;
            service::save_book_from_search(&conn, &ol, &paths, picked)
                .map_err(|e| e.to_string())
        })
        .await;

        ui.list.set_sensitive(true);
        ui.spinner.set_spinning(false);

        let book = match saved {
            Ok(Ok(book)) => book,
            Ok(Err(msg)) => {
                ui.overlay.add_toast(plain_toast(&format!("Could not save: {msg}")));
                return;
            }
            Err(_) => return,
        };

        let summary = find_user_summary_for_book(&ui.ctx.conn, &ui.user.id, &book.id)
            .and_then(|found| match found {
                Some(s) => Ok(s),
                None => create_summary(
                    &ui.ctx.conn,
                    &ui.user.id,
                    &book.id,
                    &SummaryInput::default(),
                ),
            });

        match summary {
            Ok(s) => {
                ui.ctx.nav.pop_to_tag("home");
                ui.ctx.nav.push(&editor::summary_page(&ui.ctx, &s.id));
            }
            Err(e) => ui.overlay.add_toast(plain_toast(&format!("Could not save: {e}"))),
        }
    });
}
