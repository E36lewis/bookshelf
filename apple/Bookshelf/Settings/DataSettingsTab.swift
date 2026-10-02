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
        Form {
            Section("Export") {
                LabeledContent {
                    Button("Export…") { Task { await export() } }
                        .disabled(exporting)
                        .accessibilityIdentifier("settings.export")
                } label: {
                    SettingLabel(
                        "Export my summaries",
                        detail: "Each summary becomes a Markdown file, in a “Bookshelf summaries” folder inside the folder you choose.")
                }
                if let exported {
                    LabeledContent {
                        Button("Show in Finder") { FolderPicker.reveal(URL(fileURLWithPath: exported.folder)) }
                    } label: {
                        Label("Exported \(Wording.summaries(exported.count)).", systemImage: "checkmark.circle.fill")
                    }
                }
                if let exportProblem {
                    Label(exportProblem, systemImage: "exclamationmark.circle.fill").foregroundStyle(.red)
                }
            }
            Section("Daily Backups") {
                if let backup {
                    LabeledContent("Folder") {
                        Text(backup.shownFolder)
                            .textSelection(.enabled)
                            .multilineTextAlignment(.trailing)
                            .accessibilityIdentifier("settings.backupFolder")
                    }
                    Label {
                        Text(Wording.backup(backup.status, format: (model.settings ?? settings).dateFormat))
                            .fixedSize(horizontal: false, vertical: true)
                    } icon: {
                        Image(systemName: backup.status.available ? "clock.arrow.circlepath" : "externaldrive.badge.exclamationmark")
                    }
                    .foregroundStyle(backup.status.available ? AnyShapeStyle(.secondary) : AnyShapeStyle(.orange))
                    HStack {
                        Button("Change…") { Task { await changeFolder() } }
                        if backup.status.isCustom {
                            Button("Use Default") { Task { await useDefault() } }
                        }
                        Spacer()
                        Button("Show in Finder") { FolderPicker.reveal(URL(fileURLWithPath: backup.status.folder)) }
                            .disabled(!backup.status.available)
                    }
                    .disabled(busy)
                    Text("Keep them somewhere else, such as a USB drive or a folder that syncs: a backup on the same disk won't help if that disk fails.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                } else {
                    ProgressView().frame(maxWidth: .infinity)
                }
                if let backupProblem {
                    Label(backupProblem, systemImage: "exclamationmark.circle.fill").foregroundStyle(.red)
                }
            }
        }
        .formStyle(.grouped)
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
