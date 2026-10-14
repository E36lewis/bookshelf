mod date_picker;
mod editor;
mod format;
mod help;
mod markdown;
mod reader;
mod search;
mod settings;
mod shelf_grid;
#[cfg(debug_assertions)]
mod shots;
mod theme;
mod writer;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::models::*;
use bookshelf_core::openlibrary::OpenLibrary;
use bookshelf_core::paths::AppPaths;
use bookshelf_core::present::{self, ShelfEntry, ShelfRow, YearHeading};
use bookshelf_core::rusqlite::Connection;
use bookshelf_core::{backup, db};
use gtk::{gdk, gio, glib};

/// Debug builds get their own id, so a development copy can run next to the
/// installed app instead of handing off to it.
const APP_ID: &str = if cfg!(debug_assertions) {
    "com.bookshelf.Bookshelf.Devel"
} else {
    "com.bookshelf.Bookshelf"
};

/// Shared app state. The UI thread owns this connection; worker threads
/// (search, cover downloads) open their own. The Open Library client is
/// shared by everyone (it's a cheap clone) so connections get reused.
pub(crate) struct Ctx {
    pub(crate) conn: Connection,
    pub(crate) paths: AppPaths,
    pub(crate) nav: adw::NavigationView,
    pub(crate) ol: OpenLibrary,
    /// App-wide toasts: they outlive the page that raised them ("Removed · Undo").
    pub(crate) toasts: adw::ToastOverlay,
    /// Reloads the home lists. Set by the home page; used after an Undo.
    pub(crate) home_refresher: RefCell<Option<Rc<dyn Fn()>>>,
    /// Decoded list-size covers, by file name. Covers never change once saved.
    pub(crate) covers: RefCell<HashMap<String, gdk::Texture>>,
}

impl Ctx {
    pub(crate) fn refresh_home(&self) {
        let refresh = self.home_refresher.borrow().clone();
        if let Some(refresh) = refresh {
            refresh();
        }
    }
}

fn main() -> glib::ExitCode {
    bookshelf_core::logging::log_to_stderr();
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        theme::install();
        // Titles, book details and the manual are selectable so you can copy
        // from them. By default GTK selects all of a selectable label's text
        // when it gets keyboard focus, which happens as a page opens and
        // leaves the text highlighted. Keep the copying, drop the highlight.
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_label_select_on_focus(false);
        }
    });
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    // Launched again while running: bring the open window forward instead
    // of opening a second one.
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Bookshelf")
        .default_width(1000)
        .default_height(900)
        .build();

    match open_journal() {
        Ok((paths, conn, ol)) => {
            let nav = adw::NavigationView::new();
            let toasts = adw::ToastOverlay::new();
            toasts.set_child(Some(&nav));
            let ctx = Rc::new(Ctx {
                conn,
                paths,
                nav: nav.clone(),
                ol,
                toasts: toasts.clone(),
                home_refresher: RefCell::default(),
                covers: RefCell::default(),
            });
            nav.push(&profiles_page(&ctx));
            window.set_content(Some(&toasts));
            #[cfg(debug_assertions)]
            shots::run(&ctx, &window);

            // Help from anywhere: F1 the manual, Ctrl+? every shortcut.
            let help_keys = gtk::ShortcutController::new();
            help_keys.set_scope(gtk::ShortcutScope::Global);
            {
                let ctx = Rc::downgrade(&ctx);
                writer::add_shortcut(
                    &help_keys,
                    gdk::Key::F1,
                    gdk::ModifierType::empty(),
                    move || {
                        if let Some(ctx) = ctx.upgrade() {
                            help::open_manual(&ctx);
                        }
                        true
                    },
                );
            }
            {
                let window = window.downgrade();
                writer::add_shortcut(
                    &help_keys,
                    gdk::Key::question,
                    gdk::ModifierType::CONTROL_MASK,
                    move || {
                        help::show_shortcuts(
                            window.upgrade().map(|w| w.upcast::<gtk::Window>()).as_ref(),
                        );
                        true
                    },
                );
            }
            window.add_controller(help_keys);

            // Today's copy now, and an hourly check (still one copy a day)
            // so it keeps happening if Bookshelf stays open for days.
            back_up_in_background(&ctx.paths);
            let paths = ctx.paths.clone();
            glib::timeout_add_seconds_local(60 * 60, move || {
                back_up_in_background(&paths);
                glib::ControlFlow::Continue
            });

            // On quit, fold the write-ahead log into the journal so nothing
            // stale is left beside it (it would be replayed onto a restored backup).
            let weak = Rc::downgrade(&ctx);
            app.connect_shutdown(move |_| {
                if let Some(ctx) = weak.upgrade() {
                    if let Err(e) = db::checkpoint(&ctx.conn) {
                        eprintln!("[bookshelf] could not tidy up the journal on quit: {e}");
                    }
                }
            });
        }
        Err(message) => {
            eprintln!("[bookshelf] {message}");
            window.set_content(Some(&startup_error(&message)));
        }
    }
    window.present();
}

