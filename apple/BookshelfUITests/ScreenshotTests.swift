import XCTest

/// Drives the app like a person would, on a journal of its own inside the
/// app's sandbox, and keeps screenshots of every main screen in light and
/// dark, so the Mac UI can be reviewed from CI without a Mac.
///
/// Nothing here goes online: the demo journal's books need no network, and
/// Add a Book is only shown empty.
final class ScreenshotTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    // MARK: First run

    @MainActor func testFirstRunLight() throws { try firstRun(.light) }
    @MainActor func testFirstRunDark() throws { try firstRun(.dark) }

    @MainActor
    private func firstRun(_ look: Look) throws {
        let app = launch(.fresh, look)
        let name = app.textFields["profile.name"]
        XCTAssertTrue(name.waitForExistence(timeout: 20))
        keep(app, "01-first-run", look)

        name.click()
        name.typeText("Avery")
        app.buttons["profile.create"].click()
        // A new profile opens on an empty Reading shelf.
        XCTAssertTrue(app.buttons["empty.addBook"].waitForExistence(timeout: 10))
        keep(app, "02-empty-shelf", look)
    }

    // MARK: The tour

    @MainActor func testTourLight() throws { try tour(.light) }
    @MainActor func testTourDark() throws { try tour(.dark) }

    @MainActor
    private func tour(_ look: Look) throws {
        let app = launch(.demo, look)
        let leftHand = entry(app, "The Left Hand of Darkness")
        XCTAssertTrue(leftHand.waitForExistence(timeout: 30))

        // The shelves (⌘1, ⌘2, ⌘3).
        keep(app, "03-shelf-reading", look)
        app.typeKey("2", modifierFlags: .command)
        XCTAssertTrue(entry(app, "Piranesi").waitForExistence(timeout: 5))
        keep(app, "04-shelf-finished", look)
        app.typeKey("3", modifierFlags: .command)
        XCTAssertTrue(entry(app, "Moby-Dick").waitForExistence(timeout: 5))
        keep(app, "05-shelf-eventually", look)

        // A book's page.
        app.typeKey("2", modifierFlags: .command)
        let piranesi = entry(app, "Piranesi")
        XCTAssertTrue(piranesi.waitForExistence(timeout: 5))
        piranesi.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        keep(app, "06-book-page", look)

        // The reader (⌘R), and back (Esc).
        app.typeKey("r", modifierFlags: .command)
        XCTAssertTrue(app.staticTexts["reader.title"].waitForExistence(timeout: 5))
        keep(app, "07-reader", look)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))

        // The plain writing page (⌘↩), and back.
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: .command)
        XCTAssertTrue(app.textViews["writer.text"].waitForExistence(timeout: 5))
        keep(app, "08-writer", look)
        app.buttons["page.back"].firstMatch.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))

        // Search (⌘F) across every shelf, then a search with no matches.
        app.typeKey("f", modifierFlags: .command)
        app.typeText("le guin")
        XCTAssertTrue(entry(app, "A Wizard of Earthsea").waitForExistence(timeout: 5))
        XCTAssertTrue(entry(app, "The Left Hand of Darkness").exists)
        keep(app, "09-search-results", look)
        app.typeKey("a", modifierFlags: .command)
        app.typeText("zzyzx")
        XCTAssertTrue(entry(app, "A Wizard of Earthsea").waitForNonExistence(timeout: 5))
        keep(app, "10-search-no-matches", look)
        app.typeKey("a", modifierFlags: .command)
        app.typeKey(XCUIKeyboardKey.delete.rawValue, modifierFlags: [])
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])

        // Add a Book (⌘N): empty, since CI has no network for it.
        app.typeKey("1", modifierFlags: .command)
        XCTAssertTrue(leftHand.waitForExistence(timeout: 5))
        app.typeKey("n", modifierFlags: .command)
        XCTAssertTrue(app.textFields["addBook.query"].waitForExistence(timeout: 5))
        keep(app, "11-add-book", look)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])

        // Settings (⌘,), every tab.
        app.typeKey(",", modifierFlags: .command)
        XCTAssertTrue(app.textFields["settings.name"].waitForExistence(timeout: 5))
        let settings = app.windows.containing(.textField, identifier: "settings.name").firstMatch
        keep(settings, "12-settings-profile", look)
        for (index, tab) in ["Appearance", "Writing", "Reading Log", "Data"].enumerated() {
            settingsWindow(app).toolbars.buttons[tab].click()
            sleep(1)
            let slug = tab.lowercased().replacingOccurrences(of: " ", with: "-")
            keep(settingsWindow(app), "\(13 + index)-settings-\(slug)", look)
        }
        settingsWindow(app).typeKey("w", modifierFlags: .command)

        // Help › Bookshelf Help.
        app.menuBars.menuBarItems["Help"].click()
        app.menuItems["Bookshelf Help"].click()
        let help = app.windows.matching(NSPredicate(format: "title == %@", "Bookshelf Help")).firstMatch
        XCTAssertTrue(help.waitForExistence(timeout: 10))
        XCTAssertTrue(help.staticTexts["Make a profile"].waitForExistence(timeout: 10))
        keep(help, "17-manual", look)
        help.typeKey("w", modifierFlags: .command)

        // Remove a book (⌘⌫); Edit › Undo shows what it would undo, and
        // brings the book back.
        let left = entry(app, "The Left Hand of Darkness")
        XCTAssertTrue(left.waitForExistence(timeout: 5))
        left.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        app.typeKey(XCUIKeyboardKey.delete.rawValue, modifierFlags: .command)
        XCTAssertTrue(app.buttons["removed.undo"].waitForExistence(timeout: 5))
        XCTAssertFalse(left.exists)
        keep(app, "18-removed", look)
        app.menuBars.menuBarItems["Edit"].click()
        let undo = app.menuItems["Undo Remove “The Left Hand of Darkness”"]
        XCTAssertTrue(undo.waitForExistence(timeout: 5))
        keepScreen("19-undo-menu", look)
        undo.click()
        XCTAssertTrue(entry(app, "The Left Hand of Darkness").waitForExistence(timeout: 5))
    }

    // MARK: Accessibility

    /// Xcode's automated accessibility audit on each main screen. Issues
    /// are kept as an attachment; known exceptions (see `allowed`) don't
    /// fail the test.
    @MainActor
    func testAccessibilityAudit() throws {
        let app = launch(.demo, .light)
        XCTAssertTrue(entry(app, "The Left Hand of Darkness").waitForExistence(timeout: 30))
        let found = Found()

        func audit(_ screen: String) throws {
            try app.performAccessibilityAudit { issue in
                let element = issue.element.map { "\($0.elementType.rawValue) “\($0.label)” \($0.identifier)" }
                found.lines.append(
                    "\(screen): [\(issue.auditType.rawValue)] \(issue.compactDescription) — \(element ?? "no element")")
                return Self.allowed(issue)
            }
        }

        try audit("shelf")
        entry(app, "The Left Hand of Darkness").click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        try audit("book page")
        app.typeKey("r", modifierFlags: .command)
        XCTAssertTrue(app.staticTexts["reader.title"].waitForExistence(timeout: 5))
        try audit("reader")

        let report = XCTAttachment(string: found.lines.isEmpty ? "No issues." : found.lines.joined(separator: "\n"))
        report.name = "accessibility-audit"
        report.lifetime = .keepAlways
        add(report)
    }

    /// Issues that aren't ours to fix. Each needs a reason.
    private static func allowed(_ issue: XCUIAccessibilityAuditIssue) -> Bool {
        false
    }

    // MARK: Helpers

    /// The audit's findings, collected from its issue handler.
    private final class Found {
        var lines: [String] = []
    }

    /// The Settings window: the one with the Appearance tab.
    @MainActor
    private func settingsWindow(_ app: XCUIApplication) -> XCUIElement {
        app.windows.containing(NSPredicate(format: "label == %@", "Appearance")).firstMatch
    }

    enum Look: String {
        case light, dark
    }

    enum Journal {
        case fresh, demo
    }

    @MainActor
    private func launch(_ journal: Journal, _ look: Look) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = [
            journal == .demo ? "-BookshelfDemoJournal" : "-BookshelfFreshJournal",
            "-BookshelfAppearance", look.rawValue,
            "-BookshelfWindowSize", "1000x700",
        ]
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

    @MainActor
    private func entry(_ app: XCUIApplication, _ title: String) -> XCUIElement {
        app.descendants(matching: .any)["entry.\(title)"].firstMatch
    }

    /// Keeps a screenshot of the app's front window.
    @MainActor
    private func keep(_ app: XCUIApplication, _ name: String, _ look: Look) {
        keep(app.windows.firstMatch, name, look)
    }

    @MainActor
    private func keep(_ element: XCUIElement, _ name: String, _ look: Look) {
        let shot = XCTAttachment(screenshot: element.screenshot())
        shot.name = "\(name)-\(look.rawValue)"
        shot.lifetime = .keepAlways
        add(shot)
    }

    /// Keeps the whole screen: for menus, which aren't in a window.
    @MainActor
    private func keepScreen(_ name: String, _ look: Look) {
        let shot = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        shot.name = "\(name)-\(look.rawValue)"
        shot.lifetime = .keepAlways
        add(shot)
    }
}
