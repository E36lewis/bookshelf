import XCTest
@testable import BookshelfKit

/// Only a release build may open the real journal.
final class BuildChannelTests: XCTestCase {
    func testOnlyStableIsARelease() {
        XCTAssertEqual(BuildChannel.channel(infoValue: "Stable"), .stable)
        // A missing, empty or mistyped setting is a preview, never the real journal.
        let others: [Any?] = [nil, "", "Preview", "stable", " Stable", "Release", 1]
        for value in others {
            XCTAssertEqual(BuildChannel.channel(infoValue: value), .preview, String(describing: value))
        }
        // `swift test` runs without the app's Info.plist: a preview.
        XCTAssertEqual(BuildChannel.current, .preview)
    }

    func testNames() {
        XCTAssertEqual(BuildChannel.appName(.stable), "Bookshelf")
        XCTAssertEqual(BuildChannel.appName(.preview), "Bookshelf Preview")
    }
}
