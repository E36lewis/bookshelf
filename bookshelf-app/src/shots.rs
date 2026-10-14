//! Debug builds only: screenshots of the app on a made-up journal, for the
//! README and the website (site/screenshots). Not in release builds.
//!
//!     gtk4-broadwayd :5 &    # an off-screen display: no windows pop up
//!     GDK_BACKEND=broadway BROADWAY_DISPLAY=:5 \
//!     BOOKSHELF_DIR=<empty scratch folder> BOOKSHELF_SCREENSHOTS=<out folder> \
//!         cargo run -p bookshelf-app
//!
//! Point XDG_DATA_HOME, XDG_CONFIG_HOME and XDG_CACHE_HOME at scratch folders
//! too, and FONTCONFIG_FILE at a fonts.conf that adds packaging/fonts (as
//! packaging/apprun-hooks does), so the fonts match the AppImage's. It
//! refuses to run without BOOKSHELF_DIR, or on a journal that already has
//! profiles, so it never touches a real one. The books are Avery's from the
//! Mac app's demo journal (apple/BookshelfKit/.../DemoJournal.swift).

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use bookshelf_core::models::*;
use chrono::{Datelike, Duration as Days, Local, NaiveDate};
use gtk::glib;

use crate::Ctx;

/// What to show, in order, each in light and then dark.
const STEPS: [&str; 11] = [
    "profiles", "shelf", "tabs", "search", "book", "settings", "dialog", "add", "results",
    "writer", "reader",
];

