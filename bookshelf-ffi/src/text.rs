//! The writing page's text logic: Markdown highlighting, the formatting
//! bar's edits, and Markdown laid out as blocks for the reading page. All
//! offsets are UTF-16 code units, as NSTextView and RichEditBox count.

use bookshelf_core::render;
use bookshelf_core::text::{edit, highlight};

use crate::types::count;

/// What a highlighted span of text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum StyleKind {
    /// A whole `# Heading` line.
    Heading,
    /// `**bold**`.
    Bold,
    /// `*italic*`.
    Italic,
    /// A whole `> quote` line.
    Quote,
    /// `` `code` ``.
    Code,
    /// `~~struck~~`.
    Strike,
    /// The Markdown marks themselves (`**`, `#`, `- `...), shown dimmed.
    Syntax,
}

/// `start..end` in UTF-16 code units, as NSTextView and RichEditBox count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Record)]
pub struct StyleSpan {
    /// What the text is.
    pub kind: StyleKind,
    /// Where it starts.
    pub start: u32,
    /// Where it ends (exclusive).
    pub end: u32,
}

/// Markdown highlighting for the writing page. Each line is highlighted on
/// its own, so after an edit only the lines it touched need restyling.
#[uniffi::export]
pub fn markdown_spans(text: String) -> Vec<StyleSpan> {
    use highlight::StyleKind as K;
    highlight::spans_utf16(&text)
        .into_iter()
        .map(|s| StyleSpan {
            kind: match s.kind {
                K::Heading => StyleKind::Heading,
                K::Bold => StyleKind::Bold,
                K::Italic => StyleKind::Italic,
                K::Quote => StyleKind::Quote,
                K::Code => StyleKind::Code,
                K::Strike => StyleKind::Strike,
                K::Syntax => StyleKind::Syntax,
            },
            start: count(s.start),
            end: count(s.end),
        })
        .collect()
}

/// A button on the formatting bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum FormatAction {
    /// `**bold**`.
    Bold,
    /// `*italic*`.
    Italic,
    /// `~~struck~~`.
    Strike,
    /// `` `code` ``.
    Code,
    /// `[text](url)`.
    Link,
    /// `#` to `######`; pressing the same level again removes it. Levels
    /// outside 1 to 6 are taken as the nearest one.
    Heading {
        /// 1 to 6.
        level: u8,
    },
    /// `> quote` lines.
    Quote,
    /// `- ` bullet lines.
    Bullets,
    /// `1. ` numbered lines.
    Numbered,
}

impl From<FormatAction> for edit::FormatAction {
    fn from(a: FormatAction) -> Self {
        match a {
            FormatAction::Bold => Self::Bold,
            FormatAction::Italic => Self::Italic,
            FormatAction::Strike => Self::Strike,
            FormatAction::Code => Self::Code,
            FormatAction::Link => Self::Link,
            FormatAction::Heading { level } => Self::Heading(level.clamp(1, 6)),
            FormatAction::Quote => Self::Quote,
            FormatAction::Bullets => Self::Bullets,
            FormatAction::Numbered => Self::Numbered,
        }
    }
}

/// One formatting button press: replace `start..end` with `replacement`
/// (as one undo step), then select `new_sel_start..new_sel_end` in the
/// edited text (equal: just place the caret). UTF-16 code units.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct TextEdit {
    /// Start of the text to replace.
    pub start: u32,
    /// End of the text to replace (exclusive).
    pub end: u32,
    /// What goes there.
    pub replacement: String,
    /// The new selection's start, in the edited text.
    pub new_sel_start: u32,
    /// The new selection's end, in the edited text.
    pub new_sel_end: u32,
}

/// What pressing `action` does to `text` with `sel_start..sel_end`
/// selected (equal: just the caret), in UTF-16 code units. `None` means
/// leave the text and selection alone. Offsets past the end count as the
/// end.
#[uniffi::export]
pub fn format_edit(
    text: String,
    sel_start: u32,
    sel_end: u32,
    action: FormatAction,
) -> Option<TextEdit> {
    let e = edit::format_edit_utf16(&text, sel_start as usize, sel_end as usize, action.into())?;
    Some(TextEdit {
        start: count(e.start),
        end: count(e.end),
        replacement: e.replacement,
        new_sel_start: count(e.new_sel_start),
        new_sel_end: count(e.new_sel_end),
    })
}

