import Foundation
import BookshelfKit

/// What the app was launched with. Normally nothing: the preview journal in
/// Application Support. UI tests ask for a journal of their own inside the
/// app's sandbox, a fixed window size and a light or dark look, so their
/// screenshots are repeatable and never touch anyone's real journal.
struct LaunchOptions: Sendable {
    enum JournalChoice: Sendable {
        /// This build's own journal (`Channel.preview`, never the real one).
        case preview
        /// An empty journal in a temporary folder (`-BookshelfFreshJournal`).
        case fresh
        /// A temporary journal filled by `DemoJournal` (`-BookshelfDemoJournal`).
        case demo
    }

    var journal: JournalChoice = .preview
    /// Light or dark whatever the settings say (`-BookshelfAppearance dark`).
    var appearance: Theme?
    /// The main window's content size (`-BookshelfWindowSize 1200x760`).
    var windowSize: CGSize?

    static let current = LaunchOptions(arguments: CommandLine.arguments)

    init(arguments: [String]) {
        if arguments.contains("-BookshelfDemoJournal") {
            journal = .demo
        } else if arguments.contains("-BookshelfFreshJournal") {
            journal = .fresh
        }
        // The look and size are only for test journals.
        guard journal != .preview else { return }
        switch Self.value(after: "-BookshelfAppearance", in: arguments) {
        case "light": appearance = .light
        case "dark": appearance = .dark
        default: break
        }
        if let size = Self.value(after: "-BookshelfWindowSize", in: arguments) {
            let parts = size.split(separator: "x").compactMap { Double($0) }
            if parts.count == 2, (400...4000).contains(parts[0]), (300...3000).contains(parts[1]) {
                windowSize = CGSize(width: parts[0], height: parts[1])
            }
        }
    }

    private static func value(after flag: String, in arguments: [String]) -> String? {
        guard let i = arguments.firstIndex(of: flag), arguments.indices.contains(i + 1) else { return nil }
        return arguments[i + 1]
    }

    /// Opens the journal this launch uses. Slow (it may migrate the
    /// database), so call it off the main thread.
    func openJournal() throws -> JournalWorker {
        switch journal {
        case .preview:
            return try JournalWorker(channel: .preview)
        case .fresh, .demo:
            let dir = FileManager.default.temporaryDirectory
                .appendingPathComponent("journal-\(UUID().uuidString)", isDirectory: true)
            return try JournalWorker(directory: dir)
        }
    }
}
