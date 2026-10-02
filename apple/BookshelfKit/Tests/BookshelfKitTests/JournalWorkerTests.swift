import XCTest
@testable import BookshelfKit

/// The journal as the Mac app uses it, end to end, without the network:
/// books come from a search result that already has its description and no
/// cover URL, so saving one asks Open Library for nothing.
final class JournalWorkerTests: XCTestCase {
    private var dir: URL!

    override func setUpWithError() throws {
        dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("bookshelf-tests-\(UUID().uuidString)")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: dir)
    }

    private func offlineBook(_ key: String, _ title: String) -> SearchResult {
        SearchResult(
            externalId: key, title: title, subtitle: nil, author: "Frank Herbert", isbn: nil,
            publisher: nil, description: "Sand, and the people of it.", publishedDate: "1965",
            pageCount: 412, coverUrl: nil)
    }

    private func folder(_ name: String) throws -> URL {
        let url = dir.appendingPathComponent(name, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    func testTheWholeJourneyOffline() async throws {
        // Open, and make a profile.
        let worker = try JournalWorker(directory: dir)
        let avery = try await worker.createProfile(named: "Avery", email: " avery@example.com ")
        XCTAssertEqual(avery.email, "avery@example.com")

        // Add a book to the Reading shelf.
        let book = try await worker.saveSearchResult(offlineBook("/works/OL1W", "Dune"), for: avery.id)
        XCTAssertEqual(book.description, "Sand, and the people of it.")
        XCTAssertNil(book.coverPath)
        let before = try await worker.existingEntry(for: avery.id, book: book.id)
        XCTAssertNil(before)
        let id = try await worker.addToShelf(.reading, for: avery.id, book: book.id)
        let existing = try await worker.existingEntry(for: avery.id, book: book.id)
        XCTAssertEqual(existing, id)

        // Write.
        let saved = try await worker.saveBody(id, "# Dune\r\nSpice **must** flow.")
        XCTAssertEqual(saved.words, 5)
        let entry = try await worker.entry(id)
        XCTAssertEqual(entry.body, "# Dune\nSpice **must** flow.")
        XCTAssertEqual(entry.shelf, .reading)
        XCTAssertEqual(entry.started, JournalDate.string(from: Date()))
        XCTAssertEqual(entry.book.title, "Dune")
        XCTAssertLessThan(abs(entry.createdAt.timeIntervalSinceNow), 600)

        let reading = try await worker.shelf(.reading, for: avery.id)
        XCTAssertEqual(reading.countLine, "1 book")
        guard case let .entry(item) = reading.rows.first else {
            return XCTFail("expected an entry, got \(reading.rows)")
        }
        XCTAssertEqual(item.title, "Dune")
        XCTAssertEqual(item.excerpt, "Dune Spice must flow.")
        XCTAssertTrue(item.meta.hasSuffix("day 1"), item.meta)
        XCTAssertTrue(matches(haystack: item.haystack, terms: queryTerms(query: "SPICE herbert")))

        // Finish it, with dates.
        let done = try await worker.setDates(
            id, started: JournalDate.date(from: "2026-09-01"), finished: JournalDate.date(from: "2026-09-11"))
        XCTAssertEqual(done.shelf, .finished)
        XCTAssertEqual(done.days, 10)
        XCTAssertEqual(done.finished, "2026-09-11")
        XCTAssertEqual(done.finishedDate, JournalDate.date(from: "2026-09-11"))
        do {
            _ = try await worker.setDates(
                id, started: JournalDate.date(from: "2026-09-11"), finished: JournalDate.date(from: "2026-09-01"))
            XCTFail("finishing before starting should be refused")
        } catch let error as CoreError {
            guard case .Invalid = error else { return XCTFail("unexpected \(error)") }
            XCTAssertEqual(error.userMessage, "Finished date is before started date.")
        }

        // Remove it, then Undo.
        let removed = try await worker.removeEntry(id)
        XCTAssertEqual(removed.title(), "Dune")
        let empty = try await worker.shelf(.finished, for: avery.id)
        XCTAssertEqual(empty.total, 0)
        try await worker.restoreEntry(removed)
        let back = try await worker.shelf(.finished, for: avery.id)
        XCTAssertEqual(back.total, 1)
        let restored = try await worker.entry(id)
        XCTAssertEqual(restored, done)

        // Read it again.
        let again = try await worker.readAgain(id)
        XCTAssertNotEqual(again, id)
        let second = try await worker.entry(again)
        XCTAssertEqual(second.shelf, .reading)

        // Back up into a folder of our own.
        let backups = try folder("backups")
        try await worker.setBackupFolder(backups)
        let made = try await worker.backUpNow()
        let file = try XCTUnwrap(made)
        XCTAssertTrue(FileManager.default.fileExists(atPath: file.path))
        let again2 = try await worker.backUpNow()
        XCTAssertNil(again2, "one backup a day")
        let status = try await worker.backupStatus()
        XCTAssertTrue(status.isCustom)
        XCTAssertTrue(status.available)
        XCTAssertEqual(status.latest, JournalDate.string(from: Date()))
        XCTAssertTrue(status.folder.hasSuffix("backups"))

        // Export.
        let out = try folder("export")
        let exported = try await worker.exportMarkdown(for: avery.id, into: out)
        XCTAssertEqual(exported.count, 2)
        let dune = try String(
            contentsOf: URL(fileURLWithPath: exported.folder).appendingPathComponent("dune.md"), encoding: .utf8)
        XCTAssertTrue(dune.contains("Spice **must** flow."), dune)

        try await worker.close()
        try await worker.close()
    }

    func testProfilesAndSettings() async throws {
        let worker = try JournalWorker(directory: dir)
        let none = try await worker.profiles()
        XCTAssertTrue(none.isEmpty)
        let avery = try await worker.createProfile(named: "Avery")
        let names = try await worker.profiles().map(\.name)
        XCTAssertEqual(names, ["Avery"])

        do {
            _ = try await worker.createProfile(named: "Avery")
            XCTFail("expected an error")
        } catch let error as CoreError {
            guard case .Invalid = error else { return XCTFail("unexpected \(error)") }
            XCTAssertEqual(error.userMessage, "That name is already taken.")
        }
        do {
            _ = try await worker.createProfile(named: "Sam", email: "not an email")
            XCTFail("expected an error")
        } catch let error as CoreError {
            XCTAssertEqual(error.userMessage, "That doesn't look like an email address.")
        }

        var settings = try await worker.settings(for: avery.id)
        XCTAssertEqual(settings.theme, .system)
        XCTAssertEqual(settings.writingFont, .iaDuo)
        XCTAssertEqual(settings.startShelf, .reading)
        settings.theme = .dark
        settings.dateFormat = .yearMonthDay
        settings.writingSize = 18
        try await worker.updateSettings(settings)
        let read = try await worker.settings(for: avery.id)
        XCTAssertEqual(read, settings)
        XCTAssertEqual(writerLayout(settings: read).fontPx, 24)

        let renamed = try await worker.renameProfile(avery.id, to: "Ave")
        XCTAssertEqual(renamed.name, "Ave")
        let emailed = try await worker.setProfileEmail(avery.id, to: "ave@example.com")
        XCTAssertEqual(emailed.email, "ave@example.com")
        try await worker.deleteProfile(avery.id)
        let left = try await worker.profiles()
        XCTAssertTrue(left.isEmpty)
    }

    func testDatesAndRanges() {
        let calendar = Calendar(identifier: .gregorian)
        let day = JournalDate.date(from: "2024-02-29", calendar: calendar)
        XCTAssertNotNil(day)
        XCTAssertEqual(JournalDate.string(from: day!, calendar: calendar), "2024-02-29")
        XCTAssertNil(JournalDate.date(from: "2026-02-30"))
        XCTAssertNil(JournalDate.date(from: "2026-9-1"))
        XCTAssertNil(JournalDate.date(from: "+026-09-01"))
        XCTAssertEqual(Date(unixMilliseconds: 1_500), Date(timeIntervalSince1970: 1.5))

        // "📚 word": the emoji is two UTF-16 units, as NSString counts.
        let edit = formatEdit(text: "📚 word", selection: NSRange(location: 3, length: 4), action: .bold)
        XCTAssertEqual(edit?.replacement, "**word**")
        XCTAssertEqual(edit?.range, NSRange(location: 3, length: 4))
        XCTAssertEqual(edit?.newSelection, NSRange(location: 5, length: 4))
    }

    func testHighlightRangesCountUTF16() {
        let ranges = highlightRanges(in: "📚 **b**")
        XCTAssertEqual(ranges.first?.kind, .bold)
        XCTAssertEqual(ranges.first?.range, NSRange(location: 5, length: 1))
    }

    func testReadingPageAndHelp() {
        let blocks = renderMarkdown(text: "# Hi\n\n**bold**")
        XCTAssertEqual(blocks.first, .heading(level: 1, runs: [Run(text: "Hi", styles: [])]))
        XCTAssertEqual(blocks.last, .paragraph(runs: [Run(text: "bold", styles: [.bold])]))
        XCTAssertTrue(manual().intro.hasPrefix("# Bookshelf"))
        let mac = shortcuts(platform: .mac).flatMap(\.items)
        let bold = mac.first { $0.title == "Bold" }
        XCTAssertEqual(
            bold?.accel, .keys(combo: KeyCombo(key: .character(text: "b"), command: true, control: false, shift: false)))
        XCTAssertEqual(accents().first?.hex, "#2d71e5")
        XCTAssertEqual(accentColors(hex: "#2d71e5", dark: false).bg, "#2d71e5")
        XCTAssertEqual(autosaveMs(), 700)
        XCTAssertEqual(wordCount(text: "two words"), 2)
        XCTAssertTrue(prompts(shelf: .finished).hasPrefix("What stayed with you"))
    }

    func testCoreLogIsAcceptedAsTheLogger() {
        CoreLog.start(subsystem: "io.github.e36lewis.Bookshelf.Tests")
    }
}
