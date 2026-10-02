import AppKit
import BookshelfKit

/// Asks what to do when leaving the writing page (or quitting) and the
/// summary couldn't be saved. By then the text is already safe in the
/// recovery folder, or on the clipboard; the alert says which.
@MainActor
enum RescueAlert {
    enum Choice {
        /// Stay on the writing page.
        case keepOpen
        /// Leave (or quit) anyway: the text is safe elsewhere.
        case closeAnyway
    }

    /// Shows the alert, as a sheet on `window` if it's showing, else on
    /// its own, and returns the choice. Keep Open is the default button.
    static func ask(_ outcome: WritingSession.Outcome, title: String, in window: NSWindow?) async -> Choice {
        var shownFile: String?
        if case .rescued(_, let file) = outcome {
            shownFile = await JournalWorker.displayed(file.path, home: HomeFolder.path)
        }
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = Wording.notSaved(title)
        alert.informativeText = Wording.rescue(outcome, shownFile: shownFile)
        alert.addButton(withTitle: "Keep Open")
        let close = alert.addButton(withTitle: "Close Anyway")
        close.hasDestructiveAction = true
        alert.buttons.first?.setAccessibilityIdentifier("rescue.keepOpen")
        close.setAccessibilityIdentifier("rescue.closeAnyway")

        let response: NSApplication.ModalResponse
        if let window, window.isVisible {
            response = await withCheckedContinuation { continuation in
                alert.beginSheetModal(for: window) { continuation.resume(returning: $0) }
            }
        } else {
            response = alert.runModal()
        }
        return response == .alertSecondButtonReturn ? .closeAnyway : .keepOpen
    }
}

/// Spoken notes for VoiceOver, at a priority: high interrupts what's being
/// read, medium and low wait their turn.
@MainActor
enum Announcer {
    static func say(_ message: String, priority: NSAccessibilityPriorityLevel = .medium) {
        guard let element = NSApp.mainWindow ?? NSApp.keyWindow else { return }
        NSAccessibility.post(element: element, notification: .announcementRequested, userInfo: [
            .announcement: message,
            .priority: priority.rawValue,
        ])
    }
}
