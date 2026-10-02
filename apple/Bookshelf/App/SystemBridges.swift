import AppKit
import SwiftUI
import BookshelfKit

/// The app's look: light, dark or the system's, for every window at once.
@MainActor
enum Appearance {
    /// `override` (from a UI test) wins over the profile's setting.
    static func apply(_ theme: Theme, override: Theme? = nil) {
        switch override ?? theme {
        case .system: NSApp.appearance = nil
        case .light: NSApp.appearance = NSAppearance(named: .aqua)
        case .dark: NSApp.appearance = NSAppearance(named: .darkAqua)
        }
    }
}

/// The toolbar's search field, for ⌘F on macOS 14 (which can't focus a
/// `.searchable` field from code).
@MainActor
enum SearchField {
    static func focus() {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow else { return }
        if let item = window.toolbar?.items.lazy.compactMap({ $0 as? NSSearchToolbarItem }).first {
            item.beginSearchInteraction()
            return
        }
        // Not a search toolbar item: look for the field itself.
        if let root = window.contentView?.superview, let field = find(in: root) {
            window.makeFirstResponder(field)
        }
    }

    private static func find(in view: NSView) -> NSSearchField? {
        if let field = view as? NSSearchField { return field }
        for child in view.subviews {
            if let field = find(in: child) { return field }
        }
        return nil
    }
}

/// Whether someone is typing: a menu shortcut that's also a text editing
/// key (⌘⌫) must leave the text alone.
@MainActor
enum TextEditing {
    /// The text view being typed in, if any (a text field's editor counts).
    static var activeTextView: NSTextView? {
        guard let view = NSApp.keyWindow?.firstResponder as? NSTextView, view.isEditable else { return nil }
        return view
    }
}

/// Picks a folder with the standard open panel. In the sandbox, the folder
/// picked is readable and writable until the app quits.
@MainActor
enum FolderPicker {
    static func choose(message: String, prompt: String, startingIn folder: URL? = nil) async -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        panel.message = message
        panel.prompt = prompt
        panel.directoryURL = folder
        let response = await withCheckedContinuation { continuation in
            panel.begin { continuation.resume(returning: $0) }
        }
        return response == .OK ? panel.url : nil
    }

    /// Shows `url` in the Finder.
    static func reveal(_ url: URL) {
        NSWorkspace.shared.activateFileViewerSelecting([url])
    }
}

/// The user's home folder, for showing paths with `~`. In the sandbox
/// `NSHomeDirectory()` is the app's container, which isn't what people
/// know as home.
enum HomeFolder {
    static var path: String? {
        guard let entry = getpwuid(getuid()), let dir = entry.pointee.pw_dir else { return nil }
        return String(cString: dir)
    }
}

/// Remembers the backup folder someone picked, across launches, with a
/// security-scoped bookmark: the sandbox forgets the folder when the app
/// quits, and the bookmark is what lets Bookshelf back up there tomorrow.
@MainActor
final class BackupFolderAccess {
    private let key = "backupFolderBookmark"

    /// Keeps a bookmark to `folder` (picked just now, so accessible).
    func remember(_ folder: URL) throws {
        let data = try folder.bookmarkData(
            options: [.withSecurityScope], includingResourceValuesForKeys: nil, relativeTo: nil)
        UserDefaults.standard.set(data, forKey: key)
    }

    /// Backups are back in the default folder: no bookmark needed.
    func forget() {
        UserDefaults.standard.removeObject(forKey: key)
    }

    /// Runs `work` with the remembered folder open to the app, if there is
    /// one (the folder may be on a drive that isn't plugged in).
    func whileAccessing<T>(_ work: () async throws -> T) async rethrows -> T {
        let folder = resolve()
        let accessing = folder?.startAccessingSecurityScopedResource() ?? false
        defer {
            if accessing { folder?.stopAccessingSecurityScopedResource() }
        }
        return try await work()
    }

    private func resolve() -> URL? {
        guard let data = UserDefaults.standard.data(forKey: key) else { return nil }
        var stale = false
        guard let url = try? URL(
            resolvingBookmarkData: data, options: [.withSecurityScope], relativeTo: nil,
            bookmarkDataIsStale: &stale)
        else { return nil }
        if stale, url.startAccessingSecurityScopedResource() {
            // Moved or renamed: keep a fresh bookmark.
            try? remember(url)
            url.stopAccessingSecurityScopedResource()
        }
        return url
    }
}

extension Color {
    /// A `#rrggbb` color from the core; `nil` for anything else.
    init?(hex: String) {
        guard let rgb = RGB(hex: hex) else { return nil }
        self.init(.sRGB, red: rgb.red, green: rgb.green, blue: rgb.blue)
    }

    /// The color as `#rrggbb`, for the accent setting.
    var hex: String? {
        guard let srgb = NSColor(self).usingColorSpace(.sRGB) else { return nil }
        return RGB(red: srgb.redComponent, green: srgb.greenComponent, blue: srgb.blueComponent).hex
    }
}

extension KeyCombo {
    /// The combo as a SwiftUI menu shortcut.
    var keyboardShortcut: KeyboardShortcut? {
        let equivalent: KeyEquivalent
        switch key {
        case .character(let text):
            guard text.count == 1, let character = text.first else { return nil }
            equivalent = KeyEquivalent(character)
        case .return:
            equivalent = .return
        case .escape:
            equivalent = .escape
        case .function(let number):
            guard (1...12).contains(number),
                  let scalar = UnicodeScalar(UInt32(NSF1FunctionKey) + UInt32(number) - 1)
            else { return nil }
            equivalent = KeyEquivalent(Character(scalar))
        }
        var modifiers: EventModifiers = []
        if command { modifiers.insert(.command) }
        if control { modifiers.insert(.control) }
        if shift { modifiers.insert(.shift) }
        return KeyboardShortcut(equivalent, modifiers: modifiers)
    }
}

extension AppModel {
    /// The menu shortcut the core's table gives `title` (`MenuShortcuts`).
    func shortcut(_ title: String) -> KeyboardShortcut? {
        keys[title]?.keyboardShortcut
    }
}
