import Foundation
import BookshelfKit

/// Export and backups, for Settings › Data. The slow parts (a USB stick, a
/// big export) run beside the journal's queue, off the main thread.
extension AppModel {
    /// The backup folder as Settings shows it.
    struct BackupInfo {
        var status: BackupStatus
        /// The folder, with the home folder as `~`.
        var shownFolder: String
    }

    func backupInfo() async throws -> BackupInfo? {
        guard let worker else { return nil }
        let status = try await backupFolder.whileAccessing { try await worker.backupStatus() }
        let shown = await JournalWorker.displayed(status.folder, home: HomeFolder.path)
        return BackupInfo(status: status, shownFolder: shown)
    }

    /// Keeps backups in `folder` from now on (just picked, so the sandbox
    /// lets us in), or in the default folder for `nil`, and makes a copy
    /// there right away.
    func setBackupFolder(_ folder: URL?) async throws {
        guard let worker else { return }
        guard let folder else {
            try await worker.setBackupFolder(nil)
            backupFolder.forget()
            await backUpNow()
            return
        }
        let accessing = folder.startAccessingSecurityScopedResource()
        defer { if accessing { folder.stopAccessingSecurityScopedResource() } }
        try await worker.setBackupFolder(folder)
        do {
            try backupFolder.remember(folder)
        } catch {
            // Without the bookmark the folder is out of reach after a
            // restart: back to the default rather than silently failing.
            try? await worker.setBackupFolder(nil)
            backupFolder.forget()
            throw error
        }
        _ = try await worker.backUpNow()
    }

    /// Writes the profile's summaries into a "Bookshelf summaries" folder
    /// inside `folder`.
    func export(to folder: URL) async throws -> ExportResult {
        guard let worker, let profile else { throw CoreError.NotFound(message: "No profile is open.") }
        let accessing = folder.startAccessingSecurityScopedResource()
        defer { if accessing { folder.stopAccessingSecurityScopedResource() } }
        return try await worker.exportMarkdown(for: profile.id, into: folder)
    }

    /// Text and icons on the accent (the avatar's initial).
    func accentForeground(for dark: Bool) -> String {
        guard let palettes else { return "#ffffff" }
        return dark ? palettes.dark.fg : palettes.light.fg
    }
}
