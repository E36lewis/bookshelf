import AppKit

/// The writing font and the variants Markdown needs. When a family has no
/// bold or italic face, the regular one stands in and is marked synthetic,
/// so the styler thickens or slants it instead.
public struct WritingFonts: Equatable {
    public var regular: NSFont
    public var bold: NSFont
    public var italic: NSFont
    public var boldItalic: NSFont
    /// For `code`: a monospaced face, or the writing font if it's one.
    public var code: NSFont
    /// No bold face: bold is drawn by thickening the strokes.
    public var syntheticBold: Bool
    /// No italic face: italic is drawn slanted.
    public var syntheticItalic: Bool

    /// The variants of `regular`'s family.
    @MainActor
    public init(regular: NSFont, code: NSFont? = nil) {
        let bold = Self.variant(of: regular, .boldFontMask)
        let italic = Self.variant(of: regular, .italicFontMask)
        self.regular = regular
        self.bold = bold ?? regular
        self.italic = italic ?? regular
        self.boldItalic = bold.flatMap { Self.variant(of: $0, .italicFontMask) }
            ?? italic.flatMap { Self.variant(of: $0, .boldFontMask) }
            ?? bold ?? italic ?? regular
        self.code = code ?? regular
        syntheticBold = bold == nil
        syntheticItalic = italic == nil
    }

    /// `font`'s family member with `trait`, if the family has one.
    @MainActor
    private static func variant(of font: NSFont, _ trait: NSFontTraitMask) -> NSFont? {
        let manager = NSFontManager.shared
        let converted = manager.convert(font, toHaveTrait: trait)
        return manager.traits(of: converted).contains(trait) ? converted : nil
    }

    /// The face for a run of text.
    func face(bold: Bool, italic: Bool) -> NSFont {
        switch (bold, italic) {
        case (false, false): regular
        case (true, false): self.bold
        case (false, true): self.italic
        case (true, true): boldItalic
        }
    }
}

/// Everything about how the writing page sets its text, apart from color.
public struct WritingLook: Equatable {
    public var fonts: WritingFonts
    /// Space between the wrapped rows of one line, in points.
    public var lineSpacing: CGFloat
    /// Space below each line, in points.
    public var paragraphSpacing: CGFloat

    public init(fonts: WritingFonts, lineSpacing: CGFloat, paragraphSpacing: CGFloat) {
        self.fonts = fonts
        self.lineSpacing = lineSpacing
        self.paragraphSpacing = paragraphSpacing
    }

    /// The paragraph style every line shares.
    public var paragraphStyle: NSParagraphStyle {
        let style = NSMutableParagraphStyle()
        style.lineSpacing = lineSpacing
        style.paragraphSpacing = paragraphSpacing
        style.lineBreakMode = .byWordWrapping
        return style
    }

    /// How much bigger a heading of `level` (1 to 6) is than the text.
    public static func headingScale(_ level: Int) -> CGFloat {
        switch level {
        case 1: 1.4
        case 2: 1.22
        case 3: 1.1
        default: 1
        }
    }
}

/// The writing page's colors. Each is worked out when it's drawn, for the
/// window's appearance at that moment (light, dark, increased contrast),
/// so switching appearance recolors the page without restyling anything.
public struct WritingPalette {
    /// The text.
    public var text: NSColor = .textColor
    /// Markdown marks (`**`, `#`, `- `), dimmed so the words stand out.
    public var marks: NSColor = Self.ink(0.4, highContrast: 0.62)
    /// Quoted lines.
    public var quote: NSColor = Self.ink(0.72, highContrast: 0.86)
    /// Behind `code`.
    public var codeBackground: NSColor = Self.ink(0.07, highContrast: 0.14)
    /// Everything but the current sentence, in focus mode.
    public var dimmed: NSColor = Self.ink(0.3, highContrast: 0.5)
    /// The questions on an empty page.
    public var prompt: NSColor = .placeholderTextColor

    public init() {}

    /// The text color at `alpha`, or at `highContrast` when the person has
    /// asked for more contrast.
    public static func ink(_ alpha: CGFloat, highContrast: CGFloat) -> NSColor {
        NSColor(name: nil) { appearance in
            let names: [NSAppearance.Name] = [
                .aqua, .darkAqua, .accessibilityHighContrastAqua, .accessibilityHighContrastDarkAqua,
            ]
            let best = appearance.bestMatch(from: names) ?? .aqua
            let high = best == .accessibilityHighContrastAqua || best == .accessibilityHighContrastDarkAqua
            var ink = NSColor.black
            appearance.performAsCurrentDrawingAppearance {
                ink = NSColor.textColor.usingColorSpace(.sRGB) ?? .black
            }
            return ink.withAlphaComponent(high ? highContrast : alpha)
        }
    }
}
