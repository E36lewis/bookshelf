//! The writing page: a quiet, centered column of monospace text in the
//! spirit of iA Writer. Markdown marks are dimmed, an optional focus mode
//! dims everything but the current sentence (and keeps it mid-screen), and
//! the text autosaves as you type.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use bookshelf_core::export;
use bookshelf_core::models::*;
use gtk::glib;

use crate::{display_path, editor, format, friendly, Ctx};

const AUTOSAVE_MS: u64 = 700;

// Hotkeys: Ctrl+F focus mode, Ctrl+S save now, F11 full screen.
const KEY_FOCUS: gtk::gdk::Key = gtk::gdk::Key::f;
const KEY_SAVE: gtk::gdk::Key = gtk::gdk::Key::s;

struct Tags {
    heading: gtk::TextTag,
    bold: gtk::TextTag,
    italic: gtk::TextTag,
    quote: gtk::TextTag,
    code: gtk::TextTag,
    strike: gtk::TextTag,
    syntax: gtk::TextTag,
    dim: gtk::TextTag, // created last = highest priority, so it wins over the rest
}

impl Tags {
    fn new(buf: &gtk::TextBuffer) -> Self {
        let make = |name: &str| buf.create_tag(Some(name), &[]).expect("tag");
        let t = Self {
            heading: make("heading"),
            bold: make("bold"),
            italic: make("italic"),
            quote: make("quote"),
            code: make("code"),
            strike: make("strike"),
            syntax: make("syntax"),
            dim: make("dim"),
        };
        t.heading.set_weight(700);
        t.heading.set_scale(1.25);
        t.bold.set_weight(700);
        t.italic.set_style(gtk::pango::Style::Italic);
        t.quote.set_style(gtk::pango::Style::Italic);
        t.strike.set_strikethrough(true);
        t
    }
}

/// Tag colors follow the widget's text color, so light and dark both work.
fn recolor(tv: &gtk::TextView, t: &Tags) {
    let ink = tv.color();
    let with = |a: f32| gtk::gdk::RGBA::new(ink.red(), ink.green(), ink.blue(), a);
    t.syntax.set_foreground_rgba(Some(&with(0.38)));
    t.quote.set_foreground_rgba(Some(&with(0.72)));
    t.code.set_background_rgba(Some(&with(0.09)));
    t.dim.set_foreground_rgba(Some(&with(0.28)));
}

