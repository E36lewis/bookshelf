import AppKit
import SwiftUI
import BookshelfKit

/// The writing page: a quiet, centered column of Markdown in the writing
/// font, with the marks dimmed, a formatting bar, focus mode, and a line
/// at the bottom with the word count and whether it's saved.
struct WriterView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        if let session = model.writer {
            WriterPage(session: session)
        } else {
            Color.clear
        }
    }
}

private struct WriterPage: View {
    let session: WritingSession
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            WritingEditor(
                session: session, look: look, focusMode: model.focusMode,
                checking: model.writingPreferences.values,
                onEditor: { view, attached in model.attach(view, attached) },
                onCheckingChanged: { model.writingPreferences.values = $0 },
                showsStyleProbe: model.options.styleProbe)
            // A new text view for each summary opened.
            .id(ObjectIdentifier(session))
            Divider()
            WriterStatusLine(session: session)
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
            ToolbarItem(placement: .principal) {
                FormatBar(model: model)
            }
            ToolbarItemGroup(placement: .primaryAction) {
                Toggle(isOn: $model.focusMode) {
                    Label("Focus", systemImage: "scope")
                }
                .help("Focus mode (⇧⌘F): dim everything except the sentence you're writing")
                .accessibilityIdentifier("writer.focus")
                Button {
                    model.openReader()
                } label: {
                    Label("Read", systemImage: "book.pages")
                }
                .help("Read your summary (⌘R)")
            }
        }
        .toolbarHidesInFullScreen()
        .onChange(of: session.status) { old, new in
            announceChange(from: old, to: new)
        }
        .frame(minWidth: 600, minHeight: 480)
    }

    private var look: WritingEditor.Look {
        let layout = model.layout
        return WritingEditor.Look(
            font: model.settings?.writingFont ?? .iaDuo,
            size: CGFloat(layout?.fontPx ?? 56.0 / 3) * 0.75,
            column: layout?.columnPoints ?? 540,
            lineSpacing: layout?.wrapPoints ?? 4.5,
            paragraphSpacing: layout?.linePoints ?? 9,
            caretLight: model.palettes?.light.text,
            caretDark: model.palettes?.dark.text)
    }

    /// The status line is read when someone goes to it; only a failed save,
    /// and saving again after one, are announced. Routine autosaves stay
    /// quiet, or VoiceOver would interrupt every pause in typing.
    private func announceChange(from old: WritingSession.Status, to new: WritingSession.Status) {
        var wasFailing = false
        if case .failed = old { wasFailing = true }
        switch new {
        case .failed(let message) where !wasFailing:
            Announcer.say("Not saved: \(message)", priority: .high)
        case .saved where wasFailing:
            Announcer.say("Saved")
        default:
            break
        }
    }
}

/// "65 words · Saved" under the page; "Not saved: …" in red when saving
/// fails.
private struct WriterStatusLine: View {
    let session: WritingSession

    var body: some View {
        HStack(spacing: 6) {
            Text(Wording.words(session.words))
            Text("·").accessibilityHidden(true)
            if case .failed = session.status {
                Label(session.statusText, systemImage: "exclamationmark.triangle.fill")
                    .foregroundStyle(Color.errorText)
            } else {
                Text(session.statusText)
            }
            Spacer()
        }
        .font(.callout)
        .foregroundStyle(Color.quietText)
        .padding(.horizontal, 20)
        .padding(.vertical, 8)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("writer.status")
    }
}

/// The formatting buttons above the page; the Format menu has the same.
private struct FormatBar: View {
    let model: AppModel

    var body: some View {
        HStack(spacing: 10) {
            ControlGroup {
                ForEach(FormatCommand.groups[0], id: \.self) { button($0) }
            }
            Menu {
                ForEach(FormatCommand.headings, id: \.self) { command in
                    Button(command.title) { model.format(command) }
                }
            } label: {
                Label("Heading", systemImage: "textformat.size")
            }
            .tint(.primary)
            .help("Heading")
            .accessibilityLabel("Heading")
            .accessibilityIdentifier("format.heading")
            ControlGroup {
                ForEach(FormatCommand.groups[2], id: \.self) { button($0) }
            }
            button(.link)
        }
        .labelStyle(.iconOnly)
        .fixedSize()
    }

    private func button(_ command: FormatCommand) -> some View {
        Button {
            model.format(command)
        } label: {
            Label(command.title, systemImage: command.systemImage)
        }
        .help(model.helpText(for: command))
        .accessibilityIdentifier("format.\(command)")
    }
}

extension Color {
    /// Quiet gray text that still reads clearly: system secondary label
    /// gray falls just short of 4.5:1 on the writing page.
    static let quietText = Color(nsColor: NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(white: 0.72, alpha: 1)
            : NSColor(white: 0.33, alpha: 1)
    })

    /// Red text that reads clearly on the window: darker than system red
    /// in light windows, lighter in dark ones (4.5:1 or better on both).
    static let errorText = Color(nsColor: NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(srgbRed: 1, green: 0.45, blue: 0.42, alpha: 1)
            : NSColor(srgbRed: 0.75, green: 0.11, blue: 0.09, alpha: 1)
    })
}