/// Whether, with nothing selected, `action` acts on the word around the
/// caret. The app finds that word with its own word rules and passes it
/// in as the selection. Line actions just take the caret's line.
#[uniffi::export]
pub fn format_expands_to_word(action: FormatAction) -> bool {
    edit::FormatAction::from(action).expands_to_word()
}

/// A block of Markdown laid out for reading. The model is flat: lists are
/// items with a depth. `ListEnd`, `ItemText` and empty runs exist for the
/// GTK app's exact spacing; other apps may ignore them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum Block {
    /// `#` to `######`.
    Heading {
        /// 1 to 6.
        level: u8,
        /// The heading's text.
        runs: Vec<Run>,
    },
    /// A paragraph. A blank line follows it.
    Paragraph {
        /// The paragraph's text.
        runs: Vec<Run>,
    },
    /// A list item, its marker indented `depth` levels (0 = top level).
    /// In a loose list (blank lines between items) a blank line follows.
    ListItem {
        /// How deeply nested the list is.
        depth: u32,
        /// A bullet, or the item's number.
        marker: ListMarker,
        /// The text right after the marker.
        runs: Vec<Run>,
        /// Whether a blank line follows.
        loose: bool,
    },
    /// More of a list item's text, after a block inside the item. Rare.
    ItemText {
        /// The text.
        runs: Vec<Run>,
    },
    /// A list ended; `depth` 0 means the whole list is over.
    ListEnd {
        /// The ended list's depth.
        depth: u32,
    },
    /// A code block, as written (usually ending in `\n`).
    CodeBlock {
        /// The code.
        text: String,
    },
    /// Raw HTML, as written. Shown as text, never as HTML.
    Html {
        /// The HTML source.
        text: String,
    },
    /// `---` on its own line.
    Rule,
}

/// A list item's marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum ListMarker {
    /// `•`.
    Bullet,
    /// `3.`: a numbered list counts up from its first number.
    Number {
        /// The item's number.
        value: u64,
    },
}

/// A stretch of text with one set of styles. A `\n` in `text` is a line
/// break the writer typed. May be empty (see `Block`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Run {
    /// The text.
    pub text: String,
    /// Outermost first, in the order they were written.
    pub styles: Vec<RunStyle>,
}

/// An inline style. (Not `Style`, which WinUI already uses.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum RunStyle {
    /// Bold.
    Bold,
    /// Italic.
    Italic,
    /// Struck through.
    Strike,
    /// Inline code. Always innermost.
    Code,
}

impl From<render::Block> for Block {
    fn from(b: render::Block) -> Self {
        use render::Block as B;
        let runs = |runs: Vec<render::Run>| runs.into_iter().map(Run::from).collect();
        match b {
            B::Heading { level, runs: r } => Block::Heading {
                level,
                runs: runs(r),
            },
            B::Paragraph { runs: r } => Block::Paragraph { runs: runs(r) },
            B::ListItem {
                depth,
                marker,
                runs: r,
                loose,
            } => Block::ListItem {
                depth: count(depth),
                marker: match marker {
                    render::Marker::Bullet => ListMarker::Bullet,
                    render::Marker::Number(value) => ListMarker::Number { value },
                },
                runs: runs(r),
                loose,
            },
            B::ItemText { runs: r } => Block::ItemText { runs: runs(r) },
            B::ListEnd { depth } => Block::ListEnd {
                depth: count(depth),
            },
            B::CodeBlock { text } => Block::CodeBlock { text },
            B::Html { text } => Block::Html { text },
            B::Rule => Block::Rule,
        }
    }
}

impl From<render::Run> for Run {
    fn from(r: render::Run) -> Self {
        Self {
            text: r.text,
            styles: r
                .styles
                .into_iter()
                .map(|s| match s {
                    render::Style::Bold => RunStyle::Bold,
                    render::Style::Italic => RunStyle::Italic,
                    render::Style::Strike => RunStyle::Strike,
                    render::Style::Code => RunStyle::Code,
                })
                .collect(),
        }
    }
}