pub(crate) fn run(ctx: &Rc<Ctx>, window: &adw::ApplicationWindow) {
    let Some(out) = std::env::var_os("BOOKSHELF_SCREENSHOTS").map(PathBuf::from) else {
        return;
    };
    let scratch = std::env::var_os("BOOKSHELF_DIR").is_some();
    if !scratch || !list_users(&ctx.conn).is_ok_and(|u| u.is_empty()) {
        eprintln!("[shots] needs BOOKSHELF_DIR set to an empty folder; not taking screenshots");
        return;
    }
    let (user, piranesi) = seed(&ctx.conn, &ctx.paths).expect("seeding the demo journal");
    std::fs::create_dir_all(&out).expect("making the screenshots folder");
    window.set_default_size(1000, 700);
    if let Some(settings) = gtk::Settings::default() {
        // No pages caught mid-slide.
        settings.set_gtk_enable_animations(false);
        // Broadway reports no text DPI, which shrinks libadwaita's clamps
        // (sized in scaled points) to nothing. 96 dpi is GTK's normal.
        settings.set_gtk_xft_dpi(96 * 1024);
    }

    let ctx = ctx.clone();
    let window = window.clone();
    glib::spawn_future_local(async move {
        let pause = || glib::timeout_future(Duration::from_millis(1500));
        pause().await;
        for step in STEPS {
            match step {
                // Again, now that there are profiles to list.
                "profiles" => ctx.nav.replace(&[crate::profiles_page(&ctx)]),
                "shelf" => ctx.nav.push(&crate::home_page(&ctx, &user)),
                "search" => {
                    // The shelf's own search: it filters what's there, no network.
                    if let Some(bar) = showing(&ctx).and_then(|p| find::<gtk::SearchBar>(&p)) {
                        bar.set_search_mode(true);
                    }
                    if let Some(entry) = showing(&ctx).and_then(|p| find::<gtk::SearchEntry>(&p)) {
                        entry.set_text("le guin");
                        entry.grab_focus();
                    }
                }
                "tabs" => {
                    // The keyboard focus on the shelf tabs, to show its ring.
                    window.set_focus_visible(true);
                    let header = showing(&ctx).and_then(|p| find::<adw::HeaderBar>(&p));
                    let tabs = header.and_then(|h| h.title_widget());
                    if let Some(tab) = tabs.and_then(|t| find::<gtk::ToggleButton>(&t)) {
                        tab.grab_focus();
                    }
                }
                "book" => ctx.nav.push(&crate::editor::summary_page(&ctx, &piranesi)),
                "settings" => ctx.nav.push(&crate::settings::settings_page(&ctx, &user)),
                "dialog" => {
                    // Settings' "Delete this profile…", to show a dialog's buttons.
                    ctx.nav.push(&crate::settings::settings_page(&ctx, &user));
                    pause().await;
                    let page = showing(&ctx);
                    let delete =
                        page.and_then(|p| find_class::<gtk::Button>(&p, "destructive-action"));
                    if let Some(delete) = delete {
                        delete.emit_clicked();
                    }
                }
                "add" => ctx.nav.push(&crate::search::search_page(&ctx, &user)),
                "writer" => ctx.nav.push(&crate::writer::writer_page(&ctx, &piranesi)),
                "reader" => ctx.nav.push(&crate::reader::reader_page(&ctx, &piranesi)),
                // "results": Add a book with made-up search results and covers.
                _ => {
                    let source = demo_search();
                    let (page, field) =
                        crate::search::search_page_from(&ctx, &user, ctx.ol.clone(), source);
                    ctx.nav.push(&page);
                    field.set_text("le guin");
                }
            }
            pause().await;
            // Long pages are shown a screenful at a time.
            let page = ctx.nav.visible_page();
            let scroller = page.as_ref().and_then(find::<gtk::ScrolledWindow>);
            let screens = match (step, &scroller) {
                ("settings", Some(s)) => {
                    let adj = s.vadjustment();
                    ((adj.upper() / adj.page_size().max(1.0)).ceil() as usize).clamp(1, 6)
                }
                ("add", _) => 2,  // empty, then with results
                ("tabs", _) => 2, // the shelf tabs as pills, then as one joined group
                _ => 1,
            };
            for screen in 0..screens {
                let mut name = step.to_string();
                if screens > 1 {
                    name = format!("{step}-{}", screen + 1);
                }
                if step == "settings" {
                    if let Some(s) = &scroller {
                        let adj = s.vadjustment();
                        adj.set_value(adj.page_size() * screen as f64);
                    }
                } else if step == "tabs" && screen == 1 {
                    joined_tabs(true);
                } else if step == "add" && screen == 1 {
                    if let Some(page) = &page {
                        stand_in_results(page);
                    }
                }
                for theme in ["light", "dark"] {
                    let mut s = get_settings(&ctx.conn, &user.id).expect("settings");
                    s.theme = theme.into();
                    update_settings(&ctx.conn, &s).expect("settings");
                    crate::theme::apply(&s);
                    pause().await;
                    if step == "tabs" {
                        window.set_focus_visible(true); // GTK drops it after a while
                        glib::timeout_future(Duration::from_millis(300)).await;
                    }
                    let file = out.join(format!("linux-{name}-{theme}.png"));
                    match snap(&window, &file) {
                        Ok(()) => eprintln!("[shots] {}", file.display()),
                        Err(e) => eprintln!("[shots] {}: {e}", file.display()),
                    }
                }
            }
            match step {
                "profiles" | "shelf" => {}
                "search" => {
                    if let Some(bar) = showing(&ctx).and_then(|p| find::<gtk::SearchBar>(&p)) {
                        bar.set_search_mode(false); // which also clears it
                    }
                }
                "tabs" => {
                    joined_tabs(false);
                    window.set_focus_visible(false);
                }
                "dialog" => {
                    if let Some(dialog) = find::<adw::AlertDialog>(&window) {
                        dialog.close();
                    }
                    pause().await;
                    ctx.nav.pop();
                }
                _ => {
                    ctx.nav.pop(); // back to the shelf for the next page
                }
            }
        }
        window.close();
    });
}

/// The page on top.
fn showing(ctx: &Ctx) -> Option<gtk::Widget> {
    ctx.nav.visible_page().map(|p| p.upcast())
}

