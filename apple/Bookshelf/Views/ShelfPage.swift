import SwiftUI
import BookshelfKit

/// A profile's shelves, as on Linux: the header has back (to the
/// profiles) and Add a Book on the left, the three shelf tabs in the
/// middle, and search and Settings on the right. Below it, the shelf's
/// books as a grid of cards, and the search field when it's open.
struct ShelfPage: View {
    /// Space either side of the shelf's column, in points.
    static let margin: CGFloat = 16

    @Environment(AppModel.self) private var model
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        VStack(spacing: 0) {
            if model.isSearchOpen {
                ShelfSearchField()
            }
            ShelfGrid(sections: model.visibleSections)
        }
        .overlay(alignment: .bottom) {
            if let removal = model.lastRemoval {
                RemovedNote(title: removal.title)
            }
        }
        .pageBackground(Palette.paper)
        .navigationTitle(model.shelf.title)
        .toolbar {
            ToolbarItem(placement: .navigation) {
                HStack(spacing: 8) {
                    Button {
                        model.showProfiles()
                    } label: {
                        Label("Back to Profiles", systemImage: "chevron.backward")
                    }
                    .buttonStyle(BackButtonStyle())
                    .help("Back to the profiles")
                    // Each named on its own: in one toolbar item, SwiftUI
                    // gives every button the first one's name.
                    .accessibilityLabel("Back to Profiles")
                    .accessibilityIdentifier("shelf.back")
                    Button {
                        model.isAddingBook = true
                    } label: {
                        Label("Add a Book", systemImage: "plus")
                    }
                    .buttonStyle(CircleButtonStyle())
                    .help(model.help("Add a book", keys: MenuShortcuts.addBook))
                    .accessibilityLabel("Add a Book")
                    .accessibilityIdentifier("toolbar.addBook")
                }
            }
            ToolbarItem(placement: .principal) {
                ShelfTabs()
            }
            ToolbarItem(placement: .primaryAction) {
                HStack(spacing: 8) {
                    Button {
                        model.toggleSearch()
                    } label: {
                        Label("Search", systemImage: "magnifyingglass")
                    }
                    .buttonStyle(CircleButtonStyle(isOn: model.isSearchOpen))
                    .help(model.help("Search your shelves", keys: MenuShortcuts.search))
                    .accessibilityLabel("Search")
                    .accessibilityAddTraits(model.isSearchOpen ? .isSelected : [])
                    .accessibilityIdentifier("toolbar.search")
                    Button {
                        openSettings()
                    } label: {
                        Label("Settings", systemImage: "gearshape")
                    }
                    .buttonStyle(CircleButtonStyle())
                    .help(model.help("Settings", keys: MenuShortcuts.settings))
                    .accessibilityLabel("Settings")
                    .accessibilityIdentifier("toolbar.settings")
                }
            }
        }
        .task(id: model.searchText) { await model.runSearch() }
        .frame(minWidth: 600, minHeight: 480)
    }
}

/// The three shelves as pills, each with its icon and name; the shelf
/// showing is filled with ink. VoiceOver reads the one showing as selected.
struct ShelfTabs: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Shelf.inOrder, id: \.self) { shelf in
                let selected = model.shelf == shelf
                Button {
                    model.show(shelf)
                } label: {
                    Label(shelf.title, systemImage: shelf.systemImage)
                }
                .buttonStyle(ShelfTabStyle(isSelected: selected))
                .help(model.help(shelf.title, keys: MenuShortcuts.title(for: shelf)))
                .accessibilityLabel(shelf.title)
                .accessibilityAddTraits(selected ? .isSelected : [])
                .accessibilityHint("Shows this shelf")
                .accessibilityIdentifier("shelf.\(shelf.title)")
            }
        }
        .fixedSize()
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Shelves")
    }
}

