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

    /// What the main window shows, one page at a time, as on Linux: the
    /// profiles, then a profile's shelves, a book's page over them, and its
    /// reading or writing page over that. Back goes one page down.
    enum Page: Equatable {
        case profiles
        case shelves
        case book
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

    /// The shelf whose tab is selected.
    private(set) var shelf: Shelf = .reading
    private(set) var shelves: [Shelf: ShelfView] = [:]
    private(set) var shelfSections: [Shelf: [ShelfSection]] = [:]
    /// Whether the search field shows under the header.
    private(set) var isSearchOpen = false
    /// Asks the search field to take the keyboard focus (⌘F).
    private(set) var searchFocusRequest = 0
    /// The shelf search, as typed.
    var searchText = ""
    /// Matches across every shelf; `nil` when not searching.
    private(set) var searchResults: [ShelfSection]?

    /// The book whose page is open (or was last).
    private(set) var selectedEntryID: String?
    private(set) var entry: EntryDetail?
    private(set) var summaryBlocks: [Block] = []
    private(set) var summaryPrompts = ""
    private(set) var page: Page = .shelves
    /// The writing page's text and saving, while it's open.
    private(set) var writer: WritingSession?
    /// Focus mode on the writing page.
    var focusMode = false
    /// Edit › Spelling and Grammar and Substitutions, for the writing page.
    let writingPreferences = WritingPreferences()
    /// The entry just removed, until another book is opened: the shelf
    /// says so, with an Undo button.
    private(set) var lastRemoval: Removal?

    var problem: Problem?
    var isAddingBook = false
    /// Asks the profiles page's form to take the keyboard focus (File ›
    /// New Profile…).
    var focusNewProfile = false

    let options = LaunchOptions.current
    @ObservationIgnored weak var undoManager: UndoManager?
    /// The main window, for sheets.
    @ObservationIgnored weak var mainWindow: NSWindow?
    /// The writing page's text view, which the Format, Find and other text
    /// menus act on.
    @ObservationIgnored weak var editor: WritingTextView?
    /// Leaving the writer is underway (saving, maybe asking): don't start again.
    @ObservationIgnored private var leavingWriter = false
    @ObservationIgnored private var openingWriter = false
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
            closeSearch()
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

    /// Opens a profile from the profiles page: its shelves, as they were
    /// if it's the one open already.
    func switchTo(_ profile: Profile) {
        guard profile.id != self.profile?.id else {
            if phase == .ready { page = .shelves }
            return
        }
        Task {
            guard await leavePage() else { return }
            await openProfile(profile)
        }
    }

    /// Back from the shelves: the profiles page, to switch profile or make
    /// a new one. Any writing is saved first.
    func showProfiles() {
        Task {
            guard await leavePage() else { return }
            closeSearch()
            page = .profiles
        }
    }

    /// File › New Profile…: the profiles page, with its form ready to type in.
    func newProfile() {
        Task {
            guard await leavePage() else { return }
            if phase == .ready {
                closeSearch()
                page = .profiles
            }
            focusNewProfile = true
        }
    }

    /// Makes a profile and opens it. Throws what to say under the form.
    func createProfile(name: String, email: String) async throws {
        guard let worker else { return }
        let made = try await worker.createProfile(named: name, email: email)
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

    /// The accent as buttons, switches and focus rings use it.
    func journalAccent(for scheme: ColorScheme) -> JournalAccent {
        guard let palettes else { return .standard }
        let palette = scheme == .dark ? palettes.dark : palettes.light
        return JournalAccent(base: palette.bg, onFill: palette.fg, text: palette.text)
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
            guard await leavePage() else { return }
            try await worker.deleteProfile(profile.id)
            UserDefaults.standard.removeObject(forKey: Self.lastProfileKey)
            try await loadProfiles()
            announce("Deleted \(profile.name)")
        } catch {
            show(error, title: "Couldn't delete \(profile.name)")
        }
    }

    // MARK: Shelves and search

    /// Reads all three shelves again.
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

    /// Shows a shelf (its tab, ⌘1–3), from whichever page is open. A
    /// search stays, and filters this shelf too.
    func show(_ shelf: Shelf) {
        self.shelf = shelf
        guard page != .shelves else { return }
        Task { await leaveToShelves() }
    }

    /// Back from a book's page (or its reading or writing page) to the
    /// shelves. Returns whether it got there: a failed save can keep the
    /// writing page open.
    @discardableResult
    func leaveToShelves() async -> Bool {
        guard await leavePage() else { return false }
        page = .shelves
        return true
    }

    /// Runs the shelf search for what's typed now. A newer search makes an
    /// older one's results go away unseen.
    func runSearch() async {
        let query = searchText
        let found = await JournalWorker.search(shelves, for: query)
        guard query == searchText else { return }
        searchResults = found
    }

    /// What the shelf shows: its books, or while searching its matches
    /// (without the year headings, as on Linux).
    var visibleSections: [ShelfSection] {
        guard let searchResults else { return shelfSections[shelf] ?? [] }
        return searchResults
            .filter { $0.title == shelf.title }
            .map { ShelfSection(id: $0.id, title: nil, detail: nil, entries: $0.entries) }
    }

    var isSearching: Bool { searchResults != nil }

    /// Opens the search field under the header and focuses it (⌘F), from
    /// whichever page is open.
    func focusSearch() {
        Task {
            if page != .shelves {
                guard await leaveToShelves() else { return }
            }
            isSearchOpen = true
            searchFocusRequest &+= 1
        }
    }

    /// The header's search button: opens the search field, or closes it.
    func toggleSearch() {
        if isSearchOpen {
            closeSearch()
        } else {
            focusSearch()
        }
    }

    /// Closes the search field (Esc): every book shows again.
    func closeSearch() {
        isSearchOpen = false
        searchText = ""
        searchResults = nil
    }

    // MARK: The book's page

    /// Opens a book's page over the shelf.
    func openBook(_ id: String) {
        lastRemoval = nil
        if selectedEntryID != id {
            selectedEntryID = id
            entry = nil
            summaryBlocks = []
        }
        page = .book
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
            guard selectedEntryID == id else { return }
            show(error, title: "Couldn't open this book")
            // Nothing to show: back to the shelf rather than an empty page.
            if page == .book, selectedEntry == nil { page = .shelves }
        }
    }

    /// The entry, if the book page is showing it.
    var selectedEntry: EntryDetail? {
        guard let entry, entry.summaryId == selectedEntryID else { return nil }
        return entry
    }

    /// Sets the reading dates; the book may move shelf, and the tabs
    /// follow it there.
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
                if self.selectedEntryID == id, saved.shelf != self.shelf {
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
            closeSearch()
            shelf = .reading
            openBook(fresh)
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
                // Its page is gone: back to the shelf, which says so.
                if page == .book { page = .shelves }
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
                closeSearch()
                shelf = back.shelf
            }
            announce("Restored “\(removal.title)”")
        } catch {
            show(error, title: "Couldn't undo")
        }
    }

    /// The Undo button on the shelf's "Removed" note.
    func undoLastRemoval() {
        undoManager?.undo()
    }

    /// Closes the "Removed" note. Edit › Undo still brings the book back.
    func dismissRemoval() {
        lastRemoval = nil
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
            closeSearch()
            try await reloadShelves()
            self.shelf = shelf
            openBook(id)
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
            closeSearch()
            shelf = found.shelf
            openBook(id)
        } catch {
            show(error, title: "Couldn't open your entry")
        }
    }

    // MARK: Reading and writing pages

    /// Whether a book's page is showing, ready for the Book menu.
    var canOpenPages: Bool {
        selectedEntry != nil && page == .book && !isAddingBook
    }

    /// Opens the reader for the selected entry.
    func openReader() {
        guard selectedEntry != nil else { return }
        if page == .writer {
            Task {
                guard await leavePage() else { return }
                page = .reader
            }
        } else {
            page = .reader
        }
    }

    /// Opens the writing page for the selected entry, in focus mode if the
    /// profile likes it.
    func openWriter() {
        guard let worker, let entry = selectedEntry, page != .writer, !openingWriter else { return }
        openingWriter = true
        Task {
            defer { openingWriter = false }
            let delay = await JournalWorker.autosaveDelay()
            let words = await JournalWorker.words(in: entry.body)
            guard selectedEntryID == entry.summaryId else { return }
            focusMode = settings?.focusDefault ?? false
            writer = WritingSession(
                title: entry.book.title, text: entry.body, words: words, prompts: summaryPrompts, delay: delay,
                store: writingStore(for: entry.summaryId, title: entry.book.title, worker: worker))
            page = .writer
        }
    }

    /// Where the writing page's text goes: the journal, through the worker.
    private func writingStore(for id: String, title: String, worker: JournalWorker) -> WritingSession.Store {
        #if DEBUG
        if options.failSaves {
            // UI tests of the rescue path: the journal "refuses" the text.
            let rescueFails = options.failRescue
            return WritingSession.Store(
                save: { _ in throw CoreError.Io(message: "Saving is switched off for this test.") },
                rescue: { body in
                    if rescueFails { throw CoreError.Io(message: "Recovery copies are switched off for this test.") }
                    return try await worker.rescueBody(title: title, body: body)
                })
        }
        #endif
        return WritingSession.Store(
            save: { try await worker.saveBody(id, $0).words },
            rescue: { try await worker.rescueBody(title: title, body: $0) })
    }

    /// Back from the reading or writing page to the book's page, saving
    /// the writing first.
    func closePage() {
        Task { await leavePage() }
    }

    /// Back from the book's page to the shelf.
    func closeBook() {
        guard page == .book else { return }
        page = .shelves
    }

    /// Leaves the reader or writer for the book's page; on any other page
    /// it does nothing. The writer saves first; if that fails, its text is
    /// rescued and an alert asks whether to keep writing. Returns whether
    /// the page was left.
    @discardableResult
    func leavePage() async -> Bool {
        if let writer {
            guard !leavingWriter else { return false }
            leavingWriter = true
            defer { leavingWriter = false }
            guard await finish(writer, quitting: false) else { return false }
            if self.writer === writer { self.writer = nil }
            if let id = selectedEntryID { await loadEntry(id) }
            try? await reloadShelves()
        }
        if page == .reader || page == .writer { page = .book }
        return true
    }

    /// Quitting: the writer's last save. Returns false to stay open (the
    /// person chose to keep writing after a failed save).
    func finishWritingBeforeQuit() async -> Bool {
        guard let writer else { return true }
        guard !leavingWriter else { return false }
        leavingWriter = true
        defer { leavingWriter = false }
        guard await finish(writer, quitting: true) else { return false }
        self.writer = nil
        return true
    }

    /// The writer's last save; if it failed (the text is rescued by now),
    /// asks Keep Open, or Close Anyway (Quit Anyway when `quitting`).
    /// Returns whether to go.
    private func finish(_ writer: WritingSession, quitting: Bool) async -> Bool {
        let outcome = await writer.finish()
        guard outcome != .saved else { return true }
        let choice = await RescueAlert.ask(outcome, title: writer.title, quitting: quitting, in: mainWindow)
        return choice == .closeAnyway
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
