//! Pure text transforms behind the writer's formatting bar. No UI in here,
//! so the rules are easy to test. The GTK glue lives in bookshelf-app.

// ------------------------------------------------------------ inline

#[derive(Debug, PartialEq, Eq)]
pub enum InlineEdit {
    /// Selection isn't formatted yet: add markers around it.
    Wrap,
    /// Markers sit just outside the selection: remove them.
    UnwrapOutside,
    /// The selection itself begins and ends with the markers: remove them.
    UnwrapInside,
}

/// `before` / `after` are up to marker-length + 1 characters of context on
/// each side of the selection. The extra character lets a lone `*` (italic)
/// tell itself apart from the `**` of bold.
pub fn inline_decision(before: &str, selected: &str, after: &str, marker: &str) -> InlineEdit {
    let star = marker == "*";
    let m = marker.chars().count();
    let bef: Vec<char> = before.chars().collect();
    let aft: Vec<char> = after.chars().collect();
    let sel: Vec<char> = selected.chars().collect();

    let part_of_bold_outside = star
        && ((bef.len() >= 2 && bef[bef.len() - 2] == '*') || (aft.len() >= 2 && aft[1] == '*'));
    if before.ends_with(marker) && after.starts_with(marker) && !part_of_bold_outside {
        return InlineEdit::UnwrapOutside;
    }

    let part_of_bold_inside = star && sel.len() >= 2 && (sel[1] == '*' || sel[sel.len() - 2] == '*');
    if sel.len() >= 2 * m
        && selected.starts_with(marker)
        && selected.ends_with(marker)
        && !part_of_bold_inside
    {
        return InlineEdit::UnwrapInside;
    }
    InlineEdit::Wrap
}

// ------------------------------------------------------------- lines

fn is_blank(l: &str) -> bool {
    l.trim().is_empty()
}

fn all_blank(lines: &[String]) -> bool {
    lines.iter().all(|l| is_blank(l))
}

fn split_indent(line: &str) -> (&str, &str) {
    let n = line.len() - line.trim_start_matches(' ').len();
    line.split_at(n)
}

fn bullet_body(rest: &str) -> Option<&str> {
    let b = rest.as_bytes();
    if b.len() >= 2 && matches!(b[0], b'-' | b'*' | b'+') && b[1] == b' ' {
        Some(&rest[2..])
    } else {
        None
    }
}

fn number_body(rest: &str) -> Option<&str> {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && rest[digits..].starts_with(". ") {
        Some(&rest[digits + 2..])
    } else {
        None
    }
}

/// (indent, content) with any bullet or number marker removed.
fn strip_list(line: &str) -> (&str, &str) {
    let (indent, rest) = split_indent(line);
    (indent, bullet_body(rest).or_else(|| number_body(rest)).unwrap_or(rest))
}

fn heading_level(l: &str) -> Option<usize> {
    let n = l.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&n) && l[n..].starts_with(' ') {
        Some(n)
    } else {
        None
    }
}

fn strip_heading(l: &str) -> &str {
    match heading_level(l) {
        Some(n) => &l[n + 1..],
        None => l,
    }
}

fn quote_body(l: &str) -> Option<&str> {
    l.strip_prefix("> ").or_else(|| l.strip_prefix('>'))
}

/// Make every line a heading of `level`; if they all already are, undo it.
pub fn set_heading(lines: &[String], level: usize) -> Vec<String> {
    let level = level.clamp(1, 6);
    let hashes = "#".repeat(level);
    if all_blank(lines) {
        return lines.iter().map(|_| format!("{hashes} ")).collect();
    }
    let all_same = lines
        .iter()
        .filter(|l| !is_blank(l))
        .all(|l| heading_level(l) == Some(level));
    lines
        .iter()
        .map(|l| {
            if is_blank(l) {
                l.clone()
            } else if all_same {
                strip_heading(l).to_string()
            } else {
                format!("{hashes} {}", strip_heading(l))
            }
        })
        .collect()
}

