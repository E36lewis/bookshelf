import Foundation
import BookshelfFFI

// The words and groupings the Mac app shows, kept out of the views so they
// can be tested. The core lays out the shelves; this is what's left.

extension Shelf {
    /// The shelves in the order the sidebar lists them.
    public static let inOrder: [Shelf] = [.reading, .finished, .eventually]

    /// The shelf's name in the sidebar and menus.
    public var title: String {
        switch self {
        case .reading: "Reading"
        case .finished: "Finished"
        case .eventually: "Eventually"
        }
    }

    /// The name of the shelf a new book goes on ("Someday").
    public var addTitle: String {
        switch self {
        case .reading: "Reading Now"
        case .finished: "Finished"
        case .eventually: "Someday"
        }
    }

    /// What an empty shelf says.
    public var emptyMessage: String {
        switch self {
        case .reading: "Books you've started will live here."
        case .finished: "Every book you finish gets its own page here."
        case .eventually: "Books you want to read someday wait here."
        }
    }

    /// The SF Symbol for the shelf.
    public var systemImage: String {
        switch self {
        case .reading: "book"
        case .finished: "checkmark.circle"
        case .eventually: "bookmark"
        }
    }
}

extension ShelfEntry: Identifiable {
    /// The entry's id.
    public var id: String { summaryId }
}

extension ManualEntry: Identifiable {
    /// The section it leads to.
    public var id: String { anchor }
}

extension ManualSection: Identifiable {
    /// The section's anchor.
    public var id: String { anchor }
}

extension Accent: Identifiable {
    /// The color itself.
    public var id: String { hex }
}

/// A run of entries under one heading: a year on the Finished shelf, a
/// shelf in search results, or a whole shelf with no heading.
public struct ShelfSection: Identifiable, Equatable, Sendable {
    /// Stable across refreshes.
    public var id: String
    /// "2026", "Reading"; `nil` for a shelf without headings.
    public var title: String?
    /// "12 books", beside the title.
    public var detail: String?
    /// The entries, in order.
    public var entries: [ShelfEntry]

    public init(id: String, title: String?, detail: String?, entries: [ShelfEntry]) {
        self.id = id
        self.title = title
        self.detail = detail
        self.entries = entries
    }
}

public enum ShelfLayout {
    /// A shelf's rows as sections: one per year heading, and one without a
    /// heading for any entries before the first (all of them, on the
    /// shelves that have no headings).
    public static func sections(_ rows: [ShelfRow]) -> [ShelfSection] {
        var sections: [ShelfSection] = []
        for row in rows {
            switch row {
            case let .yearHeading(year, _, label):
                sections.append(ShelfSection(id: "year-\(year)", title: String(year), detail: label, entries: []))
            case let .entry(item):
                if sections.isEmpty {
                    sections.append(ShelfSection(id: "all", title: nil, detail: nil, entries: []))
                }
                sections[sections.count - 1].entries.append(item)
            }
        }
        return sections.filter { !$0.entries.isEmpty }
    }

    /// The entries of every shelf that match `terms` (from `queryTerms`),
    /// one section per shelf that has any, in sidebar order.
    public static func search(_ shelves: [Shelf: ShelfView], terms: [String]) -> [ShelfSection] {
        Shelf.inOrder.compactMap { shelf in
            guard let view = shelves[shelf] else { return nil }
            let found = view.rows.compactMap { row -> ShelfEntry? in
                guard case let .entry(item) = row, matches(haystack: item.haystack, terms: terms) else { return nil }
                return item
            }
            guard !found.isEmpty else { return nil }
            return ShelfSection(id: "search-\(shelf.title)", title: shelf.title, detail: nil, entries: found)
        }
    }

    /// How many entries `sections` hold.
    public static func count(_ sections: [ShelfSection]) -> Int {
        sections.reduce(0) { $0 + $1.entries.count }
    }
}

extension JournalDate {
    private static let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

    /// A `YYYY-MM-DD` date written the way the profile likes, the same way
    /// the core writes it in meta lines ("Sep 5, 2026"). Anything that
    /// isn't a date comes back as it was.
    public static func display(_ ymd: String, format: DateFormat) -> String {
        let parts = ymd.split(separator: "-").compactMap { Int($0) }
        guard parts.count == 3, (1...12).contains(parts[1]) else { return ymd }
        let (year, month, day) = (parts[0], parts[1], parts[2])
        let yyyy = zeroPadded(year, 4), mm = zeroPadded(month, 2), dd = zeroPadded(day, 2)
        switch format {
        case .long: return "\(months[month - 1]) \(day), \(yyyy)"
        case .monthDayYear: return "\(mm)/\(dd)/\(yyyy)"
        case .dayMonthYear: return "\(dd)/\(mm)/\(yyyy)"
        case .yearMonthDay: return "\(yyyy)-\(mm)-\(dd)"
        }
    }

    private static func zeroPadded(_ n: Int, _ width: Int) -> String {
        let digits = String(n)
        return String(repeating: "0", count: max(0, width - digits.count)) + digits
    }

