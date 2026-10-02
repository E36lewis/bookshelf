import AppKit
import SwiftUI
import BookshelfKit

/// The main window: the welcome page, the shelves, or a book's reading
/// or writing page.
struct RootView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.undoManager) private var undoManager
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        content
            .tint(model.accent(for: colorScheme))
            .onAppear { model.undoManager = model.undoManager ?? undoManager }
            .onChange(of: undoManager) { _, manager in model.undoManager = model.undoManager ?? manager }
            .alert(
                model.problem?.title ?? "",
                isPresented: Binding(get: { model.problem != nil }, set: { if !$0 { model.problem = nil } }),
                presenting: model.problem
            ) { _ in
                Button("OK") {}
            } message: { problem in
                Text(problem.message)
            }
            .background(WindowAccessor { window in
                // Edit › Undo asks the window for its undo manager: register
                // removals there, so the menu names them.
                if let windowUndo = window.undoManager { model.undoManager = windowUndo }
                if let size = model.options.windowSize {
                    window.setContentSize(size)
                    window.center()
                }
            })
    }

    @ViewBuilder
    private var content: some View {
        switch model.phase {
        case .opening:
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .frame(minWidth: 760, minHeight: 520)
        case .failed(let message):
            StartupErrorView(message: message)
        case .welcome:
            WelcomeView()
        case .ready:
            switch model.page {
            case .shelves: MainSplitView()
            case .reader: ReaderView()
            case .writer: WriterView()
            }
        }
    }
}

/// Shown instead of the shelves when the journal can't be opened, so the
/// window explains itself.
struct StartupErrorView: View {
    let message: String

    var body: some View {
        ContentUnavailableView {
            Label("Bookshelf Couldn't Start", systemImage: "exclamationmark.triangle")
        } description: {
            Text(message + "\n\nIf your journal file is damaged, daily backups are kept in the “backups” folder beside it.")
        }
        .frame(minWidth: 760, minHeight: 520)
    }
}

/// Hands over the hosting `NSWindow` once, when the view lands in it.
struct WindowAccessor: NSViewRepresentable {
    let configure: @MainActor (NSWindow) -> Void

    func makeNSView(context: Context) -> NSView {
        let view = WindowReportingView()
        view.onWindow = configure
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}

    final class WindowReportingView: NSView {
        var onWindow: (@MainActor (NSWindow) -> Void)?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard let window, let onWindow else { return }
            self.onWindow = nil
            // After SwiftUI has finished sizing the window itself.
            Task { @MainActor in onWindow(window) }
        }
    }
}
