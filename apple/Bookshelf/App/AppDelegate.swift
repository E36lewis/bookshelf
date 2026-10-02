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