/// The search field under the header: it filters the shelf as you type,
/// and Esc closes it.
struct ShelfSearchField: View {
    @Environment(AppModel.self) private var model
    @FocusState private var focused: Bool

    var body: some View {
        @Bindable var model = model
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(Color.quietText)
                .accessibilityHidden(true)
            TextField("Search your shelves", text: $model.searchText,
                      prompt: Text("Title, author, or something you wrote"))
                .focused($focused)
                .onExitCommand { model.closeSearch() }
                .accessibilityIdentifier("shelf.search")
            if !model.searchText.isEmpty {
                Button {
                    model.searchText = ""
                    focused = true
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(Color.quietText)
                }
                .buttonStyle(.plain)
                .help("Clear the search")
                .accessibilityLabel("Clear the search")
            }
        }
        .fieldChrome(focused: focused)
        .frame(maxWidth: CGFloat(BookGrid.maxWidth))
        .padding(.horizontal, ShelfPage.margin)
        .padding(.top, 4)
        .padding(.bottom, 12)
        .frame(maxWidth: .infinity)
        // Opening the search (⌘F, or the header's button) puts the
        // keyboard in it, once it's in the window.
        .task(id: model.searchFocusRequest) {
            try? await Task.sleep(for: .milliseconds(50))
            focused = true
        }
    }
}

/// The shelf's heading, then its books as cards, a year at a time on the
/// Finished shelf: up to three a row in a readable column, every row
/// centered, every card as tall as the tallest.
struct ShelfGrid: View {
    let sections: [ShelfSection]

    @Environment(AppModel.self) private var model

    var body: some View {
        GeometryReader { geometry in
            let width = max(0, min(geometry.size.width - 2 * ShelfPage.margin, CGFloat(BookGrid.maxWidth)))
            let columns = BookGrid.columns(forWidth: Double(width))
            ShelfColumn(sections: sections, width: width, columns: columns)
                // Another shelf, search or layout measures its cards afresh.
                .id("\(model.shelf.title)|\(model.searchText)|\(columns)|\(model.headingFont)")
        }
    }
}

/// A row of cards, named after the books in it.
private struct CardRow: Identifiable {
    let id: String
    let entries: [ShelfEntry]
}

/// The tallest card measured so far.
private struct CardHeightKey: PreferenceKey {
    static let defaultValue: CGFloat = 0

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = max(value, nextValue())
    }
}

/// The grid at one width.
private struct ShelfColumn: View {
    let sections: [ShelfSection]
    let width: CGFloat
    let columns: Int

    @Environment(AppModel.self) private var model
    /// Every card takes the tallest one's height, so the rows line up. It
    /// only grows: cards are measured as they scroll into view.
    @State private var cardHeight: CGFloat = 0