/// Markdown laid out as blocks, for the reading page. HTML is never
/// interpreted: it comes through as the text that was typed.
#[uniffi::export]
pub fn render_markdown(text: String) -> Vec<Block> {
    render::blocks(&text).into_iter().map(Block::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_cross_in_utf16() {
        let s = markdown_spans("📚 **b**".into());
        assert_eq!(
            s[0],
            StyleSpan {
                kind: StyleKind::Bold,
                start: 5,
                end: 6
            }
        );
        assert!(s.iter().any(|s| s.kind == StyleKind::Syntax));
    }

    #[test]
    fn format_edits_count_utf16() {
        // "📚 word": the emoji is two code units, so "word" is 3..7.
        let e = format_edit("📚 word".into(), 3, 7, FormatAction::Bold).unwrap();
        assert_eq!(
            e,
            TextEdit {
                start: 3,
                end: 7,
                replacement: "**word**".into(),
                new_sel_start: 5,
                new_sel_end: 9,
            }
        );
        let h = format_edit("Title".into(), 0, 0, FormatAction::Heading { level: 2 }).unwrap();
        assert_eq!(h.replacement, "## Title");
        // Out-of-range levels and offsets are taken as the nearest.
        let h = format_edit("T".into(), 9, 99, FormatAction::Heading { level: 0 }).unwrap();
        assert_eq!(h.replacement, "# T");
        let h = format_edit("T".into(), 0, 0, FormatAction::Heading { level: 200 }).unwrap();
        assert_eq!(h.replacement, "###### T");
        // Only spaces selected: nothing to do.
        assert_eq!(
            format_edit("a   b".into(), 1, 4, FormatAction::Italic),
            None
        );
    }

    #[test]
    fn which_actions_take_the_word() {
        assert!(format_expands_to_word(FormatAction::Bold));
        assert!(format_expands_to_word(FormatAction::Link));
        assert!(!format_expands_to_word(FormatAction::Quote));
        assert!(!format_expands_to_word(FormatAction::Heading { level: 1 }));
    }

    #[test]
    fn markdown_as_blocks() {
        let blocks = render_markdown(
            "# Hi\n\nSome **bold** `code`\n\n3. three\n4. four\n\n---\n\n<div>x</div>\n".into(),
        );
        let run = |text: &str, styles: &[RunStyle]| Run {
            text: text.into(),
            styles: styles.to_vec(),
        };
        assert_eq!(
            blocks[0],
            Block::Heading {
                level: 1,
                runs: vec![run("Hi", &[])]
            }
        );
        assert_eq!(
            blocks[1],
            Block::Paragraph {
                runs: vec![
                    run("Some ", &[]),
                    run("bold", &[RunStyle::Bold]),
                    run(" ", &[]),
                    run("code", &[RunStyle::Code]),
                ]
            }
        );
        assert_eq!(
            blocks[2],
            Block::ListItem {
                depth: 0,
                marker: ListMarker::Number { value: 3 },
                runs: vec![run("three", &[])],
                loose: false,
            }
        );
        assert!(matches!(
            blocks[3],
            Block::ListItem {
                marker: ListMarker::Number { value: 4 },
                ..
            }
        ));
        assert_eq!(blocks[4], Block::ListEnd { depth: 0 });
        assert_eq!(blocks[5], Block::Rule);
        assert_eq!(
            blocks[6],
            Block::Html {
                text: "<div>x</div>\n".into()
            }
        );
    }

    #[test]
    fn bullets_and_italics() {
        let blocks = render_markdown("- *a*\n  - b\n".into());
        assert_eq!(
            blocks[0],
            Block::ListItem {
                depth: 0,
                marker: ListMarker::Bullet,
                runs: vec![Run {
                    text: "a".into(),
                    styles: vec![RunStyle::Italic]
                }],
                loose: false,
            }
        );
        assert!(matches!(blocks[1], Block::ListItem { depth: 1, .. }));
        assert_eq!(render_markdown(String::new()), vec![]);
        let code = render_markdown("```\nx ~~y~~\n```\n".into());
        assert_eq!(
            code,
            vec![Block::CodeBlock {
                text: "x ~~y~~\n".into()
            }]
        );
        let struck = render_markdown("~~gone~~".into());
        assert_eq!(
            struck,
            vec![Block::Paragraph {
                runs: vec![Run {
                    text: "gone".into(),
                    styles: vec![RunStyle::Strike]
                }]
            }]
        );
    }
}
