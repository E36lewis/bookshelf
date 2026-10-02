//! Markdown -> Pango markup, for the summary preview, the reader and the
//! manual. bookshelf-core parses the Markdown into blocks (shared with the
//! Mac and Windows apps); this only turns those into markup. Every bit of
//! text is escaped, so nothing the writer typed can become markup.

use bookshelf_core::render::{self, Block, Marker, Run, Style};
use gtk::glib;

pub fn to_pango(src: &str) -> String {
    blocks_to_pango(&render::blocks(src))
}

pub fn blocks_to_pango(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            Block::Heading { level, runs } => {
                let size = match level {
                    1 => "xx-large",
                    2 => "x-large",
                    _ => "large",
                };
                out.push_str(&format!("<span size=\"{size}\" weight=\"bold\">"));
                push_runs(&mut out, runs);
                out.push_str("</span>\n\n");
            }
            Block::Paragraph { runs } => {
                push_runs(&mut out, runs);
                out.push_str("\n\n");
            }
            Block::ListItem {
                depth,
                marker,
                runs,
                loose,
            } => {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&"    ".repeat(*depth));
                match marker {
                    Marker::Number(n) => out.push_str(&format!("{n}. ")),
                    Marker::Bullet => out.push_str("• "),
                }
                push_runs(&mut out, runs);
                if *loose {
                    out.push_str("\n\n");
                }
            }
            Block::ItemText { runs } => push_runs(&mut out, runs),
            Block::ListEnd { depth } => {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                // A blank line after the whole list.
                if *depth == 0 {
                    out.push('\n');
                }
            }
            Block::CodeBlock { text } => {
                out.push_str("<tt>");
                out.push_str(&glib::markup_escape_text(text));
                out.push_str("</tt>\n\n");
            }
            // HTML you typed shows as you typed it, never as markup.
            Block::Html { text } => out.push_str(&glib::markup_escape_text(text)),
            Block::Rule => out.push_str("──────────\n\n"),
        }
    }
    out.trim_end().to_string()
}

/// Opens and closes tags only where the styles change, so `**a *b* c**`
/// stays one `<b>` with an `<i>` inside, as written.
fn push_runs(out: &mut String, runs: &[Run]) {
    let mut open: &[Style] = &[];
    for run in runs {
        let keep = open
            .iter()
            .zip(&run.styles)
            .take_while(|(a, b)| a == b)
            .count();
        for style in open[keep..].iter().rev() {
            out.push_str(close_tag(*style));
        }
        for style in &run.styles[keep..] {
            out.push_str(open_tag(*style));
        }
        out.push_str(&glib::markup_escape_text(&run.text));
        open = &run.styles;
    }
    for style in open.iter().rev() {
        out.push_str(close_tag(*style));
    }
}

fn open_tag(style: Style) -> &'static str {
    match style {
        Style::Bold => "<b>",
        Style::Italic => "<i>",
        Style::Strike => "<s>",
        Style::Code => "<tt>",
    }
}

