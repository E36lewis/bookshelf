import SwiftUI
import BookshelfKit

struct ContentView: View {
    @Bindable var model: SpikeModel

    var body: some View {
        NavigationSplitView {
            List(model.profiles) { profile in
                Label(profile.name, systemImage: "person.crop.circle")
            }
            .navigationTitle("Profiles")
            .safeAreaInset(edge: .bottom) {
                HStack {
                    TextField("New profile", text: $model.newName)
                        .accessibilityIdentifier("newProfileName")
                        .onSubmit { Task { await model.addProfile() } }
                    Button("Add") { Task { await model.addProfile() } }
                        .disabled(model.newName.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                .padding(10)
            }
        } detail: {
            VStack(alignment: .leading, spacing: 16) {
                Text("Highlighting from the shared core")
                    .font(.title2.bold())
                Text(SpikeSample.highlighted)
                    .font(.system(size: 15, design: .monospaced))
                    .lineSpacing(6)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(16)
                    .background(.background.secondary, in: RoundedRectangle(cornerRadius: 10))
                if let problem = model.problem {
                    Label(problem, systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.red)
                }
                Spacer()
                Text(model.status)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("status")
            }
            .padding(24)
        }
    }
}

/// Sample Markdown, styled with the same rules as the Linux writer.
enum SpikeSample {
    static let text = """
        # What stayed with me
        A **quiet** book about *patience*, with ~~no~~ one `code` word.
        > The best thoughts arrive between chapters.
        - 📚 Emoji first, then **bold** still lines up
        """

    static var highlighted: AttributedString {
        let ns = NSMutableAttributedString(string: text)
        for (kind, range) in highlightRanges(in: text) {
            switch kind {
            case .heading:
                ns.addAttribute(.font, value: NSFont.monospacedSystemFont(ofSize: 19, weight: .bold), range: range)
            case .bold:
                ns.addAttribute(.font, value: NSFont.monospacedSystemFont(ofSize: 15, weight: .bold), range: range)
            case .italic, .quote:
                ns.addAttribute(.obliqueness, value: 0.18, range: range)
            case .code:
                ns.addAttribute(.backgroundColor, value: NSColor.quaternaryLabelColor, range: range)
            case .strike:
                ns.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: range)
            case .syntax:
                ns.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: range)
            }
        }
        return (try? AttributedString(ns, including: \.appKit)) ?? AttributedString(text)
    }
}