fn open_journal() -> Result<(AppPaths, Connection, OpenLibrary), String> {
    let paths = AppPaths::from_env()
        .map_err(|e| format!("Couldn't create the data folder: {}", friendly(&e)))?;
    let conn = db::open(&paths.db_path).map_err(|e| {
        format!(
            "Couldn't open your journal ({}): {}",
            paths.db_path.display(),
            friendly(&e)
        )
    })?;
    let ol = OpenLibrary::new()
        .map_err(|e| format!("Couldn't set up the network connection: {}", friendly(&e)))?;
    Ok((paths, conn, ol))
}

/// Shown instead of the app when the journal can't be opened, so the
/// window explains itself rather than the app silently crashing.
fn startup_error(message: &str) -> adw::ToolbarView {
    let text = format!(
        "{message}\n\nIf your journal file is damaged, daily backups are kept in the \
         “backups” folder beside it."
    );
    let toolbar = flat_toolbar();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(
        &adw::StatusPage::builder()
            .icon_name("dialog-error-symbolic")
            .title("Bookshelf couldn't start")
            .description(glib::markup_escape_text(&text)) // the description is markup
            .build(),
    ));
    toolbar
}

/// Today's safety copy of the database, on its own thread and connection.
fn back_up_in_background(paths: &AppPaths) {
    let paths = paths.clone();
    std::thread::spawn(move || {
        let made =
            db::open(&paths.db_path).and_then(|conn| backup::daily(&conn, &paths, backup::KEEP));
        if let Err(e) = made {
            eprintln!("[bookshelf] daily backup failed: {e}");
        }
    });
}

/// A toast showing `message` as plain text. Toasts parse Pango markup by
/// default, and an error message with a stray `&` or `<` would vanish.
pub(crate) fn plain_toast(message: &str) -> adw::Toast {
    adw::Toast::builder()
        .title(message)
        .use_markup(false)
        .build()
}

/// An error as a sentence for people: "That name is already taken."
pub(crate) fn friendly(e: &bookshelf_core::Error) -> String {
    e.user_message()
}

/// A path for people: the home folder shown as `~`.
pub(crate) fn display_path(path: &Path) -> String {
    bookshelf_core::paths::display_path(path, Some(&glib::home_dir()))
}

/// Keyboard shortcuts that belong to one page. They act only while that page
/// is the one showing, and they're handled before anything else on it (so
/// "1" switches shelves instead of starting a type-to-search).
pub(crate) struct PageKeys {
    controller: gtk::ShortcutController,
    nav: adw::NavigationView,
    page: glib::WeakRef<adw::NavigationPage>,
}

impl PageKeys {
    pub(crate) fn new(ctx: &Ctx, page: &adw::NavigationPage) -> Self {
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Global);
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        Self {
            controller,
            nav: ctx.nav.clone(),
            page: page.downgrade(),
        }
    }

    /// `action` returns false to let the key through to the page.
    pub(crate) fn add(
        &self,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
        action: impl Fn() -> bool + 'static,
    ) {
        let nav = self.nav.clone();
        let page = self.page.clone();
        writer::add_shortcut(&self.controller, key, modifiers, move || {
            let showing = page
                .upgrade()
                .is_some_and(|p| nav.visible_page().as_ref() == Some(&p));
            showing && action()
        });
    }

    pub(crate) fn attach(self, to: &impl IsA<gtk::Widget>) {
        to.add_controller(self.controller);
    }
}

fn flat_toolbar() -> adw::ToolbarView {
    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar
}

// ------------------------------------------------------------ profiles

