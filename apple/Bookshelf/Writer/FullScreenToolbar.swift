import AppKit
import SwiftUI

extension View {
    /// In full screen, the toolbar slides away while this page shows, and
    /// comes back when the pointer reaches the top of the screen, as in
    /// other writing apps. Elsewhere (the shelves, with their search field)
    /// it stays put.
    func toolbarHidesInFullScreen() -> some View {
        background(FullScreenToolbar())
    }
}

/// Watches its window entering and leaving full screen, and asks the
/// system to auto-hide the toolbar while it's there.
private struct FullScreenToolbar: NSViewRepresentable {
    func makeNSView(context: Context) -> Watcher { Watcher() }
    func updateNSView(_ nsView: Watcher, context: Context) {}

    static func dismantleNSView(_ nsView: Watcher, coordinator: ()) {
        nsView.stopWatching()
        Watcher.showToolbar()
    }

    final class Watcher: NSView {
        private var observers: [NSObjectProtocol] = []

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            stopWatching()
            guard let window else { return }
            let center = NotificationCenter.default
            observers = [
                center.addObserver(forName: NSWindow.didEnterFullScreenNotification, object: window, queue: .main) { _ in
                    MainActor.assumeIsolated { Watcher.hideToolbar() }
                },
                center.addObserver(forName: NSWindow.willExitFullScreenNotification, object: window, queue: .main) { _ in
                    MainActor.assumeIsolated { Watcher.showToolbar() }
                },
            ]
            if window.styleMask.contains(.fullScreen) { Self.hideToolbar() }
        }

        func stopWatching() {
            for observer in observers { NotificationCenter.default.removeObserver(observer) }
            observers = []
        }

        /// The options before the toolbar was hidden, to go back to.
        private static var before: NSApplication.PresentationOptions?

        /// Auto-hiding the toolbar is only allowed in full screen with the
        /// menu bar auto-hiding too, which is how the system sets full
        /// screen up (`currentSystemPresentationOptions`, what's in effect;
        /// `presentationOptions` is only what the app asked for).
        static func hideToolbar() {
            let current = NSApp.currentSystemPresentationOptions
            guard before == nil, current.isSuperset(of: [.fullScreen, .autoHideMenuBar]),
                  !current.contains(.autoHideToolbar)
            else { return }
            before = NSApp.presentationOptions
            NSApp.presentationOptions = current.union(.autoHideToolbar)
        }

        static func showToolbar() {
            guard let before else { return }
            self.before = nil
            NSApp.presentationOptions = before
        }
    }
}
