import XCTest

/// Drives the app like a person would and keeps screenshots, so the Mac UI
/// can be reviewed from CI without a Mac.
final class ScreenshotTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    @MainActor
    func testAddProfileAndHighlighting() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-BookshelfFreshJournal"]
        app.launch()

        let field = app.textFields["newProfileName"]
        XCTAssertTrue(field.waitForExistence(timeout: 15))
        field.click()
        field.typeText("Avery\r")
        XCTAssertTrue(app.staticTexts["Avery"].waitForExistence(timeout: 10))

        keep(app, named: "01-profiles-and-highlighting")
    }

    @MainActor
    private func keep(_ app: XCUIApplication, named name: String) {
        let shot = XCTAttachment(screenshot: app.windows.firstMatch.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }
}