pub fn writer_page(ctx: &Rc<Ctx>, summary_id: &str) -> adw::NavigationPage {
    let loaded = get_summary(&ctx.conn, summary_id)
        .and_then(|s| get_book(&ctx.conn, &s.book_id).map(|b| (s, b)));
    let (summary, book) = match loaded {
        Ok(pair) => pair,
        Err(e) => return editor::error_page(&e.to_string()),
    };

    let settings =
        get_settings(&ctx.conn, &summary.user_id).unwrap_or_else(|_| UserSettings::defaults());
    let (wrap_gap, line_gap) = match settings.line_spacing.as_str() {
        "tight" => (2, 6),
        "airy" => (12, 20),
        _ => (6, 12),
    };
    let column_width = match settings.page_width.as_str() {
        "narrow" => 600,
        "wide" => 900,
        _ => 720,
    };

    let buffer = gtk::TextBuffer::new(None);
    buffer.set_text(&summary.body);
    buffer.place_cursor(&buffer.end_iter());
    let tags = Rc::new(Tags::new(&buffer));

    let text_view = gtk::TextView::builder()
        .buffer(&buffer)
        .wrap_mode(gtk::WrapMode::WordChar)
        .pixels_inside_wrap(wrap_gap)
        .pixels_below_lines(line_gap)
        .top_margin(40)
        .bottom_margin(360) // room to scroll the last line to mid-screen
        .left_margin(24)
        .right_margin(24)
        .vexpand(true)
        .css_classes(["writer"])
        .build();

    // Faint prompts on an empty page, gone as soon as you type.
    let prompt = gtk::Label::builder()
        .label(prompts_for(summary.milestone()))
        .wrap(true)
        .xalign(0.0)
        .valign(gtk::Align::Start)
        .margin_top(40) // matches the text view's margins, so it sits where you'll type
        .margin_start(24)
        .margin_end(24)
        .can_target(false)
        .visible(summary.body.is_empty())
        .css_classes(["writer-preview", "dim-label"])
        .build();
    let page_area = gtk::Overlay::builder().child(&text_view).build();
    page_area.add_overlay(&prompt);

    let status = gtk::Label::builder().css_classes(["writer-status"]).build();
    // Counted when the text is saved, not on every keystroke.
    let words = Rc::new(Cell::new(word_count(&summary.body)));
    set_status(&status, words.get(), true);

    let focus = Rc::new(Cell::new(false));
    let dirty = Rc::new(Cell::new(false));
    let pending_save: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    // Lines (first, last) the edit in progress touches. Recorded just before
    // GTK applies an insert or delete, and used by `changed` right after.
    let touched: Rc<Cell<Option<(i32, i32)>>> = Rc::default();

    // ---- saving ---------------------------------------------------------
    // Ok when everything is in the database (or nothing needed saving).
    let save_now: Rc<dyn Fn() -> Result<(), String>> = {
        let ctx = ctx.clone();
        let id = summary.id.clone();
        let buffer = buffer.downgrade(); // the buffer owns handlers holding this
        let dirty = dirty.clone();
        let status = status.clone();
        let words = words.clone();
        Rc::new(move || {
            let Some(buffer) = buffer.upgrade() else {
                return Ok(());
            };
            if !dirty.get() {
                return Ok(());
            }
            let body = full_text(&buffer);
            match update_summary_body(&ctx.conn, &id, &body) {
                Ok(()) => {
                    dirty.set(false);
                    words.set(word_count(&body));
                    set_status(&status, words.get(), true);
                    status.remove_css_class("error");
                    Ok(())
                }
                Err(e) => {
                    let message = friendly(&e);
                    status.set_label(&format!("Not saved: {message}"));
                    status.add_css_class("error");
                    Err(message)
                }
            }
        })
    };

    {
        let touched = touched.clone();
        buffer.connect_insert_text(move |_, at, text| {
            let first = at.line();
            touched.set(Some((first, first + text.matches('\n').count() as i32)));
        });
    }
    {
        let touched = touched.clone();
        buffer.connect_delete_range(move |_, start, _| {
            touched.set(Some((start.line(), start.line()))); // what's left joins one line
        });
    }
    {
        let tags = tags.clone();
        let focus = focus.clone();
        let dirty = dirty.clone();
        let status = status.clone();
        let words = words.clone();
        let save_now = save_now.clone();
        let prompt = prompt.clone();
        buffer.connect_changed(move |b| {
            prompt.set_visible(b.char_count() == 0);
            // Highlighting is per line, so only the edited lines need redoing.
            match touched.take() {
                Some((first, last)) => {
                    restyle_lines(b, &tags, first, last);
                    if focus.get() {
                        apply_focus(b, &tags, true);
                    }
                }
                None => restyle(b, &tags, focus.get()),
            }
            dirty.set(true);
            set_status(&status, words.get(), false);

            // One autosave timer, pushed back on every change.
            if let Some(old) = pending_save.borrow_mut().take() {
                old.remove();
            }
            let slot = pending_save.clone();
            let save_now = save_now.clone();
            let id = glib::timeout_add_local_once(Duration::from_millis(AUTOSAVE_MS), move || {
                slot.borrow_mut().take(); // it has fired; nothing left to cancel
                let _ = save_now(); // a failure shows in the status line; leaving rescues the text
            });
            *pending_save.borrow_mut() = Some(id);
        });
    }

    {
        // Pasting text copied from this page brings its old highlighting
        // along, applied after our per-line restyle; redo it once the paste lands.
        let tags = tags.clone();
        let focus = focus.clone();
        buffer.connect_paste_done(move |b, _| restyle(b, &tags, focus.get()));
    }

    // ---- focus mode -----------------------------------------------------
    {
        let tags = tags.clone();
        let focus = focus.clone();
        let text_view = text_view.downgrade(); // the view owns the buffer: no cycle
        buffer.connect_cursor_position_notify(move |b| {
            // Turning focus off clears the dimming (see the button), so
            // there's nothing to do here unless it's on.
            if focus.get() {
                apply_focus(b, &tags, true);
                if let Some(tv) = text_view.upgrade() {
                    center_cursor(&tv);
                }
            }
        });
    }

    let focus_btn = gtk::ToggleButton::builder()
        .label("Focus")
        .tooltip_text("Focus mode (Ctrl+F): dim everything except the sentence you're writing")
        .build();
    {
        let tags = tags.clone();
        let focus = focus.clone();
        let buffer = buffer.clone();
        let text_view = text_view.clone();
        focus_btn.connect_toggled(move |btn| {
            focus.set(btn.is_active());
            apply_focus(&buffer, &tags, btn.is_active());
            if btn.is_active() {
                center_cursor(&text_view);
            }
        });
    }

    if settings.focus_default {
        focus_btn.set_active(true);
    }

    // ---- colors follow the theme ---------------------------------------
    {
        let tags = tags.clone();
        let focus = focus.clone();
        let buffer = buffer.clone();
        text_view.connect_map(move |tv| {
            recolor(tv, &tags);
            restyle(&buffer, &tags, focus.get());
        });
    }
    let dark_handler = Cell::new(Some({
        // Weak ref, and disconnected when the page goes (see `hidden`).
        let weak = text_view.downgrade();
        let tags = tags.clone();
        let focus = focus.clone();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            let weak = weak.clone();
            let tags = tags.clone();
            let focus = focus.clone();
            // Wait a beat so the palette swap has been applied before we read colors.
            glib::timeout_add_local_once(Duration::from_millis(80), move || {
                if let Some(tv) = weak.upgrade() {
                    recolor(&tv, &tags);
                    restyle(&tv.buffer(), &tags, focus.get());
                }
            });
        })
    }));

    // ---- layout ---------------------------------------------------------
    let title = adw::WindowTitle::new(&book.title, "Writing");
    let full_btn = gtk::Button::builder()
        .icon_name("view-fullscreen-symbolic")
        .tooltip_text("Full screen (F11)")
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&title));
    header.pack_end(&full_btn);
    header.pack_end(&focus_btn);

    let clamp = adw::Clamp::builder()
        .maximum_size(column_width)
        .tightening_threshold(column_width - 120)
        .child(&page_area)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&clamp)
        .vexpand(true)
        .css_classes(["writer-surface"])
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.set_top_bar_style(adw::ToolbarStyle::Flat);
    toolbar.set_bottom_bar_style(adw::ToolbarStyle::Flat);
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&format::bar(&text_view));
    toolbar.add_bottom_bar(&status);
    toolbar.set_content(Some(&scroll));
    toolbar.add_css_class("writer-page");

    let page = adw::NavigationPage::builder()
        .title(&book.title)
        .child(&toolbar)
        .build();

    // ---- full screen: the whole display, not just the app window ---------
    let toggle_fullscreen: Rc<dyn Fn()> = {
        let toolbar = toolbar.downgrade();
        let button = full_btn.downgrade();
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
        })
    };
    {
        let toggle = toggle_fullscreen.clone();
        full_btn.connect_clicked(move |_| toggle());
    }

    // ---- hotkeys ----------------------------------------------------------
    let shortcuts = gtk::ShortcutController::new();
    shortcuts.set_scope(gtk::ShortcutScope::Local);
    shortcuts.set_propagation_phase(gtk::PropagationPhase::Capture); // run before the text view
    let ctrl = gtk::gdk::ModifierType::CONTROL_MASK;
    let plain = gtk::gdk::ModifierType::empty();

    add_shortcut(&shortcuts, KEY_FOCUS, ctrl, {
        let focus_btn = focus_btn.clone();
        move || {
            focus_btn.set_active(!focus_btn.is_active());
            true
        }
    });
    add_shortcut(&shortcuts, KEY_SAVE, ctrl, {
        // Autosave already runs; this is for the reflex. Failures show in the status line.
        let save_now = save_now.clone();
        move || {
            let _ = save_now();
            true
        }
    });
    add_shortcut(&shortcuts, gtk::gdk::Key::F11, plain, {
        let toggle = toggle_fullscreen.clone();
        move || {
            toggle();
            true
        }
    });
    add_shortcut(&shortcuts, gtk::gdk::Key::Escape, plain, {
        let toolbar = toolbar.downgrade();
        let toggle = toggle_fullscreen.clone();
        move || {
            let fullscreen = toolbar
                .upgrade()
                .and_then(|t| t.root())
                .and_downcast::<gtk::Window>()
                .is_some_and(|w| w.is_fullscreen());
            if fullscreen {
                toggle();
            }
            fullscreen // otherwise let Escape behave as usual
        }
    });
    add_shortcut(&shortcuts, gtk::gdk::Key::b, ctrl, {
        let buffer = buffer.clone();
        move || {
            format::toggle_inline(&buffer, "**");
            true
        }
    });
    add_shortcut(&shortcuts, gtk::gdk::Key::i, ctrl, {
        let buffer = buffer.clone();
        move || {
            format::toggle_inline(&buffer, "*");
            true
        }
    });
    toolbar.add_controller(shortcuts);

    // Closing the window while writing still saves the last few words. If
    // that fails, the window stays open and says where the text was rescued to.
    let close_guard: Rc<RefCell<Option<(gtk::Window, glib::SignalHandlerId)>>> = Rc::default();
    {
        let ctx = ctx.clone();
        let title = book.title.clone();
        let buffer = buffer.downgrade();
        let text_view = text_view.clone();
        let close_guard = close_guard.clone();
        let save_now = save_now.clone();
        page.connect_shown(move |p| {
            text_view.grab_focus();
            if close_guard.borrow().is_some() {
                return;
            }
            let Some(window) = p.root().and_downcast::<gtk::Window>() else {
                return;
            };
            let ctx = ctx.clone();
            let title = title.clone();
            let buffer = buffer.clone();
            let save_now = save_now.clone();
            let force_close = Rc::new(Cell::new(false));
            let id = window.connect_close_request(move |w| {
                if force_close.get() {
                    return glib::Propagation::Proceed;
                }
                let Err(problem) = save_now() else {
                    return glib::Propagation::Proceed;
                };
                let body = buffer.upgrade().map(|b| full_text(&b)).unwrap_or_default();
                let rescued = rescue(&ctx, &title, &body, w);
                let dialog = adw::AlertDialog::builder()
                    .heading("Your writing couldn't be saved")
                    .body(format!("{problem}\n\n{rescued}"))
                    .build();
                dialog.add_responses(&[("keep", "Keep Open"), ("close", "Close Anyway")]);
                dialog.set_response_appearance("close", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("keep"));
                dialog.set_close_response("keep");
                let force_close = force_close.clone();
                let window = w.downgrade();
                dialog.connect_response(Some("close"), move |_, _| {
                    force_close.set(true);
                    if let Some(w) = window.upgrade() {
                        w.close();
                    }
                });
                dialog.present(Some(w));
                glib::Propagation::Stop
            });
            *close_guard.borrow_mut() = Some((window, id));
        });
    }
    page.connect_hidden(move |_| {
        if let Some((window, id)) = close_guard.borrow_mut().take() {
            window.disconnect(id);
        }
        if let Some(id) = dark_handler.take() {
            adw::StyleManager::default().disconnect(id);
        }
    });
    {
        let ctx = ctx.clone();
        let title = book.title.clone();
        let buffer = buffer.downgrade();
        let save_now = save_now.clone();
        page.connect_hiding(move |p| {
            if let Err(problem) = save_now() {
                // Already leaving, so keep the text somewhere and say where.
                let body = buffer.upgrade().map(|b| full_text(&b)).unwrap_or_default();
                let rescued = rescue(&ctx, &title, &body, p);
                let toast = adw::Toast::builder()
                    .title(format!("“{title}” couldn't be saved: {problem} {rescued}"))
                    .use_markup(false)
                    .timeout(0) // stays until dismissed
                    .build();
                ctx.toasts.add_toast(toast);
            }
            // Leaving the writer also leaves full screen.
            if let Some(window) = p.root().and_downcast::<gtk::Window>() {
                window.unfullscreen();
            }
        });
    }
    page
}