fn profiles_page(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let existing = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();

    // ---- new profile: name, optional email, a clear button --------------
    let name_row = adw::EntryRow::builder().title("Name").build();
    let email_entry = email_row();
    let new_group = adw::PreferencesGroup::builder()
        .title("New profile")
        .build();
    new_group.add(&name_row);
    new_group.add(&email_entry);

    let create_btn = gtk::Button::builder()
        .child(&label_then_icon("Create profile", "go-next-symbolic"))
        .halign(gtk::Align::Center)
        .sensitive(false)
        .css_classes(["pill", "suggested-action"])
        .build();
    create_btn.update_property(&[gtk::accessible::Property::Label("Create profile")]);
    let error = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .css_classes(["error"])
        .build();

    {
        let create_btn = create_btn.clone();
        let error = error.clone();
        name_row.connect_changed(move |r| {
            r.remove_css_class("error");
            error.set_visible(false);
            create_btn.set_sensitive(!r.text().trim().is_empty());
        });
    }
    {
        let error = error.clone();
        email_entry.connect_changed(move |_| error.set_visible(false));
    }

    let create: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let name_row = name_row.clone();
        let email_entry = email_entry.clone();
        let error = error.clone();
        Rc::new(move || {
            let show = |row: &adw::EntryRow, e: &bookshelf_core::Error| {
                row.add_css_class("error");
                error.set_label(&friendly(e));
                error.set_visible(true);
            };
            // Check the email first so a bad one doesn't leave a half-made profile.
            let email = match validate_email(email_entry.text().as_str()) {
                Ok(email) => email,
                Err(e) => return show(&email_entry, &e),
            };
            let created = create_user(&ctx.conn, name_row.text().as_str(), None).and_then(|user| {
                if let Some(e) = &email {
                    set_user_email(&ctx.conn, &user.id, e)?;
                }
                get_user(&ctx.conn, &user.id)
            });
            match created {
                Ok(user) => {
                    name_row.set_text("");
                    email_entry.set_text("");
                    ctx.nav.push(&home_page(&ctx, &user));
                }
                Err(e) => show(&name_row, &e),
            }
        })
    };
    {
        let create = create.clone();
        create_btn.connect_clicked(move |_| create());
    }
    {
        let create = create.clone();
        name_row.connect_entry_activated(move |_| create());
    }
    email_entry.connect_entry_activated(move |_| create());

    let heading = gtk::Label::builder()
        .label("Who's reading?")
        .xalign(0.0)
        .css_classes(["journal-title"])
        .build();
    let lede = gtk::Label::builder()
        .label("Pick up where you left off.")
        .xalign(0.0)
        .wrap(true)
        .margin_bottom(12)
        .css_classes(["journal-lede"])
        .build();

    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(32)
        .margin_bottom(24)
        .margin_start(16)
        .margin_end(16)
        .build();
    let intro = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    intro.append(&heading);
    intro.append(&lede);
    column.append(&intro);
    column.append(&existing);
    column.append(&new_group);
    column.append(&error);
    column.append(&create_btn);

    let clamp = adw::Clamp::builder()
        .maximum_size(480)
        .child(&column)
        .build();
    let scroll = gtk::ScrolledWindow::builder().child(&clamp).build();

    let toolbar = flat_toolbar();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&scroll));

    let page = adw::NavigationPage::builder()
        .title("Bookshelf")
        .tag("profiles")
        .child(&toolbar)
        .build();

    let ctx_show = ctx.clone();
    page.connect_showing(move |_| {
        theme::apply(&UserSettings::defaults()); // neutral look until someone is chosen
                                                 // If the journal can't be read, say so: "Welcome" here would suggest it's empty.
        let any = match populate_profiles(&ctx_show, &existing) {
            Ok(any) => any,
            Err(e) => {
                ctx_show.toasts.add_toast(plain_toast(&format!(
                    "Couldn't read the profiles: {}",
                    friendly(&e)
                )));
                return;
            }
        };
        // First run reads as a welcome, not a "who's back?".
        heading.set_label(if any { "Who's reading?" } else { "Welcome" });
        lede.set_label(if any {
            "Pick up where you left off."
        } else {
            "Make a profile to start your reading journal."
        });
        new_group.set_title(if any { "New profile" } else { "Your profile" });
    });
    page
}

/// A button's label with an icon after it ("Create profile →"). Buttons
/// take their accessible name from the label.
fn label_then_icon(label: &str, icon: &str) -> gtk::Box {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Center)
        .build();
    content.append(&gtk::Label::new(Some(label)));
    content.append(&gtk::Image::from_icon_name(icon));
    content
}

