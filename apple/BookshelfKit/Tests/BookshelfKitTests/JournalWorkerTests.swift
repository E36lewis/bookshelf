import XCTest
@testable import BookshelfKit

final class JournalWorkerTests: XCTestCase {
    func testProfilesRoundTrip() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("bookshelf-tests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }

        let worker = try JournalWorker(directory: dir)
        let none = try await worker.profiles()
        XCTAssertTrue(none.isEmpty)
        _ = try await worker.createProfile(named: "Avery")
        let names = try await worker.profiles().map(\.name)
        XCTAssertEqual(names, ["Avery"])
    }

    func testDuplicateNamesAreRefused() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("bookshelf-tests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }

        let worker = try JournalWorker(directory: dir)
        _ = try await worker.createProfile(named: "Avery")
        do {
            _ = try await worker.createProfile(named: "Avery")
            XCTFail("expected an error")
        } catch let error as CoreError {
            guard case .Invalid = error else { return XCTFail("unexpected \(error)") }
        }
    }

    func testHighlightRangesCountUTF16() {
        let ranges = highlightRanges(in: "📚 **b**")
        XCTAssertEqual(ranges.first?.kind, .bold)
        XCTAssertEqual(ranges.first?.range, NSRange(location: 5, length: 1))
    }
}
