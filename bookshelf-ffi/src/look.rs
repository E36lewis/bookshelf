//! The look of the app and the writing page: accent colors, the page's
//! measurements, the prompts on an empty page, the word count and the
//! autosave delay. Shared so every app looks and behaves the same.

use bookshelf_core::{theme, writer};

use crate::types::{count, ProfileSettings, Shelf};

/// One of the accent colors offered in Settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct Accent {
    /// Its name ("Teal"), for accessibility labels.
    pub name: String,
    /// `#rrggbb`.
    pub hex: String,
}

/// The accent colors offered in Settings, the default (blue) first.
#[uniffi::export]
pub fn accents() -> Vec<Accent> {
    theme::ACCENTS
        .iter()
        .map(|(name, hex)| Accent {
            name: (*name).to_string(),
            hex: (*hex).to_string(),
        })
        .collect()
}

/// Everything an app paints with one accent, as `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct AccentPalette {
    /// Filled things: suggested buttons, the selected swatch, switches.
    pub bg: String,
    /// Text and icons on `bg`: near-black on light accents, else white.
    pub fg: String,
    /// The accent as text (links) on the window background, lightened in
    /// dark mode and darkened in light mode so it reads.
    pub text: String,
    /// WinUI's SystemAccentColorLight1: 25% toward white.
    pub light1: String,
    /// SystemAccentColorLight2: 50% toward white.
    pub light2: String,
    /// SystemAccentColorLight3: 75% toward white.
    pub light3: String,
    /// SystemAccentColorDark1: 25% toward black.
    pub dark1: String,
    /// SystemAccentColorDark2: 50% toward black.
    pub dark2: String,
    /// SystemAccentColorDark3: 75% toward black.
    pub dark3: String,
}

/// The shades of accent `hex` for a light or dark window. Anything that
/// isn't a `#rrggbb` color gets the default blue's.
#[uniffi::export]
pub fn accent_colors(hex: String, dark: bool) -> AccentPalette {
    let c = theme::accent_colors(&hex, dark);
    AccentPalette {
        bg: c.bg,
        fg: c.fg,
        text: c.text,
        light1: c.light1,
        light2: c.light2,
        light3: c.light3,
        dark1: c.dark1,
        dark2: c.dark2,
        dark3: c.dark3,
    }
}

/// The writing page's measurements, in pixels (at 96 to the inch).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct PageLayout {
    /// The widest the text column gets (the reading page uses it too).
    pub column_width: i32,
    /// Space between the wrapped rows of one paragraph line.
    pub wrap_gap: i32,
    /// Space below each line.
    pub line_gap: i32,
    /// The writing size.
    pub font_px: f64,
}

/// The writing page's measurements for a profile's settings.
#[uniffi::export]
pub fn writer_layout(settings: ProfileSettings) -> PageLayout {
    let l = writer::WriterLayout::from_settings(&settings.to_stored());
    PageLayout {
        column_width: l.column_width,
        wrap_gap: l.wrap_gap,
        line_gap: l.line_gap,
        font_px: l.font_px,
    }
}

/// Questions shown on an empty writing page, one per line, depending on
/// the entry's shelf.
#[uniffi::export]
pub fn prompts(shelf: Shelf) -> String {
    writer::prompts(shelf.into()).to_string()
}

/// Words are whatever sits between spaces, Markdown marks included. The
/// same count as `SaveResult.words`.
#[uniffi::export]
pub fn word_count(text: String) -> u32 {
    count(writer::word_count(&text))
}

/// How long typing has to pause, in milliseconds, before the writing page
/// saves.
#[uniffi::export]
pub fn autosave_ms() -> u32 {
    u32::try_from(writer::AUTOSAVE_MS).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DateFormat, HeadingFont, LineSpacing, PageWidth, Theme, WeekStart, WritingFont,
    };

    pub(crate) fn settings() -> ProfileSettings {
        ProfileSettings {
            user_id: "u".into(),
            theme: Theme::System,
            accent: "#2d71e5".into(),
            heading_font: HeadingFont::Serif,
            writing_font: WritingFont::IaDuo,
            writing_size: 14,
            line_spacing: LineSpacing::Normal,
            page_width: PageWidth::Medium,
            focus_default: false,
            date_format: DateFormat::Long,
            week_start: WeekStart::Sunday,
            start_shelf: Shelf::Reading,
        }
    }

    #[test]
    fn accent_choices_and_shades() {
        let all = accents();
        assert_eq!(all.len(), 8);
        assert_eq!(
            all[0],
            Accent {
                name: "Blue".into(),
                hex: "#2d71e5".into()
            }
        );
        let light = accent_colors("#2d71e5".into(), false);
        assert_eq!(light.bg, "#2d71e5");
        assert_eq!(light.fg, "#ffffff");
        let dark = accent_colors("#2d71e5".into(), true);
        assert_ne!(light.text, dark.text);
        assert_eq!(light.light2, dark.light2, "WinUI's shades ignore dark mode");
        // Not a color: the default blue's.
        assert_eq!(accent_colors("nope".into(), false), light);
    }

    #[test]
    fn page_layout_follows_the_settings() {
        let normal = writer_layout(settings());
        assert_eq!(
            (normal.column_width, normal.wrap_gap, normal.line_gap),
            (720, 6, 12)
        );
        assert!((normal.font_px - 14.0 * 96.0 / 72.0).abs() < 1e-9);
        let roomy = writer_layout(ProfileSettings {
            line_spacing: LineSpacing::Airy,
            page_width: PageWidth::Wide,
            writing_size: 18,
            ..settings()
        });
        assert_eq!(
            (roomy.column_width, roomy.wrap_gap, roomy.line_gap),
            (900, 12, 20)
        );
        assert_eq!(roomy.font_px, 24.0);
        let snug = writer_layout(ProfileSettings {
            line_spacing: LineSpacing::Tight,
            page_width: PageWidth::Narrow,
            ..settings()
        });
        assert_eq!((snug.column_width, snug.wrap_gap), (600, 2));
    }

    #[test]
    fn writing_page_words() {
        assert!(prompts(Shelf::Finished).starts_with("What stayed with you"));
        assert!(prompts(Shelf::Eventually).starts_with("Why do you want"));
        assert!(prompts(Shelf::Reading).starts_with("Where are you"));
        assert_eq!(word_count("  two\twords\n".into()), 2);
        assert_eq!(word_count(String::new()), 0);
        assert_eq!(autosave_ms(), 700);
    }
}
