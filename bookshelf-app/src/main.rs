mod date_picker;
mod editor;
mod format;
mod markdown;
mod reader;
mod search;
mod settings;
mod theme;
mod writer;

use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::db;
use bookshelf_core::models::*;
use bookshelf_core::openlibrary::OpenLibrary;
use bookshelf_core::paths::AppPaths;
use bookshelf_core::rusqlite::Connection;
use gtk::glib;

const APP_ID: &str = "com.bookshelf.Bookshelf";

/// Shared app state. The UI thread owns this connection; worker threads
/// (search, cover downloads) open their own. The Open Library client is
/// shared by everyone (it's a cheap clone) so connections get reused.
pub(crate) struct Ctx {
    pub(crate) conn: Connection,
    pub(crate) paths: AppPaths,
    pub(crate) nav: adw::NavigationView,
    pub(crate) ol: OpenLibrary,
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| theme::install());
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let paths = AppPaths::from_env().expect("could not create data directory");
    let conn = db::open(&paths.db_path).expect("could not open database");
    let ol = OpenLibrary::new().expect("could not set up the network client");
    let nav = adw::NavigationView::new();
    let ctx = Rc::new(Ctx { conn, paths, nav: nav.clone(), ol });

    nav.push(&profiles_page(&ctx));

    adw::ApplicationWindow::builder()
        .application(app)
        .title("Bookshelf")
        .default_width(720)
        .default_height(900)
        .content(&nav)
        .build()
        .present();
}

/// A toast showing `message` as plain text. Toasts parse Pango markup by
/// default, and an error message with a stray `&` or `<` would vanish.
pub(crate) fn plain_toast(message: &str) -> adw::Toast {
    adw::Toast::builder().title(message).use_markup(false).build()
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

    let add = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    let entry = adw::EntryRow::builder()
        .title("New profile name")
        .show_apply_button(true)
        .build();
    let email_entry = email_row();
    add.append(&entry);
    add.append(&email_entry);

    let ctx_add = ctx.clone();
    entry.connect_apply(move |row| {
        // Check the email first so a bad one doesn't leave a half-made profile.
        let email = match validate_email(email_entry.text().as_str()) {
            Ok(email) => email,
            Err(e) => {
                eprintln!("could not create profile: {e}");
                email_entry.add_css_class("error");
                return;
            }
        };
        let created = create_user(&ctx_add.conn, row.text().as_str(), None).and_then(|user| {
            if let Some(e) = &email {
                set_user_email(&ctx_add.conn, &user.id, e)?;
            }
            get_user(&ctx_add.conn, &user.id)
        });
        match created {
            Ok(user) => {
                row.set_text("");
                email_entry.set_text("");
                row.remove_css_class("error");
                ctx_add.nav.push(&home_page(&ctx_add, &user));
            }
            Err(e) => {
                eprintln!("could not create profile: {e}");
                row.add_css_class("error");
            }
        }
    });

    let heading = gtk::Label::builder()
        .label("Who's reading?")
        .xalign(0.0)
        .css_classes(["journal-title"])
        .build();
    let lede = gtk::Label::builder()
        .label("Pick up where you left off.")
        .xalign(0.0)
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
    column.append(&add);

    let clamp = adw::Clamp::builder().maximum_size(480).child(&column).build();
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
        populate_profiles(&ctx_show, &existing);
    });
    page
}

