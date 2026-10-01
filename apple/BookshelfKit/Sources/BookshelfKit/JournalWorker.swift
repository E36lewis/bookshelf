import Foundation
@_exported import BookshelfFFI

/// Runs calls into the Rust core one at a time, off the main thread.
/// The core is synchronous; this actor is what keeps the UI responsive.
public actor JournalWorker {
    private let journal: Journal

    /// Opens (or creates) the journal in `directory`.
    public init(directory: URL) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        journal = try Journal.openAt(dir: directory.path)
    }

    public func profiles() throws -> [Profile] {
        try journal.profiles()
    }

    public func createProfile(named name: String) throws -> Profile {
        try journal.createProfile(name: name)
    }
}

extension Profile: Identifiable {}
