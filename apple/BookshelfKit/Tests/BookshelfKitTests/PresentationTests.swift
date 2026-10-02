import XCTest
@testable import BookshelfKit

/// The Mac app's words, groupings and lookups, without any UI.
final class PresentationTests: XCTestCase {
    private func entry(_ id: String, _ haystack: String) -> ShelfEntry {
        ShelfEntry(
            summaryId: id, bookId: "b\(id)", title: id, author: nil, coverPath: nil, meta: "", excerpt: "",
            emptyNote: nil, haystack: haystack)
    }

    func testShelvesBecomeSections() {
        let finished: [ShelfRow] = [
            .yearHeading(year: 2026, count: 2, label: "2 books"),
            .entry(item: entry("a", "dune herbert")),
            .entry(item: entry("b", "piranesi clarke")),
            .yearHeading(year: 2025, count: 1, label: "1 book"),
            .entry(item: entry("c", "emma austen")),
        ]
        let sections = ShelfLayout.sections(finished)
        XCTAssertEqual(sections.map(\.title), ["2026", "2025"])
        XCTAssertEqual(sections.map(\.detail), ["2 books", "1 book"])
        XCTAssertEqual(sections.map { $0.entries.map(\.id) }, [["a", "b"], ["c"]])
        XCTAssertEqual(ShelfLayout.count(sections), 3)

        let reading = ShelfLayout.sections([.entry(item: entry("x", "")), .entry(item: entry("y", ""))])
        XCTAssertEqual(reading.count, 1)
        XCTAssertNil(reading[0].title)
        XCTAssertEqual(reading[0].entries.map(\.id), ["x", "y"])
        XCTAssertTrue(ShelfLayout.sections([]).isEmpty)
        // A heading with nothing under it isn't shown.
        XCTAssertTrue(ShelfLayout.sections([.yearHeading(year: 2026, count: 0, label: "0 books")]).isEmpty)
    }

    func testSearchLooksAcrossEveryShelf() {
        func view(_ rows: [ShelfRow]) -> ShelfView {
            ShelfView(rows: rows, total: UInt32(rows.count), thisYear: 0, countLine: "")
        }
        let shelves: [Shelf: ShelfView] = [
            .reading: view([.entry(item: entry("r", "the left hand of darkness le guin"))]),
            .finished: view([
                .yearHeading(year: 2026, count: 2, label: "2 books"),
                .entry(item: entry("f1", "a wizard of earthsea le guin")),
                .entry(item: entry("f2", "dune herbert")),
            ]),
            .eventually: view([.entry(item: entry("e", "moby-dick melville"))]),
        ]
        let found = ShelfLayout.search(shelves, terms: queryTerms(query: "Le GUIN"))
        XCTAssertEqual(found.map(\.title), ["Reading", "Finished"])
        XCTAssertEqual(found.map { $0.entries.map(\.id) }, [["r"], ["f1"]])
        XCTAssertTrue(ShelfLayout.search(shelves, terms: queryTerms(query: "nothing like it")).isEmpty)
    }

    func testDatesAsTheProfileLikesThem() {
        XCTAssertEqual(JournalDate.display("2026-09-05", format: .long), "Sep 5, 2026")
        XCTAssertEqual(JournalDate.display("2026-09-05", format: .monthDayYear), "09/05/2026")
        XCTAssertEqual(JournalDate.display("2026-09-05", format: .dayMonthYear), "05/09/2026")
        XCTAssertEqual(JournalDate.display("2026-12-25", format: .yearMonthDay), "2026-12-25")
        XCTAssertEqual(JournalDate.display("nonsense", format: .long), "nonsense")
        XCTAssertEqual(JournalDate.display("2026-13-01", format: .long), "2026-13-01")
        let calendar = Calendar(identifier: .gregorian)
        let day = JournalDate.date(from: "2026-01-31", calendar: calendar)
        XCTAssertEqual(day.map { JournalDate.display($0, format: .long, calendar: calendar) }, "Jan 31, 2026")
    }

