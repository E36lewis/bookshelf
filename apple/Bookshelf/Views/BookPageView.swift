import SwiftUI
import BookshelfKit

/// The right-hand column: the selected book's page, or why there isn't one.
struct DetailView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        if let entry = model.selectedEntry, model.isListed(entry.summaryId) {
            BookPageView(entry: entry)
        } else if let removal = model.lastRemoval {
            ContentUnavailableView {
                Label("Removed “\(removal.title)”", systemImage: "trash")
            } description: {
                Text("Your summary and dates for this book are gone from your shelf. Undo brings everything back exactly as it was.")
            } actions: {
                Button("Undo") { model.undoLastRemoval() }
                    .accessibilityIdentifier("removed.undo")
            }
        } else if model.visibleSections.isEmpty {
            // The list already says the shelf is empty.
            Color.clear
        } else if model.selectedEntryID == nil {
            ContentUnavailableView(
                "No Book Selected", systemImage: "books.vertical",
                description: Text("Choose a book to see its page, or add one with ⌘N."))
        } else {
            // Loading: only for a moment.
            Color.clear
        }
    }
}

/// A book's page: the book, what it's about, when it was read, and the
/// summary.
struct BookPageView: View {
    let entry: EntryDetail

    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme
    @State private var aboutOpen = false

    private var book: BookInfo { entry.book }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                header
                if let about = book.description, !about.isEmpty {
                    DisclosureGroup(isExpanded: $aboutOpen) {
                        Text(about)
                            .font(Typeface.text(model.headingFont, size: 14))
                            .lineSpacing(3)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.top, 8)
                    } label: {
                        Text("About this book")
                            .font(.headline)
                            .accessibilityAddTraits(.isHeader)
                    }
                    .accessibilityIdentifier("book.about")
                }
                ReadingDatesView(entry: entry)
                summary
            }
            .frame(maxWidth: 680, alignment: .leading)
            .padding(.horizontal, 36)
            .padding(.vertical, 32)
            .frame(maxWidth: .infinity)
        }
        .id(entry.summaryId)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    model.openWriter()
                } label: {
                    Label(entry.body.isEmpty ? "Write" : "Edit", systemImage: "square.and.pencil")
                }
                .help("Write or edit your summary (⌘↩)")
                Button {
                    model.openReader()
                } label: {
                    Label("Read", systemImage: "book.pages")
                }
                .help("Read your summary, without distractions (⌘R)")
                Menu {
                    Button("Read It Again") { model.readAgain() }
                    Divider()
                    Button("Remove from My Shelf", role: .destructive) { model.removeSelected() }
                } label: {
                    Label("More", systemImage: "ellipsis.circle")
                }
                .help("Read it again, or remove it from your shelf")
                .accessibilityLabel("More")
                .accessibilityIdentifier("book.more")
            }
        }
    }

    private var header: some View {
        HStack(alignment: .top, spacing: 24) {
            CoverView(path: book.coverPath, title: book.title, author: book.author, width: 128)
            VStack(alignment: .leading, spacing: 6) {
                Text(book.title)
                    .font(Typeface.title(model.headingFont, size: 28))
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityHeading(.h1)
                    .accessibilityIdentifier("book.title")
                if let subtitle = book.subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(Typeface.text(model.headingFont, size: 17))
                        .italic()
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let author = book.author, !author.isEmpty {
                    Text(author)
                        .font(.title3.weight(.semibold))
                        .foregroundStyle(model.accentText(for: colorScheme))
                        .padding(.top, 2)
                }
                let facts = Wording.bookFacts(book)
                if !facts.isEmpty {
                    Text(facts)
                        .font(.callout)
                        .foregroundStyle(.primary)
                }
                Spacer(minLength: 0)
            }
        }
    }

    private var summary: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                Text("My Summary")
                    .font(Typeface.title(model.headingFont, size: 20))
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityHeading(.h2)
                Spacer()
                Button(entry.body.isEmpty ? "Write" : "Edit") { model.openWriter() }
                Button("Read") { model.openReader() }
                    .disabled(entry.body.isEmpty)
            }
            Button {
                model.openWriter()
            } label: {
                Group {
                    if model.summaryBlocks.isEmpty {
                        Text(model.summaryPrompts.isEmpty ? "Nothing written yet." : model.summaryPrompts)
                            .font(Typeface.text(model.headingFont, size: 15))
                            .italic()
                            .lineSpacing(5)
                            .foregroundStyle(.tertiary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        BlocksView(blocks: model.summaryBlocks, font: model.headingFont, size: 15, lineSpacing: 4)
                    }
                }
                .padding(22)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.background, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(.separator))
                .contentShape(RoundedRectangle(cornerRadius: 12))
            }
            .buttonStyle(.plain)
            .accessibilityLabel(entry.body.isEmpty ? "My summary: nothing written yet" : "My summary")
            .accessibilityHint("Opens the writing page")
            .accessibilityIdentifier("book.summary")
        }
    }
}

