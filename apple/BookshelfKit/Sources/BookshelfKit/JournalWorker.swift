import Foundation
@_exported import BookshelfFFI

/// Runs calls into the Rust core off the main thread. The core is
/// synchronous; this actor is what keeps the UI responsive.
///
/// Journal calls run one at a time, in the order they were made, so
/// autosaves land in order. Calls that wait on the network or on a slow
/// drive (searching, saving a search result, backups, export) run beside
/// that queue instead, on a background thread: a slow Open Library or an
/// unplugged USB stick never holds up an autosave. The core's own lock
/// keeps their database steps safe, and it's never held while they wait.
public actor JournalWorker {
    private let journal: Journal

    /// Opens (or creates) the journal in `directory`.
    public init(directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        journal = try Journal.openAt(dir: directory.path)
    }

    /// Opens (or creates) this Mac's journal in Application Support. A
    /// preview build has its own.
    public init(channel: Channel) throws {
        journal = try Journal.openDefault(channel: channel)
    }

    /// The folder the journal lives in.
    public nonisolated var dataDirectory: URL {
        URL(fileURLWithPath: journal.dataDir(), isDirectory: true)
    }

    /// Call when the app quits. Safe to call twice; the journal stays usable.
    public func close() throws {
        try journal.close()
    }

    /// `close` right now, from any thread, without waiting for the queue:
    /// for `applicationWillTerminate`, which can't wait. A call still queued
    /// behind it lands safely afterwards (the journal stays usable).
    public nonisolated func closeNow() throws {
        try journal.close()
    }

    /// Changes whenever the journal does: skip a refresh when it's the same.
    public func changeToken() throws -> UInt64 {
        try journal.changeToken()
    }

    // MARK: Profiles

    /// Every profile, by name.
    public func profiles() throws -> [Profile] {
        try journal.profiles()
    }

    /// Makes a profile; a bad `email` is refused before anything is made.
    public func createProfile(named name: String, email: String? = nil) throws -> Profile {
        try journal.createProfile(name: name, email: email)
    }

    /// Renames a profile. Names are unique.
    public func renameProfile(_ userId: String, to name: String) throws -> Profile {
        try journal.renameProfile(userId: userId, name: name)
    }

    /// Sets or clears (`nil` or blank) a profile's contact email.
    public func setProfileEmail(_ userId: String, to email: String?) throws -> Profile {
        try journal.setProfileEmail(userId: userId, email: email)
    }

    /// Deletes a profile with its settings and entries. No undo.
    public func deleteProfile(_ userId: String) throws {
        try journal.deleteProfile(userId: userId)
    }

    // MARK: Settings

    /// A profile's settings.
    public func settings(for userId: String) throws -> ProfileSettings {
        try journal.settings(userId: userId)
    }

    /// Saves a profile's settings (the profile is `settings.userId`).
    public func updateSettings(_ settings: ProfileSettings) throws {
        try journal.updateSettings(settings: settings)
    }

    // MARK: Shelves and entries

    /// One shelf, laid out for `date` as the calendar sees it ("day 12",
    /// "Added Sep 5").
    public func shelf(
        _ shelf: Shelf, for userId: String, on date: Date = Date(), calendar: Calendar = .current
    ) throws -> ShelfView {
        try journal.shelf(
            userId: userId, shelf: shelf,
            today: JournalDate.string(from: date, calendar: calendar),
            utcOffsetMinutes: JournalDate.utcOffsetMinutes(at: date, calendar: calendar))
    }

    /// One entry with its book.
    public func entry(_ summaryId: String) throws -> EntryDetail {
        try journal.entry(summaryId: summaryId)
    }

    /// The profile's latest entry for a book, if any.
    public func existingEntry(for userId: String, book bookId: String) throws -> String? {
        try journal.existingEntry(userId: userId, bookId: bookId)
    }

    /// Puts a saved book on a shelf, dated today. Returns the entry's id.
    public func addToShelf(_ shelf: Shelf, for userId: String, book bookId: String) throws -> String {
        try journal.addToShelf(userId: userId, bookId: bookId, shelf: shelf)
    }

    /// Sets or clears the reading dates. Finishing before starting is refused.
    public func setDates(_ summaryId: String, started: Date?, finished: Date?, calendar: Calendar = .current) throws -> EntryDetail {
        try journal.setDates(
            summaryId: summaryId,
            started: started.map { JournalDate.string(from: $0, calendar: calendar) },
            finished: finished.map { JournalDate.string(from: $0, calendar: calendar) })
    }

    /// Saves the writing page's text. Line ends are stored as `\n`.
    public func saveBody(_ summaryId: String, _ body: String) throws -> SaveResult {
        try journal.saveBody(summaryId: summaryId, body: body)
    }

    /// When saving fails: writes the text to the recovery folder instead and
    /// returns the file, to tell the user where it is.
    public func rescueBody(title: String, body: String) throws -> URL {
        URL(fileURLWithPath: try journal.rescueBody(title: title, body: body))
    }

    /// Starts another reading of the entry's book today. Returns the new id.
    public func readAgain(_ summaryId: String) throws -> String {
        try journal.readAgain(summaryId: summaryId)
    }

    /// Removes an entry; keep the result for Undo.
    public func removeEntry(_ summaryId: String) throws -> RemovedEntry {
        try journal.removeEntry(summaryId: summaryId)
    }

    /// Removes an entry, with what Undo and the announcement need.
    public func remove(_ summaryId: String) throws -> Removal {
        let removed = try journal.removeEntry(summaryId: summaryId)
        return Removal(entry: removed, summaryId: removed.summaryId(), title: removed.title())
    }

    /// Puts a removed entry back exactly as it was (Undo).
    public func restoreEntry(_ removed: RemovedEntry) throws {
        try journal.restoreEntry(removed: removed)
    }

    // MARK: Open Library (beside the queue)

    /// Searches Open Library. Nothing is saved.
    public nonisolated func searchBooks(for userId: String, query: String) async throws -> [SearchResult] {
        let journal = journal
        return try await Self.inBackground { try journal.searchBooks(userId: userId, query: query) }
    }

    /// Saves a picked result as a book, with its description and cover.
    public nonisolated func saveSearchResult(_ result: SearchResult, for userId: String) async throws -> BookInfo {
        let journal = journal
        return try await Self.inBackground { try journal.saveSearchResult(userId: userId, result: result) }
    }

    // MARK: Backups and export (beside the queue)

    /// Where backups go and the newest one's date.
    public nonisolated func backupStatus() async throws -> BackupStatus {
        let journal = journal
        return try await Self.inBackground { try journal.backupStatus() }
    }

    /// Keeps backups in `folder` from now on, or the default folder for `nil`.
    public nonisolated func setBackupFolder(_ folder: URL?) async throws {
        let journal = journal
        try await Self.inBackground { try journal.setBackupFolder(folder: folder?.path) }
    }

    /// Makes today's backup if there isn't one yet. Returns the new file.
    @discardableResult
    public nonisolated func backUpNow() async throws -> URL? {
        let journal = journal
        return try await Self.inBackground { try journal.backUpNow().map { URL(fileURLWithPath: $0) } }
    }

    /// Writes the profile's entries as Markdown into a "Bookshelf summaries"
    /// folder inside `folder`.
    public nonisolated func exportMarkdown(for userId: String, into folder: URL) async throws -> ExportResult {
        let journal = journal
        return try await Self.inBackground { try journal.exportMarkdown(userId: userId, parentDir: folder.path) }
    }

    /// Runs blocking work on a background thread rather than Swift's
    /// cooperative pool, which mustn't be kept waiting.
    private static func inBackground<T: Sendable>(
        _ work: @escaping @Sendable () throws -> T
    ) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                continuation.resume(with: Result { try work() })
            }
        }
    }
}

extension Profile: Identifiable {}

/// An entry that was just removed: the handle to put it back, and what to
/// call it ("Undo Remove “Dune”").
public struct Removal: Sendable {
    /// Pass to `restoreEntry` to undo.
    public let entry: RemovedEntry
    /// The removed entry's id.
    public let summaryId: String
    /// The book's title.
    public let title: String
}
