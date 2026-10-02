import AppKit
import BookshelfKit

/// Asks what to do when leaving the writing page (or quitting) and the
/// summary couldn't be saved. By then the text is already safe in the
/// recovery folder, or on the clipboard; the alert says which, and Show in
/// Finder shows the recovery copy.
@MainActor
enum RescueAlert {
    enum Choice {
        /// Stay on the writing page.
        case keepOpen
        /// Leave (or quit) anyway: the text is safe elsewhere.
        case closeAnyway
    }

    /// Shows the alert, as a sheet on `window` if it's showing, else on
    /// its own, and returns the choice. Keep Open is the default button;
    /// the other reads Quit Anyway when `quitting`, else Close Anyway.
    static func ask(
        _ outcome: WritingSession.Outcome, title: String, quitting: Bool, in window: NSWindow?
    ) async -> Choice {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = Wording.notSaved(title)
        alert.informativeText = Wording.rescue(outcome)
        alert.addButton(withTitle: "Keep Open")
        let close = alert.addButton(withTitle: quitting ? "Quit Anyway" : "Close Anyway")
        close.hasDestructiveAction = true
        alert.buttons.first?.setAccessibilityIdentifier("rescue.keepOpen")
        close.setAccessibilityIdentifier(quitting ? "rescue.quitAnyway" : "rescue.closeAnyway")

        // Show in Finder sits under the message and leaves the alert up.
        var revealer: Revealer?
        if case .rescued(_, let file) = outcome {
            let shower = Revealer(file)
            alert.accessoryView = shower.view
            revealer = shower
        }

        let response: NSApplication.ModalResponse
        if let window, window.isVisible {
            response = await withCheckedContinuation { continuation in
                alert.beginSheetModal(for: window) { continuation.resume(returning: $0) }
            }
        } else {
            response = alert.runModal()
        }
        withExtendedLifetime(revealer) {}
        return response == .alertSecondButtonReturn ? .closeAnyway : .keepOpen
    }

    /// The Show in Finder button, centered in the alert, and its action.
    @MainActor
    private final class Revealer: NSObject {
        let file: URL
        let view: NSView

        init(_ file: URL) {
            self.file = file
            let button = NSButton(title: "Show in Finder", target: nil, action: nil)
            button.bezelStyle = .push
            button.controlSize = .small
            button.font = .systemFont(ofSize: NSFont.systemFontSize(for: .small))
            button.setAccessibilityIdentifier("rescue.showInFinder")
            button.toolTip = "Show the recovery copy in the Finder"
            button.sizeToFit()
            // As wide as the alert's text, so the button lines up under it.
            let width = max(button.frame.width, 220)
            view = NSView(frame: NSRect(x: 0, y: 0, width: width, height: button.frame.height))
            button.setFrameOrigin(NSPoint(x: ((width - button.frame.width) / 2).rounded(), y: 0))
            button.autoresizingMask = [.minXMargin, .maxXMargin]
            view.addSubview(button)
            super.init()
            button.target = self
            button.action = #selector(reveal)
        }

        @objc private func reveal() {
            FolderPicker.reveal(file)
        }
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
