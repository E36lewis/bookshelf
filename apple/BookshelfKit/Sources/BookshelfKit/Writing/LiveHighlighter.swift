import AppKit

/// Keeps a text view's Markdown styled as it's edited, as its text
/// storage's delegate.
///
/// - Only the lines an edit touched are restyled, before the storage
///   finishes processing the edit, so they're laid out once, already
///   styled, and fonts are still fixed up afterwards (emoji and other
///   characters the writing font lacks).
/// - Styling never registers undo: attributes set on the storage directly
///   aren't undoable, unlike edits that go through the text view. An undo
///   brings back old attributes with the old text, and they're restyled
///   like any edit.
/// - Text still being composed with an input method (marked text) is left
///   alone until it's committed, which is an edit of its own.
@MainActor
public final class LiveHighlighter: NSObject, NSTextStorageDelegate {
    public var styler: MarkdownStyler
    /// The text view, to know when it's composing.
    public weak var textView: NSTextView?
    /// The lines the last edit restyled.
    public private(set) var lastRestyled: NSRange?

    public init(styler: MarkdownStyler) {
        self.styler = styler
    }

    // Text storage edits happen on the main thread: the text view's.
    public nonisolated func textStorage(
        _ textStorage: NSTextStorage, willProcessEditing editedMask: NSTextStorageEditActions,
        range editedRange: NSRange, changeInLength delta: Int
    ) {
        guard editedMask.contains(.editedCharacters) else { return }
        MainActor.assumeIsolated {
            guard textView?.hasMarkedText() != true else { return }
            lastRestyled = styler.restyle(textStorage, around: editedRange)
        }
    }
}

/// Sentences, for focus mode.
public enum Sentences {
    /// The sentence the caret at `location` is in, within its own line (a
    /// Markdown line is its own heading, list item or paragraph). Between
    /// two sentences it's the next one; at the end of a line, the last.
    /// An empty line has none: an empty range at `location`.
    public static func around(_ location: Int, in text: NSString) -> NSRange {
        let caret = min(max(location, 0), text.length)
        let line = text.paragraphRange(for: NSRange(location: caret, length: 0))
        var content = line
        while content.length > 0 {
            let last = text.character(at: NSMaxRange(content) - 1)
            guard last == 0x0A || last == 0x0D || last == 0x2029 || last == 0x2028 else { break }
            content.length -= 1
        }
        var found: NSRange?
        var last: NSRange?
        text.enumerateSubstrings(in: content, options: [.bySentences, .substringNotRequired]) { _, range, enclosing, stop in
            last = range
            if caret >= enclosing.location, caret < NSMaxRange(enclosing) {
                found = range
                stop.pointee = true
            }
        }
        return found ?? last ?? NSRange(location: caret, length: 0)
    }
}

extension NSTextView {
    /// A formatting button (or its menu item): edits the Markdown around
    /// the selection with the core's `format_edit`, as one undoable step,
    /// and selects what the core says. With nothing selected, inline styles
    /// take the word at the caret (found with the text view's own word
    /// rules). Returns whether anything changed.
    @discardableResult
    public func applyFormat(_ action: FormatAction, undoName: String? = nil) -> Bool {
        var selection = selectedRange()
        if selection.length == 0, formatExpandsToWord(action: action) {
            selection = wordRange(at: selection.location)
        }
        guard let edit = formatEdit(text: string, selection: selection, action: action) else { return false }
        // Its own undo step: not merged with the typing before or after it.
        breakUndoCoalescing()
        guard shouldChangeText(in: edit.range, replacementString: edit.replacement) else { return false }
        textStorage?.replaceCharacters(in: edit.range, with: edit.replacement)
        didChangeText()
        setSelectedRange(edit.newSelection)
        if let undoName { undoManager?.setActionName(undoName) }
        breakUndoCoalescing()
        return true
    }

    /// The word at `location`, as a double-click would select it, or the
    /// one just before it (a caret at the end of a word). Spaces and
    /// punctuation aren't a word: then just the caret.
    public func wordRange(at location: Int) -> NSRange {
        let text = string as NSString
        func isWord(_ range: NSRange) -> Bool {
            range.length > 0 && NSMaxRange(range) <= text.length
                && text.substring(with: range).rangeOfCharacter(from: .alphanumerics) != nil
        }
        let here = selectionRange(forProposedRange: NSRange(location: location, length: 0), granularity: .selectByWord)
        if isWord(here) { return here }
        if location > 0 {
            let before = selectionRange(
                forProposedRange: NSRange(location: location - 1, length: 0), granularity: .selectByWord)
            if isWord(before) { return before }
        }
        return NSRange(location: location, length: 0)
    }
}