/// Optional contact email, used when creating and editing a profile.
pub(crate) fn email_row() -> adw::EntryRow {
    const WHY: &str = "Optional. Book details and covers come from Open Library, a free \
                       public library service run by the Internet Archive. If you add an \
                       email, it's sent along with your book searches and cover downloads so \
                       they can contact you if there's ever a problem. It goes nowhere else. \
                       Leave it blank to send nothing.";
    let row = adw::EntryRow::builder()
        .title("Email (optional)")
        .input_purpose(gtk::InputPurpose::Email)
        .tooltip_text(WHY)
        .build();
    // A visible "what's this?" mark, so nobody has to discover the tooltip by accident.
    let info = gtk::Image::builder()
        .icon_name("dialog-information-symbolic")
        .tooltip_text(WHY)
        .valign(gtk::Align::Center)
        .css_classes(["dim-label"])
        .build();
    info.update_property(&[gtk::accessible::Property::Description(WHY)]);
    row.add_suffix(&info);
    row.connect_changed(|r| r.remove_css_class("error"));
    row
}

/// Fills the list of profiles; returns whether there are any. On a read
/// error the list is left as it was.
fn populate_profiles(ctx: &Rc<Ctx>, list: &gtk::ListBox) -> bookshelf_core::Result<bool> {
    let users = list_users(&ctx.conn)?;
    list.remove_all();
    list.set_visible(!users.is_empty());
    let any = !users.is_empty();
    for user in users {
        let row = adw::ActionRow::builder()
            .title(&user.name)
            .use_markup(false)
            .activatable(true)
            .build();
        row.add_prefix(&adw::Avatar::new(44, Some(&user.name), true));
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let ctx = ctx.clone();
        row.connect_activated(move |_| ctx.nav.push(&home_page(&ctx, &user)));
        list.append(&row);
    }
    Ok(any)
}

// ---------------------------------------------------------------- home

/// (stack name, tab title, icon, milestone, page heading, empty-state text)
const TABS: [(&str, &str, &str, Milestone, &str, &str); 3] = [
    (
        "reading",
        "Reading",
        "document-open-symbolic",
        Milestone::Reading,
        "Reading now",
        "Books you've started will live here.",
    ),
    (
        "finished",
        "Finished",
        "emblem-ok-symbolic",
        Milestone::Finished,
        "Finished",
        "Every book you finish gets its own page here.",
    ),
    (
        "eventually",
        "Eventually",
        "bookmark-new-symbolic",
        Milestone::Eventually,
        "Someday",
        "Books you want to read someday wait here.",
    ),
];

/// One tab of the home page.
struct Tab {
    milestone: Milestone,
    /// The book cards and year headings (see shelf_grid.rs).
    grid: gtk::Box,
    count: gtk::Label,
    empty: adw::StatusPage,
    empty_text: &'static str,
    add_btn: gtk::Button,
    /// The summary whose card was last opened (shared by the tabs).
    opened: Rc<RefCell<Option<String>>>,
}

impl Tab {
    /// Shows the cards that match the search (all of them when there's no
    /// search), and the empty state when none do. Year headings have no
    /// text to match, so they step aside while searching.
    fn filter(&self, query: &str, terms: &[String], haystacks: &HashMap<String, String>) {
        let mut any = false;
        let mut child = self.grid.first_child();
        while let Some(c) = child {
            let show = if c.is::<gtk::Button>() {
                let found = terms.is_empty()
                    || haystacks
                        .get(c.widget_name().as_str())
                        .is_some_and(|text| present::matches(text, terms));
                any |= found;
                found
            } else {
                terms.is_empty()
            };
            c.set_visible(show);
            child = c.next_sibling();
        }
        self.grid.set_visible(any);
        self.empty.set_visible(!any);
        if !any {
            self.show_empty_state(query);
        }
    }

    /// The card for summary `id`, if it's on this shelf.
    fn card(&self, id: &str) -> Option<gtk::Widget> {
        let mut child = self.grid.first_child();
        while let Some(c) = child {
            if c.is::<gtk::Button>() && c.widget_name() == id {
                return Some(c);
            }
            child = c.next_sibling();
        }
        None
    }

    /// The empty state: an invitation normally, "no matches" while searching.
    fn show_empty_state(&self, query: &str) {
        if query.is_empty() {
            self.empty.set_icon_name(Some("library-symbolic"));
            self.empty.set_title("Nothing here yet");
            self.empty.set_description(Some(self.empty_text));
            self.add_btn.set_visible(true);
        } else {
            self.empty.set_icon_name(Some("system-search-symbolic"));
            self.empty.set_title("No matches");
            let text = format!("Nothing on this shelf matches “{query}”.");
            self.empty
                .set_description(Some(&glib::markup_escape_text(&text)));
            self.add_btn.set_visible(false);
        }
    }
}

