import SwiftUI
import BookshelfKit

/// Bookshelf for the Mac: one window with your shelves, Settings, and the
/// manual.
@main
struct BookshelfApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    var body: some Scene {
        // "Bookshelf Preview" in any build but a release (Window menu, About).
        Window(BuildChannel.appName(BuildChannel.current), id: SceneID.main) {
            RootView()
                .environment(appDelegate.model)
                .task { await appDelegate.model.start() }
        }
        .defaultSize(width: 1180, height: 760)
        .commands {
            BookshelfCommands(model: appDelegate.model)
        }

        Window("Bookshelf Help", id: SceneID.help) {
            HelpView()
                .environment(appDelegate.model)
        }
        .defaultSize(width: 960, height: 680)

        Settings {
            SettingsView()
                .environment(appDelegate.model)
        }
    }
}
