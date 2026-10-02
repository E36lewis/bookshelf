//! The writing page's words and numbers, shared by every app: the prompts on
//! an empty page, the word count, the autosave delay and the page's layout
//! from the user's settings.

use crate::models::{Milestone, UserSettings};

/// How long typing has to pause before the text is saved.
pub const AUTOSAVE_MS: u64 = 700;

/// Questions to get you started, depending on where the book is on your shelf.
pub fn prompts(milestone: Milestone) -> &'static str {
    match milestone {
        Milestone::Eventually => {
            "Why do you want to read this one?\n\
             Who recommended it, and what did they say?"
        }
        Milestone::Reading => {
            "Where are you in the story?\n\
             What has surprised you so far?\n\
             A line worth remembering…"
        }
        Milestone::Finished => {
            "What stayed with you after the last page?\n\
             A line worth remembering…\n\
             Who would you give this book to?"
        }
    }
}

/// Words are whatever sits between spaces, Markdown marks included.
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// The writing page's measurements, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WriterLayout {
    /// The widest the text column gets (the reading page uses it too).
    pub column_width: i32,
    /// Space between the wrapped rows of one paragraph line.
    pub wrap_gap: i32,
    /// Space below each line.
    pub line_gap: i32,
    /// The writing size, from points at 96 pixels to the inch.
    pub font_px: f64,
}

impl WriterLayout {
    /// Unknown setting values get the middle choice.
    pub fn from_settings(s: &UserSettings) -> Self {
        let (wrap_gap, line_gap) = match s.line_spacing.as_str() {
            "tight" => (2, 6),
            "airy" => (12, 20),
            _ => (6, 12),
        };
        let column_width = match s.page_width.as_str() {
            "narrow" => 600,
            "wide" => 900,
            _ => 720,
        };
        Self {
            column_width,
            wrap_gap,
            line_gap,
            // The same bounds the GTK theme puts on the size.
            font_px: s.writing_size.clamp(10, 28) as f64 * 96.0 / 72.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(line_spacing: &str, page_width: &str, writing_size: i64) -> WriterLayout {
        WriterLayout::from_settings(&UserSettings {
            line_spacing: line_spacing.into(),
            page_width: page_width.into(),
            writing_size,
            ..UserSettings::defaults()
        })
    }

    #[test]
    fn prompts_follow_the_milestone() {
        assert_eq!(
            prompts(Milestone::Eventually),
            "Why do you want to read this one?\nWho recommended it, and what did they say?"
        );
        assert_eq!(
            prompts(Milestone::Reading),
            "Where are you in the story?\nWhat has surprised you so far?\nA line worth remembering…"
        );
        assert_eq!(
            prompts(Milestone::Finished),
            "What stayed with you after the last page?\nA line worth remembering…\n\
             Who would you give this book to?"
        );
    }

    #[test]
    fn words_are_split_on_any_space() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("   \n\t "), 0);
        assert_eq!(word_count("one"), 1);
        assert_eq!(word_count("  two\twords\n"), 2);
        assert_eq!(word_count("# Title\n\n- **bold** item"), 5, "marks count");
        assert_eq!(word_count("don't stop—ever"), 2);
        assert_eq!(word_count("a\u{a0}b"), 2, "a no-break space still splits");
    }

    #[test]
    fn layout_from_settings() {
        let d = WriterLayout::from_settings(&UserSettings::defaults());
        assert_eq!((d.column_width, d.wrap_gap, d.line_gap), (720, 6, 12));
        assert!((d.font_px - 18.666_666).abs() < 1e-5, "14pt");

        assert_eq!(layout("tight", "narrow", 14).wrap_gap, 2);
        assert_eq!(layout("tight", "narrow", 14).line_gap, 6);
        assert_eq!(layout("tight", "narrow", 14).column_width, 600);
        assert_eq!(layout("airy", "wide", 14).wrap_gap, 12);
        assert_eq!(layout("airy", "wide", 14).line_gap, 20);
        assert_eq!(layout("airy", "wide", 14).column_width, 900);
        assert_eq!(layout("normal", "medium", 14), d);
        assert_eq!(layout("", "huge", 14), d, "unknown values");
    }

    #[test]
    fn font_size_is_kept_between_10_and_28_points() {
        assert_eq!(layout("", "", 12).font_px, 16.0);
        assert_eq!(layout("", "", 9).font_px, layout("", "", 10).font_px);
        assert_eq!(layout("", "", -3).font_px, 10.0 * 96.0 / 72.0);
        assert_eq!(layout("", "", 28).font_px, 28.0 * 96.0 / 72.0);
        assert_eq!(layout("", "", 400).font_px, 28.0 * 96.0 / 72.0);
    }
}
