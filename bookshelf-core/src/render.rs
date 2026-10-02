//! Markdown -> a small, neutral block model, shared by every front end (the
//! GTK app turns it into Pango markup; the Mac and Windows apps into their
//! own text types). Line breaks in the source are kept, since this is a
//! reading journal, not a web page, and HTML you typed is never interpreted:
//! it comes through as the literal text you typed.
//!
//! The model is flat: lists are items with a depth, not a tree. A few parts
//! of it (`loose`, `ItemText`, `ListEnd`) exist only so the GTK app can keep
//! its exact spacing; other views can lean on them or ignore them.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// `#` to `######`; `level` is 1 to 6.
    Heading { level: u8, runs: Vec<Run> },
    /// A paragraph. A blank line follows it.
    Paragraph { runs: Vec<Run> },
    /// A list item: its marker, indented `depth` levels (0 = top level), and
    /// the text right after the marker. In a tight list (no blank lines
    /// between items) that's usually the whole item. In a loose list the
    /// text is the item's first paragraph and `loose` is set: a blank line
    /// follows it. Anything else in the item (more paragraphs, a code block,
    /// a nested list) follows as its own blocks, until the next item or the
    /// list's `ListEnd`.
    ListItem {
        depth: usize,
        marker: Marker,
        runs: Vec<Run>,
        loose: bool,
    },
    /// More of a tight list item's text, after a block inside the item
    /// (say, a code block). Rare; it continues the item without a gap.
    ItemText { runs: Vec<Run> },
    /// A list ended. `depth` is the ended list's depth, so 0 means the whole
    /// list is over.
    ListEnd { depth: usize },
    /// A fenced or indented code block, as written (usually ending in `\n`).
    CodeBlock { text: String },
    /// A raw HTML block, as written. Shown as text, never as HTML.
    Html { text: String },
    /// `---`, `***` or `___` on its own line.
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Bullet,
    /// The item's number: a numbered list counts up from its first number.
    Number(u64),
}

/// A stretch of text with one set of styles. A `\n` in `text` is a line
/// break the writer typed. Inline HTML is plain text here too.
///
/// A run's text may be empty: an empty span (`*![](x.png)*`), or a marker
/// between two like spans side by side (`**a**__b__`), carrying only the
/// styles that run on across it. Markup renderers close tags there; other
/// views can skip empty runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    /// Outermost first, in the order they were written (`***x***` is
    /// italic around bold). Most views only need `has`; the order lets
    /// markup renderers nest their tags the way the source did.
    pub styles: Vec<Style>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Bold,
    Italic,
    Strike,
    /// Inline code. Always innermost.
    Code,
}

impl Run {
    pub fn has(&self, style: Style) -> bool {
        self.styles.contains(&style)
    }
}

pub fn blocks(md: &str) -> Vec<Block> {
    let mut b = Builder::default();
    for event in Parser::new_ext(md, Options::ENABLE_STRIKETHROUGH) {
        b.event(event);
    }
    b.flush_bare();
    b.blocks
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    /// Inline text not yet placed in a block.
    runs: Vec<Run>,
    /// The styles open right now, outermost first.
    styles: Vec<Style>,
    /// `runs.len()` when each open style started, to spot empty spans.
    opened: Vec<usize>,
    /// How many of the open styles have stayed open since the last run.
    kept: usize,
    /// Each open list's next number (`None` for bullets).
    lists: Vec<Option<u64>>,
    /// The last block is a list item that hasn't had a block inside it yet,
    /// so bare text (and a first paragraph) belongs to it.
    item_open: bool,
    /// The paragraph being read is a loose item's first one.
    para_in_item: bool,
    /// Text of the code or HTML block being read.
    raw: Option<String>,
}

