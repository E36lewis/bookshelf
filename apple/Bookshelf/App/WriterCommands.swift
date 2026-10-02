import AppKit
import SwiftUI
import BookshelfKit

// The writing page's menus. They act on the page's text view through the
// model (`AppModel.editor`), so they work wherever focus is in the window,
// and are off when the writing page isn't showing.

/// Edit's text group, in the order Mac apps have it:
///
/// - **Search Your Shelves** (⌘F on the shelves, the book page and the
///   reader).
/// - **Find ▸** for the writing page's text: Find… takes ⌘F while that
///   page shows (Search Your Shelves can't search anything there), plus
///   Find and Replace…, Find Next/Previous, Use Selection for Find and
///   Jump to Selection, through the text view's find bar.
/// - **Spelling and Grammar ▸**, **Substitutions ▸** (remembered across
///   launches; smart quotes, smart dashes and text replacement start off,
///   since they change Markdown's characters), **Transformations ▸** and
///   **Speech ▸**.
struct TextMenuItems: View {
    let model: AppModel

    var body: some View {
        let writing = model.isWriting
        let prefs = model.writingPreferences
        Button("Search Your Shelves") { model.focusSearch() }
            .keyboardShortcut(writing ? nil : model.shortcut(MenuShortcuts.search))
            .disabled(model.phase != .ready || writing || model.page == .writer || model.isAddingBook)
        Menu("Find") {
            Button("Find…") { model.find(.showFindInterface) }
                .keyboardShortcut(writing ? KeyboardShortcut("f", modifiers: .command) : nil)
            Button("Find and Replace…") { model.find(.showReplaceInterface) }
                .keyboardShortcut("f", modifiers: [.command, .option])
            Button("Find Next") { model.find(.nextMatch) }
                .keyboardShortcut("g", modifiers: .command)
            Button("Find Previous") { model.find(.previousMatch) }
                .keyboardShortcut("g", modifiers: [.command, .shift])
            Button("Use Selection for Find") { model.find(.setSearchString) }
                .keyboardShortcut("e", modifiers: .command)
            Button("Jump to Selection") { model.textCommand(#selector(NSTextView.centerSelectionInVisibleArea(_:))) }
                .keyboardShortcut("j", modifiers: .command)
        }
        .disabled(!writing)
        Menu("Spelling and Grammar") {
            Button("Show Spelling and Grammar") { model.textCommand(#selector(NSText.showGuessPanel(_:))) }
                .keyboardShortcut(":", modifiers: .command)
            Button("Check Document Now") { model.textCommand(#selector(NSText.checkSpelling(_:))) }
                .keyboardShortcut(";", modifiers: .command)
            Divider()
            Toggle("Check Spelling While Typing", isOn: prefs.binding(\.checkSpelling))
            Toggle("Check Grammar With Spelling", isOn: prefs.binding(\.checkGrammar))
            Toggle("Correct Spelling Automatically", isOn: prefs.binding(\.correctSpelling))
        }
        .disabled(!writing)
        Menu("Substitutions") {
            Button("Show Substitutions") {
                model.textCommand(#selector(NSTextView.orderFrontSubstitutionsPanel(_:)))
            }
            Divider()
            Toggle("Smart Copy/Paste", isOn: prefs.binding(\.smartCopyPaste))
            Toggle("Smart Quotes", isOn: prefs.binding(\.smartQuotes))
            Toggle("Smart Dashes", isOn: prefs.binding(\.smartDashes))
            Toggle("Text Replacement", isOn: prefs.binding(\.textReplacement))
        }
        .disabled(!writing)
        Menu("Transformations") {
            Button("Make Upper Case") { model.textCommand(#selector(NSResponder.uppercaseWord(_:))) }
            Button("Make Lower Case") { model.textCommand(#selector(NSResponder.lowercaseWord(_:))) }
            Button("Capitalize") { model.textCommand(#selector(NSResponder.capitalizeWord(_:))) }
        }
        .disabled(!writing)
        Menu("Speech") {
            Button("Start Speaking") { model.textCommand(#selector(NSTextView.startSpeaking(_:))) }
            Button("Stop Speaking") { model.textCommand(#selector(NSTextView.stopSpeaking(_:))) }
        }
        .disabled(!writing)
    }
}

/// The Format menu: Markdown formatting for the writing page, the same as
/// its toolbar. Bold, Italic and Link take their keys from the core's
/// table.
struct FormatMenuItems: View {
    let model: AppModel

    var body: some View {
        ForEach(Array(FormatCommand.groups.enumerated()), id: \.offset) { index, group in
            if index > 0 { Divider() }
            ForEach(group, id: \.self) { command in
                Button(command.title) { model.format(command) }
                    .keyboardShortcut(command.shortcutTitle.flatMap { model.shortcut($0) })
                    .disabled(!model.isWriting)
            }
        }
    }
}

/// View › Focus Mode (⇧⌘F), also on the writing page's toolbar.
struct FocusModeMenuItem: View {
    let model: AppModel

    var body: some View {
        Divider()
        Toggle("Focus Mode", isOn: Binding(get: { model.focusMode }, set: { model.focusMode = $0 }))
            .keyboardShortcut(model.shortcut(MenuShortcuts.focusMode))
            .disabled(!model.isWriting)
    }
}