/// When the database won't take the text, keep it anyway: a recovery file
/// if possible, otherwise the clipboard. Returns a sentence saying where.
fn rescue(ctx: &Ctx, title: &str, body: &str, near: &impl IsA<gtk::Widget>) -> String {
    match export::save_recovery_copy(&ctx.paths, title, body) {
        Ok(path) => format!("A copy of your text is in {}.", display_path(&path)),
        Err(e) => {
            near.clipboard().set_text(body);
            format!(
                "A recovery copy couldn't be written either ({}), so your text was copied \
                 to the clipboard. Paste it somewhere safe before closing Bookshelf.",
                friendly(&e)
            )
        }
    }
}

pub(crate) fn add_shortcut(
    controller: &gtk::ShortcutController,
    key: gtk::gdk::Key,
    modifiers: gtk::gdk::ModifierType,
    action: impl Fn() -> bool + 'static,
) {
    let trigger = gtk::KeyvalTrigger::new(key, modifiers);
    let action = gtk::CallbackAction::new(move |_, _| {
        if action() {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    controller.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
}

fn full_text(buffer: &gtk::TextBuffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}

/// Questions to get you started, depending on where the book is on your shelf.
fn prompts_for(milestone: Milestone) -> &'static str {
    match milestone {
        Milestone::Eventually => {
            "Why do you want to read this one?\n\
                                  Who recommended it, and what did they say?"
        }
        Milestone::Reading => {
            "Where are you in the story?\n\
                               What has surprised you so far?\n\
                               A line worth remembering…"
        }
        Milestone::Finished => {
            "What stayed with you after the last page?\n\
                                A line worth remembering…\n\
                                Who would you give this book to?"
        }
    }
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn set_status(label: &gtk::Label, words: usize, saved: bool) {
    let noun = if words == 1 { "word" } else { "words" };
    let state = if saved { "Saved" } else { "Saving…" };
    label.set_label(&format!("{words} {noun}  ·  {state}"));
}

/// Typewriter scrolling: keep the line being written in the middle.
fn center_cursor(tv: &gtk::TextView) {
    let tv = tv.clone();
    glib::idle_add_local_once(move || {
        if let Some(insert) = tv.buffer().mark("insert") {
            tv.scroll_to_mark(&insert, 0.0, true, 0.0, 0.5);
        }
    });
}

// ------------------------------------------------------------ styling

fn restyle(buf: &gtk::TextBuffer, t: &Tags, focus: bool) {
    let (start, end) = buf.bounds();
    buf.remove_all_tags(&start, &end);
    highlight(buf, t);
    apply_focus(buf, t, focus);
}

/// Dim everything except the sentence around the cursor.
fn apply_focus(buf: &gtk::TextBuffer, t: &Tags, on: bool) {
    let (start, end) = buf.bounds();
    buf.remove_tag(&t.dim, &start, &end);
    if !on {
        return;
    }
    buf.apply_tag(&t.dim, &start, &end);

    let cursor = buf.iter_at_offset(buf.cursor_position());
    let mut from = cursor;
    if !from.starts_sentence() {
        from.backward_sentence_start();
    }
    let mut to = cursor;
    to.forward_sentence_end();
    buf.remove_tag(&t.dim, &from, &to);
}

/// Re-highlight just lines `first..=last` (focus dimming is left to the caller).
fn restyle_lines(buf: &gtk::TextBuffer, t: &Tags, first: i32, last: i32) {
    let start = buf.iter_at_line(first).unwrap_or_else(|| buf.end_iter());
    let mut end = buf.iter_at_line(last).unwrap_or_else(|| buf.end_iter());
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    buf.remove_all_tags(&start, &end);
    let chars: Vec<char> = buf.text(&start, &end, false).chars().collect();
    style_lines(buf, t, &chars, start.offset() as usize);
}

fn highlight(buf: &gtk::TextBuffer, t: &Tags) {
    let chars: Vec<char> = full_text(buf).chars().collect();
    style_lines(buf, t, &chars, 0);
}

/// `chars` is whole lines of the buffer, starting at offset `base`.
fn style_lines(buf: &gtk::TextBuffer, t: &Tags, chars: &[char], base: usize) {
    let mut line_start = 0;
    while line_start <= chars.len() {
        let mut line_end = line_start;
        while line_end < chars.len() && chars[line_end] != '\n' {
            line_end += 1;
        }
        style_line(buf, t, &chars[line_start..line_end], base + line_start);
        line_start = line_end + 1;
    }
}

fn mark(buf: &gtk::TextBuffer, tag: &gtk::TextTag, off: usize, a: usize, b: usize) {
    if a >= b {
        return;
    }
    let s = buf.iter_at_offset((off + a) as i32);
    let e = buf.iter_at_offset((off + b) as i32);
    buf.apply_tag(tag, &s, &e);
}

fn style_line(buf: &gtk::TextBuffer, t: &Tags, line: &[char], off: usize) {
    if line.is_empty() {
        return;
    }

    // "# Heading"
    let hashes = line.iter().take_while(|c| **c == '#').count();
    if (1..=6).contains(&hashes) && line.get(hashes) == Some(&' ') {
        mark(buf, &t.heading, off, 0, line.len());
        mark(buf, &t.syntax, off, 0, hashes + 1);
        style_inline(buf, t, line, off, hashes + 1);
        return;
    }

    // "> quote"
    if line[0] == '>' {
        let from = if line.get(1) == Some(&' ') { 2 } else { 1 };
        mark(buf, &t.quote, off, 0, line.len());
        mark(buf, &t.syntax, off, 0, from);
        style_inline(buf, t, line, off, from);
        return;
    }

    // "- item", "* item", "+ item", "12. item"
    let indent = line.iter().take_while(|c| **c == ' ').count();
    let mut from = 0;
    if matches!(line.get(indent), Some('-' | '*' | '+')) && line.get(indent + 1) == Some(&' ') {
        mark(buf, &t.syntax, off, indent, indent + 1);
        from = indent + 2;
    } else {
        let digits = line[indent..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digits > 0
            && line.get(indent + digits) == Some(&'.')
            && line.get(indent + digits + 1) == Some(&' ')
        {
            mark(buf, &t.syntax, off, indent, indent + digits + 1);
            from = indent + digits + 2;
        }
    }
    style_inline(buf, t, line, off, from);
}

fn style_inline(buf: &gtk::TextBuffer, t: &Tags, line: &[char], off: usize, from: usize) {
    let n = line.len();
    let mut i = from;
    while i < n {
        let c = line[i];
        match c {
            '\\' => {
                mark(buf, &t.syntax, off, i, i + 1);
                i += 2;
            }
            '`' => {
                if let Some(j) = (i + 1..n).find(|&j| line[j] == '`') {
                    mark(buf, &t.code, off, i, j + 1);
                    mark(buf, &t.syntax, off, i, i + 1);
                    mark(buf, &t.syntax, off, j, j + 1);
                    i = j + 1;
                } else {
                    i += 1;
                }
            }
            '~' => {
                if line.get(i + 1) == Some(&'~') {
                    let close =
                        (i + 2..n).find(|&j| line[j] == '~' && line.get(j + 1) == Some(&'~'));
                    match close {
                        Some(j) if j > i + 2 => {
                            mark(buf, &t.strike, off, i + 2, j);
                            mark(buf, &t.syntax, off, i, i + 2);
                            mark(buf, &t.syntax, off, j, j + 2);
                            i = j + 2;
                        }
                        _ => i += 2,
                    }
                } else {
                    i += 1;
                }
            }
            '*' | '_' => {
                if line.get(i + 1) == Some(&c) {
                    // **bold** or __bold__
                    let close = (i + 2..n).find(|&j| {
                        line[j] == c && line.get(j + 1) == Some(&c) && line[j - 1] != '\\'
                    });
                    match close {
                        Some(j) if j > i + 2 => {
                            mark(buf, &t.bold, off, i + 2, j);
                            mark(buf, &t.syntax, off, i, i + 2);
                            mark(buf, &t.syntax, off, j, j + 2);
                            i = j + 2;
                        }
                        _ => i += 2,
                    }
                } else {
                    // *italic* or _italic_ (underscores only at word edges)
                    let opens = line.get(i + 1).is_some_and(|x| !x.is_whitespace())
                        && (c == '*' || i == 0 || !line[i - 1].is_alphanumeric());
                    let close = if opens {
                        (i + 2..n).find(|&j| {
                            line[j] == c
                                && line[j - 1] != '\\'
                                && !line[j - 1].is_whitespace()
                                && line.get(j + 1) != Some(&c)
                                && (c == '*'
                                    || !line.get(j + 1).is_some_and(|x| x.is_alphanumeric()))
                        })
                    } else {
                        None
                    };
                    match close {
                        Some(j) => {
                            mark(buf, &t.italic, off, i + 1, j);
                            mark(buf, &t.syntax, off, i, i + 1);
                            mark(buf, &t.syntax, off, j, j + 1);
                            i = j + 1;
                        }
                        None => i += 1,
                    }
                }
            }
            _ => i += 1,
        }
    }
}