/// When reading started and finished, each with a calendar, Today and
/// Clear. Days that would contradict the other date can't be picked.
struct ReadingDatesView: View {
    let entry: EntryDetail
    @Environment(AppModel.self) private var model

    var body: some View {
        let calendar = model.calendar
        let started = entry.startedDate
        let finished = entry.finishedDate
        GroupBox {
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 10) {
                DateRow(
                    title: "Started", date: started,
                    range: (Date.distantPast...(finished ?? .distantFuture)),
                    calendar: calendar
                ) { model.setDates(started: $0, finished: finished) }
                DateRow(
                    title: "Finished", date: finished,
                    range: ((started ?? .distantPast)...Date.distantFuture),
                    calendar: calendar
                ) { model.setDates(started: started, finished: $0) }
                if let days = Wording.finishedIn(days: entry.days) {
                    GridRow {
                        Color.clear.frame(width: 1, height: 1)
                        Label(days, systemImage: "flag.checkered")
                            .foregroundStyle(.secondary)
                            .gridCellColumns(3)
                    }
                }
            }
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
        } label: {
            Text("Reading Dates")
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
        }
    }
}

/// One date: the day (a button that opens a calendar), Today, and Clear.
struct DateRow: View {
    let title: String
    let date: Date?
    let range: ClosedRange<Date>
    let calendar: Calendar
    let commit: (Date?) -> Void

    @Environment(AppModel.self) private var model
    @State private var picking = false

    private var today: Date { calendar.startOfDay(for: Date()) }

    var body: some View {
        let format = model.settings?.dateFormat ?? .long
        GridRow {
            Text(title)
                .foregroundStyle(.primary)
                .gridColumnAlignment(.trailing)
            Button {
                picking = true
            } label: {
                Label(date.map { JournalDate.display($0, format: format, calendar: calendar) } ?? "Not set",
                      systemImage: "calendar")
                    .frame(minWidth: 120, alignment: .leading)
            }
            .accessibilityLabel("\(title): \(date.map { JournalDate.display($0, format: .long, calendar: calendar) } ?? "not set")")
            .accessibilityHint("Opens a calendar")
            .popover(isPresented: $picking, arrowEdge: .bottom) {
                CalendarPopover(date: date, range: range, calendar: calendar) { picked in
                    picking = false
                    commit(picked)
                }
            }
            Button("Today") { commit(today) }
                .disabled(!range.contains(today) || date.map { calendar.isDate($0, inSameDayAs: today) } == true)
                .accessibilityLabel("\(title) today")
            if date != nil {
                Button {
                    commit(nil)
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.borderless)
                .help("Clear this date")
                .accessibilityLabel("Clear the \(title.lowercased()) date")
            } else {
                Color.clear.frame(width: 16, height: 16)
            }
        }
    }
}

/// The month calendar for one date, with the week starting on the
/// profile's day.
struct CalendarPopover: View {
    let date: Date?
    let range: ClosedRange<Date>
    let calendar: Calendar
    let onPick: (Date?) -> Void

    @State private var shown = Date()

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            DatePicker("Date", selection: Binding(get: { shown }, set: { onPick($0) }), in: range, displayedComponents: .date)
                .datePickerStyle(.graphical)
                .labelsHidden()
            HStack {
                Button("Clear Date") { onPick(nil) }
                    .disabled(date == nil)
                Spacer()
                Button("Today") { onPick(calendar.startOfDay(for: Date())) }
                    .disabled(!range.contains(calendar.startOfDay(for: Date())))
            }
        }
        .padding(14)
        .environment(\.calendar, calendar)
        .onAppear {
            let today = calendar.startOfDay(for: Date())
            shown = date ?? min(max(today, range.lowerBound), range.upperBound)
        }
    }
}