/// The first `T` with CSS class `class` inside `root`, depth first.
fn find_class<T: IsA<gtk::Widget>>(root: &impl IsA<gtk::Widget>, class: &str) -> Option<T> {
    let mut child = root.as_ref().first_child();
    while let Some(widget) = child {
        if widget.has_css_class(class) {
            if let Ok(found) = widget.clone().downcast::<T>() {
                return Some(found);
            }
        }
        if let Some(found) = find_class::<T>(&widget, class) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

thread_local! {
    static JOINED: gtk::CssProvider = gtk::CssProvider::new();
}

/// The other look considered for the shelf tabs: one group, joined, only
/// its outer corners rounded. Shown for comparison; not in the app.
fn joined_tabs(on: bool) {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    JOINED.with(|provider| {
        if on {
            provider.load_from_string(
                ".shelf-tabs { border-spacing: 0; }\n\
                 .shelf-tabs > button { border-radius: 0; margin-left: -1px; }\n\
                 .shelf-tabs > button:first-child { border-radius: 6px 0 0 6px; margin-left: 0; }\n\
                 .shelf-tabs > button:last-child { border-radius: 0 6px 6px 0; }\n",
            );
            gtk::style_context_add_provider_for_display(
                &display,
                provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
            );
        } else {
            gtk::style_context_remove_provider_for_display(&display, provider);
        }
    });
}

/// The first `T` inside `root`, depth first.
fn find<T: IsA<gtk::Widget>>(root: &impl IsA<gtk::Widget>) -> Option<T> {
    let mut child = root.as_ref().first_child();
    while let Some(widget) = child {
        if let Ok(found) = widget.clone().downcast::<T>() {
            return Some(found);
        }
        if let Some(found) = find::<T>(&widget) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

/// The add-a-book page with results in it. A real search would go to Open
/// Library, so these rows stand in for them, built the way search.rs does.
fn stand_in_results(page: &adw::NavigationPage) {
    let Some(list) = find::<gtk::ListBox>(page) else {
        return;
    };
    for (title, subtitle) in [
        ("The Dispossessed", "Ursula K. Le Guin · 1974"),
        ("The Lathe of Heaven", "Ursula K. Le Guin · 1971"),
        ("The Word for World Is Forest", "Ursula K. Le Guin · 1972"),
    ] {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(subtitle)
            .activatable(true)
            .build();
        row.add_suffix(&gtk::Image::from_icon_name("list-add-symbolic"));
        list.append(&row);
    }
}

/// The window as a PNG, cropped to its contents (so without its shadow).
fn snap(window: &adw::ApplicationWindow, file: &std::path::Path) -> Result<(), String> {
    let content = window.content().ok_or("no content")?;
    let area = content.compute_bounds(window).ok_or("no bounds")?;
    let (w, h) = (f64::from(window.width()), f64::from(window.height()));
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(window)).snapshot(&snapshot, w, h);
    let node = snapshot.to_node().ok_or("nothing drawn")?;
    let renderer = window.renderer().ok_or("no renderer")?;
    renderer
        .render_texture(&node, Some(&area))
        .save_to_png(file)
        .map_err(|e| e.to_string())
}

/// Add a book, without the network: made-up results, and made-up covers
/// (plain colours) for all but the last, which shows the stand-in.
fn demo_search() -> crate::search::Source {
    let result = |title: &str, author: &str, year: &str, cover_id: Option<i64>| NewBook {
        external_id: format!("/works/DEMO{}W", cover_id.unwrap_or(9)),
        title: title.into(),
        author: Some(author.into()),
        published_date: Some(year.into()),
        cover_id,
        ..Default::default()
    };
    let results = vec![
        result("A Wizard of Earthsea", "Ursula K. Le Guin", "1968", Some(1)),
        result(
            "The Left Hand of Darkness",
            "Ursula K. Le Guin",
            "1969",
            Some(2),
        ),
        NewBook {
            subtitle: Some("An Ambiguous Utopia".into()),
            ..result("The Dispossessed", "Ursula K. Le Guin", "1974", Some(3))
        },
        result("The Lathe of Heaven", "Ursula K. Le Guin", "1971", None),
    ];
    crate::search::Source {
        search: std::sync::Arc::new(move |_| Ok(results.clone())),
        thumbnail: std::sync::Arc::new(|id| {
            let colour = [0x6b8273ff, 0x9e6b59ff, 0x5c6e8fff][(id as usize - 1) % 3];
            let pixbuf =
                gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 180, 270)?;
            pixbuf.fill(colour);
            pixbuf.save_to_bufferv("png", &[]).ok()
        }),
    }
}

/// Avery's shelves from the Mac demo journal; returns Avery and Piranesi's entry.
/// Each book gets a made-up cover (a plain colour), as a download would leave it.
fn seed(
    conn: &bookshelf_core::rusqlite::Connection,
    paths: &bookshelf_core::paths::AppPaths,
) -> bookshelf_core::Result<(User, String)> {
    let avery = create_user(conn, "Avery", None)?;
    create_user(conn, "Sam", None)?;
    let mut s = get_settings(conn, &avery.id)?;
    s.start_tab = "finished".into();
    update_settings(conn, &s)?;

    let today = Local::now().date_naive();
    let ago = |n: i64| Some(today - Days::days(n));
    let last_year = |m, d| NaiveDate::from_ymd_opt(today.year() - 1, m, d);
    // (title, author, publisher, year, pages, about, started, finished, body)
    #[rustfmt::skip]
    let books = [
        ("The Left Hand of Darkness", "Ursula K. Le Guin", "Ace Books", "1969", 304,
         "An envoy arrives alone on a frozen world whose people have no fixed sex, and has to learn who to trust before the winter closes in.",
         ago(12), None,
         "## Where I am\n\nJust crossed the **Gobrin Ice**. The pace has slowed right down and I don't mind at all.\n\n- Estraven is the real centre of the book\n- The weather is a character of its own\n\n> Light is the left hand of darkness.\n\nNext time: the *Handdara* chapters, and words like `shifgrethor`. ~~Skim~~ Read the ice slowly."),
        ("Middlemarch", "George Eliot", "William Blackwood and Sons", "1871", 880,
         "A whole town seen from the inside: marriages, ambitions and a new hospital, and the quiet ways people's choices bend one another's lives.",
         ago(3), None, ""),
        ("Piranesi", "Susanna Clarke", "Bloomsbury", "2020", 272,
         "A man lives in a house of endless halls and tides, keeping careful journals, until the notes he finds start to contradict what he remembers.",
         ago(34), ago(20),
         "# What stayed with me\n\nThe House is *beautiful and kind*, and I believed it completely. Reading the journals alongside the narrator felt like solving the mystery **with** him rather than ahead of him.\n\n## Moments\n\n1. The first time the tides flood the lower halls\n2. Finding the numbered pages\n3. The last chapter, all of it\n\n---\n\nI'd read it again in winter."),
        ("The Remains of the Day", "Kazuo Ishiguro", "Faber & Faber", "1989", 245,
         "An English butler drives across the country in 1956 and looks back on decades of service, and on what loyalty cost him.",
         ago(60), ago(48),
         "A careful, *restrained* voice that slowly lets you see everything he can't say. The pier scene near the end undid me."),
        ("A Wizard of Earthsea", "Ursula K. Le Guin", "Parnassus Press", "1968", 183,
         "A gifted, proud boy at a school for wizards unleashes a shadow on the world, and has to follow it to the edge of the sea.",
         ago(9), ago(2), "Short and *wise*. Names have power; so does knowing your own."),
        ("Dune", "Frank Herbert", "Chilton Books", "1965", 412,
         "On the desert planet Arrakis, the only source of the most valuable substance in the universe, a young heir is drawn into a war for its future.",
         last_year(9, 2), last_year(10, 14),
         "## Big ideas\n\n- Ecology as politics\n- The danger of **heroes**\n\nThe appendices are worth it. `Spice must flow.`"),
        ("Pride and Prejudice", "Jane Austen", "T. Egerton", "1813", 279,
         "Elizabeth Bennet and Mr Darcy misjudge each other at every turn in a sharp, funny novel of manners and money.",
         last_year(5, 1), last_year(5, 19), "Funnier than I remembered. ~~Darcy is awful~~ Darcy is *shy*."),
        ("Moby-Dick", "Herman Melville", "Harper & Brothers", "1851", 635,
         "Captain Ahab's obsessive hunt for the white whale, told by Ishmael, a sailor who signs on to the Pequod.",
         None, None, ""),
        ("The Overstory", "Richard Powers", "W. W. Norton", "2018", 502,
         "Nine strangers, each changed by a tree, are drawn together to try to save the last of the old forests.",
         None, None, "Recommended by Sam. Why do I want to read it? *Trees*, mostly."),
    ];
    let mut piranesi = String::new();
    for (i, (title, author, publisher, year, pages, about, started, finished, body)) in
        books.into_iter().enumerate()
    {
        let book = find_or_create_book(
            conn,
            &NewBook {
                external_id: format!("/works/DEMO{}W", i + 1),
                title: title.into(),
                author: Some(author.into()),
                publisher: Some(publisher.into()),
                published_date: Some(year.into()),
                page_count: Some(pages),
                description: Some(about.into()),
                ..Default::default()
            },
        )?;
        let colour = [0x5c6e8fff, 0x9e6b59ff, 0x6b8273ff, 0xa08a52ff][i % 4];
        if let Some(pixbuf) =
            gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 360, 540)
        {
            pixbuf.fill(colour);
            let file = format!("{}.png", book.id);
            if pixbuf.savev(paths.cover_file(&file), "png", &[]).is_ok() {
                set_cover_path(conn, &book.id, &file)?;
            }
        }
        let input = SummaryInput {
            body: body.into(),
            started_on: started,
            finished_on: finished,
        };
        let entry = create_summary(conn, &avery.id, &book.id, &input)?;
        if title == "Piranesi" {
            piranesi = entry.id;
        }
    }
    Ok((avery, piranesi))
}
