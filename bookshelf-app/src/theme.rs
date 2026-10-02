//! Look & feel: color scheme, accent color and fonts, applied live from a
//! profile's settings. Static shape/type rules live in style.css and the
//! palette files; this module layers the user's choices on top.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use bookshelf_core::models::UserSettings;
use bookshelf_core::theme::accent_colors;
// The accent choices live in bookshelf-core, shared with the other apps.
pub use bookshelf_core::theme::ACCENTS;

const SERIF: &str = "\"Source Serif 4\", \"Noto Serif\", \"Charis SIL\", Georgia, serif";
const DUO: &str = "\"iA Writer Duo S\", \"iA Writer Mono S\", \"JetBrains Mono\", \
                   \"Ubuntu Mono\", \"DejaVu Sans Mono\", monospace";

struct Theme {
    palette: gtk::CssProvider,
    accent: gtk::CssProvider,
    fonts: gtk::CssProvider,
    accent_hex: RefCell<String>,
    /// What was last applied, so re-applying the same look costs nothing.
    applied: RefCell<String>,
}

thread_local! {
    static THEME: OnceCell<Rc<Theme>> = const { OnceCell::new() };
}

/// Call once at startup (after libadwaita is initialised).
pub fn install() {
    let display = gtk::gdk::Display::default().expect("no display");
    let app = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;
    let add = |css: &str, priority: u32| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(css);
        gtk::style_context_add_provider_for_display(&display, &provider, priority);
        provider
    };

    add(include_str!("style.css"), app);
    add(&swatch_css(), app);
    let palette = add("", app);
    let accent = add("", app + 1); // user choices sit above the defaults
    let fonts = add("", app + 1);

    let theme = Rc::new(Theme {
        palette,
        accent,
        fonts,
        accent_hex: RefCell::new(ACCENTS[0].1.to_string()),
        applied: RefCell::default(),
    });
    theme.render_palette();
    theme.render_accent();
    theme.render_fonts(&UserSettings::defaults());
    THEME.with(|t| {
        let _ = t.set(theme.clone());
    });

    // Light/dark flipped (system change or the Theme setting): re-tint.
    adw::StyleManager::default().connect_dark_notify(move |_| {
        theme.render_palette();
        theme.render_accent();
    });
}

/// Apply a profile's look. Cheap; safe to call whenever settings change.
pub fn apply(settings: &UserSettings) {
    THEME.with(|t| {
        if let Some(theme) = t.get() {
            theme.apply(settings);
        }
    });
}

impl Theme {
    fn apply(&self, s: &UserSettings) {
        // Reloading the style sheets restyles every widget; skip it when
        // nothing that affects the look has changed.
        let look = format!(
            "{}|{}|{}|{}|{}",
            s.theme, s.accent, s.heading_font, s.writing_font, s.writing_size
        );
        if *self.applied.borrow() == look {
            return;
        }
        *self.applied.borrow_mut() = look;
        adw::StyleManager::default().set_color_scheme(match s.theme.as_str() {
            "light" => adw::ColorScheme::ForceLight,
            "dark" => adw::ColorScheme::ForceDark,
            _ => adw::ColorScheme::Default,
        });
        *self.accent_hex.borrow_mut() = s.accent.clone();
        self.render_accent();
        self.render_fonts(s);
    }

    fn render_palette(&self) {
        let css = if adw::StyleManager::default().is_dark() {
            include_str!("palette-dark.css")
        } else {
            include_str!("palette-light.css")
        };
        self.palette.load_from_string(css);
    }

    fn render_accent(&self) {
        let dark = adw::StyleManager::default().is_dark();
        self.accent
            .load_from_string(&accent_css(&self.accent_hex.borrow(), dark));
    }

    fn render_fonts(&self, s: &UserSettings) {
        let heading = if s.heading_font == "sans" {
            "inherit"
        } else {
            SERIF
        };
        let writing = match s.writing_font.as_str() {
            "serif" => SERIF,
            "sans" => "inherit",
            "mono" => "monospace",
            _ => DUO,
        };
        let size = s.writing_size.clamp(10, 28);
        self.fonts.load_from_string(&format!(
            ".journal-title, .page-title, .entry-title, .entry-excerpt, .entry-excerpt-empty,\n\
             .book-title, .book-subtitle, .section-title, .preview-text, .reader-text,\n\
             statuspage > scrolledwindow > viewport > box > label.title {{ font-family: {heading}; }}\n\
             textview.writer, textview.writer text, .writer-preview {{ font-family: {writing}; font-size: {size}pt; }}\n\
             .reader-text {{ font-size: {size}pt; }}\n"
        ));
    }
}

fn swatch_css() -> String {
    ACCENTS
        .iter()
        .enumerate()
        .map(|(i, (_, hex))| {
            format!(".swatch-{i} {{ background-image: none; background-color: {hex}; }}\n")
        })
        .collect()
}

/// libadwaita's accent colors for one accent, from bookshelf-core's rule.
fn accent_css(hex: &str, dark: bool) -> String {
    let c = accent_colors(hex, dark);
    format!(
        "@define-color accent_bg_color {};\n\
         @define-color accent_fg_color {};\n\
         @define-color accent_color {};\n",
        c.bg, c.fg, c.text
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The accent CSS as it was built before the color rule moved to
    /// bookshelf-core, frozen. Don't edit it.
    fn old_accent_css(hex: &str, dark: bool) -> String {
        fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
            let s = s.strip_prefix('#')?;
            if s.len() != 6 || !s.is_ascii() {
                return None;
            }
            Some((
                u8::from_str_radix(&s[0..2], 16).ok()?,
                u8::from_str_radix(&s[2..4], 16).ok()?,
                u8::from_str_radix(&s[4..6], 16).ok()?,
            ))
        }
        fn to_hex((r, g, b): (u8, u8, u8)) -> String {
            format!("#{r:02x}{g:02x}{b:02x}")
        }
        fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
            let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
        }
        fn luminance((r, g, b): (u8, u8, u8)) -> f32 {
            (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
        }

        let base = parse_hex(hex).unwrap_or((0x2d, 0x71, 0xe5));
        let text = if dark {
            mix(base, (255, 255, 255), 0.35)
        } else {
            mix(base, (0, 0, 0), 0.15)
        };
        let fg = if luminance(base) > 0.6 {
            "#1e1a16"
        } else {
            "#ffffff"
        };
        format!(
            "@define-color accent_bg_color {};\n\
             @define-color accent_fg_color {fg};\n\
             @define-color accent_color {};\n",
            to_hex(base),
            to_hex(text)
        )
    }

    #[test]
    fn the_accent_css_is_unchanged() {
        let others = [
            "#000000", "#ffffff", "#f0e68c", "#9a9a9a", "#a0a0a0", "#ABCDEF", "", "blue", "#ééé",
            "#12345", "#1234567",
        ];
        let hexes = ACCENTS.iter().map(|(_, hex)| *hex).chain(others);
        for hex in hexes {
            for dark in [false, true] {
                assert_eq!(
                    accent_css(hex, dark),
                    old_accent_css(hex, dark),
                    "{hex} dark={dark}"
                );
            }
        }
    }
}
