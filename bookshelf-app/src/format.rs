//! The formatting bar. Every button just edits the markdown text in the
//! buffer (`**bold**`, `# Heading`, ...), so typing the marks by hand and
//! using the buttons are interchangeable.

use bookshelf_core::mdedit::{self, InlineEdit};
use gtk::prelude::*;

pub fn bar(tv: &gtk::TextView) -> gtk::Box {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Center)
        .spacing(14)
        .margin_top(2)
        .margin_bottom(8)
        .css_classes(["format-bar"])
        .build();
    let group = || {
        gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .css_classes(["linked"])
            .build()
    };

    let text = group();
    text.append(&icon_btn(tv, "format-text-bold-symbolic", "Bold (Ctrl+B)", |b| toggle_inline(b, "**")));
    text.append(&icon_btn(tv, "format-text-italic-symbolic", "Italic (Ctrl+I)", |b| toggle_inline(b, "*")));
    text.append(&icon_btn(tv, "format-text-strikethrough-symbolic", "Strikethrough", |b| toggle_inline(b, "~~")));
    text.append(&icon_btn(tv, "utilities-terminal-symbolic", "Code", |b| toggle_inline(b, "`")));

    let headings = group();
    headings.append(&text_btn(tv, "H1", "Heading 1", |b| lines(b, |l| mdedit::set_heading(l, 1))));
    headings.append(&text_btn(tv, "H2", "Heading 2", |b| lines(b, |l| mdedit::set_heading(l, 2))));
    headings.append(&text_btn(tv, "H3", "Heading 3", |b| lines(b, |l| mdedit::set_heading(l, 3))));

    let blocks = group();
    blocks.append(&icon_btn(tv, "format-indent-more-symbolic", "Quote", |b| lines(b, mdedit::toggle_quote)));
    blocks.append(&icon_btn(tv, "view-list-bullet-symbolic", "Bulleted list", |b| lines(b, mdedit::toggle_bullets)));
    blocks.append(&icon_btn(tv, "view-list-ordered-symbolic", "Numbered list", |b| lines(b, mdedit::toggle_numbered)));
    blocks.append(&icon_btn(tv, "insert-link-symbolic", "Link", insert_link));

    root.append(&text);
    root.append(&headings);
    root.append(&blocks);
    root
}

fn finish(
    btn: gtk::Button,
    tv: &gtk::TextView,
    tip: &str,
    action: impl Fn(&gtk::TextBuffer) + 'static,
) -> gtk::Button {
    btn.add_css_class("flat");
    btn.set_focus_on_click(false); // keep the selection and the caret where they are
    btn.set_tooltip_text(Some(tip));
    let weak = tv.downgrade();
    btn.connect_clicked(move |_| {
        if let Some(tv) = weak.upgrade() {
            action(&tv.buffer());
            tv.grab_focus();
        }
    });
    btn
}

fn icon_btn(tv: &gtk::TextView, icon: &str, tip: &str, f: impl Fn(&gtk::TextBuffer) + 'static) -> gtk::Button {
    finish(gtk::Button::from_icon_name(icon), tv, tip, f)
}

fn text_btn(tv: &gtk::TextView, label: &str, tip: &str, f: impl Fn(&gtk::TextBuffer) + 'static) -> gtk::Button {
    finish(gtk::Button::with_label(label), tv, tip, f)
}

// ---------------------------------------------------- buffer helpers

fn slice(buf: &gtk::TextBuffer, a: i32, b: i32) -> String {
    buf.text(&buf.iter_at_offset(a), &buf.iter_at_offset(b), false).to_string()
}

fn delete(buf: &gtk::TextBuffer, a: i32, b: i32) {
    let mut s = buf.iter_at_offset(a);
    let mut e = buf.iter_at_offset(b);
    buf.delete(&mut s, &mut e);
}

fn insert(buf: &gtk::TextBuffer, at: i32, text: &str) {
    let mut it = buf.iter_at_offset(at);
    buf.insert(&mut it, text);
}

fn select(buf: &gtk::TextBuffer, a: i32, b: i32) {
    if a == b {
        buf.place_cursor(&buf.iter_at_offset(a));
    } else {
        buf.select_range(&buf.iter_at_offset(a), &buf.iter_at_offset(b));
    }
}

