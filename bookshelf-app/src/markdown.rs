//! Tiny markdown -> Pango markup renderer for the summary preview.
//! Supports bold, italic, headings, bullet/numbered lists, inline code,
//! code blocks and rules. Line breaks in the source are kept, since this is
//! a reading journal, not a web page.

use gtk::glib;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn to_pango(src: &str) -> String {
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
