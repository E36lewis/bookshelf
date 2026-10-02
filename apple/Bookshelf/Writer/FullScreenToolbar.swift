import AppKit
import SwiftUI

extension View {
    /// In full screen, the toolbar slides away while this page shows, and
    /// comes back when the pointer reaches the top of the screen, as in
    /// other writing apps. Elsewhere (the shelves, with their search field)
    /// it stays put.
    func toolbarHidesInFullScreen() -> some View {
        modifier(FullScreenToolbarHiding())
    }
}

/// SwiftUI's own setting on macOS 15; on macOS 14, the system's
/// presentation options (SwiftUI owns the window's delegate, where this
/// would otherwise go).
private struct FullScreenToolbarHiding: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            content.windowToolbarFullScreenVisibility(.onHover)
        } else {
            content.background(FullScreenToolbar())
        }
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
                center.addObserver(forName: NSWindow.didEnterFullScreenNotification, object: window, queue: .main) { [weak window] _ in
                    MainActor.assumeIsolated { if let window { Watcher.hideToolbar(in: window) } }
                },
                center.addObserver(forName: NSWindow.willExitFullScreenNotification, object: window, queue: .main) { _ in
                    MainActor.assumeIsolated { Watcher.showToolbar() }
                },
            ]
            if window.styleMask.contains(.fullScreen) { Self.hideToolbar(in: window) }
        }

        func stopWatching() {
            for observer in observers { NotificationCenter.default.removeObserver(observer) }
            observers = []
        }

        /// The options before the toolbar was hidden, to go back to.
        private static var before: NSApplication.PresentationOptions?

        /// Only while `window` is in full screen: auto-hiding the toolbar is
        /// allowed only together with full screen and an auto-hiding menu
        /// bar, as here.
        static func hideToolbar(in window: NSWindow) {
            guard before == nil, window.styleMask.contains(.fullScreen) else { return }
            before = NSApp.presentationOptions
            NSApp.presentationOptions = [.fullScreen, .autoHideMenuBar, .autoHideToolbar]
        }

        static func showToolbar() {
            guard let before else { return }
            self.before = nil
            NSApp.presentationOptions = before
        }
    }
}
