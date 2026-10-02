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

        // The writing page (⌘↩), and back (WriterTests has the rest).
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: .command)
        XCTAssertTrue(app.textViews["writer.text"].waitForExistence(timeout: 5))
        sleep(1)
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

        /// The audit's contrast check misreads small text (see `allowed`):
        /// these are measured from the pixels instead, and must be 4.5:1.
        func checkContrast(_ name: String, _ element: XCUIElement) {
            guard element.waitForExistence(timeout: 5), let ratio = measuredContrast(element) else {
                found.lines.append("\(name): couldn't measure its contrast")
                return
            }
            let line = "\(name): \(String(format: "%.1f", ratio)):1"
            found.measured.append(line)
            if ratio < 4.5 { found.lines.append("\(line), under 4.5:1") }
        }

        /// Every control on the toolbar has a name VoiceOver can read.
        func checkToolbar(_ screen: String) {
            let toolbar = app.toolbars.firstMatch
            let fields = toolbar.searchFields.allElementsBoundByIndex.map(\.frame)
            let controls = toolbar.buttons.allElementsBoundByIndex + toolbar.menuButtons.allElementsBoundByIndex
                + toolbar.popUpButtons.allElementsBoundByIndex + toolbar.checkBoxes.allElementsBoundByIndex
            var names: [String] = []
            for control in controls {
                // The search field's own buttons are the system's.
                if fields.contains(where: { $0.contains(control.frame) }) { continue }
                let name = control.label.isEmpty ? control.title : control.label
                if name.isEmpty {
                    found.lines.append("\(screen): a toolbar control has no name — id “\(control.identifier)” at \(control.frame)")
                } else {
                    names.append(name)
                }
            }
            found.toolbars.append("\(screen): \(names.joined(separator: ", "))")
        }

        try audit("shelf")
        checkToolbar("shelf")
        entry(app, "The Left Hand of Darkness").click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        try audit("book page")
        checkToolbar("book page")
        checkContrast("Started", app.staticTexts["Started"].firstMatch)
        checkContrast("Finished", app.staticTexts["Finished"].firstMatch)
        checkContrast("Book facts", app.staticTexts.matching(NSPredicate(format: "value ENDSWITH ' pages'")).firstMatch)
        app.typeKey("r", modifierFlags: .command)
        XCTAssertTrue(app.staticTexts["reader.title"].waitForExistence(timeout: 5))
        try audit("reader")
        checkToolbar("reader")

        // The writing page, plain and in focus mode.
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: .command)
        let writing = app.textViews["writer.text"]
        XCTAssertTrue(writing.waitForExistence(timeout: 5))
        try audit("writer")
        checkToolbar("writer")
        checkContrast("Writer status line", app.descendants(matching: .any)["writer.status"].firstMatch)
        app.typeKey("f", modifierFlags: [.command, .shift])
        try audit("writer, focus mode")
        app.typeKey("f", modifierFlags: [.command, .shift])
        app.buttons["page.back"].firstMatch.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))

        // An empty writing page, with its prompts.
        app.typeKey("1", modifierFlags: .command)
        let middlemarch = entry(app, "Middlemarch")
        XCTAssertTrue(middlemarch.waitForExistence(timeout: 5))
        middlemarch.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: .command)
        XCTAssertTrue(writing.waitForExistence(timeout: 5))
        try audit("writer, empty")

        let text = (found.lines.isEmpty ? ["No issues."] : found.lines)
            + ["", "Contrast measured from the pixels (4.5:1 needed):"] + found.measured
            + ["", "Toolbar controls, by name:"] + found.toolbars
            + (found.allowed.isEmpty ? [] : ["", "Allowed:"] + found.allowed)
        let report = XCTAttachment(string: text.joined(separator: "\n"))
        report.name = "accessibility-audit"
        report.lifetime = .keepAlways
        add(report)
        XCTAssertTrue(found.lines.isEmpty, found.lines.joined(separator: "\n"))
    }

    /// macOS has no Dynamic Type; people pick the reading size in Settings ›
    /// Writing › Text size, and the reader follows it at once.
    @MainActor
    func testTheReaderFollowsTheTextSize() throws {
        let app = launch(.demo, .light)
        app.typeKey("2", modifierFlags: .command)
        let piranesi = entry(app, "Piranesi")
        XCTAssertTrue(piranesi.waitForExistence(timeout: 30))
        piranesi.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        app.typeKey("r", modifierFlags: .command)
        let title = app.staticTexts["reader.title"]
        XCTAssertTrue(title.waitForExistence(timeout: 5))
        let paragraph = app.staticTexts.matching(NSPredicate(format: "value BEGINSWITH 'The House is'")).firstMatch
        XCTAssertTrue(paragraph.waitForExistence(timeout: 5))
        let before = (title: title.frame.height, paragraph: paragraph.frame.height)

        app.typeKey(",", modifierFlags: .command)
        let writingTab = app.toolbars.buttons["Writing"]
        XCTAssertTrue(writingTab.waitForExistence(timeout: 5))
        writingTab.click()
        let size = settingsWindow(app).steppers.firstMatch
        XCTAssertTrue(size.waitForExistence(timeout: 5))
        for _ in 0..<6 { size.incrementArrows.firstMatch.click() }
        settingsWindow(app).typeKey("w", modifierFlags: .command)

        // Bigger type: the title and a wrapped paragraph take more room.
        let grew = waitFor(title) { _ in title.frame.height > before.title * 1.2 }
        note("title \(before.title) → \(title.frame.height), paragraph \(before.paragraph) → \(paragraph.frame.height)",
             named: "reader-text-size")
        XCTAssertTrue(grew, "the reader's title didn't grow: \(before.title) → \(title.frame.height)")
        XCTAssertGreaterThan(paragraph.frame.height, before.paragraph * 1.2)
        keep(app, "31-reader-larger-text", .light)
    }

    /// Issues that aren't ours to fix. Each needs a reason.
    private static func allowed(_ issue: XCUIAccessibilityAuditIssue) -> Bool {
        guard let element = issue.element else { return false }
        // The Touch Bar's emoji button, which the system shows whenever text
        // has focus (the writing page). Not ours.
        if element.label == "emoji & symbols" { return true }
        let text = String(describing: element.value ?? "")
        if issue.auditType == .contrast {
            // Apple's standard empty state (ContentUnavailableView) draws its
            // own grey title and description; we don't style it.
            if ["No Book Selected", "Choose a book to see its page, or add one with ⌘N."].contains(text) {
                return true
            }
            // Small text the audit misreads: the reading dates' labels and
            // the book's facts are in the system's primary label color, and
            // the writing page's status line in a gray chosen for 4.5:1
            // (the system's secondary label gray falls short of it). The
            // audit judges small text by its anti-aliased edges; the test
            // measures each from the pixels instead (`checkContrast`) and
            // fails if it's under 4.5:1.
            if text == "Started" || text == "Finished" || text.hasSuffix(" pages") { return true }
            if element.identifier == "writer.status" { return true }
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
        var measured: [String] = []
        var toolbars: [String] = []
    }

    /// The Settings window: its title is the tab that's showing.
    @MainActor
    private func settingsWindow(_ app: XCUIApplication) -> XCUIElement {
        let tabs = ["Profile", "Appearance", "Writing", "Reading Log", "Data"]
        return app.windows.matching(NSPredicate(format: "title IN %@", tabs)).firstMatch
    }
}
