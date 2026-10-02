import XCTest

/// Light or dark, whatever the profile's settings say.
enum Look: String {
    case light, dark
}

/// Which journal the app opens: an empty one, or the demo journal. Both
/// live in a temporary folder inside the app's sandbox.
enum Journal {
    case fresh, demo
}

extension XCTestCase {
    /// Launches the app on a journal of its own, in a fixed-size window.
    @MainActor
    func launch(_ journal: Journal, _ look: Look, _ extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = [
            journal == .demo ? "-BookshelfDemoJournal" : "-BookshelfFreshJournal",
            "-BookshelfAppearance", look.rawValue,
            "-BookshelfWindowSize", "1000x700",
        ] + extra
        app.launch()
        if !app.windows.firstMatch.waitForExistence(timeout: 8) {
            // Shouldn't happen (the app opens its own window); noted so it's seen.
            let note = XCTAttachment(string: "No window after launch: opened it from the Window menu.")
            note.name = "no-window-at-launch"
            note.lifetime = .keepAlways
            add(note)
            app.menuBars.menuBarItems["Window"].click()
            app.menuItems["Bookshelf"].firstMatch.click()
        }
        // If a test can't find what it waits for, this shows what was there.
        sleep(1)
        let shot = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        shot.name = "00-launched-\(name)-\(look.rawValue)"
        shot.lifetime = .deleteOnSuccess
        add(shot)
        return app
    }

    /// A book in the list, by title.
    @MainActor
    func entry(_ app: XCUIApplication, _ title: String) -> XCUIElement {
        app.descendants(matching: .any)["entry.\(title)"].firstMatch
    }

    /// Keeps a screenshot of the app's front window.
    @MainActor
    func keep(_ app: XCUIApplication, _ name: String, _ look: Look) {
        keep(app.windows.firstMatch, name, look)
    }

    @MainActor
    func keep(_ element: XCUIElement, _ name: String, _ look: Look) {
        let shot = XCTAttachment(screenshot: element.screenshot())
        shot.name = "\(name)-\(look.rawValue)"
        shot.lifetime = .keepAlways
        add(shot)
    }

    /// Keeps the whole screen: for menus, which aren't in a window, and
    /// full screen.
    @MainActor
    func keepScreen(_ name: String, _ look: Look) {
        let shot = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        shot.name = "\(name)-\(look.rawValue)"
        shot.lifetime = .keepAlways
        add(shot)
    }

    /// Keeps a note with the test's results.
    func note(_ text: String, named name: String) {
        let attachment = XCTAttachment(string: text)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// Waits until `element`'s value or label satisfies `test`.
    @MainActor
    @discardableResult
    func waitFor(_ element: XCUIElement, timeout: TimeInterval = 5, _ test: (String) -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            if element.exists, test(element.value as? String ?? "") || test(element.label) { return true }
            usleep(200_000)
        } while Date() < deadline
        return false
    }
}