    func testWording() {
        XCTAssertNil(Wording.finishedIn(days: nil))
        XCTAssertEqual(Wording.finishedIn(days: 0), "Finished the same day")
        XCTAssertEqual(Wording.finishedIn(days: 1), "Finished in 1 day")
        XCTAssertEqual(Wording.finishedIn(days: 12), "Finished in 12 days")
        XCTAssertEqual(Wording.words(1), "1 word")
        XCTAssertEqual(Wording.words(0), "0 words")
        XCTAssertEqual(Wording.summaries(1), "1 summary")
        XCTAssertEqual(Wording.summaries(3), "3 summaries")
        XCTAssertEqual(Wording.byline(author: "Frank Herbert", year: "1965"), "Frank Herbert · 1965")
        XCTAssertEqual(Wording.byline(author: nil, year: "1965"), "1965")
        XCTAssertEqual(Wording.byline(author: "", year: nil), "")

        let book = BookInfo(
            id: "b", title: "Dune", subtitle: nil, author: "Frank Herbert", isbn: nil, publisher: "Ace",
            description: nil, publishedDate: "1965", pageCount: 412, coverPath: nil)
        XCTAssertEqual(Wording.bookFacts(book), "Ace · 1965 · 412 pages")
        var bare = book
        bare.publisher = nil
        bare.pageCount = 0
        XCTAssertEqual(Wording.bookFacts(bare), "1965")

        let status = BackupStatus(folder: "/b", isCustom: false, available: true, latest: "2026-09-05", keep: 7)
        XCTAssertEqual(
            Wording.backup(status, format: .long),
            "One copy a day of every profile; the last 7 are kept. Latest: Sep 5, 2026.")
        var first = status
        first.latest = nil
        XCTAssertTrue(Wording.backup(first, format: .long).hasSuffix("the next time Bookshelf starts."))
        var away = status
        away.available = false
        XCTAssertTrue(Wording.backup(away, format: .long).hasPrefix("That folder isn't available"))
    }

    func testColors() {
        let blue = RGB(hex: "#2d71e5")
        XCTAssertEqual(blue?.hex, "#2d71e5")
        XCTAssertEqual(RGB(hex: "FFFFFF"), RGB(red: 1, green: 1, blue: 1))
        XCTAssertEqual(RGB(red: 2, green: -1, blue: 0.5).hex, "#ff0080")
        for bad in ["", "#fff", "#12345g", "#1234567", "blue"] {
            XCTAssertNil(RGB(hex: bad), bad)
        }
    }

    func testEveryMenuShortcutIsInTheCoresTable() {
        let table = MenuShortcuts.table(shortcuts(platform: .mac))
        for title in MenuShortcuts.all {
            XCTAssertNotNil(table[title], title)
        }
        XCTAssertEqual(
            table[MenuShortcuts.write], KeyCombo(key: .return, command: true, control: false, shift: false))
        XCTAssertEqual(
            table[MenuShortcuts.title(for: .finished)],
            KeyCombo(key: .character(text: "2"), command: true, control: false, shift: false))
        XCTAssertEqual(
            table[MenuShortcuts.fullScreen],
            KeyCombo(key: .character(text: "f"), command: true, control: true, shift: false))
    }

    func testTheWorkersPureHelpers() async throws {
        let worker = JournalWorker.self
        let blocks = await worker.blocks(for: "# Hi")
        XCTAssertEqual(blocks, [.heading(level: 1, runs: [Run(text: "Hi", styles: [])])])
        let manual = await worker.userManual()
        XCTAssertFalse(manual.sections.isEmpty)
        let keys = await worker.macShortcuts()
        XCTAssertNotNil(keys[MenuShortcuts.addBook])
        let choices = await worker.accentChoices()
        XCTAssertEqual(choices.count, 8)
        let palettes = await worker.palettes(for: "#2d71e5")
        XCTAssertNotEqual(palettes.light.text, palettes.dark.text)
        let delay = await worker.autosaveDelay()
        XCTAssertEqual(delay, .milliseconds(700))
        let words = await worker.words(in: "three small words")
        XCTAssertEqual(words, 3)
        let email = try await worker.checkedEmail("  me@example.com ")
        XCTAssertEqual(email, "me@example.com")
        let blank = try await worker.checkedEmail("  ")
        XCTAssertNil(blank)
        let none = await worker.search([:], for: "   ")
        XCTAssertNil(none)
        let shown = await worker.displayed("/Users/ann/b", home: "/Users/ann")
        XCTAssertEqual(shown, "~/b")
        let prompts = await worker.writingPrompts(for: .eventually)
        XCTAssertTrue(prompts.hasPrefix("Why do you want"))
    }
}
