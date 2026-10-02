import SwiftUI
import BookshelfKit

/// The books on the shelf (or every shelf's matches, while searching):
/// cover, title, author, dates and a taste of the summary.
struct EntryListView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        let sections = model.visibleSections
        Group {
            if sections.isEmpty {
                emptyState
            } else {
                List(selection: Binding(get: { model.selectedEntryID }, set: { model.select($0) })) {
                    ForEach(sections) { section in
                        if let title = section.title {
                            Section {
                                rows(section)
                            } header: {
                                SectionHeader(title: title, detail: section.detail)
                            }
                        } else {
                            rows(section)
                        }
                    }
                }
                .listStyle(.inset)
                .contextMenu(forSelectionType: String.self) { ids in
                    if ids.count == 1, let id = ids.first {
                        EntryContextMenu(id: id)
                    }
                } primaryAction: { ids in
                    // Double-click or Return: read what's written, or write it.
                    guard let id = ids.first, id == model.selectedEntryID else { return }
                    if model.selectedEntry?.body.isEmpty == false { model.openReader() } else { model.openWriter() }
                }
            }
        }
        .navigationTitle(model.isSearching ? "Search" : model.shelf.title)
        .navigationSubtitle(subtitle)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    model.isAddingBook = true
                } label: {
                    Label("Add a Book", systemImage: "plus")
                }
                .help("Add a book (⌘N)")
                .accessibilityIdentifier("toolbar.addBook")
            }
        }
    }

    private func rows(_ section: ShelfSection) -> some View {
        ForEach(section.entries) { entry in
            EntryRow(entry: entry, font: model.headingFont)
                .tag(entry.summaryId)
        }
    }

    private var subtitle: String {
        if model.isSearching {
            let n = ShelfLayout.count(model.visibleSections)
            return n == 1 ? "1 book" : "\(n) books"
        }
        return model.shelves[model.shelf]?.countLine ?? ""
    }

    @ViewBuilder
    private var emptyState: some View {
        if model.isSearching {
            ContentUnavailableView.search(text: model.searchText)
        } else {
            ContentUnavailableView {
                Label("Nothing Here Yet", systemImage: model.shelf.systemImage)
            } description: {
                Text(model.shelf.emptyMessage)
            } actions: {
                Button("Add a Book") { model.isAddingBook = true }
                    .buttonStyle(.borderedProminent)
                    .accessibilityIdentifier("empty.addBook")
            }
        }
    }
}

/// "2026" with "12 books" beside it, over a year's books.
struct SectionHeader: View {
    let title: String
    let detail: String?

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title)
                .font(.headline)
                .foregroundStyle(.primary)
            Spacer()
            if let detail {
                Text(detail)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// One book on a shelf.
struct EntryRow: View {
    let entry: ShelfEntry
    let font: HeadingFont

    @Environment(\.backgroundProminence) private var prominence
    @Environment(\.colorScheme) private var colorScheme
    @Environment(AppModel.self) private var model

    var body: some View {
        // On a selected row the text sits on the accent: plain colors read
        // there, accent-colored ones don't. (The list's prominence doesn't
        // reach the row on macOS 15, so ask the selection too.)
        let selected = prominence == .increased || model.selectedEntryID == entry.summaryId
        HStack(alignment: .top, spacing: 12) {
            CoverView(path: entry.coverPath, title: entry.title, width: 44)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 3) {
                Text(entry.title)
                    .font(Typeface.title(font, size: 15, weight: .semibold))
                    .lineLimit(2)
                if let author = entry.author, !author.isEmpty {
                    Text(author)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                if !entry.meta.isEmpty {
                    Text(entry.meta.uppercased())
                        .font(.caption2.weight(.semibold))
                        .tracking(0.5)
                        .foregroundStyle(selected ? AnyShapeStyle(.secondary) : AnyShapeStyle(model.accentText(for: colorScheme)))
                        .lineLimit(1)
                        .padding(.top, 1)
                }
                if !entry.excerpt.isEmpty {
                    Text(entry.excerpt)
                        .font(Typeface.text(font, size: 13))
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .padding(.top, 3)
                } else if let note = entry.emptyNote {
                    Text(note)
                        .font(Typeface.text(font, size: 13))
                        .italic()
                        .foregroundStyle(.tertiary)
                        .padding(.top, 3)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 6)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityText)
        // A row reads as one piece of text; the list row around it is what's selected.
        .accessibilityAddTraits(.isStaticText)
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

/// Right-click on a book in the list.
struct EntryContextMenu: View {
    let id: String
    @Environment(AppModel.self) private var model

    var body: some View {
        let ready = model.selectedEntry?.summaryId == id
        Button("Write") { model.openWriter() }.disabled(!ready)
        Button("Read") { model.openReader() }.disabled(!ready)
        Divider()
        Button("Read It Again") { Task { await model.readAgain(id) } }
        Button("Remove from My Shelf", role: .destructive) { Task { await model.remove(id) } }
    }
}
