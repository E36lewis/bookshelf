import AppKit
import SwiftUI
import os
import BookshelfKit

/// Everything the windows show, on the main actor. Every call into the
/// journal goes through `JournalWorker`, off the main thread; this model
/// awaits it and keeps what comes back.
@MainActor
@Observable
final class AppModel {
    enum Phase: Equatable {
        /// Opening the journal.
        case opening
        /// The journal couldn't be opened; the message says why.
        case failed(String)
        /// No profiles yet: the welcome page.
        case welcome
        /// Shelves showing.
        case ready
    }

    /// What the main window shows over the shelves.
    enum Page: Equatable {
        case shelves
        case reader
        case writer
    }

    /// An error to show in an alert.
    struct Problem: Identifiable {
        let id = UUID()
        var title: String
        var message: String
    }

    /// A book just picked in Add a Book, saved and waiting for a shelf.
    struct PickedBook: Identifiable {
        var id: String { book.id }
        var book: BookInfo
        /// This profile's entry for it, if it has one already.
        var existingEntryID: String?
    }

    // MARK: State

    private(set) var phase: Phase = .opening
    private(set) var profiles: [Profile] = []
    private(set) var profile: Profile?
    private(set) var settings: ProfileSettings?
    private(set) var palettes: AccentPalettes?
    private(set) var accentChoices: [Accent] = []
    private(set) var layout: PageLayout?
    /// The core's Mac shortcuts, by title (`MenuShortcuts`).
    private(set) var keys: [String: KeyCombo] = [:]

    /// The shelf in the sidebar.
    private(set) var shelf: Shelf = .reading
    private(set) var shelves: [Shelf: ShelfView] = [:]
    private(set) var shelfSections: [Shelf: [ShelfSection]] = [:]
    /// The shelf search, as typed.
    var searchText = ""
    /// Matches across every shelf; `nil` when not searching.
    private(set) var searchResults: [ShelfSection]?

    private(set) var selectedEntryID: String?
    private(set) var entry: EntryDetail?
    private(set) var summaryBlocks: [Block] = []
    private(set) var summaryPrompts = ""
    private(set) var page: Page = .shelves
    private(set) var writer: WriterSession?
    /// The entry just removed, until something else is picked: the book
    /// page shows it with an Undo button.
    private(set) var lastRemoval: Removal?

    var problem: Problem?
    var isAddingBook = false
    var isCreatingProfile = false

    let options = LaunchOptions.current
    @ObservationIgnored weak var undoManager: UndoManager?
    @ObservationIgnored private(set) var worker: JournalWorker?
    @ObservationIgnored private var backups: Task<Void, Never>?
    @ObservationIgnored private var settingsWrites = SerialTasks()
    @ObservationIgnored private var entryWrites = SerialTasks()
    /// The journal's change token and the day the shelves were laid out for.
    @ObservationIgnored private var shelvesStamp: (token: UInt64, day: String)?
    let backupFolder = BackupFolderAccess()

    private static let lastProfileKey = "lastProfileID"
    private let log = os.Logger(subsystem: "io.github.e36lewis.Bookshelf", category: "app")

    // MARK: Opening

    /// Opens the journal and the last profile used. Called once, as the
    /// main window appears.
    func start() async {
        guard phase == .opening, worker == nil else { return }
        CoreLog.start(subsystem: Bundle.main.bundleIdentifier ?? "io.github.e36lewis.Bookshelf.Preview")
        let options = self.options
        do {
            let worker = try await Task.detached(priority: .userInitiated) { try options.openJournal() }.value
            self.worker = worker
            keys = await JournalWorker.macShortcuts()
            accentChoices = await JournalWorker.accentChoices()
            if options.journal == .demo {
                let avery = try await DemoJournal.seed(worker, theme: options.appearance ?? .system)
                UserDefaults.standard.set(avery.id, forKey: Self.lastProfileKey)
            }
            try await loadProfiles()
            startBackups()
        } catch {
            phase = .failed(Self.message(for: error))
        }
    }

    /// Reads the profiles and opens `preferred`, else the last one used,
    /// else the first; with none, the welcome page.
    private func loadProfiles(preferring preferred: String? = nil) async throws {
        guard let worker else { return }
        profiles = try await worker.profiles()
        let wanted = preferred ?? UserDefaults.standard.string(forKey: Self.lastProfileKey)
        if let next = profiles.first(where: { $0.id == wanted }) ?? profiles.first {
            await openProfile(next)
        } else {
            profile = nil
            settings = nil
            shelves = [:]
            shelfSections = [:]
            clearSelection()
            phase = .welcome
        }
    }

