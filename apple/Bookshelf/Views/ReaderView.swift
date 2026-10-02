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
        .toolbarHidesInFullScreen()
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
