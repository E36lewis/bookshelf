import SwiftUI
import BookshelfKit

/// Add a Book (⌘N): search Open Library as you type, pick a result, then
/// choose its shelf (or, for a book already logged, open it or read it
/// again).
struct AddBookSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss

    @State private var query = ""
    @State private var results: [SearchResult] = []
    @State private var searching = false
    @State private var failure: String?
    /// The result being saved, by its Open Library key.
    @State private var saving: String?
    @State private var picked: AppModel.PickedBook?
    @State private var askingShelf = false
    @State private var askingAgain = false
    @FocusState private var fieldFocused: Bool

    private var trimmed: String { query.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                TextField("Search by title, author or ISBN", text: $query)
                    .textFieldStyle(.plain)
                    .font(.title3)
                    .focused($fieldFocused)
                    .accessibilityIdentifier("addBook.query")
                if searching {
                    ProgressView()
                        .controlSize(.small)
                        .accessibilityLabel("Searching")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
            Divider()
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            Divider()
            HStack {
                Text("Results come from Open Library.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Cancel", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
            }
            .padding(12)
        }
        .frame(width: 580, height: 480)
        .onAppear { fieldFocused = true }
        // Typing restarts this: a pause before searching, and a newer
        // search drops an older one's results.
        .task(id: trimmed) { await search(trimmed) }
        .confirmationDialog(
            "You've logged this book before", isPresented: $askingAgain, titleVisibility: .visible,
            presenting: picked
        ) { picked in
            Button("Open My Entry") {
                if let id = picked.existingEntryID { Task { await model.openEntry(id) } }
            }
            Button("Read It Again") {
                if let id = picked.existingEntryID { Task { await model.readAgain(id) } }
            }
            Button("Cancel", role: .cancel) {}
        } message: { picked in
            Text("Open your entry for “\(picked.book.title)”, or start a new one for this reading?")
        }
        .confirmationDialog(
            "Add “\(picked?.book.title ?? "")”", isPresented: $askingShelf, titleVisibility: .visible,
            presenting: picked
        ) { picked in
            ForEach([Shelf.eventually, .reading, .finished], id: \.self) { shelf in
                Button(shelf.addTitle) { Task { await model.add(picked.book, to: shelf) } }
            }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("Which shelf does it go on? You can change the dates afterwards.")
        }
    }

    @ViewBuilder
    private var content: some View {
        if let failure {
            ContentUnavailableView {
                Label("Search Failed", systemImage: "wifi.exclamationmark")
            } description: {
                Text(failure + "\nBook search needs an internet connection.")
            }
        } else if trimmed.isEmpty {
            ContentUnavailableView(
                "Find a Book", systemImage: "books.vertical",
                description: Text("Type a title, an author, or an ISBN. Results appear as you type."))
        } else if results.isEmpty && !searching {
            ContentUnavailableView.search(text: trimmed)
        } else {
            List(Array(results.enumerated()), id: \.offset) { _, result in
                Button {
                    pick(result)
                } label: {
                    ResultRow(result: result, saving: saving == result.externalId)
                }
                .buttonStyle(.plain)
                .disabled(saving != nil)
            }
            .listStyle(.inset)
        }
    }

    private func search(_ query: String) async {
        guard !query.isEmpty else {
            results = []
            failure = nil
            searching = false
            return
        }
        do {
            try await Task.sleep(for: .milliseconds(400))
        } catch {
            return  // typing went on
        }
        searching = true
        do {
            let found = try await model.searchOpenLibrary(query)
            guard !Task.isCancelled else { return }
            results = found
            failure = nil
        } catch {
            guard !Task.isCancelled else { return }
            results = []
            failure = (error as? CoreError)?.userMessage ?? "Something went wrong."
        }
        searching = false
    }

    private func pick(_ result: SearchResult) {
        guard saving == nil else { return }
        saving = result.externalId
        Task {
            defer { saving = nil }
            do {
                let book = try await model.pick(result)
                picked = book
                if book.existingEntryID != nil { askingAgain = true } else { askingShelf = true }
            } catch {
                failure = (error as? CoreError)?.userMessage ?? "Something went wrong."
            }
        }
    }
}

/// A search result: title, then author and year.
struct ResultRow: View {
    let result: SearchResult
    let saving: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "book.closed")
                .font(.title2)
                .foregroundStyle(.secondary)
                .frame(width: 28)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(result.title)
                    .font(.headline)
                if let subtitle = result.subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                let byline = Wording.byline(author: result.author, year: result.publishedDate)
                if !byline.isEmpty {
                    Text(byline)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
            if saving {
                ProgressView().controlSize(.small)
            }
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityHint("Adds this book")
    }
}