    /// Switches to `profile`'s shelves and settings.
    func openProfile(_ profile: Profile) async {
        guard let worker else { return }
        do {
            let settings = try await worker.settings(for: profile.id)
            self.profile = profile
            UserDefaults.standard.set(profile.id, forKey: Self.lastProfileKey)
            await apply(settings)
            shelf = settings.startShelf
            searchText = ""
            searchResults = nil
            clearSelection()
            page = .shelves
            // Undoing a removal from someone else's shelves would be a surprise.
            undoManager?.removeAllActions()
            shelvesStamp = nil
            try await reloadShelves()
            phase = .ready
        } catch {
            show(error, title: "Couldn't open \(profile.name)’s shelves")
        }
    }

    /// Switches profile from the sidebar's menu, saving any writing first.
    func switchTo(_ profile: Profile) {
        guard profile.id != self.profile?.id else { return }
        Task {
            await leavePage()
            await openProfile(profile)
        }
    }

    /// Makes a profile and opens it. Throws what to say under the form.
    func createProfile(name: String, email: String) async throws {
        guard let worker else { return }
        let made = try await worker.createProfile(named: name, email: email)
        isCreatingProfile = false
        await leavePage()
        profiles = try await worker.profiles()
        await openProfile(made)
    }

    private static func message(for error: Error) -> String {
        (error as? CoreError)?.userMessage ?? "Something went wrong."
    }

    func show(_ error: Error, title: String) {
        log.error("\(title, privacy: .public): \(Self.message(for: error), privacy: .public)")
        problem = Problem(title: title, message: Self.message(for: error))
    }

    // MARK: Settings

    /// The calendar the date pickers use: the profile's first weekday.
    var calendar: Calendar {
        var calendar = Calendar.current
        calendar.firstWeekday = settings?.weekStart == .monday ? 2 : 1
        return calendar
    }

    /// The accent for a light or dark window.
    func accent(for scheme: ColorScheme) -> Color {
        guard let palettes else { return .accentColor }
        return Color(hex: scheme == .dark ? palettes.dark.bg : palettes.light.bg) ?? .accentColor
    }

    /// The accent as text on the window background, for a light or dark window.
    func accentText(for scheme: ColorScheme) -> Color {
        guard let palettes else { return .accentColor }
        return Color(hex: scheme == .dark ? palettes.dark.text : palettes.light.text) ?? .accentColor
    }

    var headingFont: HeadingFont { settings?.headingFont ?? .serif }

    private func apply(_ settings: ProfileSettings) async {
        let accentChanged = settings.accent != self.settings?.accent || palettes == nil
        self.settings = settings
        if accentChanged {
            palettes = await JournalWorker.palettes(for: settings.accent)
        }
        layout = await JournalWorker.pageLayout(for: settings)
        Appearance.apply(settings.theme, override: options.appearance)
    }

    /// Changes the current profile's settings, applies them at once and
    /// saves them in order.
    func updateSettings(_ change: (inout ProfileSettings) -> Void) {
        guard let worker, let current = settings else { return }
        var changed = current
        change(&changed)
        guard changed != current else { return }
        let datesChanged = changed.dateFormat != current.dateFormat
        let new = changed
        Task { await apply(new) }
        settingsWrites.enqueue { [weak self] in
            do {
                try await worker.updateSettings(new)
                if datesChanged { try await self?.reloadShelves() }
            } catch {
                self?.show(error, title: "Couldn't save that setting")
                if let self, self.settings == new { await self.apply(current) }
            }
        }
    }

    /// Renames the current profile.
    func rename(to name: String) async throws {
        guard let worker, let profile else { return }
        let renamed = try await worker.renameProfile(profile.id, to: name)
        self.profile = renamed
        profiles = try await worker.profiles()
    }

    /// Sets or clears the current profile's email. Throws what to say.
    func setEmail(_ email: String) async throws {
        guard let worker, let profile else { return }
        let checked = try await JournalWorker.checkedEmail(email)
        let updated = try await worker.setProfileEmail(profile.id, to: checked)
        self.profile = updated
        profiles = try await worker.profiles()
    }

