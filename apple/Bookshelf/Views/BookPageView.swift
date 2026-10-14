import SwiftUI
import BookshelfKit

/// A book's page over the shelf, or a moment's wait while it's read.
struct BookScreen: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        if let entry = model.selectedEntry {
            BookPageView(entry: entry)
        } else {
            Color.clear
                .pageBackground(Palette.paper)
                .toolbar {
                    ToolbarItem(placement: .navigation) { BookBackButton() }
                }
                .frame(minWidth: 600, minHeight: 480)
                .onAppear { if model.selectedEntryID == nil { model.closeBook() } }
        }
    }
}

/// Back to the shelf (Esc).
struct BookBackButton: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Button {
            model.closeBook()
        } label: {
            Label("Back to Shelf", systemImage: "chevron.backward")
        }
        .buttonStyle(BackButtonStyle())
        .keyboardShortcut(.cancelAction)
        .help("Back to your shelf (Esc)")
        .accessibilityIdentifier("book.back")
    }
}

/// A book's page, as on Linux: the book, what it's about, when it was
/// read, and the summary, in one readable column. The header has back,
/// the book's title, and a ⋯ menu.
struct BookPageView: View {
    let entry: EntryDetail

    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme
    @State private var aboutOpen = false

    private var book: BookInfo { entry.book }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                header
                if let about = book.description, !about.isEmpty {
                    DisclosureGroup(isExpanded: $aboutOpen) {
                        Text(about)
                            .font(Typeface.text(model.headingFont, size: 14))
                            .foregroundStyle(Palette.ink)
                            .lineSpacing(3)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.top, 10)
                    } label: {
                        Text("About this book")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(Palette.ink)
                            .accessibilityAddTraits(.isHeader)
                    }
                    .tint(Palette.ink)
                    .panel(padding: 14)
                    .accessibilityIdentifier("book.about")
                }
                ReadingDatesView(entry: entry)
                summary
            }
            .frame(maxWidth: CGFloat(BookGrid.maxWidth), alignment: .leading)
            .padding(.horizontal, ShelfPage.margin)
            .padding(.top, 16)
            .padding(.bottom, 40)
            .frame(maxWidth: .infinity)
        }
        .id(entry.summaryId)
        .pageBackground(Palette.paper)
        .navigationTitle(book.title)
        .toolbar {
            ToolbarItem(placement: .navigation) {
                BookBackButton()
            }
            ToolbarItem(placement: .principal) {
                Text(book.title)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Palette.ink)
                    .lineLimit(1)
                    .frame(maxWidth: 360)
                    .accessibilityHidden(true)
            }
            ToolbarItem(placement: .primaryAction) {
                Menu {
                    Button("Read It Again") { model.readAgain() }
                    Divider()
                    Button("Remove from My Shelf", role: .destructive) { model.removeSelected() }
                } label: {
                    Image(systemName: "ellipsis")
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .tint(Palette.ink)
                .fixedSize()
                .modifier(CircleChrome())
                .help("Read it again, or remove it from your shelf")
                .accessibilityLabel("More")
                .accessibilityIdentifier("book.more")
            }
        }
        .frame(minWidth: 600, minHeight: 480)
    }

    private var header: some View {
        HStack(alignment: .top, spacing: 20) {
            CoverView(path: book.coverPath, title: book.title, author: book.author, width: 120)
            VStack(alignment: .leading, spacing: 6) {
                Text(book.title)
                    .font(Typeface.title(model.headingFont, size: 28))
                    .foregroundStyle(Palette.ink)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityHeading(.h1)
                    .accessibilityIdentifier("book.title")
                if let subtitle = book.subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(Typeface.text(model.headingFont, size: 17))
                        .italic()
                        .foregroundStyle(Color.quietText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let author = book.author, !author.isEmpty {
                    Text(author)
                        .font(.system(size: 15, weight: .bold))
                        .foregroundStyle(model.accentText(for: colorScheme))
                        .padding(.top, 2)
                }
                let facts = Wording.bookFacts(book)
                if !facts.isEmpty {
                    Text(facts)
                        .font(.system(size: 13))
                        .foregroundStyle(Color.quietText)
                }
                Spacer(minLength: 0)
            }
        }
    }

    private var summary: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .center, spacing: 8) {
                GroupHeading(title: "My Summary", font: model.headingFont)
                    .accessibilityHeading(.h2)
                Spacer()
                Button {
                    model.openReader()
                } label: {
                    Label("Read", systemImage: "book.pages")
                }
                .buttonStyle(PillButtonStyle())
                .disabled(entry.body.isEmpty)
                .help(model.help("Read your summary, without distractions", keys: MenuShortcuts.read))
                Button {
                    model.openWriter()
                } label: {
                    Label(entry.body.isEmpty ? "Write" : "Edit", systemImage: "square.and.pencil")
                }
                .buttonStyle(PillButtonStyle(kind: .primary))
                .help(model.help("Write or edit your summary", keys: MenuShortcuts.write))
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
                            .foregroundStyle(Color.quietText)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        BlocksView(blocks: model.summaryBlocks, font: model.headingFont, size: 15, lineSpacing: 4)
                            .foregroundStyle(Palette.ink)
                    }
                }
                .panel(padding: 22)
                .contentShape(RoundedRectangle(cornerRadius: CardButtonStyle.radius))
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
        VStack(alignment: .leading, spacing: 10) {
            GroupHeading(title: "Reading Dates", font: model.headingFont)
            VStack(alignment: .leading, spacing: 10) {
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
                }
                if let days = Wording.finishedIn(days: entry.days) {
                    Label(days, systemImage: "flag.checkered")
                        .font(.system(size: 13))
                        .foregroundStyle(Color.quietText)
                }
            }
            .panel(padding: 16)
        }
    }
}

/// One date: the day (a pill that opens a calendar), Today, and Clear.
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
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Palette.ink)
                .gridColumnAlignment(.trailing)
            Button {
                picking = true
            } label: {
                Label(date.map { JournalDate.display($0, format: format, calendar: calendar) } ?? "Not set",
                      systemImage: "calendar")
                    .frame(minWidth: 110, alignment: .leading)
            }
            .buttonStyle(PillButtonStyle())
            .accessibilityLabel("\(title): \(date.map { JournalDate.display($0, format: .long, calendar: calendar) } ?? "not set")")
            .accessibilityHint("Opens a calendar")
            .popover(isPresented: $picking, arrowEdge: .bottom) {
                CalendarPopover(date: date, range: range, calendar: calendar) { picked in
                    picking = false
                    commit(picked)
                }
            }
            Button("Today") { commit(today) }
                .buttonStyle(PillButtonStyle())
                .disabled(!range.contains(today) || date.map { calendar.isDate($0, inSameDayAs: today) } == true)
                .accessibilityLabel("\(title) today")
            if date != nil {
                Button {
                    commit(nil)
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(Color.quietText)
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
                    .buttonStyle(PillButtonStyle())
                    .disabled(date == nil)
                Spacer()
                Button("Today") { onPick(calendar.startOfDay(for: Date())) }
                    .buttonStyle(PillButtonStyle())
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
