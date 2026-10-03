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
const STEPS: [&str; 4] = ["shelf", "book", "writer", "reader"];

pub(crate) fn run(ctx: &Rc<Ctx>, window: &adw::ApplicationWindow) {
    let Some(out) = std::env::var_os("BOOKSHELF_SCREENSHOTS").map(PathBuf::from) else {
        return;
    };
    let scratch = std::env::var_os("BOOKSHELF_DIR").is_some();
    if !scratch || !list_users(&ctx.conn).is_ok_and(|u| u.is_empty()) {
        eprintln!("[shots] needs BOOKSHELF_DIR set to an empty folder; not taking screenshots");
        return;
    }
    let (user, piranesi) = seed(&ctx.conn).expect("seeding the demo journal");
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
                "shelf" => ctx.nav.push(&crate::home_page(&ctx, &user)),
                "book" => ctx.nav.push(&crate::editor::summary_page(&ctx, &piranesi)),
                "writer" => ctx.nav.push(&crate::writer::writer_page(&ctx, &piranesi)),
                _ => ctx.nav.push(&crate::reader::reader_page(&ctx, &piranesi)),
            }
            for theme in ["light", "dark"] {
                let mut s = get_settings(&ctx.conn, &user.id).expect("settings");
                s.theme = theme.into();
                update_settings(&ctx.conn, &s).expect("settings");
                crate::theme::apply(&s);
                pause().await;
                let file = out.join(format!("linux-{step}-{theme}.png"));
                match snap(&window, &file) {
                    Ok(()) => eprintln!("[shots] {}", file.display()),
                    Err(e) => eprintln!("[shots] {}: {e}", file.display()),
                }
            }
            if step != "shelf" {
                ctx.nav.pop(); // back to the shelf for the next page
            }
        }
        window.close();
    });
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

/// Avery's shelves from the Mac demo journal; returns Avery and Piranesi's entry.
fn seed(conn: &bookshelf_core::rusqlite::Connection) -> bookshelf_core::Result<(User, String)> {
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