    /// Deletes the current profile, for good, and opens another.
    func deleteProfile() async {
        guard let worker, let profile else { return }
        do {
            await leavePage()
            try await worker.deleteProfile(profile.id)
            UserDefaults.standard.removeObject(forKey: Self.lastProfileKey)
            try await loadProfiles()
            announce("Deleted \(profile.name)")
        } catch {
            show(error, title: "Couldn't delete \(profile.name)")
        }
    }

    // MARK: Shelves and search

    /// Reads all three shelves again (for the sidebar's counts too).
    func reloadShelves() async throws {
        guard let worker, let profile else { return }
        let today = Date()
        var views: [Shelf: ShelfView] = [:]
        for shelf in Shelf.inOrder {
            views[shelf] = try await worker.shelf(shelf, for: profile.id, on: today)
        }
        guard profile.id == self.profile?.id else { return }
        shelves = views
        shelfSections = views.mapValues { ShelfLayout.sections($0.rows) }
        let token = try await worker.changeToken()
        shelvesStamp = (token, JournalDate.string(from: today))
        await runSearch()
    }

    /// Reloads the shelves if the journal or the day changed since.
    func refreshIfChanged() async {
        guard let worker, phase == .ready, let stamp = shelvesStamp else { return }
        do {
            let token = try await worker.changeToken()
            guard token != stamp.token || JournalDate.string(from: Date()) != stamp.day else { return }
            try await reloadShelves()
        } catch {
            log.error("refresh failed: \(Self.message(for: error), privacy: .public)")
        }
    }

    /// Shows a shelf in the list.
    func show(_ shelf: Shelf) {
        if page != .shelves {
            Task { await leavePage() }
        }
        searchText = ""
        searchResults = nil
        guard shelf != self.shelf else { return }
        self.shelf = shelf
    }

    /// Runs the shelf search for what's typed now. A newer search makes an
    /// older one's results go away unseen.
    func runSearch() async {
        let query = searchText
        let found = await JournalWorker.search(shelves, for: query)
        guard query == searchText else { return }
        searchResults = found
    }

    /// What the list shows: search results, or the shelf's sections.
    var visibleSections: [ShelfSection] {
        searchResults ?? shelfSections[shelf] ?? []
    }

    var isSearching: Bool { searchResults != nil }

    /// Whether the list shows this entry: a search that leaves the open
    /// book out hides its page too.
    func isListed(_ id: String) -> Bool {
        guard let searchResults else { return true }
        return searchResults.contains { $0.entries.contains { $0.summaryId == id } }
    }

    /// Focuses the toolbar's search field (⌘F).
    func focusSearch() {
        Task {
            if page != .shelves {
                await leavePage()
                // Let the shelves, and their search field, come back first.
                try? await Task.sleep(for: .milliseconds(100))
            }
            SearchField.focus()
        }
    }

    // MARK: The selected entry

    /// Picks an entry in the list; `nil` picks nothing.
    func select(_ id: String?) {
        guard id != selectedEntryID else { return }
        selectedEntryID = id
        if id != nil { lastRemoval = nil }
        guard let id else { return }
        Task { await loadEntry(id) }
    }

    private func clearSelection() {
        selectedEntryID = nil
        entry = nil
        summaryBlocks = []
        lastRemoval = nil
    }

    /// Reads the entry and lays out its summary.
    func loadEntry(_ id: String) async {
        guard let worker else { return }
        do {
            let loaded = try await worker.entry(id)
            let blocks = await JournalWorker.blocks(for: loaded.body)
            let prompts = await JournalWorker.writingPrompts(for: loaded.shelf)
            guard selectedEntryID == id else { return }
            entry = loaded
            summaryBlocks = blocks
            summaryPrompts = prompts
        } catch {
            if selectedEntryID == id { show(error, title: "Couldn't open this book") }
        }
    }

    /// The entry, if the book page is showing it.
    var selectedEntry: EntryDetail? {
        guard let entry, entry.summaryId == selectedEntryID else { return nil }
        return entry
    }

