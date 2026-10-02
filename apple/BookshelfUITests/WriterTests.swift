import AppKit
import XCTest

/// The writing page, driven like a person would: highlighting as you type,
/// formatting and its undo, focus mode, full screen, the menus, saving,
/// and the rescue path when saving fails. Screenshots of each, light and
/// dark, on the demo journal.
final class WriterTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    // MARK: Screenshots

    @MainActor func testWriterLight() throws { try writerTour(.light) }
    @MainActor func testWriterDark() throws { try writerTour(.dark) }

    @MainActor
    private func writerTour(_ look: Look) throws {
        let app = launch(.demo, look)
        let text = openWriter(app, "The Left Hand of Darkness")
        keep(app, "20-writer-markdown", look)
        // The status line reads clearly in both looks (measured, not guessed).
        let status = app.descendants(matching: .any)["writer.status"].firstMatch
        let contrast = try XCTUnwrap(measuredContrast(status), "couldn't measure the status line")
        note("status line \(String(format: "%.1f", contrast)):1", named: "status-contrast-\(look.rawValue)")
        XCTAssertGreaterThanOrEqual(contrast, 4.5, "the status line's contrast")

        // Focus mode (⇧⌘F) on the second paragraph's first sentence.
        app.typeKey(XCUIKeyboardKey.upArrow.rawValue, modifierFlags: .command)
        app.typeKey(XCUIKeyboardKey.downArrow.rawValue, modifierFlags: [])
        app.typeKey(XCUIKeyboardKey.downArrow.rawValue, modifierFlags: [])
        app.typeKey(XCUIKeyboardKey.rightArrow.rawValue, modifierFlags: .option)
        app.typeKey("f", modifierFlags: [.command, .shift])
        sleep(1)
        // Dimming is only drawn: VoiceOver still has every word.
        let all = text.value as? String ?? ""
        XCTAssertTrue(all.hasPrefix("## Where I am") && all.hasSuffix("Read the ice slowly."), all)
        keep(app, "21-writer-focus", look)
        app.typeKey("f", modifierFlags: [.command, .shift])

        // View: Focus Mode is there, with its keys.
        let view = app.menuBars.menuBarItems["View"]
        view.click()
        let focusItem = view.menuItems["Focus Mode"]
        XCTAssertTrue(focusItem.waitForExistence(timeout: 2))
        XCTAssertTrue(focusItem.isEnabled)
        keepScreen("30-view-menu", look)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])

        // Full screen (⌃⌘F): the toolbar slides away. Esc comes back.
        let windowed = app.windows.firstMatch.frame
        app.typeKey("f", modifierFlags: [.command, .control])
        sleep(3)
        // Away from the top edge, where the toolbar would slide back.
        text.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.6)).hover()
        sleep(2)
        let screen = XCUIScreen.main.screenshot().image.size
        let full = app.windows.firstMatch.frame
        note("windowed \(windowed), full screen \(full), screen \(screen), "
             + "toolbar \(app.toolbars.firstMatch.exists ? "\(app.toolbars.firstMatch.frame)" : "gone")",
             named: "full-screen-\(look.rawValue)")
        XCTAssertEqual(full.width, screen.width, "not full screen")
        let toolbar = app.toolbars.firstMatch
        XCTAssertTrue(!toolbar.exists || toolbar.frame.maxY <= 0, "the toolbar didn't slide away: \(toolbar.frame)")
        keepScreen("22-writer-full-screen", look)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        sleep(3)
        XCTAssertTrue(text.exists)
        XCTAssertLessThan(app.windows.firstMatch.frame.width, screen.width, "still full screen after Esc")

        // The Format menu.
        let format = app.menuBars.menuBarItems["Format"]
        XCTAssertTrue(format.exists, "no Format menu")
        format.click()
        let bold = format.menuItems["Bold"]
        XCTAssertTrue(bold.waitForExistence(timeout: 2))
        XCTAssertTrue(bold.isEnabled)
        keepScreen("23-format-menu", look)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])

        // Edit › Find ▸, then the find bar (⌘F) on the page.
        let edit = app.menuBars.menuBarItems["Edit"]
        edit.click()
        let find = edit.menuItems["Find"]
        XCTAssertTrue(find.waitForExistence(timeout: 2))
        find.hover()
        sleep(1)
        keepScreen("24-edit-find-menu", look)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        app.typeKey("f", modifierFlags: .command)
        app.typeText("Estraven")
        sleep(1)
        keep(app, "25-writer-find", look)
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])

        // An empty page shows its prompts.
        app.buttons["page.back"].firstMatch.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        _ = openWriter(app, "Middlemarch")
        keep(app, "26-writer-prompts", look)
    }

    // MARK: Behaviour

    /// Typed Markdown is highlighted as it's typed; ⌘B takes the word at
    /// the caret, and ⌘Z undoes it in one step; the Format menu and the
    /// toolbar format lines.
    @MainActor
    func testTypingIsHighlightedAndFormattingUndoesInOneStep() throws {
        let app = launch(.demo, .light, ["-BookshelfStyleProbe"])
        let text = openWriter(app, "Middlemarch")
        let styles = app.staticTexts["writer.styles"]
        XCTAssertTrue(styles.waitForExistence(timeout: 5))

        app.typeText("Hello world")
        app.typeKey("b", modifierFlags: .command)
        XCTAssertTrue(waitFor(text) { $0 == "Hello **world**" }, "after ⌘B: \(text.value ?? "")")
        XCTAssertTrue(waitFor(styles) { $0.contains("bold 8-13") }, "styles: \(styles.value ?? "")")

        app.typeKey("z", modifierFlags: .command)
        XCTAssertTrue(waitFor(text) { $0 == "Hello world" }, "after ⌘Z: \(text.value ?? "")")
        XCTAssertTrue(waitFor(styles) { !$0.contains("bold") }, "styles: \(styles.value ?? "")")
        app.typeKey("z", modifierFlags: [.command, .shift])
        XCTAssertTrue(waitFor(text) { $0 == "Hello **world**" }, "after ⇧⌘Z: \(text.value ?? "")")

        // Markdown typed by hand.
        app.typeKey(XCUIKeyboardKey.rightArrow.rawValue, modifierFlags: .command)
        app.typeText("\n# A heading\n> A quote with *feeling*\nSome `code` and ~~less~~")
        XCTAssertTrue(waitFor(styles) { described in
            ["heading1", "quote", "italic", "code", "strike", "marks"].allSatisfy { described.contains($0) }
        }, "styles: \(styles.value ?? "")")
        note(styles.value as? String ?? "", named: "styles-after-typing")

        // Format › Bulleted List on a new line.
        app.typeText("\nA list item")
        let format = app.menuBars.menuBarItems["Format"]
        format.click()
        format.menuItems["Bulleted List"].click()
        XCTAssertTrue(waitFor(text) { $0.hasSuffix("\n- A list item") }, "after Bulleted List: \(text.value ?? "")")

        // Link (⌘K) on the word at the caret.
        app.typeKey(XCUIKeyboardKey.rightArrow.rawValue, modifierFlags: .command)
        app.typeKey("k", modifierFlags: .command)
        XCTAssertTrue(waitFor(text) { $0.contains("[item](") }, "after ⌘K: \(text.value ?? "")")
        app.typeKey("z", modifierFlags: .command)
        XCTAssertTrue(waitFor(text) { $0.hasSuffix("\n- A list item") }, "after undoing the link: \(text.value ?? "")")

        // A toolbar button: Heading on the first line, from its menu.
        app.typeKey(XCUIKeyboardKey.upArrow.rawValue, modifierFlags: .command)
        let heading = app.toolbars.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier == 'format.heading' OR label == 'Heading'")).firstMatch
        XCTAssertTrue(heading.waitForExistence(timeout: 2), "no Heading button on the toolbar")
        heading.click()
        app.menuItems["Heading 2"].firstMatch.click()
        XCTAssertTrue(waitFor(text) { $0.hasPrefix("## Hello **world**") }, "after Heading 2: \(text.value ?? "")")
        let italic = app.toolbars.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier == 'format.italic' OR label == 'Italic'")).firstMatch
        XCTAssertTrue(italic.exists, "no Italic button on the toolbar")
    }

    /// Typing saves by itself after a pause; leaving and coming back finds
    /// the text, and the shelf shows it.
    @MainActor
    func testAutosaveKeepsTheTextAcrossLeavingAndComingBack() throws {
        let app = launch(.demo, .light)
        let text = openWriter(app, "Middlemarch")
        let status = app.descendants(matching: .any)["writer.status"].firstMatch
        XCTAssertTrue(waitFor(status) { $0.contains("0 words") && $0.contains("Saved") }, "status: \(status.label)")

        app.typeText("Dorothea deserved better.")
        XCTAssertTrue(waitFor(status, timeout: 8) { $0.contains("3 words") && $0.contains("Saved") },
                      "status: \(status.label)")

        // ⌘S saves straight away.
        app.typeText(" So did Lydgate.")
        app.typeKey("s", modifierFlags: .command)
        XCTAssertTrue(waitFor(status, timeout: 8) { $0.contains("6 words") && $0.contains("Saved") },
                      "status: \(status.label)")

        app.buttons["page.back"].firstMatch.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        XCTAssertTrue(waitFor(entry(app, "Middlemarch")) { $0.contains("Dorothea deserved better") },
                      "the shelf's excerpt")

        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: .command)
        XCTAssertTrue(text.waitForExistence(timeout: 5))
        XCTAssertTrue(waitFor(text) { $0 == "Dorothea deserved better. So did Lydgate." }, "\(text.value ?? "")")
    }

    /// When the journal won't take the text: the status line says so, and
    /// leaving or quitting rescues it to the recovery folder, then asks.
    @MainActor
    func testWhenSavingFailsTheTextIsRescued() throws {
        let app = launch(.demo, .light, ["-BookshelfFailSaves"])
        let text = openWriter(app, "Middlemarch")
        app.typeText("Keep me safe.")
        let status = app.descendants(matching: .any)["writer.status"].firstMatch
        XCTAssertTrue(waitFor(status, timeout: 8) { $0.contains("Not saved") }, "status: \(status.label)")
        // The count follows the text, saved or not.
        XCTAssertTrue(waitFor(status) { $0.contains("3 words") }, "status: \(status.label)")
        keep(app, "27-writer-not-saved", .light)

        // Leaving: Keep Open stays on the page, with the text.
        app.buttons["page.back"].firstMatch.click()
        let keepOpen = alertButton(app, "Keep Open")
        XCTAssertTrue(keepOpen.waitForExistence(timeout: 5), "no rescue alert")
        let said = app.staticTexts.matching(NSPredicate(format: "value CONTAINS 'recovery folder' OR label CONTAINS 'recovery folder'"))
        XCTAssertGreaterThan(said.count, 0, "the alert doesn't say where the copy is")
        let container = app.staticTexts.matching(NSPredicate(format: "value CONTAINS 'Containers' OR label CONTAINS 'Containers'"))
        XCTAssertEqual(container.count, 0, "the alert spells out the sandbox path")
        // Show in Finder is there (not clicked: the Finder would take over the screen).
        XCTAssertTrue(app.sheets.buttons["rescue.showInFinder"].exists, "no Show in Finder")
        XCTAssertTrue(alertButton(app, "Close Anyway").exists)
        keep(app, "28-rescue-alert", .light)
        keepOpen.click()
        XCTAssertTrue(waitFor(text) { $0 == "Keep me safe." })

        // Quitting asks too, with Quit Anyway; Keep Open cancels the quit.
        app.typeKey("q", modifierFlags: .command)
        XCTAssertTrue(keepOpen.waitForExistence(timeout: 5), "no rescue alert on quitting")
        XCTAssertTrue(alertButton(app, "Quit Anyway").exists, "quitting, the alert should offer Quit Anyway")
        XCTAssertFalse(alertButton(app, "Close Anyway").exists)
        keep(app, "28-rescue-alert-quitting", .light)
        keepOpen.click()
        XCTAssertTrue(text.waitForExistence(timeout: 5))
        XCTAssertEqual(app.state, .runningForeground)

        // Close Anyway leaves the page.
        app.buttons["page.back"].firstMatch.click()
        let closeAnyway = alertButton(app, "Close Anyway")
        XCTAssertTrue(closeAnyway.waitForExistence(timeout: 5))
        closeAnyway.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
    }

    /// When even the recovery copy can't be written, the text goes on the
    /// clipboard, and the alert says so.
    @MainActor
    func testWhenTheRecoveryCopyFailsTooTheTextIsOnTheClipboard() throws {
        let app = launch(.demo, .light, ["-BookshelfFailSaves", "-BookshelfFailRescue"])
        _ = openWriter(app, "Middlemarch")
        app.typeText("Clipboard words.")
        app.buttons["page.back"].firstMatch.click()
        let closeAnyway = alertButton(app, "Close Anyway")
        XCTAssertTrue(closeAnyway.waitForExistence(timeout: 5), "no rescue alert")
        let said = app.staticTexts.matching(NSPredicate(format: "value CONTAINS 'clipboard' OR label CONTAINS 'clipboard'"))
        XCTAssertGreaterThan(said.count, 0, "the alert doesn't mention the clipboard")
        keep(app, "29-rescue-clipboard", .light)
        XCTAssertEqual(NSPasteboard.general.string(forType: .string), "Clipboard words.")
        closeAnyway.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
    }

    // MARK: Helpers

    /// A button on the rescue alert (a sheet on the window). The Touch Bar
    /// has copies of alert buttons, which can't be clicked.
    @MainActor
    private func alertButton(_ app: XCUIApplication, _ title: String) -> XCUIElement {
        app.sheets.buttons[title].firstMatch
    }

    /// Opens `title`'s writing page from the Reading shelf.
    @MainActor
    private func openWriter(_ app: XCUIApplication, _ title: String) -> XCUIElement {
        app.typeKey("1", modifierFlags: .command)
        let book = entry(app, title)
        XCTAssertTrue(book.waitForExistence(timeout: 30), "no \(title)")
        book.click()
        XCTAssertTrue(app.staticTexts["book.title"].waitForExistence(timeout: 5))
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: .command)
        let text = app.textViews["writer.text"]
        XCTAssertTrue(text.waitForExistence(timeout: 5), "no writing page")
        sleep(1)
        return text
    }
}