    var body: some View {
        let cardWidth = CGFloat(BookGrid.cardWidth(columns: columns, in: Double(width)))
        let gap = CGFloat(BookGrid.gap)
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    heading
                        .padding(.bottom, 14)
                    if sections.isEmpty {
                        EmptyShelf()
                    }
                    ForEach(sections) { section in
                        if let title = section.title {
                            SectionHeader(title: title, detail: section.detail)
                                .padding(.top, 22)
                                .padding(.bottom, 12)
                        }
                        ForEach(rows(section)) { row in
                            HStack(alignment: .top, spacing: gap) {
                                ForEach(row.entries) { entry in
                                    EntryCard(entry: entry, height: cardHeight)
                                        .frame(width: cardWidth)
                                }
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.bottom, gap)
                        }
                    }
                }
                .accessibilityElement(children: .contain)
                .accessibilityLabel(model.isSearching ? "Matches on \(model.shelf.title)" : "Books on \(model.shelf.title)")
                .frame(width: width)
                .padding(.horizontal, ShelfPage.margin)
                .padding(.top, 12)
                .padding(.bottom, 64)
                .frame(maxWidth: .infinity)
            }
            .onPreferenceChange(CardHeightKey.self) { tallest in
                if tallest > cardHeight { cardHeight = tallest }
            }
            .onAppear {
                // Back from a book's page: its card comes into view, and
                // takes the focus (`EntryCard`).
                guard let id = model.returningTo else { return }
                let row = sections.flatMap { rows($0) }.first { $0.entries.contains { $0.summaryId == id } }
                if let row {
                    proxy.scrollTo(row.id, anchor: .center)
                } else {
                    model.returned()
                }
            }
        }
    }

    private func rows(_ section: ShelfSection) -> [CardRow] {
        BookGrid.rows(section.entries, columns: columns).map { entries in
            CardRow(id: entries.map(\.summaryId).joined(separator: "|"), entries: entries)
        }
    }

    /// "Reading now", with "2 books" under it.
    private var heading: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(model.shelf.heading)
                .font(Typeface.title(model.headingFont, size: 26))
                .foregroundStyle(Palette.ink)
                .accessibilityAddTraits(.isHeader)
                .accessibilityIdentifier("shelf.heading")
            Text(countLine)
                .font(.system(size: 13))
                .foregroundStyle(Color.quietText)
        }
        .accessibilityElement(children: .combine)
    }

    private var countLine: String {
        if model.isSearching {
            let n = ShelfLayout.count(sections)
            return n == 1 ? "1 match" : "\(n) matches"
        }
        return model.shelves[model.shelf]?.countLine ?? ""
    }
}

/// "2026" with "12 books" beside it, over a year's books.
struct SectionHeader: View {
    let title: String
    let detail: String?

    @Environment(AppModel.self) private var model

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(title)
                .font(Typeface.title(model.headingFont, size: 19))
                .foregroundStyle(Palette.ink)
            if let detail {
                Text(detail)
                    .font(.system(size: 13))
                    .foregroundStyle(Color.quietText)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// What an empty shelf, or a search with nothing on this shelf, says.
struct EmptyShelf: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let searching = model.isSearching
        VStack(spacing: 10) {
            Image(systemName: searching ? "magnifyingglass" : "books.vertical")
                .font(.system(size: 38, weight: .light))
                .foregroundStyle(Color.quietText)
                .accessibilityHidden(true)
            Text(searching ? "No Matches" : "Nothing Here Yet")
                .font(Typeface.title(model.headingFont, size: 22))
                .foregroundStyle(Palette.ink)
                .accessibilityAddTraits(.isHeader)
            Text(searching
                 ? "Nothing on this shelf matches “\(model.searchText.trimmingCharacters(in: .whitespaces))”."
                 : model.shelf.emptyMessage)
                .font(.system(size: 14))
                .foregroundStyle(Color.quietText)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            if !searching {
                Button {
                    model.isAddingBook = true
                } label: {
                    Label("Add a Book", systemImage: "plus")
                }
                .buttonStyle(PillButtonStyle(kind: .primary))
                .padding(.top, 8)
                .accessibilityIdentifier("empty.addBook")
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 56)
    }
}

/// One book on the shelf: a card with the cover, then the title, author,
/// dates and a taste of the summary, centered. It's one button to
/// VoiceOver; click, Space or Return opens the book's page.
struct EntryCard: View {
    let entry: ShelfEntry
    /// The tallest card's height on the shelf, which this one takes too.
    let height: CGFloat

    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme
    @FocusState private var focused: Bool
    @AccessibilityFocusState private var voiceOverFocused: Bool

