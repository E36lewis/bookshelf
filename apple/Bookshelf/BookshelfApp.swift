import SwiftUI
import BookshelfKit

/// Phase 0 experiment: proves the SwiftUI app can open a journal through the
/// shared Rust core and use its highlighting. Not the real app yet.
@main
struct BookshelfApp: App {
    @State private var model = SpikeModel()

    var body: some Scene {
        Window("Bookshelf", id: "main") {
            ContentView(model: model)
                .frame(minWidth: 720, minHeight: 520)
                .task { await model.load() }
        }
    }
}

@MainActor
@Observable
final class SpikeModel {
    var profiles: [Profile] = []
    var newName = ""
    var status = "Opening the journal…"
    var problem: String?
    private var worker: JournalWorker?

    func load() async {
        do {
            let dir = Self.dataDirectory()
            let worker = try JournalWorker(directory: dir)
            self.worker = worker
            profiles = try await worker.profiles()
            status = "Rust core \(coreVersion()) · \(dir.path)"
        } catch {
            problem = "Couldn't open the journal: \(error.localizedDescription)"
        }
    }

    func addProfile() async {
        let name = newName.trimmingCharacters(in: .whitespaces)
        guard let worker, !name.isEmpty else { return }
        do {
            _ = try await worker.createProfile(named: name)
            profiles = try await worker.profiles()
            newName = ""
            problem = nil
        } catch {
            problem = error.localizedDescription
        }
    }

    /// Previews use their own journal, never a real one. `-BookshelfFreshJournal`
    /// (UI tests) starts from an empty one inside the app's own sandbox.
    static func dataDirectory() -> URL {
        let fm = FileManager.default
        if CommandLine.arguments.contains("-BookshelfFreshJournal") {
            return fm.temporaryDirectory.appendingPathComponent("journal-\(UUID().uuidString)")
        }
        return fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("io.github.e36lewis.Bookshelf.Preview")
    }
}