fn home_page(ctx: &Rc<Ctx>, user: &User) -> adw::NavigationPage {
    let open_search: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let user = user.clone();
        Rc::new(move || ctx.nav.push(&search::search_page(&ctx, &user)))
    };
    // Search state shared by every tab: the query (trimmed and lowercased,
    // for the "no matches" text), its words, and per summary the lowercased
    // text it can be found by (title, author, what you wrote).
    let query: Rc<RefCell<String>> = Rc::default();
    let terms: Rc<RefCell<Vec<String>>> = Rc::default();
    let haystacks: Rc<RefCell<HashMap<String, String>>> = Rc::default();
    let opened: Rc<RefCell<Option<String>>> = Rc::default();

    let stack = adw::ViewStack::new();
    let mut tabs: Vec<Tab> = vec![];

    for (name, title, icon, milestone, heading, empty_text) in TABS {
        let grid = shelf_grid::shelf_grid();

        let add_btn = gtk::Button::builder()
            .child(
                &adw::ButtonContent::builder()
                    .icon_name("list-add-symbolic")
                    .label("Add a book")
                    .build(),
            )
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        {
            let open_search = open_search.clone();
            add_btn.connect_clicked(move |_| open_search());
        }
        let empty = adw::StatusPage::builder().child(&add_btn).build();

        let title_label = gtk::Label::builder()
            .label(heading)
            .xalign(0.0)
            .css_classes(["page-title"])
            .build();
        let count = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["page-count"])
            .build();
        let head = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .margin_bottom(14)
            .build();
        head.append(&title_label);
        head.append(&count);

        let column = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(16)
            .margin_end(16)
            .build();
        column.append(&head);
        column.append(&grid);
        column.append(&empty);

        let clamp = adw::Clamp::builder()
            .maximum_size(720)
            .child(&column)
            .build();
        let scroll = gtk::ScrolledWindow::builder().child(&clamp).build();
        stack.add_titled_with_icon(&scroll, Some(name), title, icon);

        let tab = Tab {
            milestone,
            grid,
            count,
            empty,
            empty_text,
            add_btn,
            opened: opened.clone(),
        };
        tab.filter("", &[], &HashMap::new());
        tabs.push(tab);
    }
    let tabs = Rc::new(tabs);

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&shelf_tabs(&stack)));

    let add_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add a book (Ctrl+N)")
        .build();
    {
        let open_search = open_search.clone();
        add_btn.connect_clicked(move |_| open_search());
    }
    header.pack_start(&add_btn);

    let open_settings: Rc<dyn Fn()> = {
        let ctx = ctx.clone();
        let user = user.clone();
        Rc::new(move || ctx.nav.push(&settings::settings_page(&ctx, &user)))
    };
    let settings_btn = gtk::Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("Settings (Ctrl+,)")
        .build();
    {
        let open_settings = open_settings.clone();
        settings_btn.connect_clicked(move |_| open_settings());
    }
    header.pack_end(&settings_btn);

    // ---- search your shelf ----------------------------------------------
    let search_btn = gtk::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text("Search your shelf (Ctrl+F)")
        .build();
    header.pack_end(&search_btn);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Title, author, or something you wrote")
        .hexpand(true)
        .build();
    let search_bar = gtk::SearchBar::builder()
        .child(
            &adw::Clamp::builder()
                .maximum_size(720)
                .child(&search_entry)
                .build(),
        )
        .build();
    search_bar.connect_entry(&search_entry);
    search_btn
        .bind_property("active", &search_bar, "search-mode-enabled")
        .bidirectional()
        .build();
    {
        let search_entry = search_entry.clone();
        search_bar.connect_search_mode_enabled_notify(move |bar| {
            if !bar.is_search_mode() {
                search_entry.set_text(""); // closing the search shows everything again
            }
        });
    }
    {
        let query = query.clone();
        let terms = terms.clone();
        let haystacks = haystacks.clone();
        let tabs = tabs.clone();
        search_entry.connect_search_changed(move |entry| {
            *query.borrow_mut() = entry.text().trim().to_lowercase();
            *terms.borrow_mut() = present::query_terms(&query.borrow());
            for tab in tabs.iter() {
                tab.filter(&query.borrow(), &terms.borrow(), &haystacks.borrow());
            }
        });
    }

    if let Ok(s) = get_settings(&ctx.conn, &user.id) {
        stack.set_visible_child_name(&s.start_tab);
    }

    let toolbar = flat_toolbar();
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&search_bar);
    toolbar.set_content(Some(&stack));
    search_bar.set_key_capture_widget(Some(&toolbar)); // just start typing to search

    let page = adw::NavigationPage::builder()
        .title(&user.name)
        .tag("home")
        .child(&toolbar)
        .build();

    // ---- keyboard shortcuts (only while this page is the one showing) -----
    let keys = PageKeys::new(ctx, &page);
    let ctrl = gdk::ModifierType::CONTROL_MASK;
    {
        let open_search = open_search.clone();
        keys.add(gdk::Key::n, ctrl, move || {
            open_search();
            true
        });
    }
    keys.add(gdk::Key::comma, ctrl, move || {
        open_settings();
        true
    });
    {
        let search_btn = search_btn.clone();
        keys.add(gdk::Key::f, ctrl, move || {
            search_btn.set_active(!search_btn.is_active());
            true
        });
    }
    // 1, 2, 3: the three shelves, in the order of the tabs.
    let shelf_keys = [
        ([gdk::Key::_1, gdk::Key::KP_1], "reading"),
        ([gdk::Key::_2, gdk::Key::KP_2], "finished"),
        ([gdk::Key::_3, gdk::Key::KP_3], "eventually"),
    ];
    for (key_pair, shelf) in shelf_keys {
        for key in key_pair {
            let stack = stack.clone();
            let search_bar = search_bar.clone();
            keys.add(key, gdk::ModifierType::empty(), move || {
                if search_bar.is_search_mode() {
                    return false; // while searching, digits are part of the search
                }
                stack.set_visible_child_name(shelf);
                true
            });
        }
    }
    keys.attach(&toolbar);

    // ---- filling the lists ------------------------------------------------
    let refresh: Rc<dyn Fn()> = {
        let ctx = Rc::downgrade(ctx);
        let user_id = user.id.clone();
        let tabs = tabs.clone();
        // What the lists were last built from. Coming back from a book page
        // you only looked at shouldn't rebuild every card.
        let built_from: Cell<Option<(u64, i64, chrono::NaiveDate)>> = Cell::new(None);
        Rc::new(move || {
            let Some(ctx) = ctx.upgrade() else { return };
            let settings =
                get_settings(&ctx.conn, &user_id).unwrap_or_else(|_| UserSettings::defaults());
            theme::apply(&settings);

            // Writes on this connection, writes by others (cover downloads,
            // saves from worker threads), and today's date ("day 12").
            let others = ctx
                .conn
                .query_row("PRAGMA data_version", [], |r| r.get::<_, i64>(0))
                .unwrap_or(-1);
            let today = chrono::Local::now().date_naive();
            let now = (ctx.conn.total_changes(), others, today);
            if others >= 0 && built_from.get() == Some(now) {
                return;
            }
            built_from.set(Some(now));

            haystacks.borrow_mut().clear();
            for tab in tabs.iter() {
                let count = populate_list(
                    &ctx,
                    &user_id,
                    tab,
                    &settings.date_format,
                    today,
                    &haystacks,
                );
                tab.count.set_label(&count);
                tab.filter(&query.borrow(), &terms.borrow(), &haystacks.borrow());
            }
        })
    };
    *ctx.home_refresher.borrow_mut() = Some(refresh.clone());
    page.connect_showing(move |_| refresh());

    // Back from a book, the focus returns to its card, even when the shelf
    // was rebuilt meanwhile (after an edit) and the card is a new one.
    page.connect_shown(move |page| {
        let Some(id) = opened.borrow_mut().take() else {
            return;
        };
        let focus = page.root().and_then(|r| r.focus());
        if focus.is_some_and(|f| f.has_css_class("book-card") && f.widget_name() == id) {
            return; // the same card, still there
        }
        let showing = stack.visible_child_name();
        let tab = TABS
            .iter()
            .zip(tabs.iter())
            .find(|((name, ..), _)| showing.as_deref() == Some(*name));
        if let Some(card) = tab.and_then(|(_, tab)| tab.card(&id)) {
            card.grab_focus();
        }
    });
    page
}

