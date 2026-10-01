//! Markdown syntax highlighting for the writing page, shared by every app.
//!
//! Highlighting is per line: no state carries from one line to the next, so
//! an editor only needs to restyle the lines an edit touched. The result is a
//! list of spans; each app maps the kinds to its own text styles.

/// What a span of text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    /// A whole `# Heading` line.
    Heading,
    Bold,
    Italic,
    /// A whole `> quote` line.
    Quote,
    Code,
    Strike,
    /// The Markdown marks themselves (`**`, `#`, `- `...), shown dimmed.
    Syntax,
}

/// `start..end` in the text, counted in characters (`spans`) or UTF-16
/// code units (`spans_utf16`, what macOS and Windows text views use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleSpan {
    pub kind: StyleKind,
    pub start: usize,
    pub end: usize,
}

/// Highlights `text`, with offsets in characters.
pub fn spans(text: &str) -> Vec<StyleSpan> {
    let chars: Vec<char> = text.chars().collect();
    spans_of_chars(&chars)
}

/// Same as `spans`, for text already split into characters: whole lines of
/// a larger text, with offsets relative to `chars[0]`.
pub fn spans_of_chars(chars: &[char]) -> Vec<StyleSpan> {
    let mut out = Spans(Vec::new());
    let mut line_start = 0;
    while line_start <= chars.len() {
        let mut line_end = line_start;
        while line_end < chars.len() && chars[line_end] != '\n' {
            line_end += 1;
        }
        style_line(&mut out, &chars[line_start..line_end], line_start);
        line_start = line_end + 1;
    }
    out.0
}

/// Same as `spans`, with offsets in UTF-16 code units: what NSTextView
/// (macOS) and RichEditBox (Windows) count in. It differs from `spans` only
/// after characters outside the Basic Multilingual Plane, such as emoji.
pub fn spans_utf16(text: &str) -> Vec<StyleSpan> {
    // utf16_at[i] = the UTF-16 offset of the i-th character (and one past the end).
    let mut utf16_at = Vec::with_capacity(text.len() + 1);
    let mut at = 0;
    for c in text.chars() {
        utf16_at.push(at);
        at += c.len_utf16();
    }
    utf16_at.push(at);
    spans(text)
        .into_iter()
        .map(|s| StyleSpan {
            start: utf16_at[s.start],
            end: utf16_at[s.end],
            ..s
        })
        .collect()
}

struct Spans(Vec<StyleSpan>);

impl Spans {
    /// `a..b` within the line that starts at `off`. Empty ranges are skipped.
    fn mark(&mut self, kind: StyleKind, off: usize, a: usize, b: usize) {
        if a < b {
            self.0.push(StyleSpan {
                kind,
                start: off + a,
                end: off + b,
            });
        }
    }
}

fn style_line(out: &mut Spans, line: &[char], off: usize) {
    if line.is_empty() {
        return;
    }

    // "# Heading"
    let hashes = line.iter().take_while(|c| **c == '#').count();
    if (1..=6).contains(&hashes) && line.get(hashes) == Some(&' ') {
        out.mark(StyleKind::Heading, off, 0, line.len());
        out.mark(StyleKind::Syntax, off, 0, hashes + 1);
        style_inline(out, line, off, hashes + 1);
        return;
    }

    // "> quote"
    if line[0] == '>' {
        let from = if line.get(1) == Some(&' ') { 2 } else { 1 };
        out.mark(StyleKind::Quote, off, 0, line.len());
        out.mark(StyleKind::Syntax, off, 0, from);
        style_inline(out, line, off, from);
        return;
    }

    // "- item", "* item", "+ item", "12. item"
    let indent = line.iter().take_while(|c| **c == ' ').count();
    let mut from = 0;
    if matches!(line.get(indent), Some('-' | '*' | '+')) && line.get(indent + 1) == Some(&' ') {
        out.mark(StyleKind::Syntax, off, indent, indent + 1);
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
            out.mark(StyleKind::Syntax, off, indent, indent + digits + 1);
            from = indent + digits + 2;
        }
    }
    style_inline(out, line, off, from);
}

