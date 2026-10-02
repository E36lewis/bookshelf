//! The formatting bar's edits, shared by every app's writing page.
//!
//! Every button just edits the Markdown text (`**bold**`, `# Heading`, ...),
//! so typing the marks by hand and using the buttons are interchangeable.
//! Each press comes back as one replacement, which an app applies as one
//! undo step and then sets the selection it names.
//!
//! The rules themselves live in `mdedit`; this module finds the text they
//! work on and turns their answer into an edit.

use crate::mdedit::{self, InlineEdit};

/// A button on the formatting bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatAction {
    Bold,
    Italic,
    Strike,
    Code,
    Link,
    /// `#` to `######` (the bar offers three). Pressing the same level
    /// again removes it.
    Heading(u8),
    Quote,
    Bullets,
    Numbered,
}

impl FormatAction {
    /// With nothing selected, inline marks and links act on the word around
    /// the caret, which the app finds with its own word rules and passes in
    /// as the selection. The line actions just take the caret's line.
    pub fn expands_to_word(self) -> bool {
        matches!(
            self,
            Self::Bold | Self::Italic | Self::Strike | Self::Code | Self::Link
        )
    }

    fn marker(self) -> Option<&'static str> {
        match self {
            Self::Bold => Some("**"),
            Self::Italic => Some("*"),
            Self::Strike => Some("~~"),
            Self::Code => Some("`"),
            _ => None,
        }
    }
}

/// Replace `start..end` with `replacement`, then select
/// `new_sel_start..new_sel_end` (equal: just place the caret there).
/// Offsets count characters (`format_edit`) or UTF-16 code units
/// (`format_edit_utf16`); the new selection is in the edited text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub new_sel_start: usize,
    pub new_sel_end: usize,
}

/// What pressing `action` does to `text` with `sel_start..sel_end` selected
/// (equal: just the caret), in characters. `None` means leave the text and
/// the selection alone (an inline mark on a selection of only spaces).
pub fn format_edit(
    text: &str,
    sel_start: usize,
    sel_end: usize,
    action: FormatAction,
) -> Option<TextEdit> {
    let chars: Vec<char> = text.chars().collect();
    let a = sel_start.min(sel_end).min(chars.len());
    let b = sel_start.max(sel_end).min(chars.len());
    match action {
        FormatAction::Link => Some(link(&chars, a, b)),
        FormatAction::Heading(level) => Some(lines(&chars, a, b, |l| {
            mdedit::set_heading(l, level as usize)
        })),
        FormatAction::Quote => Some(lines(&chars, a, b, mdedit::toggle_quote)),
        FormatAction::Bullets => Some(lines(&chars, a, b, mdedit::toggle_bullets)),
        FormatAction::Numbered => Some(lines(&chars, a, b, mdedit::toggle_numbered)),
        _ => inline(&chars, a, b, action.marker()?),
    }
}

/// Same as `format_edit`, with offsets in and out in UTF-16 code units: what
/// NSTextView (macOS) and RichEditBox (Windows) count in. An offset inside a
/// surrogate pair counts as the start of that character.
pub fn format_edit_utf16(
    text: &str,
    sel_start: usize,
    sel_end: usize,
    action: FormatAction,
) -> Option<TextEdit> {
    // utf16_at[i] = the UTF-16 offset of the i-th character (and one past the end).
    let mut utf16_at = Vec::with_capacity(text.len() + 1);
    let mut at = 0;
    for c in text.chars() {
        utf16_at.push(at);
        at += c.len_utf16();
    }
    utf16_at.push(at);
    let to_char = |u: usize| utf16_at.partition_point(|&x| x <= u) - 1;
    let edit = format_edit(text, to_char(sel_start), to_char(sel_end), action)?;

    // The new selection is in the edited text, so its offsets past the edit
    // come from the old table, shifted by how much the edit grew or shrank.
    let old_len = utf16_at[edit.end] - utf16_at[edit.start];
    let new_len: usize = edit.replacement.chars().map(char::len_utf16).sum();
    let new_chars = edit.replacement.chars().count();
    let after_new = edit.start + new_chars;
    let map_new = |c: usize| {
        if c <= edit.start {
            utf16_at[c]
        } else if c <= after_new {
            let inside: usize = edit
                .replacement
                .chars()
                .take(c - edit.start)
                .map(char::len_utf16)
                .sum();
            utf16_at[edit.start] + inside
        } else {
            utf16_at[c - after_new + edit.end] - old_len + new_len
        }
    };
    Some(TextEdit {
        start: utf16_at[edit.start],
        end: utf16_at[edit.end],
        new_sel_start: map_new(edit.new_sel_start),
        new_sel_end: map_new(edit.new_sel_end),
        replacement: edit.replacement,
    })
}

fn string(chars: &[char]) -> String {
    chars.iter().collect()
}

