import AppKit
import SwiftUI
import BookshelfKit

/// The app's typefaces. iA Writer Duo and Source Serif 4 are bundled
/// (Resources/Fonts, registered by `ATSApplicationFontsPath`); sans-serif
/// and monospace are the system's.
enum Typeface {
    static let serifFamily = "Source Serif 4"
    static let duoFamily = "iA Writer Duo S"

    /// Titles and headings: Source Serif or the system's sans-serif.
    static func title(_ font: HeadingFont, size: CGFloat, weight: Font.Weight = .bold) -> Font {
        switch font {
        case .serif: .custom(serifFamily, size: size).weight(weight)
        case .sans: .system(size: size, weight: weight)
        }
    }

    /// Summaries and excerpts, in the same family as titles.
    static func text(_ font: HeadingFont, size: CGFloat) -> Font {
        switch font {
        case .serif: .custom(serifFamily, size: size)
        case .sans: .system(size: size)
        }
    }

    /// The writing page's typeface.
    static func writing(_ font: WritingFont, size: CGFloat) -> Font {
        switch font {
        case .iaDuo: .custom(duoFamily, size: size)
        case .serif: .custom(serifFamily, size: size)
        case .sans: .system(size: size)
        case .mono: .system(size: size, design: .monospaced)
        }
    }

    /// The writing page's typeface as AppKit fonts, with the bold and
    /// italic faces Markdown needs. Code is set in the system's monospace,
    /// unless the writing font is already fixed-width (or iA Writer Duo,
    /// which is nearly).
    @MainActor
    static func writingFonts(_ font: WritingFont, size: CGFloat) -> WritingFonts {
        let mono = NSFont.monospacedSystemFont(ofSize: size, weight: .regular)
        let regular: NSFont
        switch font {
        case .iaDuo: regular = family(duoFamily, size: size) ?? mono
        case .serif: regular = family(serifFamily, size: size) ?? .systemFont(ofSize: size)
        case .sans: regular = .systemFont(ofSize: size)
        case .mono: regular = mono
        }
        let code = font == .iaDuo || font == .mono
            ? regular : NSFont.monospacedSystemFont(ofSize: (size * 0.9).rounded(), weight: .regular)
        return WritingFonts(regular: regular, code: code)
    }

    /// A bundled family's regular face, if it's registered.
    @MainActor
    private static func family(_ name: String, size: CGFloat) -> NSFont? {
        NSFontManager.shared.font(withFamily: name, traits: [], weight: 5, size: size)
    }

    /// Code in summaries.
    static func code(size: CGFloat) -> Font {
        .system(size: size * 0.9, design: .monospaced)
    }
}

extension ProfileSettings {
    /// The writing size in points.
    var textSize: CGFloat { CGFloat(writingSize) }
}

extension PageLayout {
    /// The widest the writing and reading column gets, in points. The core
    /// measures at 96 pixels to the inch, the Mac at 72 points; the column
    /// holds the same number of characters either way, so it scales by the
    /// same factor as the font (pixels → points).
    var columnPoints: CGFloat { CGFloat(columnWidth) * 0.75 }
    /// Space between wrapped rows of a paragraph, in points.
    var wrapPoints: CGFloat { CGFloat(wrapGap) * 0.75 }
    /// Space after each line, in points.
    var linePoints: CGFloat { CGFloat(lineGap) * 0.75 }
}