/// The selection, or the word around the caret, or an empty range at the caret.
fn selection_or_word(buf: &gtk::TextBuffer) -> (i32, i32) {
    if let Some((s, e)) = buf.selection_bounds() {
        return (s.offset(), e.offset());
    }
    let caret = buf.iter_at_offset(buf.cursor_position());
    if caret.inside_word() || caret.ends_word() {
        let mut s = caret;
        if !s.starts_word() {
            s.backward_word_start();
        }
        let mut e = caret;
        if !e.ends_word() {
            e.forward_word_end();
        }
        (s.offset(), e.offset())
    } else {
        let o = caret.offset();
        (o, o)
    }
}

// ------------------------------------------------------------ inline

pub fn toggle_inline(buf: &gtk::TextBuffer, marker: &str) {
    let m = marker.chars().count() as i32;
    let (mut a, mut b) = selection_or_word(buf);

    buf.begin_user_action();
    if a == b {
        // Nothing to wrap: drop in an empty pair and stand between them.
        insert(buf, a, &format!("{marker}{marker}"));
        select(buf, a + m, a + m);
        buf.end_user_action();
        return;
    }

    // Keep stray spaces outside the markers ("** word**" isn't bold).
    let text: Vec<char> = slice(buf, a, b).chars().collect();
    let lead = text.iter().take_while(|c| c.is_whitespace()).count() as i32;
    let trail = text.iter().rev().take_while(|c| c.is_whitespace()).count() as i32;
    if lead + trail >= text.len() as i32 {
        buf.end_user_action();
        return;
    }
    a += lead;
    b -= trail;

    let len = buf.char_count();
    let before = slice(buf, (a - m - 1).max(0), a);
    let selected = slice(buf, a, b);
    let after = slice(buf, b, (b + m + 1).min(len));

    match mdedit::inline_decision(&before, &selected, &after, marker) {
        InlineEdit::UnwrapOutside => {
            delete(buf, b, b + m);
            delete(buf, a - m, a);
            select(buf, a - m, b - m);
        }
        InlineEdit::UnwrapInside => {
            delete(buf, b - m, b);
            delete(buf, a, a + m);
            select(buf, a, b - 2 * m);
        }
        InlineEdit::Wrap => {
            insert(buf, b, marker);
            insert(buf, a, marker);
            select(buf, a + m, b + m);
        }
    }
    buf.end_user_action();
}

pub fn insert_link(buf: &gtk::TextBuffer) {
    let (a, b) = selection_or_word(buf);
    buf.begin_user_action();
    if a == b {
        insert(buf, a, "[]()");
        select(buf, a + 1, a + 1);
    } else {
        insert(buf, b, "]()");
        insert(buf, a, "[");
        select(buf, b + 3, b + 3); // caret inside the parentheses, ready for the URL
    }
    buf.end_user_action();
}

// ------------------------------------------------------------- lines

fn line_bounds(buf: &gtk::TextBuffer, line: i32) -> (gtk::TextIter, gtk::TextIter) {
    let start = buf.iter_at_line(line).unwrap_or_else(|| buf.end_iter());
    let mut end = start;
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    (start, end)
}

fn line_text(buf: &gtk::TextBuffer, line: i32) -> String {
    let (s, e) = line_bounds(buf, line);
    buf.text(&s, &e, false).to_string()
}

fn set_line(buf: &gtk::TextBuffer, line: i32, text: &str) {
    let (mut s, mut e) = line_bounds(buf, line);
    buf.delete(&mut s, &mut e);
    buf.insert(&mut s, text);
}

/// First and last line touched by the selection (or the caret's line).
fn selected_lines(buf: &gtk::TextBuffer) -> (i32, i32) {
    match buf.selection_bounds() {
        Some((s, e)) => {
            let first = s.line();
            let mut last = e.line();
            if e.line_offset() == 0 && last > first {
                last -= 1; // selection ends at the start of a line: leave that line alone
            }
            (first, last)
        }
        None => {
            let l = buf.iter_at_offset(buf.cursor_position()).line();
            (l, l)
        }
    }
}

fn lines(buf: &gtk::TextBuffer, f: impl Fn(&[String]) -> Vec<String>) {
    let had_selection = buf.selection_bounds().is_some();
    let (first, last) = selected_lines(buf);
    let old: Vec<String> = (first..=last).map(|n| line_text(buf, n)).collect();
    let new = f(&old);

    buf.begin_user_action();
    for (i, n) in (first..=last).enumerate() {
        if new[i] != old[i] {
            set_line(buf, n, &new[i]);
        }
    }
    buf.end_user_action();

    let start = buf.iter_at_line(first).unwrap_or_else(|| buf.end_iter());
    let (_, end) = line_bounds(buf, last);
    if had_selection {
        buf.select_range(&start, &end);
    } else {
        buf.place_cursor(&end); // caret at the end of the line, ready to type
    }
}