// ------------------------------------------------------------ inline

fn inline(chars: &[char], mut a: usize, mut b: usize, marker: &str) -> Option<TextEdit> {
    let m = marker.chars().count();
    if a == b {
        // Nothing to wrap: drop in an empty pair and stand between them.
        return Some(TextEdit {
            start: a,
            end: a,
            replacement: format!("{marker}{marker}"),
            new_sel_start: a + m,
            new_sel_end: a + m,
        });
    }

    // Keep stray spaces outside the markers ("** word**" isn't bold).
    let text = &chars[a..b];
    let lead = text.iter().take_while(|c| c.is_whitespace()).count();
    let trail = text.iter().rev().take_while(|c| c.is_whitespace()).count();
    if lead + trail >= text.len() {
        return None;
    }
    a += lead;
    b -= trail;

    // One more character of context than the marker, so a lone `*` can
    // tell itself apart from the `**` of bold.
    let before = string(&chars[a.saturating_sub(m + 1)..a]);
    let selected = string(&chars[a..b]);
    let after = string(&chars[b..(b + m + 1).min(chars.len())]);

    Some(
        match mdedit::inline_decision(&before, &selected, &after, marker) {
            InlineEdit::UnwrapOutside => TextEdit {
                start: a - m,
                end: b + m,
                replacement: selected,
                new_sel_start: a - m,
                new_sel_end: b - m,
            },
            InlineEdit::UnwrapInside => TextEdit {
                start: a,
                end: b,
                replacement: string(&chars[a + m..b - m]),
                new_sel_start: a,
                new_sel_end: b - 2 * m,
            },
            InlineEdit::Wrap => TextEdit {
                start: a,
                end: b,
                replacement: format!("{marker}{selected}{marker}"),
                new_sel_start: a + m,
                new_sel_end: b + m,
            },
        },
    )
}

fn link(chars: &[char], a: usize, b: usize) -> TextEdit {
    if a == b {
        TextEdit {
            start: a,
            end: a,
            replacement: "[]()".into(),
            new_sel_start: a + 1,
            new_sel_end: a + 1,
        }
    } else {
        // Caret inside the parentheses, ready for the URL.
        TextEdit {
            start: a,
            end: b,
            replacement: format!("[{}]()", string(&chars[a..b])),
            new_sel_start: b + 3,
            new_sel_end: b + 3,
        }
    }
}

// ------------------------------------------------------------- lines

/// One line of the text: `start..end` is its text, `end..next` its line
/// break. Breaks are the ones GTK's text buffer counts (and Pango's):
/// `\n`, `\r\n`, `\r` and U+2029, so Windows' `\r` paragraphs work too.
struct Line {
    start: usize,
    end: usize,
    next: usize,
}

fn split_lines(chars: &[char]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < chars.len() {
        let brk = match chars[i] {
            '\r' if chars.get(i + 1) == Some(&'\n') => 2,
            '\n' | '\r' | '\u{2029}' => 1,
            _ => {
                i += 1;
                continue;
            }
        };
        out.push(Line {
            start,
            end: i,
            next: i + brk,
        });
        i += brk;
        start = i;
    }
    // The last line has no break, and is empty when the text ends with one.
    out.push(Line {
        start,
        end: chars.len(),
        next: chars.len(),
    });
    out
}

/// The line holding offset `at`.
fn line_at(lines: &[Line], at: usize) -> usize {
    lines.partition_point(|l| l.start <= at) - 1
}

/// Runs `f` over every line the selection touches (or the caret's line).
/// Afterwards those whole lines are selected, or with no selection the
/// caret goes to the end of the last one, ready to type.
fn lines(chars: &[char], a: usize, b: usize, f: impl Fn(&[String]) -> Vec<String>) -> TextEdit {
    let all = split_lines(chars);
    let first = line_at(&all, a);
    let mut last = line_at(&all, b);
    if a != b && b == all[last].start && last > first {
        last -= 1; // selection ends at the start of a line: leave that line alone
    }
    let touched = &all[first..=last];
    let old: Vec<String> = touched
        .iter()
        .map(|l| string(&chars[l.start..l.end]))
        .collect();
    let new = f(&old);

    // Only the lines that changed are replaced, keeping their breaks.
    let changed: Vec<usize> = (0..old.len()).filter(|&i| new[i] != old[i]).collect();
    let (start, end, replacement) = match (changed.first(), changed.last()) {
        (Some(&i), Some(&j)) => {
            let mut replacement = String::new();
            for k in i..=j {
                replacement.push_str(&new[k]);
                if k < j {
                    let l = &touched[k];
                    replacement.extend(&chars[l.end..l.next]);
                }
            }
            (touched[i].start, touched[j].end, replacement)
        }
        _ => (touched[0].start, touched[0].start, String::new()),
    };

    let grew: isize = (0..old.len())
        .map(|i| new[i].chars().count() as isize - old[i].chars().count() as isize)
        .sum();
    let sel_start = touched[0].start;
    let sel_end = (touched[touched.len() - 1].end as isize + grew) as usize;
    TextEdit {
        start,
        end,
        replacement,
        new_sel_start: if a != b { sel_start } else { sel_end },
        new_sel_end: sel_end,
    }
}

