//! The list keys on the writing page, shared by every app: Enter carries a
//! list on, Tab and Shift+Tab nest and un-nest its items. Like the
//! formatting bar, each key press comes back as one replacement, which an
//! app applies as one undo step.
//!
//! - **Enter** in a list item starts the next one below, with the same
//!   indent and the next marker: `- `, `* ` and `+ ` stay as they are,
//!   `1. ` and `1) ` count on, and a task (`- [ ] `, `- [x] `) gives an
//!   unticked one. Text after the caret moves into the new item.
//! - **Enter in an empty item** (only its marker) ends the list: the marker
//!   goes, leaving a blank line, and the caret goes below it (in Markdown,
//!   a line right under an item is more of that item). A nested empty item
//!   un-nests one level instead, becoming the next item of the list it was
//!   nested in, so pressing Enter again ends that one too.
//! - **Tab** nests each list item it covers under the item above, at that
//!   item's text (two spaces past `- `, three past `1. `); **Shift+Tab**
//!   brings each back out to its parent's indent.
//!
//! Anywhere else (an ordinary line, the caret in a list item's marker, a
//! fenced code block) there's no answer, and the app does what the key
//! always does there. Shift+Enter is never a list key: it's a plain line
//! break. Quotes don't carry on: a line after one joins the quote anyway.

use super::edit::{in_utf16, line_at, split_lines, string, Line, TextEdit};

/// A list key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKey {
    /// Enter (Return), without modifiers.
    Enter,
    /// Tab: nest.
    Indent,
    /// Shift+Tab: un-nest.
    Outdent,
}

/// What pressing `key` does to `text` with `sel_start..sel_end` selected
/// (equal: just the caret), in characters.
///
/// `None`: not a list key here, so the app does what it usually does. An
/// edit replacing nothing with nothing means the key was the list's but
/// there's nothing to do (Shift+Tab on an item that isn't nested): the app
/// swallows the key and leaves the text alone.
pub fn list_edit(text: &str, sel_start: usize, sel_end: usize, key: ListKey) -> Option<TextEdit> {
    let chars: Vec<char> = text.chars().collect();
    let a = sel_start.min(sel_end).min(chars.len());
    let b = sel_start.max(sel_end).min(chars.len());
    let lines = split_lines(&chars);
    match key {
        ListKey::Enter => enter(&chars, &lines, a, b),
        ListKey::Indent | ListKey::Outdent => nest(&chars, &lines, a, b, key == ListKey::Indent),
    }
}

/// Same as `list_edit`, with offsets in and out in UTF-16 code units: what
/// NSTextView (macOS) and RichEditBox (Windows) count in.
pub fn list_edit_utf16(
    text: &str,
    sel_start: usize,
    sel_end: usize,
    key: ListKey,
) -> Option<TextEdit> {
    in_utf16(text, sel_start, sel_end, |a, b| list_edit(text, a, b, key))
}

// ------------------------------------------------------------- items

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Bullet(char),
    /// `digits` long (zero-padded if `padded`), then `.` or `)`.
    Number {
        value: u64,
        digits: usize,
        padded: bool,
        delim: char,
    },
}

/// A list item's line, in characters from the line's start.
#[derive(Debug, Clone, Copy)]
struct Item {
    /// The indent's length in characters, and its width in columns (a tab
    /// reaches the next multiple of four, as in Markdown).
    indent: usize,
    width: usize,
    marker: Marker,
    /// Where the marker ends, and the spaces after it (1 to 4).
    marker_end: usize,
    gap: usize,
    task: bool,
    /// Where the item's text starts: past the spaces, and a task's box.
    text: usize,
}

impl Item {
    /// The columns from the indent to the item's text (a task's box aside):
    /// how far in an item nested under this one goes.
    fn nest_width(&self) -> usize {
        self.marker_end - self.indent + self.gap
    }

    /// The marker, spaces and box of the item after this one.
    fn next(&self) -> String {
        let marker = match self.marker {
            Marker::Bullet(c) => c.to_string(),
            Marker::Number {
                value,
                digits,
                padded,
                delim,
            } => {
                let next = value.saturating_add(1);
                if padded {
                    format!("{next:0digits$}{delim}")
                } else {
                    format!("{next}{delim}")
                }
            }
        };
        let task = if self.task { "[ ] " } else { "" };
        format!("{marker}{}{task}", " ".repeat(self.gap))
    }
}

