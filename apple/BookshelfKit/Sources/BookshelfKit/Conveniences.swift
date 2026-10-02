import Foundation
import BookshelfFFI

/// Calendar dates as the core writes them: `YYYY-MM-DD`, a day on the
/// user's calendar with no time or time zone.
public enum JournalDate {
    /// The day `date` falls on in `calendar`, as `YYYY-MM-DD`.
    public static func string(from date: Date, calendar: Calendar = .current) -> String {
        let c = calendar.dateComponents([.year, .month, .day], from: date)
        return "\(padded(c.year ?? 0, 4))-\(padded(c.month ?? 0, 2))-\(padded(c.day ?? 0, 2))"
    }

    private static func padded(_ n: Int, _ width: Int) -> String {
        let digits = String(n)
        return String(repeating: "0", count: max(0, width - digits.count)) + digits
    }

    /// The start of the day `string` names in `calendar`, or `nil` if it
    /// isn't a `YYYY-MM-DD` date.
    public static func date(from string: String, calendar: Calendar = .current) -> Date? {
        let parts = string.split(separator: "-", omittingEmptySubsequences: false)
        guard string.count == 10, parts.count == 3, parts[0].count == 4,
              string.allSatisfy({ $0 == "-" || ("0"..."9").contains($0) }),
              let year = Int(parts[0]), let month = Int(parts[1]), let day = Int(parts[2])
        else { return nil }
        // Refuses days that don't exist (Feb 30), which `date(from:)` would
        // roll over into the next month.
        guard let date = calendar.date(from: DateComponents(year: year, month: month, day: day))
        else { return nil }
        let back = calendar.dateComponents([.year, .month, .day], from: date)
        guard back.year == year, back.month == month, back.day == day else { return nil }
        return date
    }

    /// The calendar's offset from UTC at `date`, in minutes (UTC+2 is 120).
    public static func utcOffsetMinutes(at date: Date = Date(), calendar: Calendar = .current) -> Int32 {
        Int32(calendar.timeZone.secondsFromGMT(for: date) / 60)
    }
}

extension Date {
    /// A timestamp from the core (Unix milliseconds).
    public init(unixMilliseconds ms: Int64) {
        self.init(timeIntervalSince1970: TimeInterval(ms) / 1000)
    }
}

extension EntryDetail {
    /// When reading started, as a date on the current calendar.
    public var startedDate: Date? { started.flatMap { JournalDate.date(from: $0) } }
    /// When it was finished, as a date on the current calendar.
    public var finishedDate: Date? { finished.flatMap { JournalDate.date(from: $0) } }
    /// When the entry was made.
    public var createdAt: Date { Date(unixMilliseconds: createdAtMs) }
    /// When it last changed.
    public var updatedAt: Date { Date(unixMilliseconds: updatedAtMs) }
}

extension TextEdit {
    /// The text to replace, as NSTextView counts (UTF-16).
    public var range: NSRange { NSRange(location: Int(start), length: max(0, Int(end) - Int(start))) }
    /// The selection to set after replacing it.
    public var newSelection: NSRange {
        NSRange(location: Int(newSelStart), length: max(0, Int(newSelEnd) - Int(newSelStart)))
    }
}

/// What pressing a formatting button does with `selection` (as NSTextView
/// reports it) selected. `nil` means leave everything alone.
public func formatEdit(text: String, selection: NSRange, action: FormatAction) -> TextEdit? {
    let start = UInt32(clamping: max(selection.location, 0))
    let end = UInt32(clamping: max(selection.location + selection.length, 0))
    return BookshelfFFI.formatEdit(text: text, selStart: start, selEnd: end, action: action)
}

extension CoreError {
    /// The error as a sentence to show people.
    public var userMessage: String {
        switch self {
        case let .Database(message), let .Network(message), let .Io(message),
             let .NotFound(message), let .Invalid(message):
            return message
        case let .NewerJournal(_, _, message):
            return message
        }
    }
}
