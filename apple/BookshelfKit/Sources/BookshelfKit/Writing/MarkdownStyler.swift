import AppKit
import BookshelfFFI

extension NSAttributedString.Key {
    /// The Markdown styles a run of the writing page has, as a
    /// `MarkdownStyle` raw value. Never drawn; it's what the styler set,
    /// for tests and the UI tests' probe.
    public static let markdownStyle = NSAttributedString.Key("BookshelfMarkdownStyle")
}

/// The Markdown styles one character can have at once.
public struct MarkdownStyle: OptionSet, Hashable {
    public let rawValue: UInt16
    public init(rawValue: UInt16) { self.rawValue = rawValue }

    public static let bold = MarkdownStyle(rawValue: 1 << 0)
    public static let italic = MarkdownStyle(rawValue: 1 << 1)
    public static let quote = MarkdownStyle(rawValue: 1 << 2)
    public static let code = MarkdownStyle(rawValue: 1 << 3)
    public static let strike = MarkdownStyle(rawValue: 1 << 4)
    /// The marks themselves (`**`, `#`, `- `).
    public static let marks = MarkdownStyle(rawValue: 1 << 5)

    /// Bits 8 to 10: the heading level, 0 for none.
    private static let levelShift: UInt16 = 8
    private static let levelMask: UInt16 = 0b111 << levelShift

    /// The heading level, 1 to 6, or 0 for text that isn't a heading.
    public var headingLevel: Int {
        get { Int((rawValue & Self.levelMask) >> Self.levelShift) }
        set {
            let level = UInt16(min(max(newValue, 0), 6))
            self = MarkdownStyle(rawValue: (rawValue & ~Self.levelMask) | (level << Self.levelShift))
        }
    }

    /// The bit for one of the core's span kinds (headings are levels).
    init(_ kind: StyleKind) {
        switch kind {
        case .bold: self = .bold
        case .italic: self = .italic
        case .quote: self = .quote
        case .code: self = .code
        case .strike: self = .strike
        case .syntax: self = .marks
        case .heading: self = []
        }
    }
}

/// Styles Markdown on the writing page from the core's spans: headings
/// bold and larger, real bold and italic faces, quotes in italic and a
/// softer color, code on a faint background, struck text, and the marks
/// themselves dimmed.
///
/// The core highlights each line on its own, so after an edit only the
/// lines it touched need restyling (`restyle(_:around:)`). Attributes are
/// set straight on the text storage, which never registers undo.
@MainActor
public final class MarkdownStyler {
    public let look: WritingLook
    public let palette: WritingPalette
    public let paragraphStyle: NSParagraphStyle
    /// For blank lines and the lines just before them (`WritingLook`).
    public let blankLineStyle: NSParagraphStyle
    /// Attributes by style, made once each.
    private var cache: [MarkdownStyle: [NSAttributedString.Key: Any]] = [:]

    public init(look: WritingLook, palette: WritingPalette = WritingPalette()) {
        self.look = look
        self.palette = palette
        paragraphStyle = look.paragraphStyle
        blankLineStyle = look.blankLineStyle
    }

    /// Plain text's attributes: the text view's typing attributes too.
    public var plainAttributes: [NSAttributedString.Key: Any] { attributes(for: []) }

    /// Restyles all of `storage`, as one batch of changes.
    public func restyleAll(_ storage: NSTextStorage) {
        storage.beginEditing()
        restyle(storage, around: NSRange(location: 0, length: storage.length))
        storage.endEditing()
    }

    /// Restyles the whole lines `range` touches, and returns them. Call
    /// inside a batch of edits (or from the storage's delegate while it
    /// processes one), so the changes are laid out once. The line before
    /// them gets its spacing redone too (`spaceLines`).
    @discardableResult
    public func restyle(_ storage: NSTextStorage, around range: NSRange) -> NSRange {
        let all = storage.string as NSString
        let safe = NSRange(location: min(range.location, all.length),
                           length: min(range.length, all.length - min(range.location, all.length)))
        let lines = all.paragraphRange(for: safe)
        guard lines.length > 0 else { return lines }
        let text = all.substring(with: lines)
        let styles = Self.styles(of: text, length: lines.length)
        var start = 0
        while start < styles.count {
            var end = start + 1
            while end < styles.count, styles[end] == styles[start] { end += 1 }
            storage.setAttributes(attributes(for: styles[start]),
                                  range: NSRange(location: lines.location + start, length: end - start))
            start = end
        }
        spaceLines(storage, lines, in: all)
        return lines
    }

