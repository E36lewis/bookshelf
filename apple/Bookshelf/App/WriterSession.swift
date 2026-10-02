import Foundation
import BookshelfKit

/// The plain writing page's text while it's open: saves a moment after
/// typing stops, and once more on leaving. A stand-in until the real
/// writer (Phase 4).
@MainActor
@Observable
final class WriterSession {
    enum Status: Equatable {
        case saved
        case editing
        case saving
        case failed(String)
    }

    /// How leaving went.
    enum Outcome {
        case saved
        /// Saving failed; the text is in this file instead.
        case rescued(URL)
        /// Saving failed and so did keeping a copy.
        case lost(String)
    }

    let summaryID: String
    let title: String
    /// The questions shown while the page is empty.
    let prompts: String
    private(set) var text: String
    private(set) var words: UInt32
    private(set) var status: Status = .saved

    @ObservationIgnored private let worker: JournalWorker
    @ObservationIgnored private let delay: Duration
    @ObservationIgnored private var savedText: String
    @ObservationIgnored private var pending: Task<Void, Never>?

    init(
        worker: JournalWorker, summaryID: String, title: String, text: String, words: UInt32, delay: Duration,
        prompts: String
    ) {
        self.worker = worker
        self.summaryID = summaryID
        self.title = title
        self.text = text
        self.savedText = text
        self.words = words
        self.delay = delay
        self.prompts = prompts
    }

    /// The text changed: save once typing pauses.
    func update(_ newText: String) {
        guard newText != text else { return }
        text = newText
        status = .editing
        pending?.cancel()
        pending = Task { [weak self, delay] in
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            _ = await self?.save()
        }
    }

    /// Saves what's there now. Returns whether it's saved.
    @discardableResult
    func save() async -> Bool {
        let snapshot = text
        guard snapshot != savedText else {
            if text == snapshot { status = .saved }
            return true
        }
        status = .saving
        do {
            let result = try await worker.saveBody(summaryID, snapshot)
            savedText = snapshot
            words = result.words
            if text == snapshot { status = .saved }
            return true
        } catch {
            status = .failed((error as? CoreError)?.userMessage ?? "Something went wrong.")
            return false
        }
    }

    /// Leaving the page: one last save, and if that fails, a copy in the
    /// recovery folder so nothing is lost.
    func finish() async -> Outcome {
        pending?.cancel()
        if await save() { return .saved }
        do {
            return .rescued(try await worker.rescueBody(title: title, body: text))
        } catch {
            return .lost((error as? CoreError)?.userMessage ?? "Something went wrong.")
        }
    }

    /// "Saved", "Not saved: …", for the line under the page.
    var statusText: String {
        switch status {
        case .saved: "Saved"
        case .editing: "Edited"
        case .saving: "Saving…"
        case .failed(let message): "Not saved: \(message)"
        }
    }
}