    /// The day `date` falls on, written the way the profile likes.
    public static func display(_ date: Date, format: DateFormat, calendar: Calendar = .current) -> String {
        display(string(from: date, calendar: calendar), format: format)
    }
}

public enum Wording {
    /// "Finished in 10 days", once a book has both dates.
    public static func finishedIn(days: Int64?) -> String? {
        switch days {
        case nil: nil
        case 0: "Finished the same day"
        case 1: "Finished in 1 day"
        case let n?: "Finished in \(n) days"
        }
    }

    /// "1 word", "250 words".
    public static func words(_ n: UInt32) -> String {
        n == 1 ? "1 word" : "\(n) words"
    }

    /// "1 summary", "12 summaries".
    public static func summaries(_ n: UInt32) -> String {
        n == 1 ? "1 summary" : "\(n) summaries"
    }

    /// "Frank Herbert · 1965": a search result's second line.
    public static func byline(author: String?, year: String?) -> String {
        [author, year].compactMap { $0?.isEmpty == false ? $0 : nil }.joined(separator: " · ")
    }

    /// "Ace · 1965 · 412 pages": the facts under a book's title.
    public static func bookFacts(_ book: BookInfo) -> String {
        var facts: [String] = []
        if let publisher = book.publisher, !publisher.isEmpty { facts.append(publisher) }
        if let year = book.publishedDate, !year.isEmpty { facts.append(year) }
        if let pages = book.pageCount, pages > 0 { facts.append(pages == 1 ? "1 page" : "\(pages) pages") }
        return facts.joined(separator: " · ")
    }

    /// The line under the backup folder in Settings.
    public static func backup(_ status: BackupStatus, format: DateFormat) -> String {
        guard status.available else {
            return "That folder isn't available right now (is the drive plugged in?). Backups are paused "
                + "until it's back, or until you choose another folder."
        }
        let when = status.latest.map { "Latest: \(JournalDate.display($0, format: format))." }
            ?? "The first one is made the next time Bookshelf starts."
        return "One copy a day of every profile; the last \(status.keep) are kept. \(when)"
    }

    /// Why a profile can have an email: shown under the field.
    public static let emailWhy = "Optional. Book details and covers come from Open Library, a free public library "
        + "service run by the Internet Archive. If you add an email, it's sent along with your book searches and "
        + "cover downloads so they can contact you if there's ever a problem. It goes nowhere else. Leave it "
        + "blank to send nothing."
}

/// An sRGB color as `#rrggbb`, as the core stores accents.
public struct RGB: Equatable, Sendable {
    /// 0 to 1.
    public var red, green, blue: Double

    public init(red: Double, green: Double, blue: Double) {
        self.red = red
        self.green = green
        self.blue = blue
    }

    /// `#rrggbb` (the `#` is optional); `nil` for anything else.
    public init?(hex: String) {
        var digits = Substring(hex.trimmingCharacters(in: .whitespaces))
        if digits.hasPrefix("#") { digits = digits.dropFirst() }
        guard digits.count == 6, digits.allSatisfy(\.isHexDigit), let value = UInt32(digits, radix: 16) else {
            return nil
        }
        red = Double((value >> 16) & 0xff) / 255
        green = Double((value >> 8) & 0xff) / 255
        blue = Double(value & 0xff) / 255
    }

    /// `#rrggbb`, lowercase.
    public var hex: String {
        func byte(_ c: Double) -> String {
            let hex = String(Int((min(max(c, 0), 1) * 255).rounded()), radix: 16)
            return hex.count == 1 ? "0" + hex : hex
        }
        return "#" + byte(red) + byte(green) + byte(blue)
    }
}

/// The menu commands that take their keys from the core's shortcut table,
/// by the title the table gives them.
public enum MenuShortcuts {
    public static let reading = "Reading"
    public static let finished = "Finished"
    public static let eventually = "Eventually"
    public static let addBook = "Add a book"
    public static let search = "Search your shelves"
    public static let settings = "Settings"
    public static let write = "Write or edit your summary"
    public static let read = "Read your summary"
    public static let fullScreen = "Full screen"
    public static let manual = "User manual"
    public static let save = "Save now (it also saves as you type)"
    public static let bold = "Bold"
    public static let italic = "Italic"
    public static let link = "Link"
    public static let focusMode = "Focus mode"

    /// Every title the Mac app looks up.
    public static let all = [
        reading, finished, eventually, addBook, search, settings, write, read, fullScreen, manual,
        save, bold, italic, link, focusMode,
    ]

    /// The keys for each title in `groups`.
    public static func table(_ groups: [ShortcutGroup]) -> [String: KeyCombo] {
        var table: [String: KeyCombo] = [:]
        for item in groups.flatMap(\.items) {
            if case let .keys(combo) = item.accel { table[item.title] = combo }
        }
        return table
    }

    /// The menu key for a shelf.
    public static func title(for shelf: Shelf) -> String {
        switch shelf {
        case .reading: reading
        case .finished: finished
        case .eventually: eventually
        }
    }
}
