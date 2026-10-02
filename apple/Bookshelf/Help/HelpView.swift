import SwiftUI
import BookshelfKit

/// Help › Bookshelf Help (⌘?): the user manual, from the same source as
/// every other Bookshelf, with its contents down the side.
struct HelpView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme

    @State private var manual: LaidOutManual?
    @State private var selection: String?

    /// The manual's sections, laid out once.
    struct LaidOutManual {
        var contents: [ManualEntry]
        var intro: [Block]
        var sections: [Part]

        struct Part: Identifiable {
            /// The anchor the contents link to.
            var id: String
            var blocks: [Block]
        }
    }

    var body: some View {
        NavigationSplitView {
            List(manual?.contents ?? [], selection: $selection) { entry in
                Text(entry.title)
                    .font(entry.nested ? .body : .body.weight(.medium))
                    .foregroundStyle(entry.nested ? .secondary : .primary)
                    .padding(.leading, entry.nested ? 14 : 0)
                    .tag(entry.anchor)
            }
            .listStyle(.sidebar)
            // Wide enough for the longest titles ("Writing and reading your summary").
            .navigationSplitViewColumnWidth(min: 220, ideal: 280, max: 340)
            .accessibilityLabel("Contents")
            .accessibilityIdentifier("help.contents")
        } detail: {
            ScrollViewReader { proxy in
                ScrollView {
                    if let manual {
                        VStack(alignment: .leading, spacing: 0) {
                            BlocksView(blocks: manual.intro, font: .serif, size: 15)
                                .id("top")
                            ForEach(manual.sections) { section in
                                BlocksView(blocks: section.blocks, font: .serif, size: 15)
                                    .padding(.top, 8)
                                    .id(section.id)
                            }
                        }
                        .textSelection(.enabled)
                        .frame(maxWidth: 680, alignment: .leading)
                        .padding(.horizontal, 40)
                        .padding(.vertical, 32)
                        .frame(maxWidth: .infinity)
                    }
                }
                .background(Color(nsColor: .textBackgroundColor))
                .onChange(of: selection) { _, anchor in
                    guard let anchor else { return }
                    withAnimation { proxy.scrollTo(anchor, anchor: .top) }
                }
            }
        }
        .navigationTitle("Bookshelf Help")
        .tint(model.accent(for: colorScheme))
        .frame(minWidth: 720, minHeight: 480)
        .task {
            guard manual == nil else { return }
            manual = await Self.layOut()
        }
    }

    private static func layOut() async -> LaidOutManual {
        let manual = await JournalWorker.userManual()
        let intro = await JournalWorker.blocks(for: manual.intro)
        var sections: [LaidOutManual.Part] = []
        // The sidebar is the contents, so the manual's own list isn't repeated.
        for section in manual.sections where section.anchor != "contents" {
            let blocks = await JournalWorker.blocks(for: section.markdown)
            sections.append(LaidOutManual.Part(id: section.anchor, blocks: blocks))
        }
        return LaidOutManual(contents: manual.contents, intro: intro, sections: sections)
    }
}
