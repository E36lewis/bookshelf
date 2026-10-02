import AppKit
import XCTest
@testable import BookshelfKit

/// The writing page's text engine on an offscreen TextKit 1 text view, as
/// the app builds it: live highlighting, formatting as one undo step,
/// focus mode's sentences, and how quickly a long summary is styled.
@MainActor
final class WritingTests: XCTestCase {
    /// A text view with its own undo manager, styled as the app styles it.
    @MainActor
    private final class Page: NSObject, NSTextViewDelegate {
        let textView: NSTextView
        let highlighter: LiveHighlighter
        let undo = UndoManager()

        init(_ text: String, styler: MarkdownStyler) {
            let storage = NSTextStorage(string: text)
            let layout = NSLayoutManager()
            storage.addLayoutManager(layout)
            let container = NSTextContainer(containerSize: NSSize(width: 400, height: CGFloat.greatestFiniteMagnitude))
            layout.addTextContainer(container)
            textView = NSTextView(frame: NSRect(x: 0, y: 0, width: 400, height: 300), textContainer: container)
            textView.isRichText = false
            textView.allowsUndo = true
            highlighter = LiveHighlighter(styler: styler)
            highlighter.textView = textView
            styler.restyleAll(storage)
            storage.delegate = highlighter
            super.init()
            textView.delegate = self
            // Each step below is its own event, as typing and menu commands are.
            undo.groupsByEvent = false
        }

        func undoManager(for view: NSTextView) -> UndoManager? { undo }

        /// Runs `work` as one event's worth of changes.
        func step(_ work: () -> Void) {
            undo.beginUndoGrouping()
            work()
            undo.endUndoGrouping()
        }

        func type(_ text: String) {
            step { textView.insertText(text, replacementRange: textView.selectedRange()) }
        }

        func style(at location: Int) -> MarkdownStyle {
            let raw = textView.textStorage?.attribute(.markdownStyle, at: location, effectiveRange: nil) as? NSNumber
            return MarkdownStyle(rawValue: raw?.uint16Value ?? 0)
        }

        func font(at location: Int) -> NSFont? {
            textView.textStorage?.attribute(.font, at: location, effectiveRange: nil) as? NSFont
        }
    }

    private func styler(_ font: NSFont = .systemFont(ofSize: 14)) -> MarkdownStyler {
        MarkdownStyler(look: WritingLook(fonts: WritingFonts(regular: font), lineSpacing: 4, paragraphSpacing: 9))
    }

    // MARK: Highlighting

    func testMarkdownIsStyledFromTheCoresSpans() throws {
        let page = Page("# Title\nSome **bold**, *italic*, `code` and ~~gone~~.\n> Quoted", styler: styler())
        let text = page.textView.string as NSString

        XCTAssertEqual(page.style(at: 0).headingLevel, 1)
        XCTAssertTrue(page.style(at: 0).contains(.marks), "the # is a mark")
        XCTAssertEqual(try XCTUnwrap(page.font(at: 3)).pointSize, 14 * WritingLook.headingScale(1), accuracy: 0.01)
        XCTAssertTrue(NSFontManager.shared.traits(of: try XCTUnwrap(page.font(at: 3))).contains(.boldFontMask))

        let bold = text.range(of: "bold").location
        XCTAssertEqual(page.style(at: bold), .bold)
        XCTAssertTrue(NSFontManager.shared.traits(of: try XCTUnwrap(page.font(at: bold))).contains(.boldFontMask))
        XCTAssertTrue(page.style(at: bold - 1).contains(.marks), "the ** before it")

        let italic = text.range(of: "italic").location
        XCTAssertTrue(page.style(at: italic).contains(.italic))
        XCTAssertTrue(NSFontManager.shared.traits(of: try XCTUnwrap(page.font(at: italic))).contains(.italicFontMask))

        let code = text.range(of: "code").location
        XCTAssertTrue(page.style(at: code).contains(.code))
        XCTAssertNotNil(page.textView.textStorage?.attribute(.backgroundColor, at: code, effectiveRange: nil))

        let gone = text.range(of: "gone").location
        XCTAssertEqual(
            page.textView.textStorage?.attribute(.strikethroughStyle, at: gone, effectiveRange: nil) as? Int,
            NSUnderlineStyle.single.rawValue)

        let quoted = text.range(of: "Quoted").location
        XCTAssertTrue(page.style(at: quoted).contains(.quote))
        XCTAssertTrue(NSFontManager.shared.traits(of: try XCTUnwrap(page.font(at: quoted))).contains(.italicFontMask))

        // Plain text is plain.
        let some = text.range(of: "Some").location
        XCTAssertEqual(page.style(at: some), [])
        XCTAssertEqual(page.font(at: some), NSFont.systemFont(ofSize: 14))
    }

