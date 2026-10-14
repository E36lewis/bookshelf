import SwiftUI
import BookshelfKit

/// Bookshelf for the Mac: one window with your shelves (laid out as on
/// Linux, one page at a time), Settings, and the manual.
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
        .defaultSize(width: 1000, height: 760)
        // The toolbar is each page's header, as on Linux; the page says
        // where you are, so the window's title isn't shown in it.
        .windowToolbarStyle(.unified(showsTitle: false))
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