fn indent_width(indent: &[char]) -> usize {
    indent.iter().fold(0, |w, c| match c {
        '\t' => w + 4 - w % 4,
        _ => w + 1,
    })
}

/// The list item on `line`, if it is one.
fn parse_item(line: &[char]) -> Option<Item> {
    let indent = line.iter().take_while(|c| matches!(c, ' ' | '\t')).count();
    let rest = &line[indent..];
    let (marker, len) = match rest.first()? {
        &c @ ('-' | '*' | '+') => {
            // `- - -` and `* * *` are rules, not lists.
            let marks = rest.iter().filter(|&&x| x == c).count();
            if marks >= 3 && rest.iter().all(|&x| x == c || x == ' ' || x == '\t') {
                return None;
            }
            (Marker::Bullet(c), 1)
        }
        _ => {
            // Markdown allows up to nine digits.
            let digits = rest.iter().take_while(|c| c.is_ascii_digit()).count();
            let delim = *rest.get(digits)?;
            if !(1..=9).contains(&digits) || !matches!(delim, '.' | ')') {
                return None;
            }
            let number: String = rest[..digits].iter().collect();
            let marker = Marker::Number {
                value: number.parse().ok()?,
                digits,
                padded: digits > 1 && rest[0] == '0',
                delim,
            };
            (marker, digits + 1)
        }
    };
    let marker_end = indent + len;
    let spaces = line[marker_end..].iter().take_while(|&&c| c == ' ').count();
    if spaces == 0 {
        return None; // "-word" or "1.5" isn't an item
    }
    let after = marker_end + spaces;
    // Five spaces or more start an indented code block inside the item, so
    // only the first one belongs to the marker. On an empty item, one.
    let gap = if spaces > 4 || after == line.len() {
        1
    } else {
        spaces
    };
    let mut text = marker_end + gap;
    let task = matches!(line.get(text..text + 3), Some(['[', ' ' | 'x' | 'X', ']']))
        && matches!(line.get(text + 3), None | Some(' '));
    if task {
        text = (text + 4).min(line.len());
    }
    Some(Item {
        indent,
        width: indent_width(&line[..indent]),
        marker,
        marker_end,
        gap,
        task,
        text,
    })
}

fn line_chars<'a>(chars: &'a [char], line: &Line) -> &'a [char] {
    &chars[line.start..line.end]
}

fn is_blank(line: &[char]) -> bool {
    line.iter().all(|c| c.is_whitespace())
}

/// For each line, whether it's inside a fenced code block (a fence itself
/// counts as inside). Lists don't carry on in code.
fn fenced(chars: &[char], lines: &[Line], upto: usize) -> Vec<bool> {
    let mut open: Option<(char, usize)> = None;
    let mut out = Vec::with_capacity(upto + 1);
    for line in &lines[..=upto] {
        let l = line_chars(chars, line);
        let at = l.iter().take_while(|c| matches!(c, ' ' | '\t')).count();
        let fence = match l.get(at) {
            Some(&c @ ('`' | '~')) => Some((c, l[at..].iter().take_while(|&&x| x == c).count())),
            _ => None,
        };
        match (open, fence) {
            (None, Some((c, n))) if n >= 3 => {
                open = Some((c, n));
                out.push(true);
            }
            (Some((c, n)), Some((f, m))) if f == c && m >= n && is_blank(&l[at + m..]) => {
                open = None;
                out.push(true);
            }
            _ => out.push(open.is_some()),
        }
    }
    out
}

/// The nearest item above line `i`, among the lines of the same list, that
/// `want` picks out by its indent's width; `None` once the list is behind
/// (a line of ordinary text that isn't indented under an item).
fn above(
    chars: &[char],
    lines: &[Line],
    i: usize,
    want: impl Fn(usize) -> Want,
) -> Option<(usize, Item)> {
    for j in (0..i).rev() {
        let l = line_chars(chars, &lines[j]);
        if is_blank(l) {
            continue;
        }
        match parse_item(l) {
            Some(item) => match want(item.width) {
                Want::This => return Some((j, item)),
                Want::Stop => return None,
                Want::Further => {}
            },
            // More of an item's text, indented under it.
            None if matches!(l.first(), Some(' ' | '\t')) => {}
            None => return None,
        }
    }
    None
}