/// The three shelves as a row of pill buttons (styled in style.css), in
/// place of libadwaita's view switcher. Like it, they're a tab list to
/// assistive technology, one tab selected, and they follow the stack when
/// something else (the 1/2/3 keys) changes the shelf.
fn shelf_tabs(stack: &adw::ViewStack) -> gtk::Box {
    let tabs = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .accessible_role(gtk::AccessibleRole::TabList)
        .css_classes(["shelf-tabs"])
        .build();
    tabs.update_property(&[gtk::accessible::Property::Label("Shelves")]);

    let mut buttons: Vec<(gtk::ToggleButton, &'static str)> = vec![];
    for (i, (name, title, icon, ..)) in TABS.into_iter().enumerate() {
        let button = gtk::ToggleButton::builder()
            .child(
                &adw::ButtonContent::builder()
                    .icon_name(icon)
                    .label(title)
                    .build(),
            )
            .tooltip_text(format!("{title} ({})", i + 1))
            .accessible_role(gtk::AccessibleRole::Tab)
            .build();
        button.update_property(&[gtk::accessible::Property::Label(title)]);
        button.update_state(&[gtk::accessible::State::Selected(Some(false))]);
        if let Some((first, _)) = buttons.first() {
            button.set_group(Some(first));
        }
        let stack = stack.downgrade();
        button.connect_toggled(move |b| {
            b.update_state(&[gtk::accessible::State::Selected(Some(b.is_active()))]);
            if let (true, Some(stack)) = (b.is_active(), stack.upgrade()) {
                stack.set_visible_child_name(name);
            }
        });
        tabs.append(&button);
        buttons.push((button, name));
    }

    let sync = move |stack: &adw::ViewStack| {
        let showing = stack.visible_child_name();
        for (button, name) in &buttons {
            if showing.as_deref() == Some(*name) {
                button.set_active(true); // its group turns the others off
            }
        }
    };
    sync(stack);
    stack.connect_visible_child_name_notify(sync);
    tabs
}

/// Fills one tab from the core's layout of it (finished books grouped under
/// year headings). Returns the count line for the tab's heading.
fn populate_list(
    ctx: &Rc<Ctx>,
    user_id: &str,
    tab: &Tab,
    date_format: &str,
    today: chrono::NaiveDate,
    haystacks: &RefCell<HashMap<String, String>>,
) -> String {
    let rows = match list_summaries(&ctx.conn, user_id, tab.milestone) {
        Ok(rows) => rows,
        Err(e) => {
            // Keep what's showing; an empty shelf would look like lost books.
            ctx.toasts.add_toast(plain_toast(&format!(
                "Couldn't read your shelf: {}",
                friendly(&e)
            )));
            let mut cards = 0;
            let mut child = tab.grid.first_child();
            while let Some(c) = child {
                cards += usize::from(c.is::<gtk::Button>());
                child = c.next_sibling();
            }
            return present::count_line(cards, 0);
        }
    };
    while let Some(child) = tab.grid.first_child() {
        tab.grid.remove(&child);
    }

    let view = present::shelf_view(&rows, tab.milestone, date_format, today, &chrono::Local);
    let mut haystacks = haystacks.borrow_mut();
    for row in view.rows {
        match row {
            ShelfRow::Year(heading) => tab.grid.append(&year_heading(&heading)),
            ShelfRow::Entry(mut entry) => {
                let haystack = std::mem::take(&mut entry.haystack);
                haystacks.insert(entry.item.summary.id.clone(), haystack);
                tab.grid.append(&book_card(ctx, &entry, &tab.opened));
            }
        }
    }
    view.count_line
}

/// "2026 · 12 books" above the finished books of that year.
fn year_heading(heading: &YearHeading) -> gtk::Box {
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .margin_top(22)
        .margin_bottom(12)
        .build();
    line.append(
        &gtk::Label::builder()
            .label(heading.year.to_string())
            .xalign(0.0)
            .css_classes(["section-title"])
            .build(),
    );
    line.append(
        &gtk::Label::builder()
            .label(&heading.count_label)
            .xalign(0.0)
            .valign(gtk::Align::Baseline)
            .css_classes(["page-count"])
            .build(),
    );
    line
}

/// A centred line of a card, cut short after `lines` lines.
fn card_label(text: &str, class: &str, lines: i32) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .justify(gtk::Justification::Center)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .lines(lines)
        .css_classes([class])
        .build()
}