    /// Sets the reading dates; the book may move shelf, and the list
    /// follows it there.
    func setDates(started: Date?, finished: Date?) {
        guard let worker, let entry = selectedEntry else { return }
        let id = entry.summaryId
        let calendar = self.calendar
        entryWrites.enqueue { [weak self] in
            guard let self else { return }
            do {
                let saved = try await worker.setDates(id, started: started, finished: finished, calendar: calendar)
                if self.selectedEntryID == id {
                    self.entry = saved
                    self.summaryPrompts = await JournalWorker.writingPrompts(for: saved.shelf)
                }
                try await self.reloadShelves()
                if self.selectedEntryID == id, !self.isSearching, saved.shelf != self.shelf {
                    self.shelf = saved.shelf
                    self.announce("Moved to \(saved.shelf.title)")
                }
            } catch {
                self.show(error, title: "Couldn't save the dates")
                await self.loadEntry(id)
            }
        }
    }

    /// Starts another reading of the selected book, today.
    func readAgain() {
        guard let entry = selectedEntry else { return }
        Task { await readAgain(entry.summaryId) }
    }

    func readAgain(_ id: String) async {
        guard let worker else { return }
        do {
            let fresh = try await worker.readAgain(id)
            isAddingBook = false
            try await reloadShelves()
            searchText = ""
            searchResults = nil
            shelf = .reading
            select(fresh)
            announce("New reading started today")
        } catch {
            show(error, title: "Couldn't start a new reading")
        }
    }

    // MARK: Remove and Undo

    /// Removes the selected entry. Edit › Undo puts it back.
    func removeSelected() {
        guard let entry = selectedEntry else { return }
        Task { await remove(entry.summaryId) }
    }

    func remove(_ id: String) async {
        guard let worker else { return }
        do {
            let removal = try await worker.remove(id)
            if selectedEntryID == id {
                selectedEntryID = nil
                entry = nil
            }
            lastRemoval = removal
            registerUndo(of: removal)
            try await reloadShelves()
            announce("Removed “\(removal.title)”. Undo with Command-Z.")
        } catch {
            show(error, title: "Couldn't remove it")
        }
    }

    private func registerUndo(of removal: Removal) {
        guard let undoManager else { return }
        undoManager.registerUndo(withTarget: self) { model in
            MainActor.assumeIsolated { model.undo(removal) }
        }
        undoManager.setActionName("Remove “\(removal.title)”")
    }

    /// Edit › Undo: puts the entry back, and offers Redo (registered while
    /// the undo runs, so it lands on the redo stack).
    private func undo(_ removal: Removal) {
        if let undoManager {
            undoManager.registerUndo(withTarget: self) { model in
                MainActor.assumeIsolated { model.redo(removal) }
            }
            undoManager.setActionName("Remove “\(removal.title)”")
        }
        Task { await restore(removal) }
    }

    /// Edit › Redo: removes it again.
    private func redo(_ removal: Removal) {
        Task { await remove(removal.summaryId) }
    }

    private func restore(_ removal: Removal) async {
        guard let worker else { return }
        do {
            try await worker.restoreEntry(removal.entry)
            if lastRemoval?.summaryId == removal.summaryId { lastRemoval = nil }
            try await reloadShelves()
            let back = try await worker.entry(removal.summaryId)
            if page == .shelves {
                searchText = ""
                searchResults = nil
                shelf = back.shelf
                select(removal.summaryId)
            }
            announce("Restored “\(removal.title)”")
        } catch {
            show(error, title: "Couldn't undo")
        }
    }

    /// The Undo button on the "Removed" page.
    func undoLastRemoval() {
        undoManager?.undo()
    }

    func announce(_ message: String) {
        AccessibilityNotification.Announcement(message).post()
    }

    // MARK: Adding a book

    /// Searches Open Library. Nothing is saved.
    func searchOpenLibrary(_ query: String) async throws -> [SearchResult] {
        guard let worker, let profile else { return [] }
        return try await worker.searchBooks(for: profile.id, query: query)
    }

    /// Saves a picked result as a book and says whether it's on a shelf already.
    func pick(_ result: SearchResult) async throws -> PickedBook {
        guard let worker, let profile else { throw CoreError.NotFound(message: "No profile is open.") }
        let book = try await worker.saveSearchResult(result, for: profile.id)
        let existing = try await worker.existingEntry(for: profile.id, book: book.id)
        return PickedBook(book: book, existingEntryID: existing)
    }

    /// Puts a picked book on a shelf and opens its page.
    func add(_ book: BookInfo, to shelf: Shelf) async {
        guard let worker, let profile else { return }
        do {
            let id = try await worker.addToShelf(shelf, for: profile.id, book: book.id)
            isAddingBook = false
            searchText = ""
            searchResults = nil
            try await reloadShelves()
            self.shelf = shelf
            select(id)
            announce("Added “\(book.title)” to \(shelf.title)")
        } catch {
            show(error, title: "Couldn't add “\(book.title)”")
        }
    }

