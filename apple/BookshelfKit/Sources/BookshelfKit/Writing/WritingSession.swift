import AppKit
import Observation

/// The writing page's text while it's open, and getting it saved: a moment
/// after typing stops, at once on Save, and on leaving. When the journal
/// won't take it, the text goes to the recovery folder, and if even that
/// fails, onto the clipboard: it's never just dropped.
@MainActor
@Observable
public final class WritingSession {
    public enum Status: Equatable {
        case saved
        /// Typed since the last save; it saves once typing pauses.
        case unsaved
        case saving
        /// The last save failed, for this reason.
        case failed(String)
    }

    /// How leaving went.
    public enum Outcome: Equatable {
        case saved
        /// Saving failed (`problem`); the text is in `file` instead.
        case rescued(problem: String, file: URL)
        /// Saving failed and so did the recovery copy: the text is on the
        /// clipboard.
        case copied(problem: String, rescueProblem: String)
    }

    /// Where the text goes: the journal, and the recovery folder when the
    /// journal won't take it.
    public struct Store: Sendable {
        /// Saves the text; returns its word count.
        public var save: @Sendable (String) async throws -> UInt32
        /// Writes the text to the recovery folder; returns the file.
        public var rescue: @Sendable (String) async throws -> URL
        /// Counts the words when a save fails, as saving would have.
        public var count: @Sendable (String) async -> UInt32

        public init(
            save: @escaping @Sendable (String) async throws -> UInt32,
            rescue: @escaping @Sendable (String) async throws -> URL,
            count: @escaping @Sendable (String) async -> UInt32 = { await JournalWorker.words(in: $0) }
        ) {
            self.save = save
            self.rescue = rescue
            self.count = count
        }
    }

    public let title: String
    /// The questions shown while the page is empty.
    public let prompts: String
    public private(set) var words: UInt32
    public private(set) var status: Status = .saved

    /// Not observed: views don't redraw on every keystroke.
    @ObservationIgnored public private(set) var text: String
    @ObservationIgnored private var savedText: String
    @ObservationIgnored private var pending: Task<Void, Never>?
    @ObservationIgnored private let delay: Duration
    @ObservationIgnored private let store: Store

    public init(title: String, text: String, words: UInt32, prompts: String, delay: Duration, store: Store) {
        self.title = title
        self.text = text
        self.savedText = text
        self.words = words
        self.prompts = prompts
        self.delay = delay
        self.store = store
    }

    /// The text changed: save once typing pauses.
    public func update(_ newText: String) {
        text = newText
        if status != .unsaved { status = .unsaved }
        pending?.cancel()
        pending = Task { [weak self, delay] in
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            await self?.save()
        }
    }

    /// Saves what's there now. Returns whether it's saved. The word count
    /// follows the text either way: a failed save still counts it.
    @discardableResult
    public func save() async -> Bool {
        pending?.cancel()
        let snapshot = text
        guard snapshot != savedText else {
            if status != .saved { status = .saved }
            return true
        }
        status = .saving
        do {
            let counted = try await store.save(snapshot)
            savedText = snapshot
            words = counted
            if text == snapshot { status = .saved } else if status == .saving { status = .unsaved }
            return true
        } catch {
            status = .failed(Self.message(for: error))
            let counted = await store.count(snapshot)
            // Unless a newer attempt has counted newer text meanwhile.
            if text == snapshot { words = counted }
            return false
        }
    }

    /// Leaving the page: one last save. If that fails, a copy in the
    /// recovery folder; if that fails too, the text goes on `pasteboard`.
    public func finish(pasteboard: NSPasteboard = .general) async -> Outcome {
        if await save() { return .saved }
        let problem = failure ?? "Something went wrong."
        do {
            return .rescued(problem: problem, file: try await store.rescue(text))
        } catch {
            pasteboard.clearContents()
            pasteboard.setString(text, forType: .string)
            return .copied(problem: problem, rescueProblem: Self.message(for: error))
        }
    }

    /// Why the last save failed, if it did.
    public var failure: String? {
        if case .failed(let message) = status { return message }
        return nil
    }

    /// "Saved", "Saving…", "Not saved: …", for the line under the page.
    public var statusText: String {
        switch status {
        case .saved: "Saved"
        case .unsaved, .saving: "Saving…"
        case .failed(let message): "Not saved: \(message)"
        }
    }

    private static func message(for error: Error) -> String {
        (error as? CoreError)?.userMessage ?? "Something went wrong."
    }
}

extension Wording {
    /// The alert's title when a summary couldn't be saved.
    public static func notSaved(_ title: String) -> String {
        "Your summary of “\(title)” couldn't be saved"
    }

    /// What the alert says happened to the text. The recovery folder is
    /// deep in the app's sandbox container, so it's named rather than
    /// spelled out: the alert has a Show in Finder button for it.
    public static func rescue(_ outcome: WritingSession.Outcome) -> String {
        switch outcome {
        case .saved:
            return ""
        case let .rescued(problem, file):
            return "\(problem)\n\nBookshelf kept a copy of your text in its recovery folder, "
                + "as “\(file.lastPathComponent)”. Nothing was lost."
        case let .copied(problem, rescueProblem):
            let why = rescueProblem.hasSuffix(".") ? String(rescueProblem.dropLast()) : rescueProblem
            return "\(problem)\n\nA recovery copy couldn't be written either (\(why)), so your text was "
                + "copied to the clipboard. Paste it somewhere safe before closing Bookshelf."
        }
    }
}