impl Builder {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(Tag::Strong) => self.open(Style::Bold),
            Event::Start(Tag::Emphasis) => self.open(Style::Italic),
            Event::Start(Tag::Strikethrough) => self.open(Style::Strike),
            Event::End(TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough) => self.close(),
            Event::Start(Tag::Paragraph) => {
                self.flush_bare();
                self.para_in_item = self.item_open;
                self.item_open = false;
            }
            Event::End(TagEnd::Paragraph) => {
                let runs = std::mem::take(&mut self.runs);
                if std::mem::take(&mut self.para_in_item) {
                    if let Some(Block::ListItem {
                        runs: item, loose, ..
                    }) = self.blocks.last_mut()
                    {
                        append(item, runs);
                        *loose = true;
                    }
                } else {
                    self.blocks.push(Block::Paragraph { runs });
                }
            }
            Event::Start(Tag::Heading { .. }) => self.start_block(),
            Event::End(TagEnd::Heading(level)) => {
                let runs = std::mem::take(&mut self.runs);
                self.blocks.push(Block::Heading {
                    level: heading_level(level),
                    runs,
                });
            }
            Event::Start(Tag::List(start)) => {
                self.start_block();
                self.lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                self.flush_bare();
                self.item_open = false;
                self.lists.pop();
                self.blocks.push(Block::ListEnd {
                    depth: self.lists.len(),
                });
            }
            Event::Start(Tag::Item) => {
                self.flush_bare();
                let depth = self.lists.len().saturating_sub(1);
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        Marker::Number(*n - 1)
                    }
                    _ => Marker::Bullet,
                };
                self.blocks.push(Block::ListItem {
                    depth,
                    marker,
                    runs: vec![],
                    loose: false,
                });
                self.item_open = true;
            }
            Event::End(TagEnd::Item) => {
                self.flush_bare();
                self.item_open = false;
            }
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => {
                self.start_block();
                self.raw = Some(String::new());
            }
            Event::End(TagEnd::CodeBlock) => {
                let text = self.raw.take().unwrap_or_default();
                self.blocks.push(Block::CodeBlock { text });
            }
            Event::End(TagEnd::HtmlBlock) => {
                let text = self.raw.take().unwrap_or_default();
                self.blocks.push(Block::Html { text });
            }
            Event::Rule => {
                self.start_block();
                self.blocks.push(Block::Rule);
            }
            Event::Text(text) | Event::Html(text) => match &mut self.raw {
                Some(raw) => raw.push_str(&text),
                None => self.text(&text, false),
            },
            Event::Code(text) => self.text(&text, true),
            Event::InlineHtml(text) => self.text(&text, false),
            Event::SoftBreak | Event::HardBreak => self.text("\n", false),
            // Links and images show their text; block quotes their contents.
            _ => {}
        }
    }

    fn open(&mut self, style: Style) {
        self.styles.push(style);
        self.opened.push(self.runs.len());
    }

    fn close(&mut self) {
        // A span with nothing in it (an image with no alt text, say) still
        // gets an empty run, so markup renderers keep its tags.
        if self.opened.pop() == Some(self.runs.len()) {
            self.push("", self.styles.clone(), false);
        }
        self.styles.pop();
        self.kept = self.kept.min(self.styles.len());
    }

    fn text(&mut self, text: &str, code: bool) {
        let mut styles = self.styles.clone();
        if code {
            styles.push(Style::Code);
        }
        // Code spans are always their own run.
        self.push(text, styles, !code);
    }

    fn push(&mut self, text: &str, styles: Vec<Style>, merge: bool) {
        if let Some(last) = self.runs.last_mut() {
            // Neighbouring text in the same spans is one run (the parser
            // splits text at every `[`, `<`, ...).
            if merge && last.styles == styles && self.kept == styles.len() {
                last.text.push_str(text);
                return;
            }
            // Spans that closed and opened again in between (`**a**__b__`)
            // get an empty run with only the spans that carried on.
            if self.kept < shared(&last.styles, &styles) {
                self.runs.push(Run {
                    text: String::new(),
                    styles: styles[..self.kept].to_vec(),
                });
            }
        }
        self.runs.push(Run {
            text: text.to_string(),
            styles,
        });
        self.kept = self.styles.len();
    }

    /// A block begins: any bare text so far is done, and an item's later
    /// text no longer sits next to its marker.
    fn start_block(&mut self) {
        self.flush_bare();
        self.item_open = false;
    }

    /// Places text that isn't in a paragraph or heading: a tight list
    /// item's.
    fn flush_bare(&mut self) {
        if self.runs.is_empty() {
            return;
        }
        let runs = std::mem::take(&mut self.runs);
        match self.blocks.last_mut() {
            Some(Block::ListItem { runs: item, .. }) if self.item_open => append(item, runs),
            Some(Block::ItemText { runs: more }) => append(more, runs),
            _ => self.blocks.push(Block::ItemText { runs }),
        }
    }
}