/// Optional contact email, used when creating and editing a profile.
pub(crate) fn email_row() -> adw::EntryRow {
    const WHY: &str = "Optional. Book details and covers come from Open Library, a free \
                       public library service. If you add an email, it's sent to them along \
                       with your book searches so they can contact you if there's ever a \
                       problem. It goes only to Open Library. Leave it blank to send nothing.";
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

fn populate_profiles(ctx: &Rc<Ctx>, list: &gtk::ListBox) {
    list.remove_all();
    let users = list_users(&ctx.conn).unwrap_or_default();
    list.set_visible(!users.is_empty());
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
}

// ---------------------------------------------------------------- home

/// (stack name, tab title, icon, milestone, page heading, empty-state text)
const TABS: [(&str, &str, &str, Milestone, &str, &str); 3] = [
    ("reading", "Reading", "document-open-symbolic", Milestone::Reading,
     "Reading now", "Books you've started will live here."),
    ("finished", "Finished", "emblem-ok-symbolic", Milestone::Finished,
     "Finished", "Every book you finish gets its own page here."),
    ("eventually", "Eventually", "bookmark-new-symbolic", Milestone::Eventually,
     "Someday", "Books you want to read someday wait here."),
];

fn home_page(ctx: &Rc<Ctx>, user: &User) -> adw::NavigationPage {
    let stack = adw::ViewStack::new();
    let mut lists: Vec<(Milestone, gtk::ListBox, gtk::Label)> = vec![];

    for (name, title, icon, milestone, heading, empty_text) in TABS {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["entries"])
            .valign(gtk::Align::Start)
            .build();
        list.set_placeholder(Some(
            &adw::StatusPage::builder()
                .icon_name("library-symbolic")
                .title("Nothing here yet")
                .description(empty_text)
                .build(),
        ));
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
        let count_label = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["page-count"])
            .build();
        let head = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .margin_bottom(14)
            .build();
        head.append(&title_label);
        head.append(&count_label);

        let column = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(16)
            .margin_end(16)
            .build();
        column.append(&head);
        column.append(&list);

        let clamp = adw::Clamp::builder().maximum_size(720).child(&column).build();
        let scroll = gtk::ScrolledWindow::builder().child(&clamp).build();
        stack.add_titled_with_icon(&scroll, Some(name), title, icon);
        lists.push((milestone, list, count_label));
    }

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&switcher));

    let add_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Add a book")
        .build();
    {
        let ctx = ctx.clone();
        let user = user.clone();
        add_btn.connect_clicked(move |_| ctx.nav.push(&search::search_page(&ctx, &user)));
    }
    header.pack_start(&add_btn);

    let settings_btn = gtk::Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("Settings")
        .build();
    {
        let ctx = ctx.clone();
        let user = user.clone();
        settings_btn.connect_clicked(move |_| ctx.nav.push(&settings::settings_page(&ctx, &user)));
    }
    header.pack_end(&settings_btn);

    let first_look = get_settings(&ctx.conn, &user.id).ok();
    if let Some(s) = &first_look {
        stack.set_visible_child_name(&s.start_tab);
    }

    let toolbar = flat_toolbar();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&stack));

    let page = adw::NavigationPage::builder()
        .title(&user.name)
        .tag("home")
        .child(&toolbar)
        .build();

    let ctx = ctx.clone();
    let user_id = user.id.clone();
    page.connect_showing(move |_| {
        let settings = get_settings(&ctx.conn, &user_id).unwrap_or_else(|_| UserSettings::defaults());
        theme::apply(&settings);
        for (milestone, list, count) in &lists {
            let n = populate_list(&ctx, &user_id, *milestone, list, &settings.date_format);
            count.set_label(&match n {
                1 => "1 book".to_string(),
                n => format!("{n} books"),
            });
        }
    });
    page
}

fn populate_list(
    ctx: &Rc<Ctx>,
    user_id: &str,
    milestone: Milestone,
    list: &gtk::ListBox,
    date_format: &str,
) -> usize {
    list.remove_all();
    match list_summaries(&ctx.conn, user_id, milestone) {
        Ok(rows) => {
            for row in &rows {
                list.append(&summary_row(ctx, row, milestone, date_format));
            }
            rows.len()
        }
        Err(e) => {
            eprintln!("could not load {milestone:?} list: {e}");
            0
        }
    }
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
fn summary_row(
    ctx: &Rc<Ctx>,
    item: &SummaryWithBook,
    milestone: Milestone,
    date_format: &str,
) -> gtk::ListBoxRow {
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
    let meta = meta_line(item, milestone, date_format);
    if !meta.is_empty() {
        text.append(&wrapped_label(&meta, "entry-meta", 1));
    }
    let taste = excerpt(&item.summary.body);
    if !taste.is_empty() {
        text.append(&wrapped_label(&taste, "entry-excerpt", 3));
    } else if milestone != Milestone::Eventually {
        text.append(&wrapped_label("Nothing written yet.", "entry-excerpt-empty", 1));
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

/// Cover image from the local covers folder, or an empty tinted box.
pub(crate) fn cover_picture(ctx: &Ctx, book: &Book, width: i32, height: i32) -> gtk::Picture {
    let picture = match &book.cover_path {
        Some(file) => gtk::Picture::for_filename(ctx.paths.cover_file(file)),
        None => gtk::Picture::new(),
    };
    picture.set_size_request(width, height);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_overflow(gtk::Overflow::Hidden);
    picture.set_valign(gtk::Align::Start);
    picture.set_halign(gtk::Align::Start);
    picture.add_css_class("cover");
    picture
}

fn meta_line(item: &SummaryWithBook, milestone: Milestone, date_format: &str) -> String {
    let s = &item.summary;
    let mut parts: Vec<String> = vec![];
    match milestone {
        Milestone::Finished => {
            if let Some(d) = s.finished_on {
                parts.push(format!("Finished {}", format_date(date_format, d)));
            }
            if let Some(n) = s.days_to_complete {
                parts.push(if n == 1 { "1 day".into() } else { format!("{n} days") });
            }
        }
        Milestone::Reading => {
            if let Some(d) = s.started_on {
                parts.push(format!("Started {}", format_date(date_format, d)));
                let day = (chrono::Local::now().date_naive() - d).num_days() + 1;
                if day >= 1 {
                    parts.push(format!("day {day}"));
                }
            }
        }
        Milestone::Eventually => {
            parts.push(format!("Added {}", format_date(date_format, s.created_at.date_naive())));
        }
    }
    parts.join(" · ")
}

/// A plain-text taste of the summary for the list: markdown marks removed,
/// whitespace flattened, trimmed to about two sentences' worth.
fn excerpt(body: &str) -> String {
    let cleaned: String = body
        .chars()
        .filter(|c| !matches!(c, '*' | '#' | '>' | '`' | '\\'))
        .collect();
    let flat = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 160 {
        let cut: String = flat.chars().take(160).collect();
        format!("{}…", cut.trim_end())
    } else {
        flat
    }
}
