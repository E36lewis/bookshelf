//! Look & feel: color scheme, accent color and fonts, applied live from a
//! profile's settings. Static shape/type rules live in style.css and the
//! palette files; this module layers the user's choices on top.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use bookshelf_core::models::UserSettings;

pub const ACCENTS: [(&str, &str); 8] = [
    ("Blue", "#2d71e5"), // the shared palette's link color: the default
    ("Terracotta", "#b4532a"),
    ("Rose", "#b8475f"),
    ("Plum", "#8a4f7d"),
    ("Teal", "#2a7f7a"),
    ("Sage", "#4f7a5a"),
    ("Gold", "#a3741a"),
    ("Slate", "#56657a"),
];

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
        let base = parse_hex(&self.accent_hex.borrow()).unwrap_or((0x2d, 0x71, 0xe5));
        let text = if dark {
            mix(base, (255, 255, 255), 0.35)
        } else {
            mix(base, (0, 0, 0), 0.15)
        };
        let fg = if luminance(base) > 0.6 { "#1e1a16" } else { "#ffffff" };
        self.accent.load_from_string(&format!(
            "@define-color accent_bg_color {};\n\
             @define-color accent_fg_color {fg};\n\
             @define-color accent_color {};\n",
            to_hex(base),
            to_hex(text)
        ));
    }

    fn render_fonts(&self, s: &UserSettings) {
        let heading = if s.heading_font == "sans" { "inherit" } else { SERIF };
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
        .map(|(i, (_, hex))| format!(".swatch-{i} {{ background-image: none; background-color: {hex}; }}\n"))
        .collect()
}

// ------------------------------------------------------------- colors

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(parse_hex("#b4532a"), Some((0xb4, 0x53, 0x2a)));
        assert_eq!(to_hex((0xb4, 0x53, 0x2a)), "#b4532a");
        assert_eq!(parse_hex("b4532a"), None);
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#ééé"), None); // 6 bytes, not 6 hex digits
    }

    #[test]
    fn mixing() {
        assert_eq!(mix((0, 0, 0), (255, 255, 255), 0.5), (128, 128, 128));
        assert_eq!(mix((10, 20, 30), (200, 200, 200), 0.0), (10, 20, 30));
    }
}
