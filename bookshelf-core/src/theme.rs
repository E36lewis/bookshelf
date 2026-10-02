//! Accent colors: the choices offered in Settings and the shades each app
//! derives from the one picked, so a profile's accent looks the same on
//! every platform. Colors are `#rrggbb` strings, as stored in the settings.

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

/// Used when the stored accent isn't a color (hand-edited, or from a newer
/// version): the default blue.
const FALLBACK: (u8, u8, u8) = (0x2d, 0x71, 0xe5);
const WHITE: (u8, u8, u8) = (255, 255, 255);
const BLACK: (u8, u8, u8) = (0, 0, 0);

/// Everything an app needs to paint with one accent, as `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccentColors {
    /// Filled things: suggested buttons, the selected swatch, switches.
    pub bg: String,
    /// Text and icons on `bg`: near-black on light accents, else white.
    pub fg: String,
    /// The accent as text (links, highlights) on the window background:
    /// lightened in dark mode and darkened in light mode, so it reads.
    pub text: String,
    /// WinUI's accent palette (SystemAccentColorLight1..3 / Dark1..3).
    /// Not used by GTK. LightN is the accent mixed N quarters of the way
    /// to white (25%, 50%, 75%); DarkN the same toward black. Windows' own
    /// palette for its default blue lands close to those steps. They don't
    /// depend on `dark`, as in Windows.
    pub light1: String,
    pub light2: String,
    pub light3: String,
    pub dark1: String,
    pub dark2: String,
    pub dark3: String,
}

/// The accent `hex` (one of [`ACCENTS`], ideally) for a light or dark
/// window.
pub fn accent_colors(hex: &str, dark: bool) -> AccentColors {
    let base = parse_hex(hex).unwrap_or(FALLBACK);
    let text = if dark {
        mix(base, WHITE, 0.35)
    } else {
        mix(base, BLACK, 0.15)
    };
    let fg = if luminance(base) > 0.6 {
        "#1e1a16"
    } else {
        "#ffffff"
    };
    let toward = |to, t| to_hex(mix(base, to, t));
    AccentColors {
        bg: to_hex(base),
        fg: fg.to_string(),
        text: to_hex(text),
        light1: toward(WHITE, 0.25),
        light2: toward(WHITE, 0.5),
        light3: toward(WHITE, 0.75),
        dark1: toward(BLACK, 0.25),
        dark2: toward(BLACK, 0.5),
        dark3: toward(BLACK, 0.75),
    }
}

/// `#rrggbb` to bytes. Anything else, including non-ASCII that happens to
/// be six bytes long, is `None` (and slicing it would panic).
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
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

pub fn to_hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// `t` of the way from `a` to `b`, per channel, rounded.
pub fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

/// Rough perceived brightness, 0 to 1 (Rec. 709 weights on the raw
/// channels; close enough to pick black or white text).
pub fn luminance((r, g, b): (u8, u8, u8)) -> f32 {
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

    /// (bg, fg, text) as GTK has always drawn them.
    fn gtk_trio(hex: &str, dark: bool) -> (String, String, String) {
        let c = accent_colors(hex, dark);
        (c.bg, c.fg, c.text)
    }

    fn trio(bg: &str, fg: &str, text: &str) -> (String, String, String) {
        (bg.into(), fg.into(), text.into())
    }

    #[test]
    fn accents_in_light_and_dark() {
        assert_eq!(
            gtk_trio("#2d71e5", false),
            trio("#2d71e5", "#ffffff", "#2660c3")
        );
        assert_eq!(
            gtk_trio("#2d71e5", true),
            trio("#2d71e5", "#ffffff", "#77a3ee")
        );
        assert_eq!(
            gtk_trio("#b4532a", false),
            trio("#b4532a", "#ffffff", "#994724")
        );
        assert_eq!(
            gtk_trio("#b4532a", true),
            trio("#b4532a", "#ffffff", "#ce8f75")
        );
        assert_eq!(
            gtk_trio("#a3741a", false),
            trio("#a3741a", "#ffffff", "#8b6316")
        );
        assert_eq!(
            gtk_trio("#56657a", true),
            trio("#56657a", "#ffffff", "#919ba9")
        );
    }

    #[test]
    fn a_light_accent_gets_dark_text() {
        let c = accent_colors("#f0e68c", false);
        assert_eq!(c.fg, "#1e1a16");
        assert_eq!(c.text, "#ccc477");
    }

    #[test]
    fn a_bad_accent_falls_back_to_blue() {
        assert_eq!(
            accent_colors("blue", false),
            accent_colors("#2d71e5", false)
        );
        assert_eq!(accent_colors("#ééé", true), accent_colors("#2d71e5", true));
    }

    #[test]
    fn windows_shades_step_toward_white_and_black() {
        let c = accent_colors("#2d71e5", false);
        assert_eq!(
            [&c.light1, &c.light2, &c.light3],
            ["#6295ec", "#96b8f2", "#cbdcf9"]
        );
        assert_eq!(
            [&c.dark1, &c.dark2, &c.dark3],
            ["#2255ac", "#173973", "#0b1c39"]
        );
        // The same in dark mode.
        let d = accent_colors("#2d71e5", true);
        assert_eq!((d.light1, d.dark3), (c.light1, c.dark3));
    }
}
