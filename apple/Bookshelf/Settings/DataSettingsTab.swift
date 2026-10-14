import SwiftUI
import BookshelfKit

/// Export and daily backups.
struct DataSettingsTab: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    @State private var exported: ExportResult?
    @State private var exportProblem: String?
    @State private var exporting = false
    @State private var backup: AppModel.BackupInfo?
    @State private var backupProblem: String?
    @State private var busy = false

    var body: some View {
        SettingsPage {
            SettingsGroup(title: "Export") {
                SettingRow(
                    title: "Export my summaries",
                    detail: "Each summary becomes a Markdown file, in a “Bookshelf summaries” folder inside the folder you choose."
                ) {
                    Button("Export…") { Task { await export() } }
                        .buttonStyle(PillButtonStyle())
                        .disabled(exporting)
                        .accessibilityIdentifier("settings.export")
                }
                if let exported {
                    RowRule()
                    SettingRow(title: "Exported \(Wording.summaries(exported.count)).") {
                        Button("Show in Finder") { FolderPicker.reveal(URL(fileURLWithPath: exported.folder)) }
                            .buttonStyle(PillButtonStyle())
                    }
                }
                if let exportProblem {
                    SettingNote(text: exportProblem, isProblem: true)
                }
            }
            SettingsGroup(title: "Daily Backups") {
                if let backup {
                    SettingRow(title: "Folder") {
                        Text(backup.shownFolder)
                            .font(.system(size: 13))
                            .foregroundStyle(Palette.ink)
                            .textSelection(.enabled)
                            .lineLimit(1)
                            // The folder's own name stays in view.
                            .truncationMode(.middle)
                            .help(backup.shownFolder)
                            .accessibilityIdentifier("settings.backupFolder")
                    }
                    RowRule()
                    VStack(alignment: .leading, spacing: 10) {
                        Label {
                            Text(Wording.backup(backup.status, format: (model.settings ?? settings).dateFormat))
                                .fixedSize(horizontal: false, vertical: true)
                        } icon: {
                            Image(systemName: backup.status.available ? "clock.arrow.circlepath" : "externaldrive.badge.exclamationmark")
                        }
                        .font(.system(size: 13))
                        .foregroundStyle(backup.status.available ? Color.quietText : Color.errorText)
                        HStack(spacing: 8) {
                            Button("Change…") { Task { await changeFolder() } }
                            if backup.status.isCustom {
                                Button("Use Default") { Task { await useDefault() } }
                            }
                            Spacer()
                            Button("Show in Finder") { FolderPicker.reveal(URL(fileURLWithPath: backup.status.folder)) }
                                .disabled(!backup.status.available)
                        }
                        .buttonStyle(PillButtonStyle())
                        .disabled(busy)
                        Text("Keep them somewhere else, such as a USB drive or a folder that syncs: a backup on the same disk won't help if that disk fails.")
                            .font(.system(size: 12))
                            .foregroundStyle(Color.quietText)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(.vertical, 12)
                } else {
                    ProgressView()
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 12)
                }
                if let backupProblem {
                    SettingNote(text: backupProblem, isProblem: true)
                }
            }
        }
        .task { await refresh() }
    }

    private func refresh() async {
        do {
            backup = try await model.backupInfo()
        } catch {
            backupProblem = (error as? CoreError)?.userMessage ?? "Something went wrong."
        }
    }

    private func export() async {
        guard let folder = await FolderPicker.choose(
            message: "Choose where to save the summaries", prompt: "Export Here")
        else { return }
        exporting = true
        defer { exporting = false }
        do {
            exported = try await model.export(to: folder)
            exportProblem = nil
        } catch {
            exported = nil
            exportProblem = "Export failed: \((error as? CoreError)?.userMessage ?? "Something went wrong.")"
        }
    }

    private func changeFolder() async {
        guard let folder = await FolderPicker.choose(
            message: "Choose where to keep backups", prompt: "Keep Backups Here")
        else { return }
        busy = true
        defer { busy = false }
        do {
            try await model.setBackupFolder(folder)
            backupProblem = nil
        } catch {
            backupProblem = (error as? CoreError)?.userMessage ?? "Couldn't use that folder."
        }
        await refresh()
    }

    private func useDefault() async {
        busy = true
        defer { busy = false }
        do {
            try await model.setBackupFolder(nil)
            backupProblem = nil
        } catch {
            backupProblem = (error as? CoreError)?.userMessage ?? "Something went wrong."
        }
        await refresh()
    }
}
