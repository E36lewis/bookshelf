import XCTest
@testable import BookshelfKit

/// The demo journal the UI tests and screenshots use.
final class DemoJournalTests: XCTestCase {
    func testTheDemoJournalFillsEveryShelfOffline() async throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("bookshelf-demo-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let worker = try JournalWorker(directory: dir)
        let calendar = Calendar(identifier: .gregorian)
        let today = try XCTUnwrap(JournalDate.date(from: "2026-10-02", calendar: calendar))

        let avery = try await DemoJournal.seed(worker, today: today, calendar: calendar, theme: .dark)
        XCTAssertEqual(avery.name, "Avery")
        let names = try await worker.profiles().map(\.name)
        XCTAssertEqual(names.sorted(), ["Avery", "Jordan", "Sam"])
        let settings = try await worker.settings(for: avery.id)
        XCTAssertEqual(settings.theme, .dark)

        let reading = try await worker.shelf(.reading, for: avery.id, on: today, calendar: calendar)
        let finished = try await worker.shelf(.finished, for: avery.id, on: today, calendar: calendar)
        let eventually = try await worker.shelf(.eventually, for: avery.id, on: today, calendar: calendar)
        XCTAssertEqual(reading.total, 2)
        XCTAssertEqual(finished.total, 5)
        XCTAssertEqual(eventually.total, 2)
        XCTAssertEqual(finished.thisYear, 3)

        // Two years on the Finished shelf, newest first.
        let years = ShelfLayout.sections(finished.rows).map(\.title)
        XCTAssertEqual(years, ["2026", "2025"])

        // No covers: nothing was downloaded.
        let all = [reading, finished, eventually].flatMap { ShelfLayout.sections($0.rows).flatMap(\.entries) }
        XCTAssertTrue(all.allSatisfy { $0.coverPath == nil })
        // Summaries with Markdown, and one without anything written.
        XCTAssertTrue(all.contains { $0.excerpt.contains("Gobrin Ice") })
        XCTAssertTrue(all.contains { $0.emptyNote != nil })

        // Removing hands back what Undo needs; restoring puts it back.
        guard case let .entry(first) = reading.rows.first else { return XCTFail("no entry") }
        let removal = try await worker.remove(first.summaryId)
        XCTAssertEqual(removal.summaryId, first.summaryId)
        XCTAssertEqual(removal.title, first.title)
        let fewer = try await worker.shelf(.reading, for: avery.id, on: today, calendar: calendar)
        XCTAssertEqual(fewer.total, 1)
        try await worker.restoreEntry(removal.entry)
        let back = try await worker.shelf(.reading, for: avery.id, on: today, calendar: calendar)
        XCTAssertEqual(back, reading)
        try worker.closeNow()

        // Sam has Dune too: the same book, a separate entry.
        let sam = try await worker.profiles().first { $0.name == "Sam" }
        let samsReading = try await worker.shelf(.reading, for: try XCTUnwrap(sam).id, on: today, calendar: calendar)
        XCTAssertEqual(samsReading.total, 1)
    }
}