enum Want {
    This,
    Further,
    Stop,
}

/// The item on line `i` nests under the item before it at the same depth,
/// at that item's text. The first item of a list has none, so it moves in
/// by its own marker's width.
fn nested_width(chars: &[char], lines: &[Line], i: usize, item: &Item) -> usize {
    let sibling = above(chars, lines, i, |w| match w.cmp(&item.width) {
        std::cmp::Ordering::Equal => Want::This,
        std::cmp::Ordering::Less => Want::Stop,
        std::cmp::Ordering::Greater => Want::Further,
    });
    match sibling {
        Some((_, s)) => s.width + s.nest_width(),
        None => item.width + item.nest_width(),
    }
}

/// The item the one on line `i` is nested under (its parent), if found.
fn parent(chars: &[char], lines: &[Line], i: usize, item: &Item) -> Option<(usize, Item)> {
    if item.width == 0 {
        return None;
    }
    above(chars, lines, i, |w| {
        if w < item.width {
            Want::This
        } else {
            Want::Further
        }
    })
}

// ------------------------------------------------------------- Enter

fn enter(chars: &[char], lines: &[Line], a: usize, b: usize) -> Option<TextEdit> {
    let i = line_at(lines, a);
    if line_at(lines, b) != i {
        return None; // a selection across lines: Enter just replaces it
    }
    let line = &lines[i];
    let l = line_chars(chars, line);
    let item = parse_item(l)?;
    if a - line.start < item.text || fenced(chars, lines, i)[i] {
        return None; // the caret is in the marker (or the indent): a plain line break
    }

    // The line's own break, else the one before it (the last line has none).
    let brk = if line.next > line.end {
        string(&chars[line.end..line.next])
    } else if i > 0 {
        let before = &lines[i - 1];
        string(&chars[before.end..before.next])
    } else {
        "\n".to_string()
    };

    if is_blank(&l[item.text..]) {
        // An empty item: un-nest it, or end the list.
        let prefix = match parent(chars, lines, i, &item) {
            Some((p, parent)) => {
                let p = line_chars(chars, &lines[p]);
                format!("{}{}", string(&p[..parent.indent]), parent.next())
            }
            None if item.width > 0 => {
                let width = item.width.saturating_sub(item.nest_width());
                format!(
                    "{}{}",
                    " ".repeat(width),
                    string(&l[item.indent..item.text])
                )
            }
            // Ended: the line is left blank and the caret goes below it, as
            // text right under an item would carry the item on.
            None if i > 0 && !is_blank(line_chars(chars, &lines[i - 1])) => brk,
            None => String::new(),
        };
        let caret = line.start + prefix.chars().count();
        return Some(narrow(chars, line.start, line.end, &prefix, (caret, caret)));
    }

    let replacement = format!("{brk}{}{}", string(&l[..item.indent]), item.next());
    let caret = a + replacement.chars().count();
    Some(TextEdit {
        start: a,
        end: b,
        replacement,
        new_sel_start: caret,
        new_sel_end: caret,
    })
}

// ------------------------------------------------------------- Tab

