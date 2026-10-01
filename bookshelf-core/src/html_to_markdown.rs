//! Converts Action Text / Trix HTML into markdown for the migration.
//! Handles what Trix produces: bold, italic, strike, links, headings,
//! quotes, code, lists (nested), and div/br line structure.

pub fn html_to_markdown(html: &str) -> String {
    let mut c = Conv::default();
    let mut i = 0;

    while i < html.len() {
        let rest = &html[i..];
        if rest.starts_with('<') {
            if let Some(end) = rest.find('>') {
                c.tag(&rest[1..end]);
                i += end + 1;
                continue;
            }
        }
        // A run of text up to the next '<' (always take at least one char).
        let first = rest.chars().next().map(char::len_utf8).unwrap_or(1);
        let stop = rest[first..]
            .find('<')
            .map(|p| p + first)
            .unwrap_or(rest.len());
        c.text(&rest[..stop]);
        i += stop;
    }

    let mut s = c.out;
    while s.contains("\n\n\n") {
        s = s.replace("\n\n\n", "\n\n");
    }
    s.trim().to_string()
}

#[derive(Default)]
struct Conv {
    out: String,
    lists: Vec<(bool, u32)>, // (ordered, next number)
    links: Vec<Option<String>>,
    in_pre: bool,
    quote: usize,
}

impl Conv {
    fn ensure_newline(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    fn block_start(&mut self) {
        self.ensure_newline();
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    fn block_end(&mut self) {
        while self.out.ends_with(' ') {
            self.out.pop();
        }
        if self.out.is_empty() || self.out.ends_with("\n\n") {
            return;
        }
        if self.out.ends_with('\n') {
            self.out.push('\n');
        } else {
            self.out.push_str("\n\n");
        }
    }

    fn tag(&mut self, raw: &str) {
        let closing = raw.starts_with('/');
        let body = raw.trim_start_matches('/');
        let name = body
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        match (name.as_str(), closing) {
            ("br", _) => {
                self.out.push('\n');
                if self.quote > 0 {
                    self.out.push_str("> ");
                }
            }
            ("strong" | "b", _) => self.out.push_str("**"),
            ("em" | "i", _) => self.out.push('*'),
            ("del" | "s" | "strike", _) => self.out.push_str("~~"),
            ("code", _) if !self.in_pre => self.out.push('`'),
            ("pre", false) => {
                self.block_start();
                self.out.push_str("```\n");
                self.in_pre = true;
            }
            ("pre", true) => {
                self.ensure_newline();
                self.out.push_str("```\n\n");
                self.in_pre = false;
            }
            ("a", false) => {
                let href = attr(raw, "href");
                if href.is_some() {
                    self.out.push('[');
                }
                self.links.push(href);
            }
            ("a", true) => {
                if let Some(Some(href)) = self.links.pop() {
                    self.out.push_str(&format!("]({href})"));
                }
            }
            (h, false) if is_heading(h) => {
                self.block_start();
                let level = (h.as_bytes()[1] - b'0').clamp(1, 6) as usize;
                self.out.push_str(&"#".repeat(level));
                self.out.push(' ');
            }
            (h, true) if is_heading(h) => self.block_end(),
            ("blockquote", false) => {
                self.block_start();
                self.out.push_str("> ");
                self.quote += 1;
            }
            ("blockquote", true) => {
                self.quote = self.quote.saturating_sub(1);
                self.block_end();
            }
            ("ul" | "ol", false) => {
                if self.lists.is_empty() {
                    self.block_start();
                } else {
                    self.ensure_newline();
                }
                self.lists.push((name == "ol", 1));
            }
            ("ul" | "ol", true) => {
                self.lists.pop();
                if self.lists.is_empty() {
                    self.block_end();
                }
            }
            ("li", false) => {
                self.ensure_newline();
                let indent = "  ".repeat(self.lists.len().saturating_sub(1));
                let marker = match self.lists.last_mut() {
                    Some((true, n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "- ".to_string(),
                };
                self.out.push_str(&indent);
                self.out.push_str(&marker);
            }
            ("div" | "p", true) => {
                if self.lists.is_empty() {
                    self.block_end();
                } else {
                    self.ensure_newline();
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, raw: &str) {
        if self.in_pre {
            self.out.push_str(&decode(raw));
            return;
        }
        let text = escape(&decode(&collapse_ws(raw)));
        if self.out.is_empty() || self.out.ends_with('\n') || self.out.ends_with(' ') {
            self.out.push_str(text.trim_start());
        } else {
            self.out.push_str(&text);
        }
    }
}

fn is_heading(name: &str) -> bool {
    name.len() == 2 && name.starts_with('h') && name.as_bytes()[1].is_ascii_digit()
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(decode(&tag[start..start + end]))
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

fn decode(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Keep literal asterisks, backticks and backslashes literal in markdown.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use super::html_to_markdown as md;

    #[test]
    fn trix_lines_and_bold() {
        let html = r#"<div class="trix-content">
  <div>Hello <strong>world</strong><br>second</div>
  <div>Para</div>
</div>"#;
        assert_eq!(md(html), "Hello **world**\nsecond\n\nPara");
    }

    #[test]
    fn lists() {
        assert_eq!(md("<ul><li>a</li><li>b</li></ul>"), "- a\n- b");
        assert_eq!(md("<ol><li>a</li><li>b</li></ol>"), "1. a\n2. b");
    }

    #[test]
    fn links_and_entities() {
        assert_eq!(
            md(r#"<div><a href="https://x.y/?a=1&amp;b=2">site</a> &amp; more</div>"#),
            "[site](https://x.y/?a=1&b=2) & more"
        );
    }

    #[test]
    fn literal_asterisk_is_escaped() {
        assert_eq!(md("<div>5 * 3</div>"), "5 \\* 3");
    }
}