    /// Sets each line's spacing: space below a line only when a line of
    /// text follows it, none below a blank line or the line before one.
    /// The line before `lines` is redone as well, since whether a blank
    /// line follows it may just have changed.
    private func spaceLines(_ storage: NSTextStorage, _ lines: NSRange, in text: NSString) {
        var location = lines.location
        if location > 0 {
            location = text.paragraphRange(for: NSRange(location: location - 1, length: 0)).location
        }
        let end = NSMaxRange(lines)
        while location < end {
            let line = text.paragraphRange(for: NSRange(location: location, length: 0))
            let next = NSMaxRange(line)
            let close = Self.isBlank(line, in: text)
                || (next < text.length && Self.isBlank(text.paragraphRange(for: NSRange(location: next, length: 0)), in: text))
            storage.addAttribute(.paragraphStyle, value: close ? blankLineStyle : paragraphStyle, range: line)
            guard next > location else { break }
            location = next
        }
    }

    /// Whether `line` has nothing but spaces before its line break.
    static func isBlank(_ line: NSRange, in text: NSString) -> Bool {
        for i in line.location..<NSMaxRange(line) {
            switch text.character(at: i) {
            case 0x20, 0x09, 0x0A, 0x0D, 0x2028, 0x2029: continue
            default: return false
            }
        }
        return true
    }

    /// Each UTF-16 unit's styles in `text` (whole lines, `length` units).
    static func styles(of text: String, length: Int) -> [MarkdownStyle] {
        var styles = [MarkdownStyle](repeating: [], count: length)
        var units: [UInt16]?
        for span in markdownSpans(text: text) {
            let start = min(Int(span.start), length)
            let end = min(Int(span.end), length)
            guard start < end else { continue }
            if span.kind == .heading {
                // The level is the number of `#`s the line starts with.
                if units == nil { units = Array(text.utf16) }
                let hash = UInt16(UInt8(ascii: "#"))
                var level = 0
                if let units {
                    var i = start
                    while i < end, units[i] == UInt16(UInt8(ascii: " ")) { i += 1 }
                    while i < end, units[i] == hash, level < 6 { level += 1; i += 1 }
                }
                for i in start..<end { styles[i].headingLevel = max(level, 1) }
            } else {
                let bit = MarkdownStyle(span.kind)
                for i in start..<end { styles[i].formUnion(bit) }
            }
        }
        return styles
    }

    /// The attributes for text with `style`.
    public func attributes(for style: MarkdownStyle) -> [NSAttributedString.Key: Any] {
        if let made = cache[style] { return made }
        let fonts = look.fonts
        let heading = style.headingLevel
        let bold = style.contains(.bold) || heading > 0
        let italic = style.contains(.italic) || style.contains(.quote)
        var font = style.contains(.code) ? fonts.code : fonts.face(bold: bold, italic: italic)
        let scale = WritingLook.headingScale(heading)
        if scale != 1 {
            font = NSFont(descriptor: font.fontDescriptor, size: font.pointSize * scale) ?? font
        }
        var made: [NSAttributedString.Key: Any] = [
            .font: font,
            .paragraphStyle: paragraphStyle,
            .foregroundColor: style.contains(.marks) ? palette.marks
                : style.contains(.quote) ? palette.quote : palette.text,
            .markdownStyle: NSNumber(value: style.rawValue),
        ]
        if !style.contains(.code) {
            if italic, fonts.syntheticItalic { made[.obliqueness] = 0.18 }
            if bold, fonts.syntheticBold { made[.strokeWidth] = -3 }
        }
        if style.contains(.code) { made[.backgroundColor] = palette.codeBackground }
        if style.contains(.strike) { made[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
        cache[style] = made
        return made
    }

    /// What the styler set on `storage`, for tests and the UI tests' probe:
    /// one line per style, with the ranges that have it, like
    /// `bold 8-13` or `marks 6-8 13-15`.
    public static func describe(_ storage: NSAttributedString) -> String {
        var ranges: [String: [NSRange]] = [:]
        storage.enumerateAttribute(.markdownStyle, in: NSRange(location: 0, length: storage.length)) { value, range, _ in
            guard let raw = (value as? NSNumber)?.uint16Value else { return }
            let style = MarkdownStyle(rawValue: raw)
            var names: [String] = []
            if style.headingLevel > 0 { names.append("heading\(style.headingLevel)") }
            for (bit, name) in [(MarkdownStyle.bold, "bold"), (.italic, "italic"), (.quote, "quote"),
                                (.code, "code"), (.strike, "strike"), (.marks, "marks")]
            where style.contains(bit) {
                names.append(name)
            }
            for name in names {
                // Runs next to each other with the same style join up.
                var list = ranges[name, default: []]
                if let last = list.last, NSMaxRange(last) == range.location {
                    list[list.count - 1].length += range.length
                } else {
                    list.append(range)
                }
                ranges[name] = list
            }
        }
        return ranges.keys.sorted().map { name in
            ([name] + (ranges[name] ?? []).map { "\($0.location)-\(NSMaxRange($0))" }).joined(separator: " ")
        }.joined(separator: "\n")
    }
}