fn nest(chars: &[char], lines: &[Line], a: usize, b: usize, inward: bool) -> Option<TextEdit> {
    let first = line_at(lines, a);
    let mut last = line_at(lines, b);
    if a != b && b == lines[last].start && last > first {
        last -= 1; // selection ends at the start of a line: leave that line alone
    }
    let fenced = fenced(chars, lines, last);

    // Each item's new indent, worked out from the text as it was.
    let mut changes = Vec::new(); // (line, old indent length, new indent)
    let mut any_item = false;
    for i in first..=last {
        let l = line_chars(chars, &lines[i]);
        let Some(item) = parse_item(l).filter(|_| !fenced[i]) else {
            continue;
        };
        any_item = true;
        let indent = if inward {
            " ".repeat(nested_width(chars, lines, i, &item))
        } else if item.width == 0 {
            continue;
        } else if let Some((p, parent)) = parent(chars, lines, i, &item) {
            string(&line_chars(chars, &lines[p])[..parent.indent])
        } else {
            " ".repeat(item.width.saturating_sub(item.nest_width()))
        };
        if string(&l[..item.indent]) != indent {
            changes.push((i, item.indent, indent));
        }
    }
    if !any_item {
        return None;
    }
    let (Some(&(i, ..)), Some(&(j, ..))) = (changes.first(), changes.last()) else {
        // Items, but none can move: the key is still theirs.
        return Some(TextEdit {
            start: a,
            end: a,
            replacement: String::new(),
            new_sel_start: a,
            new_sel_end: b,
        });
    };

    // The selection keeps to its text: past an indent it moves with the
    // line, inside one it stays within the new indent. A selection that
    // starts a line keeps the whole line.
    let moved = |at: usize, stay: bool| {
        let mut shift = 0isize;
        for (line, old, new) in &changes {
            let start = lines[*line].start;
            if at < start {
                break;
            }
            let new = new.chars().count();
            if at < start + old || (stay && at == start) {
                return (start as isize + shift) as usize + (at - start).min(new);
            }
            shift += new as isize - *old as isize;
        }
        (at as isize + shift) as usize
    };

    let mut replacement = String::new();
    let mut from = lines[i].start;
    for (line, old, new) in &changes {
        let start = lines[*line].start;
        replacement.extend(&chars[from..start]);
        replacement.push_str(new);
        from = start + old;
    }
    let end = lines[j].start + changes[changes.len() - 1].1;
    replacement.extend(&chars[from..end]);
    Some(narrow(
        chars,
        lines[i].start,
        end,
        &replacement,
        (moved(a, a != b), moved(b, false)),
    ))
}

/// Replacing `start..end` with `replacement`, trimmed to the part that
/// changes, so an app retypes (and restyles) as little as it can.
fn narrow(
    chars: &[char],
    start: usize,
    end: usize,
    replacement: &str,
    selection: (usize, usize),
) -> TextEdit {
    let old = &chars[start..end];
    let new: Vec<char> = replacement.chars().collect();
    let head = old.iter().zip(&new).take_while(|(x, y)| x == y).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    TextEdit {
        start: start + head,
        end: end - tail,
        replacement: string(&new[head..new.len() - tail]),
        new_sel_start: selection.0,
        new_sel_end: selection.1,
    }
}

#[cfg(test)]
mod tests {
    use super::ListKey::*;
    use super::*;

    /// Presses `key` in `marked`, where `«…»` is the selection and `‸` the
    /// caret, and returns the result marked the same way; unchanged and
    /// with a `∅` in front when the key isn't the list's.
    fn press(marked: &str, key: ListKey) -> String {
        let (text, a, b) = parse(marked);
        match list_edit(&text, a, b, key) {
            Some(edit) => show(&apply(&text, &edit), edit.new_sel_start, edit.new_sel_end),
            None => format!("∅{marked}"),
        }
    }

    fn parse(marked: &str) -> (String, usize, usize) {
        let mut text = String::new();
        let (mut a, mut b) = (None, None);
        for c in marked.chars() {
            let at = text.chars().count();
            match c {
                '«' | '‸' => a = Some(at),
                '»' => b = Some(at),
                _ => text.push(c),
            }
        }
        let a = a.expect("a selection or a caret");
        (text, a, b.unwrap_or(a))
    }

    fn apply(text: &str, e: &TextEdit) -> String {
        let chars: Vec<char> = text.chars().collect();
        format!(
            "{}{}{}",
            string(&chars[..e.start]),
            e.replacement,
            string(&chars[e.end..])
        )
    }

