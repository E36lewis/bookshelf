//! The formatting bar. Every button just edits the markdown text in the
//! buffer (`**bold**`, `# Heading`, ...), so typing the marks by hand and
//! using the buttons are interchangeable. The edits themselves come from
//! `bookshelf_core::text::edit`; this file is the GTK glue.

use bookshelf_core::text::edit::{format_edit, FormatAction, TextEdit};
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
    text.append(&icon_btn(
        tv,
        "format-text-bold-symbolic",
        "Bold (Ctrl+B)",
        |b| apply(b, FormatAction::Bold),
    ));
    text.append(&icon_btn(
        tv,
        "format-text-italic-symbolic",
        "Italic (Ctrl+I)",
        |b| apply(b, FormatAction::Italic),
    ));
    text.append(&icon_btn(
        tv,
        "format-text-strikethrough-symbolic",
        "Strikethrough",
        |b| apply(b, FormatAction::Strike),
    ));
    text.append(&icon_btn(tv, "utilities-terminal-symbolic", "Code", |b| {
        apply(b, FormatAction::Code)
    }));

    let headings = group();
    headings.append(&text_btn(tv, "H1", "Heading 1", |b| {
        apply(b, FormatAction::Heading(1))
    }));
    headings.append(&text_btn(tv, "H2", "Heading 2", |b| {
        apply(b, FormatAction::Heading(2))
    }));
    headings.append(&text_btn(tv, "H3", "Heading 3", |b| {
        apply(b, FormatAction::Heading(3))
    }));

    let blocks = group();
    blocks.append(&icon_btn(tv, "format-indent-more-symbolic", "Quote", |b| {
        apply(b, FormatAction::Quote)
    }));
    blocks.append(&icon_btn(
        tv,
        "view-list-bullet-symbolic",
        "Bulleted list",
        |b| apply(b, FormatAction::Bullets),
    ));
    blocks.append(&icon_btn(
        tv,
        "view-list-ordered-symbolic",
        "Numbered list",
        |b| apply(b, FormatAction::Numbered),
    ));
    blocks.append(&icon_btn(tv, "insert-link-symbolic", "Link", |b| {
        apply(b, FormatAction::Link)
    }));

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

fn icon_btn(
    tv: &gtk::TextView,
    icon: &str,
    tip: &str,
    f: impl Fn(&gtk::TextBuffer) + 'static,
) -> gtk::Button {
    finish(gtk::Button::from_icon_name(icon), tv, tip, f)
}

fn text_btn(
    tv: &gtk::TextView,
    label: &str,
    tip: &str,
    f: impl Fn(&gtk::TextBuffer) + 'static,
) -> gtk::Button {
    finish(gtk::Button::with_label(label), tv, tip, f)
}

// ------------------------------------------------------------- apply

/// Runs one formatting button on the buffer as one undo step.
pub fn apply(buf: &gtk::TextBuffer, action: FormatAction) {
    let inline = action.expands_to_word();
    let (a, b) = if inline {
        selection_or_word(buf)
    } else {
        selection_or_caret(buf)
    };
    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
    let edit = format_edit(&text, a as usize, b as usize, action);

    buf.begin_user_action();
    if let Some(edit) = &edit {
        if inline {
            edit_around(buf, edit);
            // Inline marks set the selection inside the undo step, line
            // actions just after it, as they always have.
            select(buf, edit.new_sel_start as i32, edit.new_sel_end as i32);
        } else {
            edit_lines(buf, edit);
        }
    }
    buf.end_user_action();
    if let (Some(edit), false) = (&edit, inline) {
        select(buf, edit.new_sel_start as i32, edit.new_sel_end as i32);
    }
}

// The edit is applied in the same small steps the bar always used, because
// GTK's undo puts the caret back step by step: Ctrl+Z stays as it was.

/// Inline marks and links add text on both sides of the selection, or take
/// it away, end first. The selected text itself is never retyped.
fn edit_around(buf: &gtk::TextBuffer, edit: &TextEdit) {
    let (a, b) = (edit.start as i32, edit.end as i32);
    let old = slice(buf, a, b);
    let new = edit.replacement.as_str();
    let chars = |s: &str| s.chars().count() as i32;
    if old.is_empty() {
        insert(buf, a, new);
    } else if let Some(i) = split_around(new, &old) {
        insert(buf, b, &new[i + old.len()..]);
        insert(buf, a, &new[..i]);
    } else if let Some(i) = split_around(&old, new) {
        delete(buf, b - chars(&old[i + new.len()..]), b);
        delete(buf, a, a + chars(&old[..i]));
    } else {
        delete(buf, a, b);
        insert(buf, a, new);
    }
}

/// Where `inner` sits inside `outer` with text on both sides: preferably
/// the same text (a pair of marks), else the first such place (a link).
fn split_around(outer: &str, inner: &str) -> Option<usize> {
    let extra = outer.len().checked_sub(inner.len())?;
    let fits: Vec<usize> = (1..extra)
        .filter(|&i| outer.is_char_boundary(i) && outer[i..].starts_with(inner))
        .collect();
    fits.iter()
        .copied()
        .find(|&i| outer[..i] == outer[i + inner.len()..])
        .or(fits.first().copied())
}

/// Line actions retype each changed line, top down. The edit covers whole
/// lines and keeps their breaks, so it splits back into lines at them.
fn edit_lines(buf: &gtk::TextBuffer, edit: &TextEdit) {
    if edit.start == edit.end && edit.replacement.is_empty() {
        return; // no line changed
    }
    let mut line = buf.iter_at_offset(edit.start as i32).line();
    let mut rest = edit.replacement.as_str();
    loop {
        let (mut start, mut end) = line_bounds(buf, line);
        let mut next = end;
        next.forward_line();
        let brk = buf.text(&end, &next, false);
        let (new, more) = match rest.find(brk.as_str()).filter(|_| !brk.is_empty()) {
            Some(i) => (&rest[..i], Some(&rest[i + brk.len()..])),
            None => (rest, None),
        };
        if buf.text(&start, &end, false) != new {
            buf.delete(&mut start, &mut end);
            buf.insert(&mut start, new);
        }
        match more {
            Some(more) => rest = more,
            None => break,
        }
        line += 1;
    }
}

fn line_bounds(buf: &gtk::TextBuffer, line: i32) -> (gtk::TextIter, gtk::TextIter) {
    let start = buf.iter_at_line(line).unwrap_or_else(|| buf.end_iter());
    let mut end = start;
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    (start, end)
}

fn slice(buf: &gtk::TextBuffer, a: i32, b: i32) -> String {
    buf.text(&buf.iter_at_offset(a), &buf.iter_at_offset(b), false)
        .to_string()
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

/// The selection, or an empty range at the caret.
fn selection_or_caret(buf: &gtk::TextBuffer) -> (i32, i32) {
    match buf.selection_bounds() {
        Some((s, e)) => (s.offset(), e.offset()),
        None => {
            let o = buf.cursor_position();
            (o, o)
        }
    }
}