fn style_inline(out: &mut Spans, line: &[char], off: usize, from: usize) {
    let n = line.len();
    let mut i = from;
    while i < n {
        let c = line[i];
        match c {
            '\\' => {
                out.mark(StyleKind::Syntax, off, i, i + 1);
                i += 2;
            }
            '`' => {
                if let Some(j) = (i + 1..n).find(|&j| line[j] == '`') {
                    out.mark(StyleKind::Code, off, i, j + 1);
                    out.mark(StyleKind::Syntax, off, i, i + 1);
                    out.mark(StyleKind::Syntax, off, j, j + 1);
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
                            out.mark(StyleKind::Strike, off, i + 2, j);
                            out.mark(StyleKind::Syntax, off, i, i + 2);
                            out.mark(StyleKind::Syntax, off, j, j + 2);
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
                            out.mark(StyleKind::Bold, off, i + 2, j);
                            out.mark(StyleKind::Syntax, off, i, i + 2);
                            out.mark(StyleKind::Syntax, off, j, j + 2);
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
                            out.mark(StyleKind::Italic, off, i + 1, j);
                            out.mark(StyleKind::Syntax, off, i, i + 1);
                            out.mark(StyleKind::Syntax, off, j, j + 1);
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

#[cfg(test)]
mod tests {
    use super::StyleKind::*;
    use super::*;

    /// (kind, the text it covers), in the order they're produced.
    fn marked(text: &str) -> Vec<(StyleKind, String)> {
        let chars: Vec<char> = text.chars().collect();
        spans(text)
            .into_iter()
            .map(|s| (s.kind, chars[s.start..s.end].iter().collect()))
            .collect()
    }

    fn m(list: &[(StyleKind, &str)]) -> Vec<(StyleKind, String)> {
        list.iter().map(|(k, s)| (*k, s.to_string())).collect()
    }

    #[test]
    fn headings_and_quotes_cover_the_line() {
        assert_eq!(
            marked("## Two **b**"),
            m(&[
                (Heading, "## Two **b**"),
                (Syntax, "## "),
                (Bold, "b"),
                (Syntax, "**"),
                (Syntax, "**")
            ])
        );
        assert_eq!(marked("> said"), m(&[(Quote, "> said"), (Syntax, "> ")]));
        assert_eq!(marked(">said"), m(&[(Quote, ">said"), (Syntax, ">")]));
        assert_eq!(
            marked("####### seven"),
            m(&[]),
            "only 1-6 hashes make a heading"
        );
        assert_eq!(marked("#nospace"), m(&[]));
    }

    #[test]
    fn lists() {
        assert_eq!(marked("- item"), m(&[(Syntax, "-")]));
        assert_eq!(marked("  * item"), m(&[(Syntax, "*")]));
        assert_eq!(marked("12. item"), m(&[(Syntax, "12.")]));
        assert_eq!(marked("12.item"), m(&[]));
    }

    #[test]
    fn inline_styles() {
        assert_eq!(
            marked("a **b** c"),
            m(&[(Bold, "b"), (Syntax, "**"), (Syntax, "**")])
        );
        assert_eq!(
            marked("a *i* c"),
            m(&[(Italic, "i"), (Syntax, "*"), (Syntax, "*")])
        );
        assert_eq!(
            marked("a _i_ c"),
            m(&[(Italic, "i"), (Syntax, "_"), (Syntax, "_")])
        );
        assert_eq!(
            marked("snake_case_name"),
            m(&[]),
            "underscores inside words aren't italic"
        );
        assert_eq!(
            marked("`x`"),
            m(&[(Code, "`x`"), (Syntax, "`"), (Syntax, "`")])
        );
        assert_eq!(
            marked("~~s~~"),
            m(&[(Strike, "s"), (Syntax, "~~"), (Syntax, "~~")])
        );
        assert_eq!(marked(r"\*not\*"), m(&[(Syntax, "\\"), (Syntax, "\\")]));
        assert_eq!(
            marked("**"),
            m(&[]),
            "an unclosed or empty pair marks nothing"
        );
    }

    #[test]
    fn offsets_count_across_lines() {
        let s = spans("plain\n**b**");
        assert_eq!(
            s[0],
            StyleSpan {
                kind: Bold,
                start: 8,
                end: 9
            }
        );
    }

    #[test]
    fn utf16_offsets_count_emoji_as_two() {
        // 📚 is one character but two UTF-16 code units.
        let text = "📚 **b**";
        assert_eq!(
            spans(text)[0],
            StyleSpan {
                kind: Bold,
                start: 4,
                end: 5
            }
        );
        assert_eq!(
            spans_utf16(text)[0],
            StyleSpan {
                kind: Bold,
                start: 5,
                end: 6
            }
        );
        assert_eq!(
            spans_utf16("é **b**")[0].start,
            4,
            "accents are one unit in both"
        );
    }
}
