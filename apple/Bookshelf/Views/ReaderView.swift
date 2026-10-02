import AppKit
import SwiftUI
import BookshelfKit

/// The summary on its own, for distraction-free reading: one centered
/// column in the profile's reading font and size. Full screen (⌃⌘F) hides
/// everything else; Esc leaves full screen, then the page.
struct ReaderView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        if let entry = model.selectedEntry {
            page(entry)
        } else {
            Color.clear.onAppear { model.closePage() }
        }
    }

    private func page(_ entry: EntryDetail) -> some View {
        let size = model.settings?.textSize ?? 15
        let width = model.layout?.columnPoints ?? 540
        return ScrollView {
            VStack(alignment: .leading, spacing: 6) {
                Text(entry.book.title)
                    .font(Typeface.title(model.headingFont, size: size * 2.1))
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityHeading(.h1)
                    .accessibilityIdentifier("reader.title")
                if let author = entry.book.author, !author.isEmpty {
                    Text(author)
                        .font(.system(size: size * 1.1, weight: .semibold))
                        .foregroundStyle(model.accentText(for: colorScheme))
                }
                Group {
                    if model.summaryBlocks.isEmpty {
                        Text("Nothing written yet.")
                            .font(Typeface.text(model.headingFont, size: size))
                            .italic()
                            .foregroundStyle(.tertiary)
                    } else {
                        // Generous leading: the reading page is for reading.
                        BlocksView(blocks: model.summaryBlocks, font: model.headingFont, size: size, lineSpacing: size * 0.4)
                    }
                }
                .padding(.top, 28)
            }
            .textSelection(.enabled)
            .frame(maxWidth: width, alignment: .leading)
            .padding(.horizontal, 40)
            .padding(.top, 56)
            .padding(.bottom, 160)
            .frame(maxWidth: .infinity)
        }
        .background(Color(nsColor: .textBackgroundColor))
        .navigationTitle(entry.book.title)
        .navigationSubtitle("Reading")
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Button {
                    leaveFullScreenOrPage()
                } label: {
                    Label("Back to Shelf", systemImage: "chevron.backward")
                }
                // Esc: out of full screen first, then back to the shelf.
                .keyboardShortcut(.cancelAction)
                .help("Back to your shelf (Esc)")
                .accessibilityIdentifier("page.back")
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    model.openWriter()
                } label: {
                    Label("Edit", systemImage: "square.and.pencil")
                }
                .help("Edit this summary (⌘↩)")
            }
        }
        .frame(minWidth: 600, minHeight: 480)
    }

    private func leaveFullScreenOrPage() {
        if let window = NSApp.keyWindow, window.styleMask.contains(.fullScreen) {
            window.toggleFullScreen(nil)
        } else {
            model.closePage()
        }
    }
}

/// A plain writing page until the real writer arrives (Phase 4): the text
/// in the profile's writing font, saved a moment after typing stops and
/// again on leaving.
struct WriterView: View {
    @Environment(AppModel.self) private var model
    @FocusState private var focused: Bool

    var body: some View {
        if let session = model.writer {
            page(session)
        } else {
            Color.clear
        }
    }

    private func page(_ session: WriterSession) -> some View {
        let settings = model.settings
        let size = settings?.textSize ?? 15
        let font = Typeface.writing(settings?.writingFont ?? .iaDuo, size: size)
        let layout = model.layout
        return VStack(spacing: 0) {
            ZStack(alignment: .topLeading) {
                TextEditor(text: Binding(get: { session.text }, set: { session.update($0) }))
                    .font(font)
                    .lineSpacing(layout?.wrapPoints ?? 4)
                    .scrollContentBackground(.hidden)
                    .focused($focused)
                    .accessibilityLabel("Summary of \(session.title)")
                    .accessibilityIdentifier("writer.text")
                if session.text.isEmpty {
                    Text(session.prompts)
                        .font(font)
                        .lineSpacing(layout?.wrapPoints ?? 4)
                        .foregroundStyle(.tertiary)
                        // Lines up with the text view's own inset.
                        .padding(.leading, 5)
                        .allowsHitTesting(false)
                        .accessibilityHidden(true)
                }
            }
            .frame(maxWidth: (layout?.columnPoints ?? 540) + 10)
            .padding(.horizontal, 32)
            .padding(.top, 28)
            .frame(maxWidth: .infinity)
            Divider()
            HStack(spacing: 6) {
                Text(Wording.words(session.words))
                Text("·").accessibilityHidden(true)
                if case .failed = session.status {
                    Label(session.statusText, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                } else {
                    Text(session.statusText)
                }
                Spacer()
            }
            .font(.callout)
            .foregroundStyle(.secondary)
            .padding(.horizontal, 20)
            .padding(.vertical, 8)
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("writer.status")
        }
        .background(Color(nsColor: .textBackgroundColor))
        .navigationTitle(session.title)
        .navigationSubtitle("Writing")
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Button {
                    model.closePage()
                } label: {
                    Label("Done", systemImage: "chevron.backward")
                }
                .help("Save and go back to your shelf")
                .accessibilityIdentifier("page.back")
            }
            ToolbarItem(placement: .primaryAction) {
                Button {
                    model.openReader()
                } label: {
                    Label("Read", systemImage: "book.pages")
                }
                .help("Read your summary (⌘R)")
            }
        }
        .onAppear { focused = true }
        .frame(minWidth: 600, minHeight: 480)
    }
}