/// How many styles, outermost first, two runs have in common.
fn shared(a: &[Style], b: &[Style]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// Adds runs read separately to a block's runs. Every span closed in
/// between, so like spans either side are kept apart.
fn append(runs: &mut Vec<Run>, more: Vec<Run>) {
    if let (Some(last), Some(first)) = (runs.last(), more.first()) {
        if shared(&last.styles, &first.styles) > 0 {
            runs.push(Run {
                text: String::new(),
                styles: vec![],
            });
        }
    }
    runs.extend(more);
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, styles: &[Style]) -> Run {
        Run {
            text: text.to_string(),
            styles: styles.to_vec(),
        }
    }

    fn plain(text: &str) -> Vec<Run> {
        vec![run(text, &[])]
    }

    #[test]
    fn paragraphs_keep_their_line_breaks() {
        assert_eq!(
            blocks("one\ntwo  \nthree\n\nfour"),
            vec![
                Block::Paragraph {
                    runs: plain("one\ntwo\nthree")
                },
                Block::Paragraph {
                    runs: plain("four")
                },
            ]
        );
    }

    #[test]
    fn styles_nest_in_the_order_written() {
        assert_eq!(
            blocks("***a*** **_b_** ~~`c`~~"),
            vec![Block::Paragraph {
                runs: vec![
                    run("a", &[Style::Italic, Style::Bold]),
                    run(" ", &[]),
                    run("b", &[Style::Bold, Style::Italic]),
                    run(" ", &[]),
                    run("c", &[Style::Strike, Style::Code]),
                ]
            }]
        );
        let r = run("x", &[Style::Bold, Style::Code]);
        assert!(r.has(Style::Bold) && r.has(Style::Code) && !r.has(Style::Italic));
    }

    #[test]
    fn headings_rules_and_code() {
        assert_eq!(
            blocks("# One\n\n### Three\n\n---\n\n```\nlet x = 1;\n```\n"),
            vec![
                Block::Heading {
                    level: 1,
                    runs: plain("One")
                },
                Block::Heading {
                    level: 3,
                    runs: plain("Three")
                },
                Block::Rule,
                Block::CodeBlock {
                    text: "let x = 1;\n".into()
                },
            ]
        );
    }

    #[test]
    fn lists_are_flat_with_depth() {
        assert_eq!(
            blocks("3. a\n4. b\n   - c\n"),
            vec![
                Block::ListItem {
                    depth: 0,
                    marker: Marker::Number(3),
                    runs: plain("a"),
                    loose: false
                },
                Block::ListItem {
                    depth: 0,
                    marker: Marker::Number(4),
                    runs: plain("b"),
                    loose: false
                },
                Block::ListItem {
                    depth: 1,
                    marker: Marker::Bullet,
                    runs: plain("c"),
                    loose: false
                },
                Block::ListEnd { depth: 1 },
                Block::ListEnd { depth: 0 },
            ]
        );
    }

    #[test]
    fn a_loose_items_first_paragraph_is_its_text() {
        assert_eq!(
            blocks("- a\n\n  more\n- b\n"),
            vec![
                Block::ListItem {
                    depth: 0,
                    marker: Marker::Bullet,
                    runs: plain("a"),
                    loose: true
                },
                Block::Paragraph {
                    runs: plain("more")
                },
                Block::ListItem {
                    depth: 0,
                    marker: Marker::Bullet,
                    runs: plain("b"),
                    loose: true
                },
                Block::ListEnd { depth: 0 },
            ]
        );
    }

    #[test]
    fn html_is_kept_as_text() {
        assert_eq!(
            blocks("I <3 <b>this</b>\n\n<div>\nhi\n</div>\n"),
            vec![
                Block::Paragraph {
                    runs: plain("I <3 <b>this</b>")
                },
                Block::Html {
                    text: "<div>\nhi\n</div>\n".into()
                },
            ]
        );
    }

    #[test]
    fn an_empty_span_keeps_a_run() {
        assert_eq!(
            blocks("*![](x.png)*"),
            vec![Block::Paragraph {
                runs: vec![run("", &[Style::Italic])]
            }]
        );
    }

    #[test]
    fn spans_side_by_side_stay_apart() {
        assert_eq!(
            blocks("**a**__b__ *x **c**__d__*"),
            vec![Block::Paragraph {
                runs: vec![
                    run("a", &[Style::Bold]),
                    run("", &[]),
                    run("b", &[Style::Bold]),
                    run(" ", &[]),
                    run("x ", &[Style::Italic]),
                    run("c", &[Style::Italic, Style::Bold]),
                    run("", &[Style::Italic]),
                    run("d", &[Style::Italic, Style::Bold]),
                ]
            }]
        );
    }
}