#[cfg(test)]
mod tests {
    use super::FormatAction::*;
    use super::*;

    /// Runs `action` on `marked`, where `«…»` is the selection and `‸` the
    /// caret, and returns the result marked the same way.
    fn run(marked: &str, action: FormatAction) -> String {
        let (text, a, b) = parse(marked);
        let Some(edit) = format_edit(&text, a, b, action) else {
            return marked.to_string();
        };
        show(&apply(&text, &edit), edit.new_sel_start, edit.new_sel_end)
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
    fn bold_wraps_and_unwraps() {
        assert_eq!(run("a «word» b", Bold), "a **«word»** b");
        assert_eq!(run("«word» rest", Bold), "**«word»** rest");
        assert_eq!(run("a **«word»** b", Bold), "a «word» b", "markers outside");
        assert_eq!(run("**«word»**", Bold), "«word»", "at the text's edges");
        assert_eq!(run("a «**word**» b", Bold), "a «word» b", "markers inside");
        assert_eq!(run("«**»", Bold), "**«**»**", "too short to unwrap");
        assert_eq!(run("«****»", Bold), "‸", "an empty pair unwraps to nothing");
        assert_eq!(run("«a\nb»", Bold), "**«a\nb»**", "across lines");
    }

    #[test]
    fn other_marks_wrap_and_unwrap() {
        assert_eq!(run("a «word» b", Strike), "a ~~«word»~~ b");
        assert_eq!(run("~~«x»~~", Strike), "«x»");
        assert_eq!(run("«x»", Code), "`«x»`");
        assert_eq!(run("`«x»`", Code), "«x»");
        assert_eq!(run("«`x`»", Code), "«x»");
        assert_eq!(run("a «word» b", Italic), "a *«word»* b");
    }

    #[test]
    fn stray_spaces_stay_outside_the_marks() {
        assert_eq!(run("a « word » b", Bold), "a  **«word»**  b");
        assert_eq!(run("«word\n»next", Bold), "**«word»**\nnext");
        assert_eq!(run("a «  » b", Bold), "a «  » b", "only spaces: no change");
        assert_eq!(
            format_edit("a \n b", 1, 4, Bold),
            None,
            "newlines count as spaces"
        );
    }

    #[test]
    fn nothing_selected_drops_in_an_empty_pair() {
        assert_eq!(run("a ‸ b", Bold), "a **‸** b");
        assert_eq!(run("‸", Italic), "*‸*");
        assert_eq!(run("‸", Strike), "~~‸~~");
        assert_eq!(run("x ‸", Code), "x `‸`");
    }

    #[test]
    fn italic_is_not_confused_with_bold() {
        assert_eq!(run("a *«word»* b", Italic), "a «word» b");
        assert_eq!(run("a «*word*» b", Italic), "a «word» b");
        assert_eq!(run("a **«word»** b", Italic), "a ***«word»*** b");
        assert_eq!(run("a «**word**» b", Italic), "a *«**word**»* b");
        assert_eq!(run("«**»", Italic), "*«**»*");
        // Bold italic: bold comes off, italic can only be added again.
        assert_eq!(run("***«word»***", Bold), "*«word»*");
        assert_eq!(run("***«word»***", Italic), "****«word»****");
        assert_eq!(run("*«word»*", Bold), "***«word»***");
    }

    #[test]
    fn links() {
        assert_eq!(run("see «here» now", Link), "see [here](‸) now");
        assert_eq!(run("a ‸ b", Link), "a [‸]() b");
        assert_eq!(run("‸", Link), "[‸]()");
        assert_eq!(
            run("a« word »b", Link),
            "a[ word ](‸)b",
            "links keep the spaces"
        );
    }

    #[test]
    fn headings() {
        assert_eq!(run("hel‸lo", Heading(1)), "# hello‸");
        assert_eq!(run("# hel‸lo", Heading(1)), "hello‸");
        assert_eq!(run("## hel‸lo", Heading(1)), "# hello‸");
        assert_eq!(run("‸", Heading(2)), "## ‸");
        assert_eq!(run("«a\nb»", Heading(2)), "«## a\n## b»");
        assert_eq!(run("«### a\n### b»", Heading(3)), "«a\nb»");
        assert_eq!(run("x\nb‸\ny", Heading(3)), "x\n### b‸\ny");
    }

    #[test]
    fn quotes_and_lists() {
        assert_eq!(run("a«b\nc»d\ne", Quote), "«> ab\n> cd»\ne");
        assert_eq!(run("«> a\nb»", Quote), "«> a\n> b»");
        assert_eq!(run("«> a\n>b»", Quote), "«a\nb»");
        assert_eq!(run("«a\n\nb»", Numbered), "«1. a\n\n2. b»");
        assert_eq!(run("«- a\n- b»", Bullets), "«a\nb»");
        assert_eq!(run("«- a\nb»", Bullets), "«- a\n- b»");
        assert_eq!(run("  «a»", Bullets), "«  - a»", "indent kept");
        assert_eq!(run("«\n\n»", Bullets), "«- \n- »\n", "blank lines");
    }

    #[test]
    fn the_caret_picks_its_line() {
        assert_eq!(run("a\n‸b", Quote), "a\n> b‸");
        assert_eq!(run("a‸\nb", Quote), "> a‸\nb");
        assert_eq!(run("a\n‸", Bullets), "a\n- ‸", "the empty last line");
        assert_eq!(run("‸a\nb", Numbered), "1. a‸\nb");
    }

    #[test]
    fn a_selection_ending_at_a_line_start_leaves_that_line() {
        assert_eq!(run("«a\n»b", Bullets), "«- a»\nb");
        assert_eq!(run("a«\n»b", Bullets), "«- a»\nb");
        assert_eq!(run("a\n«b\n»c", Numbered), "a\n«1. b»\nc");
        assert_eq!(run("«a\n\n»b", Quote), "«> a\n»\nb");
    }

    #[test]
    fn only_changed_lines_are_replaced() {
        let e = format_edit("> a\nb", 0, 5, Quote).unwrap();
        assert_eq!(
            e,
            TextEdit {
                start: 4,
                end: 5,
                replacement: "> b".into(),
                new_sel_start: 0,
                new_sel_end: 7
            }
        );
        let e = format_edit("a\nb\nc", 0, 5, Bullets).unwrap();
        assert_eq!((e.start, e.end), (0, 5));
        assert_eq!(e.replacement, "- a\n- b\n- c");
    }

    #[test]
    fn line_breaks_are_kept() {
        assert_eq!(run("«a\r\nb»", Bullets), "«- a\r\n- b»");
        assert_eq!(run("«a\rb»", Quote), "«> a\r> b»");
        assert_eq!(run("«a\u{2029}b»", Quote), "«> a\u{2029}> b»");
        assert_eq!(run("a\r\n‸b", Heading(1)), "a\r\n# b‸");
    }

    #[test]
    fn selections_may_come_backwards_or_past_the_end() {
        assert_eq!(
            format_edit("a word", 6, 2, Bold),
            format_edit("a word", 2, 6, Bold)
        );
        assert_eq!(
            format_edit("ab", 9, 9, Bold).map(|e| e.new_sel_start),
            Some(4)
        );
    }

    #[test]
    fn emoji_count_as_one_character() {
        assert_eq!(run("📚 «word»", Bold), "📚 **«word»**");
        assert_eq!(run("«📚»", Italic), "*«📚»*");
        assert_eq!(run("📚 b‸", Bullets), "- 📚 b‸");
    }

    #[test]
    fn utf16_offsets_count_emoji_as_two() {
        // 📚 is one character but two UTF-16 code units.
        assert_eq!(
            format_edit_utf16("📚 word", 3, 7, Bold),
            Some(TextEdit {
                start: 3,
                end: 7,
                replacement: "**word**".into(),
                new_sel_start: 5,
                new_sel_end: 9
            })
        );
        assert_eq!(
            format_edit_utf16("📚\nx", 4, 4, Bullets),
            Some(TextEdit {
                start: 3,
                end: 4,
                replacement: "- x".into(),
                new_sel_start: 6,
                new_sel_end: 6
            })
        );
        // The new selection lands after an emoji added by the edit itself.
        assert_eq!(
            format_edit_utf16("a 📚", 2, 4, Link),
            Some(TextEdit {
                start: 2,
                end: 4,
                replacement: "[📚]()".into(),
                new_sel_start: 7,
                new_sel_end: 7
            })
        );
        // ...and past the end of the edit, after emoji it left alone.
        assert_eq!(
            format_edit_utf16("📚 a\n> 📚", 0, 9, Quote),
            Some(TextEdit {
                start: 0,
                end: 4,
                replacement: "> 📚 a".into(),
                new_sel_start: 0,
                new_sel_end: 11
            })
        );
        assert_eq!(
            format_edit_utf16("📚 a\n📚 b", 0, 9, Bullets),
            Some(TextEdit {
                start: 0,
                end: 9,
                replacement: "- 📚 a\n- 📚 b".into(),
                new_sel_start: 0,
                new_sel_end: 13
            })
        );
    }
}