    func testHeadingLevelsGetSmaller() {
        let page = Page("# One\n## Two\n### Three\n#### Four", styler: styler())
        let text = page.textView.string as NSString
        let sizes = ["One", "Two", "Three", "Four"].map { page.font(at: text.range(of: $0).location)?.pointSize ?? 0 }
        XCTAssertEqual(sizes, sizes.sorted(by: >))
        XCTAssertEqual(page.style(at: text.range(of: "Three").location).headingLevel, 3)
        XCTAssertEqual(sizes.last ?? 0, 14, accuracy: 0.01, "level 4 and below are just bold")
    }

    func testEmojiAndCRLFKeepTheirOffsets() {
        // UTF-16 throughout: the emoji is two units, CRLF ends a line.
        let page = Page("📚 **b**\r\n*i* 🧭", styler: styler())
        let text = page.textView.string as NSString
        XCTAssertEqual(page.style(at: text.range(of: "b").location), .bold)
        XCTAssertTrue(page.style(at: text.range(of: "i").location).contains(.italic))
        XCTAssertEqual(page.style(at: text.range(of: "🧭").location), [])
    }

    func testTypingRestylesOnlyTheEditedLine() throws {
        let lines = (1...5).map { "Line \($0) has some words." }
        let page = Page(lines.joined(separator: "\n"), styler: styler())
        let text = page.textView.string as NSString
        let third = text.paragraphRange(for: text.range(of: "Line 3"))

        page.textView.setSelectedRange(NSRange(location: text.range(of: "some", options: [], range: third).location, length: 0))
        page.type("**")
        let opened = page.textView.string as NSString
        page.textView.setSelectedRange(NSRange(location: NSMaxRange(opened.range(of: "Line 3 has **some")), length: 0))
        page.type("**")

        let edited = page.textView.string as NSString
        XCTAssertTrue(edited.contains("Line 3 has **some** words."))
        let restyled = try XCTUnwrap(page.highlighter.lastRestyled)
        XCTAssertEqual(restyled, edited.paragraphRange(for: edited.range(of: "Line 3")))
        XCTAssertEqual(page.style(at: edited.range(of: "**some**").location + 2), .bold)
        XCTAssertEqual(page.style(at: edited.range(of: "Line 4").location), [])
    }

    func testStylingNeverEntersTheUndoStack() {
        let styler = self.styler()
        let page = Page("Plain words", styler: styler)
        styler.restyleAll(page.textView.textStorage ?? NSTextStorage())
        XCTAssertFalse(page.undo.canUndo, "styling registered undo")

        // Typing is one step; undoing it brings the old text back, restyled.
        page.textView.setSelectedRange(NSRange(location: 0, length: 0))
        page.type("# ")
        XCTAssertEqual(page.style(at: 2).headingLevel, 1)
        page.undo.undo()
        XCTAssertEqual(page.textView.string, "Plain words")
        XCTAssertEqual(page.style(at: 0), [])
        XCTAssertFalse(page.undo.canUndo)
    }

