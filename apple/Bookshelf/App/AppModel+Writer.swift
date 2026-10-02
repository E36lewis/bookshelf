import AppKit
import BookshelfKit

/// The writing page's menu commands. They act on its text view, wherever
/// the keyboard focus is in the window (the find bar's field, say).
extension AppModel {
    /// Whether the writing page is showing: the Format menu, Find and the
    /// other text menus work only then.
    var isWriting: Bool { page == .writer && writer != nil }

    /// The writing page's text view arrived (`attached`) or left.
    func attach(_ view: WritingTextView, _ attached: Bool) {
        if attached {
            editor = view
        } else if editor === view {
            editor = nil
        }
    }

    /// Format menu and toolbar: one undoable edit, named after the command.
    func format(_ command: FormatCommand) {
        editor?.format(command.action, named: command.title)
    }

    /// Edit › Find.
    func find(_ action: NSTextFinder.Action) {
        editor?.performFind(action)
    }

    /// A standard text command: spelling, transformations, speech.
    func textCommand(_ selector: Selector) {
        editor?.perform(command: selector)
    }

    /// File › Save (⌘S): saves at once, and says so.
    func saveWriting() {
        guard let writer else { return }
        Task {
            if await writer.save() { Announcer.say("Saved") }
        }
    }

    /// A toolbar button's tooltip: "Bold (⌘B)".
    func helpText(for command: FormatCommand) -> String {
        guard let title = command.shortcutTitle, let keys = keys[title] else { return command.title }
        return "\(command.title) (\(keys.symbols))"
    }
}

extension KeyCombo {
    /// The combo as the menus write it: ⌃⇧⌘F.
    var symbols: String {
        var text = ""
        if control { text += "⌃" }
        if shift { text += "⇧" }
        if command { text += "⌘" }
        switch key {
        case .character(let character): text += character.uppercased()
        case .return: text += "↩"
        case .escape: text += "⎋"
        case .function(let number): text += "F\(number)"
        }
        return text
    }
}
