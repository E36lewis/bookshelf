import AppKit
import BookshelfKit

/// Owns the app model, and looks after quitting: the writing page gets its
/// last save in, then the journal is checkpointed.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let model = AppModel()

    func applicationWillFinishLaunching(_ notification: Notification) {
        // Before any window shows, so screenshots never flash the wrong look.
        Appearance.apply(.system, override: model.options.appearance)
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        Task {
            try? await Task.sleep(for: .milliseconds(500))
            showMainWindowIfNeeded()
        }
    }

    /// On macOS 15, a SwiftUI app with an app delegate can finish launching
    /// without opening its window (it happens when launched from the
    /// command line, as UI tests do). Open it the way Window › Bookshelf
    /// would.
    private func showMainWindowIfNeeded() {
        guard !NSApp.windows.contains(where: { $0.isVisible && $0.canBecomeMain }),
              let menu = NSApp.windowsMenu,
              let index = menu.items.firstIndex(where: { $0.title == "Bookshelf" })
        else { return }
        menu.performActionForItem(at: index)
    }

    func applicationDidBecomeActive(_ notification: Notification) {
        // Coming back the next day: "day 12" is now "day 13".
        Task { await model.refreshIfChanged() }
    }

    /// One window: closing it quits, like other single-window apps.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard model.writer != nil else { return .terminateNow }
        Task {
            await model.finishWritingBeforeQuit()
            sender.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }

    func applicationWillTerminate(_ notification: Notification) {
        model.closeJournal()
    }
}