    func testColorsFollowTheAppearance() throws {
        let palette = WritingPalette()
        func whiteness(_ color: NSColor, _ name: NSAppearance.Name) throws -> CGFloat {
            var resolved: NSColor?
            try XCTUnwrap(NSAppearance(named: name)).performAsCurrentDrawingAppearance {
                resolved = color.usingColorSpace(.sRGB)
            }
            let rgb = try XCTUnwrap(resolved)
            return (rgb.redComponent + rgb.greenComponent + rgb.blueComponent) / 3
        }
        for color in [palette.marks, palette.quote, palette.dimmed, palette.codeBackground] {
            XCTAssertLessThan(try whiteness(color, .aqua), 0.5)
            XCTAssertGreaterThan(try whiteness(color, .darkAqua), 0.5)
        }
        // More contrast asked for: marks are less faint.
        var normal: CGFloat = 0
        var high: CGFloat = 0
        try XCTUnwrap(NSAppearance(named: .aqua)).performAsCurrentDrawingAppearance {
            normal = palette.marks.usingColorSpace(.sRGB)?.alphaComponent ?? 0
        }
        try XCTUnwrap(NSAppearance(named: .accessibilityHighContrastAqua)).performAsCurrentDrawingAppearance {
            high = palette.marks.usingColorSpace(.sRGB)?.alphaComponent ?? 0
        }
        XCTAssertGreaterThan(high, normal)
    }

    func testFontsWithoutVariantsAreSynthesized() throws {
        let fonts = WritingFonts(regular: .systemFont(ofSize: 12))
        XCTAssertFalse(fonts.syntheticBold)
        XCTAssertFalse(fonts.syntheticItalic)
        XCTAssertTrue(NSFontManager.shared.traits(of: fonts.boldItalic).contains([.boldFontMask, .italicFontMask]))

        // A face whose family has no italic: slanted instead.
        var fake = fonts
        fake.italic = fake.regular
        fake.syntheticItalic = true
        let styler = MarkdownStyler(look: WritingLook(fonts: fake, lineSpacing: 0, paragraphSpacing: 0))
        let italic = styler.attributes(for: .italic)
        XCTAssertNotNil(italic[.obliqueness])
        XCTAssertNil(styler.attributes(for: .bold)[.obliqueness])
    }

    func testTheProbeDescribesTheStyles() {
        let page = Page("Hello **world**", styler: styler())
        let text = MarkdownStyler.describe(page.textView.textStorage ?? NSTextStorage())
        XCTAssertEqual(text, "bold 8-13\nmarks 6-8 13-15")
    }

    // MARK: Formatting

    func testBoldOnTheWordAtTheCaretIsOneUndoStep() {
        let page = Page("", styler: styler())
        page.type("Hello world")
        // Caret at the end of "world": Bold takes the word.
        page.step { XCTAssertTrue(page.textView.applyFormat(.bold, undoName: "Bold")) }
        XCTAssertEqual(page.textView.string, "Hello **world**")
        XCTAssertEqual(page.textView.selectedRange(), NSRange(location: 8, length: 5))
        XCTAssertEqual(page.undo.undoActionName, "Bold")
        XCTAssertEqual(page.style(at: 8), .bold)

        page.undo.undo()
        XCTAssertEqual(page.textView.string, "Hello world", "one step undoes the formatting only")
        page.undo.redo()
        XCTAssertEqual(page.textView.string, "Hello **world**")
    }

    func testFormattingWithASelectionAndLineActions() {
        let page = Page("one two three", styler: styler())
        page.textView.setSelectedRange(NSRange(location: 4, length: 3))
        page.step { page.textView.applyFormat(.italic) }
        XCTAssertEqual(page.textView.string, "one *two* three")

        page.textView.setSelectedRange(NSRange(location: 0, length: 0))
        page.step { page.textView.applyFormat(.heading(level: 2)) }
        XCTAssertEqual(page.textView.string, "## one *two* three")
        XCTAssertEqual(page.style(at: 3).headingLevel, 2)
        page.step { page.textView.applyFormat(.heading(level: 2)) }
        XCTAssertEqual(page.textView.string, "one *two* three", "the same level again removes it")

        page.step { page.textView.applyFormat(.bullets) }
        XCTAssertTrue(page.textView.string.hasPrefix("- "))

        // Caret in the middle of a word.
        page.textView.setSelectedRange(NSRange(location: (page.textView.string as NSString).range(of: "hree").location, length: 0))
        page.step { page.textView.applyFormat(.code) }
        XCTAssertTrue(page.textView.string.hasSuffix("`three`"), page.textView.string)
    }