    var body: some View {
        let font = model.headingFont
        Button {
            model.openBook(entry.summaryId)
        } label: {
            VStack(spacing: 0) {
                CoverView(path: entry.coverPath, title: entry.title, width: 104)
                    .padding(.bottom, 14)
                Text(entry.title)
                    .font(Typeface.title(font, size: 16))
                    .foregroundStyle(Palette.ink)
                    .lineLimit(2)
                if let author = entry.author, !author.isEmpty {
                    Text(author)
                        .font(.system(size: 14))
                        .foregroundStyle(Color.quietText)
                        .lineLimit(1)
                        .padding(.top, 2)
                }
                if !entry.meta.isEmpty {
                    Text(entry.meta.uppercased())
                        .font(.system(size: 10.7, weight: .bold))
                        .tracking(0.32)
                        .foregroundStyle(model.accentText(for: colorScheme))
                        .lineLimit(1)
                        .padding(.top, 4)
                }
                if !entry.excerpt.isEmpty {
                    Text(entry.excerpt)
                        .font(Typeface.text(font, size: 13.2))
                        .foregroundStyle(Palette.ink.opacity(0.82))
                        .lineLimit(3)
                        .padding(.top, 8)
                } else if let note = entry.emptyNote {
                    Text(note)
                        .font(Typeface.text(font, size: 13.2))
                        .italic()
                        .foregroundStyle(Color.quietText)
                        .lineLimit(1)
                        .padding(.top, 8)
                }
            }
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity)
            .background(GeometryReader { geometry in
                Color.clear.preference(key: CardHeightKey.self, value: geometry.size.height)
            })
            .frame(minHeight: height, alignment: .top)
        }
        .buttonStyle(CardButtonStyle(isFocused: focused))
        .focused($focused)
        .focusEffectDisabled()
        .accessibilityFocused($voiceOverFocused)
        .onKeyPress(.return) {
            model.openBook(entry.summaryId)
            return .handled
        }
        .contextMenu {
            Button("Open") { model.openBook(entry.summaryId) }
            Divider()
            Button("Read It Again") { Task { await model.readAgain(entry.summaryId) } }
            Button("Remove from My Shelf", role: .destructive) { Task { await model.remove(entry.summaryId) } }
        }
        .accessibilityLabel(accessibilityText)
        .accessibilityHint("Opens the book's page")
        .accessibilityIdentifier("entry.\(entry.title)")
        .task {
            // Back from this book's page: the focus comes back here.
            guard model.returningTo == entry.summaryId else { return }
            try? await Task.sleep(for: .milliseconds(150))
            focused = true
            voiceOverFocused = true
            model.returned()
        }
    }

    /// "Dune. By Frank Herbert. Finished Sep 5, 2026 · 10 days. Spice…"
    private var accessibilityText: String {
        var parts = [entry.title]
        if let author = entry.author, !author.isEmpty { parts.append("By \(author)") }
        if !entry.meta.isEmpty { parts.append(entry.meta) }
        if !entry.excerpt.isEmpty {
            parts.append(entry.excerpt)
        } else if let note = entry.emptyNote {
            parts.append(note)
        }
        return parts.joined(separator: ". ")
    }
}

/// "Removed “Dune”", with Undo, at the bottom of the shelf until another
/// book is opened.
struct RemovedNote: View {
    let title: String
    @Environment(AppModel.self) private var model

    var body: some View {
        HStack(spacing: 12) {
            Text("Removed “\(title)”")
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(Palette.ink)
                .lineLimit(1)
            Button("Undo") { model.undoLastRemoval() }
                .buttonStyle(PillButtonStyle(kind: .primary))
                .help("Bring it back, exactly as it was (⌘Z)")
                .accessibilityIdentifier("removed.undo")
            Button {
                model.dismissRemoval()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(Color.quietText)
                    .frame(width: 24, height: 24)
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .help("Dismiss")
            .accessibilityLabel("Dismiss")
        }
        .padding(.leading, 18)
        .padding(.trailing, 8)
        .padding(.vertical, 7)
        .background(Capsule().fill(Palette.card))
        .overlay(Capsule().strokeBorder(Palette.edge, lineWidth: 1))
        .shadow(color: .black.opacity(0.12), radius: 10, y: 4)
        .padding(.horizontal, ShelfPage.margin)
        .padding(.bottom, 20)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("removed.note")
    }
}
