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

        /// Only valid in full screen with the menu bar hiding too, which is
        /// how the system sets full screen up; anything else is left alone.
        static func hideToolbar() {
            let options = NSApp.presentationOptions
            guard options.isSuperset(of: [.fullScreen, .autoHideMenuBar]), !options.contains(.autoHideToolbar)
            else { return }
            NSApp.presentationOptions = options.union(.autoHideToolbar)
        }

        static func showToolbar() {
            let options = NSApp.presentationOptions
            guard options.contains(.autoHideToolbar) else { return }
            NSApp.presentationOptions = options.subtracting(.autoHideToolbar)
        }
    }
}