/// One journal entry as a card: the cover, then the title, author, a dated
/// line and a taste of what you wrote. The card's widget name carries the
/// summary id. To assistive technology it's one button named by all of
/// that in a sentence; what's inside is left out (the cover is decoration).
fn book_card(
    ctx: &Rc<Ctx>,
    entry: &ShelfEntry,
    opened: &Rc<RefCell<Option<String>>>,
) -> gtk::Button {
    let item = entry.item;
    let (width, height) = LIST_COVER;
    let picture = cover_picture(ctx, &item.book, width, height);
    picture.set_halign(gtk::Align::Fill);
    picture.set_valign(gtk::Align::Fill);
    let cover = shelf_grid::fixed_size(&picture, width, height);
    cover.set_halign(gtk::Align::Center);
    cover.set_margin_bottom(14);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .valign(gtk::Align::Start)
        .build();
    content.append(&cover);

    let mut sentence = item.book.title.clone();
    let mut then = |text: &str| {
        let ended = sentence.ends_with(['.', '!', '?', '…']);
        sentence.push_str(if ended { " " } else { ". " });
        sentence.push_str(text);
    };
    content.append(&card_label(&item.book.title, "entry-title", 2));
    if let Some(author) = &item.book.author {
        content.append(&card_label(author, "entry-byline", 1));
        then(&format!("By {author}"));
    }
    if !entry.meta.is_empty() {
        content.append(&card_label(&entry.meta, "entry-meta", 1));
        then(&entry.meta);
    }
    if !entry.excerpt.is_empty() {
        content.append(&card_label(&entry.excerpt, "entry-excerpt", 3));
        then(&entry.excerpt);
    } else if let Some(note) = entry.empty_note {
        content.append(&card_label(note, "entry-excerpt-empty", 1));
        then(note);
    }
    content.update_state(&[gtk::accessible::State::Hidden(true)]);

    let card = gtk::Button::builder()
        .child(&content)
        .css_classes(["book-card"])
        .build();
    card.set_widget_name(&item.summary.id);
    card.update_property(&[gtk::accessible::Property::Label(&sentence)]);
    let ctx = Rc::downgrade(ctx);
    let opened = Rc::downgrade(opened);
    card.connect_clicked(move |card| {
        let (Some(ctx), Some(opened)) = (ctx.upgrade(), opened.upgrade()) else {
            return;
        };
        let id = card.widget_name();
        *opened.borrow_mut() = Some(id.to_string());
        ctx.nav.push(&editor::summary_page(&ctx, id.as_str()));
    });
    card
}