    /// Opens an entry from Add a Book ("Open My Entry").
    func openEntry(_ id: String) async {
        guard let worker else { return }
        do {
            let found = try await worker.entry(id)
            isAddingBook = false
            searchText = ""
            searchResults = nil
            shelf = found.shelf
            select(id)
        } catch {
            show(error, title: "Couldn't open your entry")
        }
    }

    // MARK: Reading and writing pages

    var canOpenPages: Bool {
        selectedEntry != nil && page == .shelves && !isAddingBook
    }

    /// Opens the reader for the selected entry.
    func openReader() {
        guard selectedEntry != nil else { return }
        if page == .writer {
            Task {
                await leavePage()
                page = .reader
            }
        } else {
            page = .reader
        }
    }

    /// Opens the writing page for the selected entry.
    func openWriter() {
        guard let worker, let entry = selectedEntry, page != .writer else { return }
        Task {
            let delay = await JournalWorker.autosaveDelay()
            let words = await JournalWorker.words(in: entry.body)
            writer = WriterSession(
                worker: worker, summaryID: entry.summaryId, title: entry.book.title, text: entry.body,
                words: words, delay: delay, prompts: summaryPrompts)
            page = .writer
        }
    }

    /// Back to the shelves, saving the writing page first.
    func closePage() {
        Task { await leavePage() }
    }

    /// Leaves the reader or writer. The writer saves first; if that fails,
    /// its text goes to the recovery folder and an alert says where.
    func leavePage() async {
        if let writer {
            let outcome = await writer.finish()
            self.writer = nil
            report(outcome, title: writer.title)
            if let id = selectedEntryID { await loadEntry(id) }
            try? await reloadShelves()
        }
        page = .shelves
    }

    /// Quitting: the writer's last save, before the journal closes.
    func finishWritingBeforeQuit() async {
        guard let writer else { return }
        let outcome = await writer.finish()
        self.writer = nil
        if case .rescued(let file) = outcome {
            let alert = NSAlert()
            alert.messageText = "Your summary of “\(writer.title)” couldn't be saved"
            alert.informativeText = "Bookshelf kept a copy of your text in \(file.path)."
            alert.runModal()
        } else if case .lost(let message) = outcome {
            let alert = NSAlert()
            alert.messageText = "Your summary of “\(writer.title)” couldn't be saved"
            alert.informativeText = message
            alert.runModal()
        }
    }

    private func report(_ outcome: WriterSession.Outcome, title: String) {
        switch outcome {
        case .saved:
            break
        case .rescued(let file):
            problem = Problem(
                title: "Your summary couldn't be saved",
                message: "Bookshelf kept a copy of your summary of “\(title)” in \(file.path). "
                    + "Nothing was lost.")
        case .lost(let message):
            problem = Problem(title: "Your summary couldn't be saved", message: message)
        }
    }

    // MARK: Backups, export, quitting

    /// Today's backup at launch, then an hourly check (still one a day), so
    /// it keeps happening if Bookshelf stays open for days.
    private func startBackups() {
        backups?.cancel()
        backups = Task { [weak self] in
            while !Task.isCancelled {
                await self?.backUpNow()
                try? await Task.sleep(for: .seconds(60 * 60))
            }
        }
    }

    /// Makes today's backup, if it isn't made yet. Off the main thread.
    func backUpNow() async {
        guard let worker else { return }
        do {
            try await backupFolder.whileAccessing { _ = try await worker.backUpNow() }
        } catch {
            log.error("daily backup failed: \(Self.message(for: error), privacy: .public)")
        }
    }

    /// Checkpoints the journal as the app quits.
    func closeJournal() {
        backups?.cancel()
        do {
            try worker?.closeNow()
        } catch {
            log.error("couldn't tidy up the journal on quit: \(Self.message(for: error), privacy: .public)")
        }
    }
}

/// Runs async work one piece at a time, in the order it was added, so
/// writes land in order.
@MainActor
struct SerialTasks {
    private var last: Task<Void, Never>?

    mutating func enqueue(_ work: @escaping @MainActor () async -> Void) {
        let previous = last
        last = Task {
            await previous?.value
            await work()
        }
    }
}