    func testTheWordAtTheCaret() {
        let page = Page("Hello, world!  x", styler: styler())
        XCTAssertEqual(page.textView.wordRange(at: 2), NSRange(location: 0, length: 5))
        XCTAssertEqual(page.textView.wordRange(at: 5), NSRange(location: 0, length: 5), "just after the word")
        XCTAssertEqual(page.textView.wordRange(at: 12), NSRange(location: 7, length: 5))
        XCTAssertEqual(page.textView.wordRange(at: 14), NSRange(location: 14, length: 0), "between spaces")
    }

    func testTheFormatMenuHasEveryCommandAndTheCoresKeys() {
        XCTAssertEqual(Set(FormatCommand.groups.flatMap { $0 }), Set(FormatCommand.allCases))
        XCTAssertEqual(FormatCommand.groups.flatMap { $0 }.count, FormatCommand.allCases.count, "listed twice")
        let table = MenuShortcuts.table(shortcuts(platform: .mac))
        let keyed = FormatCommand.allCases.compactMap(\.shortcutTitle)
        XCTAssertEqual(keyed, [MenuShortcuts.bold, MenuShortcuts.italic, MenuShortcuts.link])
        for title in keyed {
            XCTAssertNotNil(table[title], title)
        }
        XCTAssertEqual(table[MenuShortcuts.link], KeyCombo(key: .character(text: "k"), command: true, control: false, shift: false))
        XCTAssertEqual(
            table[MenuShortcuts.focusMode], KeyCombo(key: .character(text: "f"), command: true, control: false, shift: true))
        XCTAssertEqual(FormatCommand.heading2.action, .heading(level: 2))
    }

    // MARK: Focus mode

    func testTheSentenceAroundTheCaret() {
        let text = "One. Two three. Four\nNext line here.\n\nLast." as NSString
        func lit(_ at: Int) -> String { text.substring(with: Sentences.around(at, in: text)) }
        XCTAssertEqual(lit(7), "Two three.")
        XCTAssertEqual(lit(0), "One.")
        XCTAssertEqual(lit(5), "Two three.", "between sentences: the next")
        XCTAssertEqual(lit(20), "Four", "end of the line: the last")
        XCTAssertEqual(lit(text.range(of: "line").location), "Next line here.")
        XCTAssertEqual(Sentences.around(text.range(of: "\n\n").location + 1, in: text).length, 0, "an empty line")
        XCTAssertEqual(lit(text.length), "Last.")
        XCTAssertEqual(Sentences.around(0, in: ""), NSRange(location: 0, length: 0))
        let emoji = "📚 Read it. Then 🧭 more." as NSString
        XCTAssertEqual(emoji.substring(with: Sentences.around(emoji.length - 2, in: emoji)), "Then 🧭 more.")
    }

    // MARK: Speed

    /// A guard, not a benchmark: styling a 10,000-word summary in full (as
    /// opening the page does) and restyling after one keystroke (as typing
    /// does) must stay well within a frame or two.
    func testHighlightingTenThousandWordsIsQuick() throws {
        let block = """
            ## A heading for this part

            Some **bold** words, some *italic* ones, a `code` span and ~~struck~~ text, then plain \
            words to make up a sentence of a sensible length for a summary.
            > A quoted line with *emphasis* in it.
            - A bullet item
            1. A numbered item


            """
        var text = ""
        while wordCount(text: text) < 10_000 { text += block }
        let storage = NSTextStorage(string: text)
        let styler = self.styler()

        let full = Date()
        styler.restyleAll(storage)
        let fullTime = Date().timeIntervalSince(full)
        XCTAssertLessThan(fullTime, 1.0, "styling 10,000 words took \(fullTime)s")
        XCTAssertTrue(MarkdownStyler.describe(storage).contains("bold"))

        let highlighter = LiveHighlighter(styler: styler)
        storage.delegate = highlighter
        let middle = (text as NSString).length / 2
        let keystroke = Date()
        for i in 0..<100 {
            storage.replaceCharacters(in: NSRange(location: middle + i, length: 0), with: "x")
        }
        let perKeystroke = Date().timeIntervalSince(keystroke) / 100
        XCTAssertLessThan(perKeystroke, 0.01, "restyling after a keystroke took \(perKeystroke)s")
        XCTAssertLessThan(try XCTUnwrap(highlighter.lastRestyled).length, 400, "restyled more than the line")

        measure { styler.restyleAll(storage) }
    }
}