fn close_tag(style: Style) -> &'static str {
    match style {
        Style::Bold => "</b>",
        Style::Italic => "</i>",
        Style::Strike => "</s>",
        Style::Code => "</tt>",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The renderer as it was before the parsing moved to bookshelf-core,
    /// frozen. The new one must match it byte for byte. Don't edit it.
    fn old_to_pango(src: &str) -> String {
        use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

        let mut out = String::new();
        let mut lists: Vec<Option<u64>> = vec![];

        for event in Parser::new_ext(src, Options::ENABLE_STRIKETHROUGH) {
            match event {
                Event::Start(Tag::Strong) => out.push_str("<b>"),
                Event::End(TagEnd::Strong) => out.push_str("</b>"),
                Event::Start(Tag::Strikethrough) => out.push_str("<s>"),
                Event::End(TagEnd::Strikethrough) => out.push_str("</s>"),
                Event::Start(Tag::Emphasis) => out.push_str("<i>"),
                Event::End(TagEnd::Emphasis) => out.push_str("</i>"),
                Event::Start(Tag::Heading { level, .. }) => {
                    let size = match level {
                        HeadingLevel::H1 => "xx-large",
                        HeadingLevel::H2 => "x-large",
                        _ => "large",
                    };
                    out.push_str(&format!("<span size=\"{size}\" weight=\"bold\">"))
                }
                Event::End(TagEnd::Heading(_)) => out.push_str("</span>\n\n"),
                Event::End(TagEnd::Paragraph) => out.push_str("\n\n"),
                Event::Start(Tag::List(start)) => {
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    lists.push(start);
                }
                Event::End(TagEnd::List(_)) => {
                    lists.pop();
                    if lists.is_empty() {
                        out.push('\n');
                    }
                }
                Event::Start(Tag::Item) => {
                    let indent = "    ".repeat(lists.len().saturating_sub(1));
                    match lists.last_mut() {
                        Some(Some(n)) => {
                            out.push_str(&format!("{indent}{n}. "));
                            *n += 1;
                        }
                        _ => out.push_str(&format!("{indent}• ")),
                    }
                }
                Event::End(TagEnd::Item) => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Event::Start(Tag::CodeBlock(_)) => out.push_str("<tt>"),
                Event::End(TagEnd::CodeBlock) => out.push_str("</tt>\n\n"),
                Event::Code(text) => {
                    out.push_str("<tt>");
                    out.push_str(&glib::markup_escape_text(&text));
                    out.push_str("</tt>");
                }
                Event::Text(text) => out.push_str(&glib::markup_escape_text(&text)),
                // HTML you typed shows as you typed it, never as markup.
                Event::Html(text) | Event::InlineHtml(text) => {
                    out.push_str(&glib::markup_escape_text(&text))
                }
                Event::SoftBreak | Event::HardBreak => out.push('\n'),
                Event::Rule => out.push_str("──────────\n\n"),
                _ => {}
            }
        }
        out.trim_end().to_string()
    }

    /// One fixture per construct, plus a few that mix them.
    const FIXTURES: &[&str] = &[
        "",
        "Just a line.",
        // headings, every level, ATX and setext
        "# One\n\n## Two\n\n### Three\n\n#### Four\n\n##### Five\n\n###### Six",
        "# **Bold** and *italic* heading\n\nText under it.",
        "Setext one\nover two lines\n===\n\nSetext two\n---",
        // styles, alone and nested every way
        "**bold** *italic* _also_ __bold__ ~~strike~~ `code`",
        "***bold italic*** and **_bold italic_** and _**italic bold**_",
        "**bold *italic `code` inside* ~~struck **deeper**~~ end**",
        "~~struck *italic **bold `code`***~~ and `<b>` in code",
        "*a **b** c* **a *b* c** ~~a **b** c~~",
        // spans side by side, which must not merge into one
        "**a**__b__ and *a*_b_ and _**a**_*__b__* and *x **a**__b__ y*",
        "*![](empty.png)* and **[a link](https://example.com)** and ![alt *text*](x.png)",
        // lists: bullets, numbers, start numbers, nesting, loose
        "- one\n- two\n- three",
        "* star\n+ plus\n- dash",
        "1. one\n2. two\n3. three",
        "7. seven\n8. eight\n\n0. zero\n1. one",
        "- a\n  - b\n    - c\n      - d\n  - e\n- f",
        "1. a\n   1. a.1\n   2. a.2\n2. b\n   - bullet\n     3. three",
        "- loose\n\n- items\n\n  with more\n\n- here",
        "- a\n\n  - nested loose\n\n  after\n- b",
        "Text right before\n- a list\n\nand after.",
        "- **bold** item\n- `code` item\n- ~~struck~~ *item*",
        "-\n- empty first",
        "- # heading item\n- ---\n- > quoted item",
        "- a\n  ```\n  code in item\n  ```\n  more text",
        "- a\n  - b\n  ```\n  x\n  ```\n- c",
        "1. one\n\n   ```\n   code\n   ```\n2. two",
        "- item\n\n      indented code\n- next",
        "- **a**\n  > **b**\n- *c*\n  ```\n  x\n  ```\n  *d*",
        // code blocks
        "```\nfn main() {\n    println!(\"<hi> & bye\");\n}\n```",
        "```rust\nlet x = 1;\n```\n\n    indented\n    code\n\nafter",
        "```\n```",
        // rules
        "above\n\n---\n\nbelow\n\n***\n\n___",
        // breaks
        "soft\nbreak\nhere",
        "hard  \nbreak\\\nhere",
        "**bold over\na soft break** and *italic  \nover a hard one*",
        // raw HTML, inline and block
        "I <3 this & that > those",
        "Some <b>bold</b> HTML and <i>italic</i> and <span style=\"x\">span</span>",
        "<div>\nblock html\n</div>\n\nafter the block",
        "<b>not bold</b>\n\npara",
        "<!-- a comment -->\n\ntext <br> and <br/>",
        "- <div>\n- html in item",
        "<script>alert('x')</script>",
        // emoji and other non-ASCII
        "📚 Reading *😀 emoji* **👨‍👩‍👧 family** — “quotes” ‘and’ é ñ 中文",
        "- 📖 one\n- 🎉 two",
        // escapes and other Pango-special characters
        "Ampersand & less < greater > quote \" apostrophe ' \\*not italic\\*",
        "> quoted\n> lines\n\n> > nested quote",
        "Line with a footnote[^1] and a [ref link][r].\n\n[r]: https://example.com\n[^1]: not enabled",
        "| a | b |\n|---|---|\n| 1 | 2 |",
        "- [ ] task\n- [x] done",
        "trailing spaces   \n\n\n\n",
        "\n\n# after blank lines",
    ];

    #[test]
    fn matches_the_old_renderer_on_every_construct() {
        for md in FIXTURES {
            assert_eq!(to_pango(md), old_to_pango(md), "{md:?}");
        }
    }

    #[test]
    fn matches_the_old_renderer_on_the_manual() {
        let manual = crate::help::MANUAL;
        assert_eq!(to_pango(manual), old_to_pango(manual));
        let parsed = bookshelf_core::manual::parse_manual(manual);
        for section in &parsed.sections {
            assert_eq!(
                to_pango(&section.markdown),
                old_to_pango(&section.markdown),
                "{}",
                section.anchor
            );
        }
        assert_eq!(to_pango(&parsed.intro), old_to_pango(&parsed.intro));
    }

    /// Every pair and triple of the fixtures run together, to shake out
    /// spacing between constructs that no single fixture shows.
    #[test]
    fn matches_the_old_renderer_on_mixtures() {
        let parts: Vec<&str> = FIXTURES.iter().copied().filter(|f| !f.is_empty()).collect();
        for sep in ["\n", "\n\n", "\n  "] {
            for a in &parts {
                for b in &parts {
                    let md = format!("{a}{sep}{b}");
                    assert_eq!(to_pango(&md), old_to_pango(&md), "{md:?}");
                }
            }
        }
        for (i, a) in parts.iter().enumerate() {
            let b = parts[(i * 7 + 3) % parts.len()];
            let c = parts[(i * 13 + 5) % parts.len()];
            let md = format!("{a}\n{b}\n\n{c}");
            assert_eq!(to_pango(&md), old_to_pango(&md), "{md:?}");
        }
    }

    #[test]
    fn everything_is_valid_markup() {
        for md in FIXTURES {
            gtk::pango::parse_markup(&to_pango(md), '\0').expect(md);
        }
    }
}
