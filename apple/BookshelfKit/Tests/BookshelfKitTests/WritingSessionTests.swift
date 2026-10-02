import AppKit
import XCTest
@testable import BookshelfKit

/// Saving the writing page: autosave after a pause, the status line, and
/// the rescue path (recovery file, then the clipboard) when saving fails.
@MainActor
final class WritingSessionTests: XCTestCase {
    /// A journal stand-in that records what it was asked to keep.
    private actor FakeStore {
        var saved: [String] = []
        var rescued: [String] = []
        var failSaves = false
        var failRescue = false
        /// How long a save takes.
        var slowness: Duration = .zero

        func save(_ text: String) async throws -> UInt32 {
            if slowness > .zero { try await Task.sleep(for: slowness) }
            if failSaves { throw CoreError.Io(message: "The disk is full.") }
            saved.append(text)
            return wordCount(text: text)
        }

        func rescue(_ text: String) throws -> URL {
            if failRescue { throw CoreError.Io(message: "The recovery folder is read-only.") }
            rescued.append(text)
            return URL(fileURLWithPath: "/tmp/recovery/Dune.md")
        }

        func slow(_ duration: Duration) {
            slowness = duration
        }

        func failing(saves: Bool, rescue: Bool) {
            failSaves = saves
            failRescue = rescue
        }
    }

    private var fake: FakeStore!
    private var pasteboard: NSPasteboard!

    override func setUp() async throws {
        fake = FakeStore()
        pasteboard = NSPasteboard(name: NSPasteboard.Name("bookshelf-tests-\(UUID().uuidString)"))
    }

    override func tearDown() async throws {
        pasteboard.releaseGlobally()
    }

    private func session(_ text: String = "", delay: Duration = .milliseconds(50)) -> WritingSession {
        let fake = self.fake!
        return WritingSession(
            title: "Dune", text: text, words: wordCount(text: text), prompts: "Why this one?", delay: delay,
            store: .init(save: { try await fake.save($0) }, rescue: { try await fake.rescue($0) }))
    }

    /// Waits for `condition`, up to a couple of seconds.
    private func eventually(_ condition: () -> Bool) async {
        for _ in 0..<200 where !condition() {
            try? await Task.sleep(for: .milliseconds(10))
        }
    }

    func testAutosavesOnceTypingPauses() async {
        let page = session()
        XCTAssertEqual(page.statusText, "Saved")
        page.update("S")
        page.update("Sp")
        page.update("Spice must flow")
        XCTAssertEqual(page.status, .unsaved)
        XCTAssertEqual(page.statusText, "Saving…")
        await eventually { page.status == .saved }
        XCTAssertEqual(page.status, .saved)
        let saved = await fake.saved
        XCTAssertEqual(saved, ["Spice must flow"], "one save, after the pause")
        XCTAssertEqual(page.words, 3)
    }

    func testSaveNowAndNothingToSave() async {
        let page = session("Old", delay: .seconds(60))
        let ok = await page.save()
        XCTAssertTrue(ok)
        var saved = await fake.saved
        XCTAssertEqual(saved, [], "unchanged text isn't saved again")
        page.update("New words")
        let okAgain = await page.save()
        XCTAssertTrue(okAgain)
        saved = await fake.saved
        XCTAssertEqual(saved, ["New words"])
        XCTAssertEqual(page.status, .saved)
    }

    func testAFailedSaveSaysWhyAndLeavingRescuesTheText() async throws {
        await fake.failing(saves: true, rescue: false)
        let page = session()
        page.update("Keep me safe")
        let ok = await page.save()
        XCTAssertFalse(ok)
        XCTAssertEqual(page.status, .failed("The disk is full."))
        XCTAssertEqual(page.statusText, "Not saved: The disk is full.")

        let outcome = await page.finish(pasteboard: pasteboard)
        XCTAssertEqual(outcome, .rescued(problem: "The disk is full.", file: URL(fileURLWithPath: "/tmp/recovery/Dune.md")))
        let rescued = await fake.rescued
        XCTAssertEqual(rescued, ["Keep me safe"])
        XCTAssertNil(pasteboard.string(forType: .string))

        let message = Wording.rescue(outcome, shownFile: "~/recovery/Dune.md")
        XCTAssertTrue(message.hasPrefix("The disk is full."))
        XCTAssertTrue(message.contains("~/recovery/Dune.md"))
        XCTAssertEqual(Wording.notSaved("Dune"), "Your summary of “Dune” couldn't be saved")
    }

    func testWhenEvenTheRecoveryCopyFailsTheTextGoesOnTheClipboard() async {
        await fake.failing(saves: true, rescue: true)
        let page = session()
        page.update("Last resort")
        let outcome = await page.finish(pasteboard: pasteboard)
        XCTAssertEqual(outcome, .copied(problem: "The disk is full.", rescueProblem: "The recovery folder is read-only."))
        XCTAssertEqual(pasteboard.string(forType: .string), "Last resort")
        let message = Wording.rescue(outcome, shownFile: nil)
        XCTAssertTrue(message.contains("(The recovery folder is read-only), so your text was copied to the clipboard"),
                      message)
    }

    func testSavingWorksAgainAfterAFailure() async {
        await fake.failing(saves: true, rescue: false)
        let page = session()
        page.update("Try")
        let ok = await page.save()
        XCTAssertFalse(ok)
        await fake.failing(saves: false, rescue: false)
        page.update("Try again")
        await eventually { page.status == .saved }
        XCTAssertEqual(page.status, .saved)
        let outcome = await page.finish(pasteboard: pasteboard)
        XCTAssertEqual(outcome, .saved)
    }

    func testTypingDuringASaveKeepsItUnsaved() async {
        await fake.slow(.milliseconds(300))
        let page = session(delay: .seconds(60))
        page.update("First")
        let saving = Task { await page.save() }
        try? await Task.sleep(for: .milliseconds(50))
        XCTAssertEqual(page.status, .saving)
        page.update("First and more")
        _ = await saving.value
        XCTAssertNotEqual(page.status, .saved, "newer text isn't saved yet")
        let ok = await page.save()
        XCTAssertTrue(ok)
        XCTAssertEqual(page.status, .saved)
        let saved = await fake.saved
        XCTAssertEqual(saved.last, "First and more")
    }
}