    fn show(text: &str, a: usize, b: usize) -> String {
        let mut out = String::new();
        for (i, c) in text.chars().chain(['\0']).enumerate() {
            if a == b && i == a {
                out.push('‸');
            } else if a != b && i == a {
                out.push('«');
            } else if a != b && i == b {
                out.push('»');
            }
            if c != '\0' {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn enter_continues_bullets() {
        assert_eq!(press("- a‸", Enter), "- a\n- ‸");
        assert_eq!(press("* a‸", Enter), "* a\n* ‸");
        assert_eq!(press("+ a‸", Enter), "+ a\n+ ‸");
        assert_eq!(press("x\n- a‸\ny", Enter), "x\n- a\n- ‸\ny");
        assert_eq!(press("  - nested‸", Enter), "  - nested\n  - ‸");
        assert_eq!(
            press("\t- tab‸", Enter),
            "\t- tab\n\t- ‸",
            "the indent as typed"
        );
    }

    #[test]
    fn enter_counts_numbers_on() {
        assert_eq!(press("1. a‸", Enter), "1. a\n2. ‸");
        assert_eq!(press("1) a‸", Enter), "1) a\n2) ‸");
        assert_eq!(press("9. a‸", Enter), "9. a\n10. ‸");
        assert_eq!(press("07. a‸", Enter), "07. a\n08. ‸");
        assert_eq!(press("   3. c‸", Enter), "   3. c\n   4. ‸");
        assert_eq!(
            press("1.  wide‸", Enter),
            "1.  wide\n2.  ‸",
            "the spacing kept"
        );
        assert_eq!(press("1234567890. ten‸", Enter), "∅1234567890. ten‸");
    }

    #[test]
    fn enter_gives_tasks_an_empty_box() {
        assert_eq!(press("- [ ] a‸", Enter), "- [ ] a\n- [ ] ‸");
        assert_eq!(press("- [x] done‸", Enter), "- [x] done\n- [ ] ‸");
        assert_eq!(press("- [X] done‸", Enter), "- [X] done\n- [ ] ‸");
        assert_eq!(press("1. [x] a‸", Enter), "1. [x] a\n2. [ ] ‸");
        assert_eq!(press("- [x]not‸", Enter), "- [x]not\n- ‸", "not a box");
    }

    #[test]
    fn enter_mid_line_takes_the_rest_along() {
        assert_eq!(press("- ab‸cd", Enter), "- ab\n- ‸cd");
        assert_eq!(press("- ‸abc", Enter), "- \n- ‸abc", "at the text's start");
        assert_eq!(press("1. a«bc»d", Enter), "1. a\n2. ‸d", "a selection goes");
        assert_eq!(press("- [ ] a‸b", Enter), "- [ ] a\n- [ ] ‸b");
    }

    #[test]
    fn enter_in_the_marker_is_a_plain_line_break() {
        assert_eq!(press("‸- a", Enter), "∅‸- a");
        assert_eq!(press("-‸ a", Enter), "∅-‸ a");
        assert_eq!(press("  ‸- a", Enter), "∅  ‸- a");
        assert_eq!(press("- [ ‸] a", Enter), "∅- [ ‸] a");
        assert_eq!(press("«- a\n- b»", Enter), "∅«- a\n- b»", "across lines");
    }

    #[test]
    fn enter_elsewhere_is_left_alone() {
        assert_eq!(press("plain‸", Enter), "∅plain‸");
        assert_eq!(press("‸", Enter), "∅‸");
        assert_eq!(press("-word‸", Enter), "∅-word‸");
        assert_eq!(press("-‸", Enter), "∅-‸");
        assert_eq!(press("1.5 litres‸", Enter), "∅1.5 litres‸");
        assert_eq!(press("# - not a list‸", Enter), "∅# - not a list‸");
        assert_eq!(press("> quote‸", Enter), "∅> quote‸");
        assert_eq!(press("- - -‸", Enter), "∅- - -‸", "a rule");
        assert_eq!(press("* * *‸", Enter), "∅* * *‸", "a rule");
        assert_eq!(press("- -‸", Enter), "- -\n- ‸", "two dashes are an item");
    }

    #[test]
    fn enter_in_a_code_block_is_left_alone() {
        assert_eq!(press("```\n- a‸\n```", Enter), "∅```\n- a‸\n```");
        assert_eq!(press("~~~~md\n1. a‸", Enter), "∅~~~~md\n1. a‸", "unclosed");
        assert_eq!(press("```\nx\n```\n- a‸", Enter), "```\nx\n```\n- a\n- ‸");
        assert_eq!(
            press("````\n```\n- a‸\n````", Enter),
            "∅````\n```\n- a‸\n````",
            "a shorter fence doesn't close it"
        );
        assert_eq!(
            press("- x\n  ```\n  - a‸\n  ```", Enter),
            "∅- x\n  ```\n  - a‸\n  ```",
            "in an item"
        );
    }

    #[test]
    fn enter_on_an_empty_item_ends_the_list() {
        // The item's line is left blank, and the caret goes below it: text
        // typed right under an item would carry the item on.
        assert_eq!(press("- a\n- ‸", Enter), "- a\n\n‸");
        assert_eq!(press("- a\n-   ‸", Enter), "- a\n\n‸", "trailing spaces");
        assert_eq!(press("1. a\n2. ‸", Enter), "1. a\n\n‸");
        assert_eq!(press("- [ ] a\n- [ ] ‸", Enter), "- [ ] a\n\n‸");
        assert_eq!(press("- a\n- ‸\nnext", Enter), "- a\n\n‸\nnext");
        assert_eq!(press("- a\r\n- ‸", Enter), "- a\r\n\r\n‸");
        // With nothing above it (or a blank line), just the marker goes.
        assert_eq!(press("- ‸\nnext", Enter), "‸\nnext");
        assert_eq!(press("a\n\n- ‸", Enter), "a\n\n‸");
    }

    #[test]
    fn enter_on_an_empty_nested_item_un_nests_it() {
        assert_eq!(press("- a\n  - b\n  - ‸", Enter), "- a\n  - b\n- ‸");
        assert_eq!(
            press("1. a\n   - b\n   - ‸", Enter),
            "1. a\n   - b\n2. ‸",
            "the next item of the outer list"
        );
        assert_eq!(
            press("- a\n  - b\n    - c\n    - ‸", Enter),
            "- a\n  - b\n    - c\n  - ‸",
            "one level at a time"
        );
        assert_eq!(press("  - ‸", Enter), "- ‸", "no parent: by its own width");
        assert_eq!(press("- [x] a\n  - [ ] ‸", Enter), "- [x] a\n- [ ] ‸");
    }

    #[test]
    fn tab_nests_under_the_item_above() {
        assert_eq!(press("- a\n- b‸", Indent), "- a\n  - b‸");
        assert_eq!(press("1. a\n2. b‸", Indent), "1. a\n   2. b‸");
        assert_eq!(press("10. a\n11. b‸", Indent), "10. a\n    11. b‸");
        assert_eq!(press("1) a\n2) b‸", Indent), "1) a\n   2) b‸");
        assert_eq!(press("- [ ] a\n- [ ] b‸", Indent), "- [ ] a\n  - [ ] b‸");
        assert_eq!(
            press("- a\n  - b\n    - c\n  - d‸", Indent),
            "- a\n  - b\n    - c\n    - d‸",
            "past the deeper items, to the sibling"
        );
        assert_eq!(
            press("- a\n\n- b‸", Indent),
            "- a\n\n  - b‸",
            "across a blank line"
        );
        assert_eq!(
            press("- a\n  more of a\n- b‸", Indent),
            "- a\n  more of a\n  - b‸",
            "across an item's text"
        );
    }

    #[test]
    fn tab_on_a_first_item_moves_it_in_by_its_marker() {
        assert_eq!(press("- a‸", Indent), "  - a‸");
        assert_eq!(press("para\n1. a‸", Indent), "para\n   1. a‸");
        assert_eq!(press("- a\n  - b‸", Indent), "- a\n    - b‸");
    }

    #[test]
    fn tab_keeps_the_caret_with_its_text() {
        assert_eq!(press("- a\n- b‸c", Indent), "- a\n  - b‸c");
        assert_eq!(press("- a\n‸- b", Indent), "- a\n  ‸- b");
        assert_eq!(press("- a\n  - b‸", Outdent), "- a\n- b‸");
        assert_eq!(
            press("- a\n ‸ - b", Outdent),
            "- a\n‸- b",
            "inside the indent"
        );
    }

    #[test]
    fn shift_tab_un_nests_to_the_parent() {
        assert_eq!(press("- a\n  - b‸", Outdent), "- a\n- b‸");
        assert_eq!(
            press("1. a\n   - b‸", Outdent),
            "1. a\n- b‸",
            "the marker stays"
        );
        assert_eq!(press("- a\n  - b\n    - c‸", Outdent), "- a\n  - b\n  - c‸");
        assert_eq!(
            press("\t- a\n\t\t- b‸", Outdent),
            "\t- a\n\t- b‸",
            "the parent's indent as typed"
        );
        assert_eq!(
            press("    - a‸", Outdent),
            "  - a‸",
            "no parent: by its marker"
        );
        assert_eq!(press(" - a‸", Outdent), "- a‸");
    }

    #[test]
    fn shift_tab_on_an_outer_item_does_nothing_but_takes_the_key() {
        let edit = list_edit("- a", 3, 3, Outdent).unwrap();
        assert_eq!(
            edit,
            TextEdit {
                start: 3,
                end: 3,
                replacement: String::new(),
                new_sel_start: 3,
                new_sel_end: 3
            }
        );
        assert_eq!(press("«- a\n- b»", Outdent), "«- a\n- b»");
    }

    #[test]
    fn tab_elsewhere_is_left_alone() {
        assert_eq!(press("plain‸", Indent), "∅plain‸");
        assert_eq!(press("plain‸", Outdent), "∅plain‸");
        assert_eq!(press("«a\nb»", Indent), "∅«a\nb»");
        assert_eq!(press("```\n- a‸\n```", Indent), "∅```\n- a‸\n```");
    }

    #[test]
    fn tab_takes_every_item_selected() {
        assert_eq!(
            press("- a\n«- b\n- c»", Indent),
            "- a\n«  - b\n  - c»",
            "both under a"
        );
        assert_eq!(press("«- a\n- b\n- c»", Indent), "«  - a\n  - b\n  - c»");
        assert_eq!(
            press("- a\n«- b\nplain\n- c»", Indent),
            "- a\n«  - b\nplain\n  - c»",
            "ordinary lines stay"
        );
        assert_eq!(press("- a\n  «- b\n  - c»", Outdent), "- a\n«- b\n- c»");
        assert_eq!(
            press("- a\n«  - b\n- c»", Outdent),
            "- a\n«- b\n- c»",
            "only what can move"
        );
        assert_eq!(
            press("- a\n«- b\n»- c", Indent),
            "- a\n«  - b\n»- c",
            "a selection ending at a line's start leaves that line"
        );
    }

    #[test]
    fn only_what_changes_is_replaced() {
        let edit = list_edit("- a\n- b\nc", 7, 7, Indent).unwrap();
        assert_eq!(
            edit,
            TextEdit {
                start: 4,
                end: 4,
                replacement: "  ".into(),
                new_sel_start: 9,
                new_sel_end: 9
            }
        );
        let edit = list_edit("- a\n  - b", 9, 9, Outdent).unwrap();
        assert_eq!(
            (edit.start, edit.end, edit.replacement.as_str()),
            (4, 6, "")
        );
        let edit = list_edit("- a\n- ", 6, 6, Enter).unwrap();
        assert_eq!(
            (edit.start, edit.end, edit.replacement.as_str()),
            (4, 6, "\n")
        );
    }

    #[test]
    fn line_breaks_are_kept() {
        assert_eq!(press("- a‸\r\n- b", Enter), "- a\r\n- ‸\r\n- b");
        assert_eq!(
            press("x\r\n- a‸", Enter),
            "x\r\n- a\r\n- ‸",
            "the last line"
        );
        assert_eq!(press("- a\r\n- b‸", Indent), "- a\r\n  - b‸");
        assert_eq!(press("- a‸\rb", Enter), "- a\r- ‸\rb");
        assert_eq!(press("- a\u{2029}- b‸", Indent), "- a\u{2029}  - b‸");
    }

    #[test]
    fn emoji_and_cjk_are_one_character() {
        assert_eq!(press("- 📚 読書‸", Enter), "- 📚 読書\n- ‸");
        assert_eq!(press("- 📚‸読書", Enter), "- 📚\n- ‸読書");
        assert_eq!(press("- 📚\n- 読書‸", Indent), "- 📚\n  - 読書‸");
    }

    #[test]
    fn utf16_offsets_count_emoji_as_two() {
        // "- 📚x": 📚 is two code units, so x is at 4 and the end is 5.
        assert_eq!(
            list_edit_utf16("- 📚x", 4, 4, Enter),
            Some(TextEdit {
                start: 4,
                end: 4,
                replacement: "\n- ".into(),
                new_sel_start: 7,
                new_sel_end: 7
            })
        );
        assert_eq!(
            list_edit_utf16("- 📚\n- b", 8, 8, Indent),
            Some(TextEdit {
                start: 5,
                end: 5,
                replacement: "  ".into(),
                new_sel_start: 10,
                new_sel_end: 10
            })
        );
        assert_eq!(list_edit_utf16("📚", 2, 2, Enter), None);
    }

    #[test]
    fn selections_may_come_backwards_or_past_the_end() {
        assert_eq!(
            list_edit("- a\n- b", 7, 4, Indent),
            list_edit("- a\n- b", 4, 7, Indent)
        );
        assert_eq!(
            list_edit("- a", 99, 99, Enter).map(|e| e.replacement),
            Some("\n- ".into())
        );
    }
}
