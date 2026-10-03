import AppKit
import SwiftUI
import BookshelfKit

/// The menu bar. Keys come from the core's shortcut table, the one every
/// Bookshelf and the manual share.
struct BookshelfCommands: Commands {
    let model: AppModel

    var body: some Commands {
        // About names the build: "Bookshelf Preview" in anything but a release.
        CommandGroup(replacing: .appInfo) {
            let name = BuildChannel.appName(BuildChannel.current)
            Button("About \(name)") {
                NSApp.orderFrontStandardAboutPanel(options: [.applicationName: name])
            }
        }
        CommandGroup(replacing: .newItem) {
            FileMenuItems(model: model)
        }
        // ⌘F searches the shelves; on the writing page it finds in the
        // text instead (Find ▸ Find…). See `TextMenuItems`.
        CommandGroup(replacing: .textEditing) {
            TextMenuItems(model: model)
        }
        CommandGroup(replacing: .textFormatting) {
            FormatMenuItems(model: model)
        }
        CommandGroup(before: .sidebar) {
            ShelfMenuItems(model: model)
            Divider()
        }
        CommandGroup(after: .sidebar) {
            FocusModeMenuItem(model: model)
        }
        CommandMenu("Book") {
            BookMenuItems(model: model)
        }
        CommandGroup(replacing: .help) {
            HelpMenuItem(model: model)
        }
    }
}

// Menu items are small views so they follow the model as it changes.

private struct FileMenuItems: View {
    let model: AppModel

    var body: some View {
        Button("Add a Book…") { model.isAddingBook = true }
            .keyboardShortcut(model.shortcut(MenuShortcuts.addBook))
            .disabled(model.phase != .ready || model.page == .writer || model.isAddingBook)
        Divider()
        Button("New Profile…") { model.isCreatingProfile = true }
            .disabled(model.phase != .ready || model.page != .shelves)
        Divider()
        // It saves as you type; this is for the reflex.
        Button("Save") { model.saveWriting() }
            .keyboardShortcut(model.shortcut(MenuShortcuts.save))
            .disabled(!model.isWriting)
    }
}

private struct ShelfMenuItems: View {
    let model: AppModel

    var body: some View {
        ForEach(Shelf.inOrder, id: \.self) { shelf in
            Button(shelf.title) { model.show(shelf) }
                .keyboardShortcut(model.shortcut(MenuShortcuts.title(for: shelf)))
                .disabled(model.phase != .ready || model.page == .writer || model.isAddingBook)
        }
    }
}

private struct BookMenuItems: View {
    let model: AppModel

    var body: some View {
        let entry = model.selectedEntry
        let onShelves = model.canOpenPages
        Button(entry?.body.isEmpty == false ? "Edit Summary" : "Write Summary") { model.openWriter() }
            .keyboardShortcut(model.shortcut(MenuShortcuts.write))
            .disabled(!onShelves && model.page != .reader)
        Button("Read Summary") { model.openReader() }
            .keyboardShortcut(model.shortcut(MenuShortcuts.read))
            .disabled(!onShelves && !model.isWriting)
        Divider()
        Button("Read It Again") { model.readAgain() }
            .disabled(!onShelves)
        Button("Remove from My Shelf") { remove() }
            .keyboardShortcut(.delete, modifiers: .command)
            .disabled(!onShelves)
    }

    /// ⌘⌫ also deletes to the start of the line while typing: then it's
    /// the text's, not the book's.
    private func remove() {
        if let text = TextEditing.activeTextView {
            text.deleteToBeginningOfLine(nil)
            return
        }
        model.removeSelected()
    }
}

private struct HelpMenuItem: View {
    let model: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("Bookshelf Help") { openWindow(id: SceneID.help) }
            .keyboardShortcut(model.shortcut(MenuShortcuts.manual))
    }
}

enum SceneID {
    static let main = "main"
    static let help = "help"
}
