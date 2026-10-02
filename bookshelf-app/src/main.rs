mod date_picker;
mod editor;
mod format;
mod help;
mod markdown;
mod reader;
mod search;
mod settings;
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
        .label("Create profile")
        .halign(gtk::Align::Center)
        .sensitive(false)
        .css_classes(["pill", "suggested-action"])
        .build();
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
    list: gtk::ListBox,
    count: gtk::Label,
    empty: adw::StatusPage,
    empty_text: &'static str,
    add_btn: gtk::Button,
}

impl Tab {
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

    let stack = adw::ViewStack::new();
    let mut tabs: Vec<Tab> = vec![];

    for (name, title, icon, milestone, heading, empty_text) in TABS {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["entries"])
            .valign(gtk::Align::Start)
            .build();

        let add_btn = gtk::Button::builder()
            .label("Add a book")
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        {
            let open_search = open_search.clone();
            add_btn.connect_clicked(move |_| open_search());
        }
        let empty = adw::StatusPage::builder().child(&add_btn).build();
        list.set_placeholder(Some(&empty));

        {
            let terms = terms.clone();
            let haystacks = haystacks.clone();
            list.set_filter_func(move |row| {
                let terms = terms.borrow();
                if terms.is_empty() {
                    return true;
                }
                // Year headings have no entry, so they step aside while searching.
                haystacks
                    .borrow()
                    .get(row.widget_name().as_str())
                    .is_some_and(|text| present::matches(text, &terms))
            });
        }
        {
            let ctx = ctx.clone();
            list.connect_row_activated(move |_, row| {
                let id = row.widget_name();
                ctx.nav.push(&editor::summary_page(&ctx, id.as_str()));
            });
        }

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
        column.append(&list);

        let clamp = adw::Clamp::builder()
            .maximum_size(720)
            .child(&column)
            .build();
        let scroll = gtk::ScrolledWindow::builder().child(&clamp).build();
        stack.add_titled_with_icon(&scroll, Some(name), title, icon);

        let tab = Tab {
            milestone,
            list,
            count,
            empty,
            empty_text,
            add_btn,
        };
        tab.show_empty_state("");
        tabs.push(tab);
    }
    let tabs = Rc::new(tabs);

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&switcher));

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
        let tabs = tabs.clone();
        search_entry.connect_search_changed(move |entry| {
            *query.borrow_mut() = entry.text().trim().to_lowercase();
            *terms.borrow_mut() = present::query_terms(&query.borrow());
            for tab in tabs.iter() {
                tab.list.invalidate_filter();
                tab.show_empty_state(&query.borrow());
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
        // What the lists were last built from. Coming back from a book page
        // you only looked at shouldn't rebuild every row.
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
                tab.show_empty_state(&query.borrow());
            }
        })
    };
    *ctx.home_refresher.borrow_mut() = Some(refresh.clone());
    page.connect_showing(move |_| refresh());
    page
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
            return present::count_line(tab.list.observe_children().n_items() as usize, 0);
        }
    };
    tab.list.remove_all();

    let view = present::shelf_view(&rows, tab.milestone, date_format, today, &chrono::Local);
    for row in view.rows {
        match row {
            ShelfRow::Year(heading) => tab.list.append(&year_row(&heading)),
            ShelfRow::Entry(mut entry) => {
                // Before appending: the list filters a row as it's added,
                // so a search in progress needs the text already there.
                let haystack = std::mem::take(&mut entry.haystack);
                haystacks
                    .borrow_mut()
                    .insert(entry.item.summary.id.clone(), haystack);
                tab.list.append(&summary_row(ctx, &entry));
            }
        }
    }
    view.count_line
}

/// "2026 · 12 books" between the finished entries.
fn year_row(heading: &YearHeading) -> gtk::ListBoxRow {
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .margin_top(18)
        .margin_bottom(4)
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
    gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .focusable(false)
        .child(&line)
        .build()
}

fn wrapped_label(text: &str, class: &str, lines: i32) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .lines(lines)
        .css_classes([class])
        .build()
}

/// One journal entry: cover, title, author, a dated line, and a taste of
/// what you wrote. The row's widget name carries the summary id.
fn summary_row(ctx: &Rc<Ctx>, entry: &ShelfEntry) -> gtk::ListBoxRow {
    let item = entry.item;
    let cover = cover_picture(ctx, &item.book, 64, 96);

    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    text.append(&wrapped_label(&item.book.title, "entry-title", 2));
    if let Some(author) = &item.book.author {
        text.append(&wrapped_label(author, "entry-byline", 1));
    }
    if !entry.meta.is_empty() {
        text.append(&wrapped_label(&entry.meta, "entry-meta", 1));
    }
    if !entry.excerpt.is_empty() {
        text.append(&wrapped_label(&entry.excerpt, "entry-excerpt", 3));
    } else if let Some(note) = entry.empty_note {
        text.append(&wrapped_label(note, "entry-excerpt-empty", 1));
    }

    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(14)
        .build();
    card.append(&cover);
    card.append(&text);

    let row = gtk::ListBoxRow::builder()
        .activatable(true)
        .child(&card)
        .css_classes(["entry"])
        .build();
    row.set_widget_name(&item.summary.id);
    row
}

/// Size of the covers in the home lists; these are kept decoded in memory.
const LIST_COVER: (i32, i32) = (64, 96);

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