pub fn toggle_quote(lines: &[String]) -> Vec<String> {
    if all_blank(lines) {
        return lines.iter().map(|_| "> ".to_string()).collect();
    }
    let all = lines.iter().filter(|l| !is_blank(l)).all(|l| quote_body(l).is_some());
    lines
        .iter()
        .map(|l| {
            if is_blank(l) {
                l.clone()
            } else if all {
                quote_body(l).unwrap_or(l).to_string()
            } else if quote_body(l).is_some() {
                l.clone()
            } else {
                format!("> {l}")
            }
        })
        .collect()
}

pub fn toggle_bullets(lines: &[String]) -> Vec<String> {
    if all_blank(lines) {
        return lines.iter().map(|_| "- ".to_string()).collect();
    }
    let all = lines
        .iter()
        .filter(|l| !is_blank(l))
        .all(|l| bullet_body(split_indent(l).1).is_some());
    lines
        .iter()
        .map(|l| {
            if is_blank(l) {
                return l.clone();
            }
            let (indent, body) = strip_list(l);
            if all { format!("{indent}{body}") } else { format!("{indent}- {body}") }
        })
        .collect()
}

pub fn toggle_numbered(lines: &[String]) -> Vec<String> {
    if all_blank(lines) {
        return lines.iter().map(|_| "1. ".to_string()).collect();
    }
    let all = lines
        .iter()
        .filter(|l| !is_blank(l))
        .all(|l| number_body(split_indent(l).1).is_some());
    let mut n = 0;
    let mut out = Vec::with_capacity(lines.len());
    for l in lines {
        if is_blank(l) {
            out.push(l.clone());
            continue;
        }
        let (indent, body) = strip_list(l);
        if all {
            out.push(format!("{indent}{body}"));
        } else {
            n += 1;
            out.push(format!("{indent}{n}. {body}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn inline_bold() {
        assert_eq!(inline_decision("", "word", "", "**"), InlineEdit::Wrap);
        assert_eq!(inline_decision("**", "word", "**", "**"), InlineEdit::UnwrapOutside);
        assert_eq!(inline_decision("", "**word**", "", "**"), InlineEdit::UnwrapInside);
    }

    #[test]
    fn italic_is_not_confused_with_bold() {
        assert_eq!(inline_decision(" *", "word", "* ", "*"), InlineEdit::UnwrapOutside);
        assert_eq!(inline_decision("**", "word", "**", "*"), InlineEdit::Wrap);
        assert_eq!(inline_decision("", "**word**", "", "*"), InlineEdit::Wrap);
        assert_eq!(inline_decision("", "*word*", "", "*"), InlineEdit::UnwrapInside);
    }

    #[test]
    fn headings() {
        assert_eq!(set_heading(&v(&["hello"]), 2), v(&["## hello"]));
        assert_eq!(set_heading(&v(&["## hello"]), 2), v(&["hello"]));
        assert_eq!(set_heading(&v(&["# hello"]), 2), v(&["## hello"]));
        assert_eq!(set_heading(&v(&[""]), 1), v(&["# "]));
    }

    #[test]
    fn bullets_and_numbers() {
        assert_eq!(toggle_bullets(&v(&["a", "b"])), v(&["- a", "- b"]));
        assert_eq!(toggle_bullets(&v(&["- a", "- b"])), v(&["a", "b"]));
        assert_eq!(toggle_bullets(&v(&["- a", "b"])), v(&["- a", "- b"]));
        assert_eq!(toggle_bullets(&v(&["1. a"])), v(&["- a"]));
        assert_eq!(toggle_numbered(&v(&["a", "", "b"])), v(&["1. a", "", "2. b"]));
        assert_eq!(toggle_numbered(&v(&["1. a", "2. b"])), v(&["a", "b"]));
        assert_eq!(toggle_bullets(&v(&[""])), v(&["- "]));
    }

    #[test]
    fn quotes() {
        assert_eq!(toggle_quote(&v(&["a", "b"])), v(&["> a", "> b"]));
        assert_eq!(toggle_quote(&v(&["> a", "> b"])), v(&["a", "b"]));
    }
}