/// Size of the covers on the shelf; these are kept decoded in memory.
const LIST_COVER: (i32, i32) = (104, 156);

/// Cover image from the local covers folder, or an empty tinted box. The
/// image is decoded on a worker thread and fills in when ready.
pub(crate) fn cover_picture(ctx: &Rc<Ctx>, book: &Book, width: i32, height: i32) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_size_request(width, height);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_overflow(gtk::Overflow::Hidden);
    picture.set_valign(gtk::Align::Start);
    picture.set_halign(gtk::Align::Start);
    picture.add_css_class("cover");

    let Some(file) = book.cover_path.clone() else {
        return picture;
    };
    let cache = (width, height) == LIST_COVER;
    if cache {
        if let Some(texture) = ctx.covers.borrow().get(&file) {
            picture.set_paintable(Some(texture));
            return picture;
        }
    }
    let path = ctx.paths.cover_file(&file);
    let ctx = Rc::downgrade(ctx);
    let target = picture.downgrade();
    glib::spawn_future_local(async move {
        // Twice the size on screen, so it's sharp on high-density displays.
        let loaded = gio::spawn_blocking(move || load_cover(&path, width * 2, height * 2)).await;
        let Ok(Some(texture)) = loaded else { return };
        if cache {
            if let Some(ctx) = ctx.upgrade() {
                ctx.covers.borrow_mut().insert(file, texture.clone());
            }
        }
        if let Some(picture) = target.upgrade() {
            picture.set_paintable(Some(&texture));
        }
    });
    picture
}

/// Decodes a cover scaled to fit `width`×`height`. Runs on a worker thread.
/// Images with an absurd pixel count are refused: a small file can still
/// decode to gigabytes.
fn load_cover(path: &Path, width: i32, height: i32) -> Option<gdk::Texture> {
    const MAX_PIXELS: i64 = 40_000_000;
    let (_, w, h) = gtk::gdk_pixbuf::Pixbuf::file_info(path)?;
    if i64::from(w) * i64::from(h) > MAX_PIXELS {
        eprintln!(
            "[bookshelf] skipping oversized cover {} ({w}×{h})",
            path.display()
        );
        return None;
    }
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, width, height, true).ok()?;
    Some(gdk::Texture::for_pixbuf(&pixbuf))
}
