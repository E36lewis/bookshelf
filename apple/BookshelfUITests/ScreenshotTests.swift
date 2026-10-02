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
        XCTAssertTrue(app.textFields["addBook.query"].waitForNonExistence(timeout: 5))

        // Settings (⌘,), every tab.
        app.typeKey(",", modifierFlags: .command)
        // Settings opens on the tab it was left on (Data, after the other look's run).
        let profileTab = app.toolbars.buttons["Profile"]
        XCTAssertTrue(profileTab.waitForExistence(timeout: 5))
        profileTab.click()
        XCTAssertTrue(app.textFields["settings.name"].waitForExistence(timeout: 5))
        let settings = app.windows.containing(.textField, identifier: "settings.name").firstMatch
        keep(settings, "12-settings-profile", look)
        for (index, tab) in ["Appearance", "Writing", "Reading Log", "Data"].enumerated() {
            app.toolbars.buttons[tab].click()
            sleep(1)
            let slug = tab.lowercased().replacingOccurrences(of: " ", with: "-")
            keep(settingsWindow(app), "\(13 + index)-settings-\(slug)", look)
        }
        settingsWindow(app).typeKey("w", modifierFlags: .command)

        // Help › Bookshelf Help.
        let helpMenu = app.menuBars.menuBarItems["Help"]
        helpMenu.click()
        helpMenu.menuItems["Bookshelf Help"].click()
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
        sleep(1)
        keepScreen("19-undo-menu", look)
        // AppKit retitles the item ("Undo Remove “…”") as the menu opens;
        // the accessibility tree keeps its plain title, so find it by action.
        // The screenshot above shows the name.
        let undo = app.menuBars.menuBarItems["Edit"].menuItems["undo:"]
        XCTAssertTrue(undo.waitForExistence(timeout: 5))
        XCTAssertTrue(undo.isEnabled)
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
            // Let pages finish fading in: the contrast check samples pixels.
            sleep(2)
            try app.performAccessibilityAudit { issue in
                let element = issue.element.map {
                    "type \($0.elementType.rawValue) label “\($0.label)” title “\($0.title)” "
                        + "value “\(String(describing: $0.value ?? ""))” id “\($0.identifier)” at \($0.frame)"
                }
                let line = "\(screen): [\(issue.auditType.rawValue)] \(issue.compactDescription) — \(element ?? "no element")"
                if Self.allowed(issue) {
                    found.allowed.append(line)
                } else {
                    found.lines.append(line)
                }
                // Collected, and failed below, so one run reports them all.
                return true
            }
        }

        try audit("shelf")
        entry(app, "The Left Hand of Darkness").click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        try audit("book page")
        app.typeKey("r", modifierFlags: .command)
        XCTAssertTrue(app.staticTexts["reader.title"].waitForExistence(timeout: 5))
        try audit("reader")

        let text = (found.lines.isEmpty ? ["No issues."] : found.lines)
            + (found.allowed.isEmpty ? [] : ["", "Allowed:"] + found.allowed)
        let report = XCTAttachment(string: text.joined(separator: "\n"))
        report.name = "accessibility-audit"
        report.lifetime = .keepAlways
        add(report)
        XCTAssertTrue(found.lines.isEmpty, found.lines.joined(separator: "\n"))
    }

    /// Issues that aren't ours to fix. Each needs a reason.
    private static func allowed(_ issue: XCUIAccessibilityAuditIssue) -> Bool {
        guard let element = issue.element else { return false }
        let text = String(describing: element.value ?? "")
        if issue.auditType == .contrast {
            // Apple's standard empty state (ContentUnavailableView) draws its
            // own grey title and description; we don't style it.
            if ["No Book Selected", "Choose a book to see its page, or add one with ⌘N."].contains(text) {
                return true
            }
            // Near misses, not failures: a row's secondary text over the
            // list's own background. Kept in the report.
            if issue.compactDescription.localizedCaseInsensitiveContains("nearly passed") { return true }
        }
        switch element.elementType {
        case .popUpButton where !element.label.isEmpty:
            // SwiftUI's Menu (the profile menu, the book's ⋯ menu) is a
            // pop-up button that opens with AXShowMenu rather than AXPress,
            // and its label isn't exposed as the AX description. VoiceOver
            // reads and opens both; nothing in our code can change this.
            return issue.auditType == .action || issue.auditType == .sufficientElementDescription
        case .group where element.label.isEmpty && element.identifier.isEmpty:
            // SwiftUI's own layout containers (split view columns, scroll
            // and stack containers): they hold labelled content, none of
            // their own, and our code can't name them.
            return issue.auditType == .sufficientElementDescription || issue.auditType == .parentChild
        case .touchBar:
            // The system's Touch Bar placeholder for the window; empty.
            return true
        default:
            return false
        }
    }

    // MARK: Helpers

    /// The audit's findings, collected from its issue handler.
    private final class Found {
        var lines: [String] = []
        var allowed: [String] = []
    }

    /// The Settings window: its title is the tab that's showing.
    @MainActor
    private func settingsWindow(_ app: XCUIApplication) -> XCUIElement {
        let tabs = ["Profile", "Appearance", "Writing", "Reading Log", "Data"]
        return app.windows.matching(NSPredicate(format: "title IN %@", tabs)).firstMatch
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
