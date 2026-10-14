import SwiftUI
import BookshelfKit

/// A profile's shelves, as on Linux: the header has back (to the
/// profiles) and Add a Book on the left, the three shelf tabs in the
/// middle, and search and Settings on the right. Below it, the shelf's
/// books as a grid of cards, and the search field when it's open.
struct ShelfPage: View {
    /// Space either side of the shelf's column, in points.
    static let margin: CGFloat = 24

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
                    .accessibilityIdentifier("shelf.back")
                    Button {
                        model.isAddingBook = true
                    } label: {
                        Label("Add a Book", systemImage: "plus")
                    }
                    .buttonStyle(CircleButtonStyle())
                    .help(model.help("Add a book", keys: MenuShortcuts.addBook))
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
                    .accessibilityAddTraits(model.isSearchOpen ? .isSelected : [])
                    .accessibilityIdentifier("toolbar.search")
                    Button {
                        openSettings()
                    } label: {
                        Label("Settings", systemImage: "gearshape")
                    }
                    .buttonStyle(CircleButtonStyle())
                    .help(model.help("Settings", keys: MenuShortcuts.settings))
                    .accessibilityIdentifier("toolbar.settings")
                }
            }
        }
        .task(id: model.searchText) { await model.runSearch() }
        .frame(minWidth: 600, minHeight: 480)
    }
}

/// The three shelves as pills, each with its icon and name; the shelf
/// showing is filled with ink. VoiceOver reads them as the Shelves group,
/// the one showing as selected.
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
/// Finished shelf: up to three a row in a readable column, with a short
/// row centered.
struct ShelfGrid: View {
    let sections: [ShelfSection]

    @Environment(AppModel.self) private var model

    var body: some View {
        GeometryReader { geometry in
            let width = max(0, min(geometry.size.width - 2 * ShelfPage.margin, CGFloat(BookGrid.maxWidth)))
            let columns = BookGrid.columns(forWidth: Double(width))
            let cardWidth = CGFloat(BookGrid.cardWidth(columns: columns, in: Double(width)))
            // 2:3 covers, as wide as the card allows up to 112 pt.
            let coverWidth = max(64, min(112, ((cardWidth - 32) * 0.6).rounded()))
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
                                .padding(.top, section.id == sections.first?.id ? 4 : 14)
                                .padding(.bottom, 10)
                        }
                        ForEach(BookGrid.rows(section.entries, columns: columns), id: \.first?.id) { row in
                            HStack(alignment: .top, spacing: CGFloat(BookGrid.gap)) {
                                ForEach(row) { entry in
                                    EntryCard(entry: entry, coverWidth: coverWidth)
                                        .frame(width: cardWidth)
                                }
                            }
                            // Every card in a row as tall as the tallest.
                            .fixedSize(horizontal: false, vertical: true)
                            .frame(maxWidth: .infinity)
                            .padding(.bottom, CGFloat(BookGrid.gap))
                        }
                    }
                }
                .frame(width: width)
                .padding(.horizontal, ShelfPage.margin)
                .padding(.top, 12)
                .padding(.bottom, 64)
                .frame(maxWidth: .infinity)
            }
            .accessibilityLabel(model.isSearching ? "Matches on \(model.shelf.title)" : "Books on \(model.shelf.title)")
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
    let coverWidth: CGFloat

    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let font = model.headingFont
        Button {
            model.openBook(entry.summaryId)
        } label: {
            VStack(spacing: 0) {
                CoverView(path: entry.coverPath, title: entry.title, width: coverWidth)
                    .padding(.bottom, 12)
                Text(entry.title)
                    .font(Typeface.title(font, size: 16))
                    .foregroundStyle(Palette.ink)
                    .lineLimit(2)
                if let author = entry.author, !author.isEmpty {
                    Text(author)
                        .font(.system(size: 13))
                        .foregroundStyle(Color.quietText)
                        .lineLimit(1)
                        .padding(.top, 3)
                }
                if !entry.meta.isEmpty {
                    Text(entry.meta.uppercased())
                        .font(.system(size: 10.5, weight: .bold))
                        .tracking(0.6)
                        .foregroundStyle(model.accentText(for: colorScheme))
                        .lineLimit(1)
                        .padding(.top, 6)
                }
                if !entry.excerpt.isEmpty {
                    Text(entry.excerpt)
                        .font(Typeface.text(font, size: 13))
                        .foregroundStyle(Color.quietText)
                        .lineLimit(3)
                        .padding(.top, 8)
                } else if let note = entry.emptyNote {
                    Text(note)
                        .font(Typeface.text(font, size: 13))
                        .italic()
                        .foregroundStyle(Color.quietText)
                        .lineLimit(2)
                        .padding(.top, 8)
                }
            }
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        }
        .buttonStyle(CardButtonStyle())
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
    }

    /// "Dune, by Frank Herbert. Finished Sep 5, 2026 · 10 days. Spice…"
    private var accessibilityText: String {
        var parts = [entry.title]
        if let author = entry.author, !author.isEmpty { parts[0] += ", by \(author)" }
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
