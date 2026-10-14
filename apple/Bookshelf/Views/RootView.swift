import AppKit
import SwiftUI
import BookshelfKit

/// The main window: one page at a time, as on Linux. The profiles (or
/// the welcome), a profile's shelves, a book's page, and its reading or
/// writing page; each page's header is the window's toolbar.
struct RootView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.undoManager) private var undoManager
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        @Bindable var model = model
        content
            .sheet(isPresented: $model.isAddingBook) {
                AddBookSheet()
            }
            // Outside the sheet, so it has the accent too.
            .tint(model.accent(for: colorScheme))
            .environment(\.journalAccent, model.journalAccent(for: colorScheme))
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
                model.mainWindow = window
                // The header is flat, as on Linux: no line under it.
                window.titlebarSeparatorStyle = .none
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
                .frame(minWidth: 600, minHeight: 480)
                .background(Palette.paper)
        case .failed(let message):
            StartupErrorView(message: message)
        case .welcome:
            ProfilesView()
        case .ready:
            switch model.page {
            case .profiles: ProfilesView()
            case .shelves: ShelfPage()
            case .book: BookScreen()
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
        .frame(minWidth: 600, minHeight: 480)
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
