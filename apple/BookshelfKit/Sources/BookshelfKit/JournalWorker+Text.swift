import Foundation
import BookshelfFFI

/// The core's journal-free helpers (Markdown, the manual, colors, search),
/// so the app makes every core call through the worker and none on the
/// main actor. They don't need a journal (the manual opens even when the
/// journal can't), so they're static, and they run on the shared pool
/// rather than the queue: each is quick, pure work.
extension JournalWorker {
    /// Markdown laid out as blocks, for the book page and the reader.
    public static func blocks(for markdown: String) async -> [Block] {
        renderMarkdown(text: markdown)
    }

    /// The user manual.
    public static func userManual() async -> Manual {
        manual()
    }

    /// The Mac's keyboard shortcuts, by title.
    public static func macShortcuts() async -> [String: KeyCombo] {
        MenuShortcuts.table(shortcuts(platform: .mac))
    }

    /// The accent colors offered in Settings.
    public static func accentChoices() async -> [Accent] {
        accents()
    }

    /// An accent's shades for a light window and a dark one.
    public static func palettes(for hex: String) async -> AccentPalettes {
        AccentPalettes(light: accentColors(hex: hex, dark: false), dark: accentColors(hex: hex, dark: true))
    }

    /// The writing and reading page's measurements for `settings`.
    public static func pageLayout(for settings: ProfileSettings) async -> PageLayout {
        writerLayout(settings: settings)
    }

    /// The questions on an empty writing page for an entry on `shelf`.
    public static func writingPrompts(for shelf: Shelf) async -> String {
        prompts(shelf: shelf)
    }

    /// How long typing has to pause before the writing page saves.
    public static func autosaveDelay() async -> Duration {
        .milliseconds(Int(autosaveMs()))
    }

    /// The word count of `text`, as saving counts it.
    public static func words(in text: String) async -> UInt32 {
        wordCount(text: text)
    }

    /// Checks an email as typed: `nil` for blank, else the trimmed address,
    /// or an `Invalid` error to show under the field.
    public static func checkedEmail(_ email: String) async throws -> String? {
        try validateEmail(email: email)
    }

    /// The entries on any shelf that match `query`; `nil` when there's no
    /// query (show the shelf as it is).
    public static func search(_ shelves: [Shelf: ShelfView], for query: String) async -> [ShelfSection]? {
        let terms = queryTerms(query: query)
        guard !terms.isEmpty else { return nil }
        return ShelfLayout.search(shelves, terms: terms)
    }

    /// `path` for people, with the home folder as `~`.
    public static func displayed(_ path: String, home: String?) async -> String {
        displayPath(path: path, home: home)
    }
}

/// One accent's shades, for light and dark windows.
public struct AccentPalettes: Equatable, Sendable {
    public var light: AccentPalette
    public var dark: AccentPalette

    public init(light: AccentPalette, dark: AccentPalette) {
        self.light = light
        self.dark = dark
    }
}
