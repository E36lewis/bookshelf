import SwiftUI
import BookshelfKit

/// Settings (⌘,), for the profile that's open. Every change is saved and
/// applied at once.
struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Group {
            if let settings = model.settings, let profile = model.profile {
                TabView {
                    ProfileSettingsTab(profile: profile)
                        .tabItem { Label("Profile", systemImage: "person.crop.circle") }
                    AppearanceSettingsTab(settings: settings)
                        .tabItem { Label("Appearance", systemImage: "paintpalette") }
                    WritingSettingsTab(settings: settings)
                        .tabItem { Label("Writing", systemImage: "pencil.line") }
                    ReadingLogSettingsTab(settings: settings)
                        .tabItem { Label("Reading Log", systemImage: "calendar") }
                    DataSettingsTab(settings: settings)
                        .tabItem { Label("Data", systemImage: "externaldrive") }
                }
                .frame(width: 560)
            } else {
                ContentUnavailableView(
                    "No Profile Yet", systemImage: "person.crop.circle.badge.questionmark",
                    description: Text("Make a profile first; its settings appear here."))
                    .frame(width: 440, height: 260)
            }
        }
        .tint(model.accent(for: colorScheme))
    }
}

extension AppModel {
    /// A binding to one of the current profile's settings, saved on change.
    func setting<Value>(_ key: WritableKeyPath<ProfileSettings, Value>, fallback: ProfileSettings) -> Binding<Value> {
        Binding(
            get: { (self.settings ?? fallback)[keyPath: key] },
            set: { value in self.updateSettings { $0[keyPath: key] = value } })
    }
}

/// A setting's name with a line under it explaining it.
struct SettingLabel: View {
    let title: String
    let detail: String

    init(_ title: String, detail: String) {
        self.title = title
        self.detail = detail
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            Text(detail)
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

// MARK: - Profile

struct ProfileSettingsTab: View {
    let profile: Profile
    @Environment(AppModel.self) private var model

    @State private var name = ""
    @State private var email = ""
    @State private var nameProblem: String?
    @State private var emailProblem: String?
    @State private var saved: String?
    @State private var askingDelete = false
    @State private var confirmingDelete = false
    @State private var exportNote: String?

    var body: some View {
        Form {
            Section {
                LabeledContent("Name") {
                    HStack {
                        TextField("Name", text: $name)
                            .labelsHidden()
                            .onSubmit(saveName)
                            .accessibilityIdentifier("settings.name")
                        Button("Save", action: saveName)
                            .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || name == profile.name)
                    }
                }
                if let nameProblem {
                    Label(nameProblem, systemImage: "exclamationmark.circle.fill").foregroundStyle(.red)
                }
                LabeledContent {
                    HStack {
                        TextField("Email (optional)", text: $email, prompt: Text("you@example.com"))
                            .labelsHidden()
                            .onSubmit(saveEmail)
                            .accessibilityLabel("Email (optional)")
                            .accessibilityIdentifier("settings.email")
                        Button("Save", action: saveEmail)
                            .disabled(email == (profile.email ?? ""))
                    }
                } label: {
                    SettingLabel("Email (optional)", detail: Wording.emailWhy)
                }
                if let emailProblem {
                    Label(emailProblem, systemImage: "exclamationmark.circle.fill").foregroundStyle(.red)
                }
                if let saved {
                    Label(saved, systemImage: "checkmark.circle.fill").foregroundStyle(.secondary)
                }
            }
            Section {
                LabeledContent {
                    Button("Delete Profile…", role: .destructive) { askingDelete = true }
                        .accessibilityIdentifier("settings.delete")
                } label: {
                    SettingLabel(
                        "Delete this profile",
                        detail: "Removes \(profile.name)’s summaries, dates and settings. The books themselves stay.")
                }
                if let exportNote {
                    Text(exportNote).font(.callout).foregroundStyle(.secondary)
                }
            }
        }
        .formStyle(.grouped)
        .onAppear(perform: reset)
        .onChange(of: profile) { reset() }
        .confirmationDialog("Delete \(profile.name)?", isPresented: $askingDelete, titleVisibility: .visible) {
            Button("Export First…") { Task { await exportFirst() } }
            Button("Delete…", role: .destructive) { confirmingDelete = true }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Their summaries, dates and settings will be deleted. The books themselves stay.\n\nExport first to keep a copy of everything they wrote.")
        }
        .sheet(isPresented: $confirmingDelete) {
            DeleteProfileSheet(name: profile.name) {
                confirmingDelete = false
                Task { await model.deleteProfile() }
            } cancel: {
                confirmingDelete = false
            }
        }
    }

    private func reset() {
        name = profile.name
        email = profile.email ?? ""
        nameProblem = nil
        emailProblem = nil
        saved = nil
    }

    private func saveName() {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != profile.name else { return }
        Task {
            do {
                try await model.rename(to: trimmed)
                nameProblem = nil
                saved = "Name saved"
            } catch {
                nameProblem = (error as? CoreError)?.userMessage ?? "Something went wrong."
            }
        }
    }

    private func saveEmail() {
        Task {
            do {
                try await model.setEmail(email)
                emailProblem = nil
                email = model.profile?.email ?? ""
                saved = email.isEmpty ? "Email removed" : "Email saved"
            } catch {
                emailProblem = (error as? CoreError)?.userMessage ?? "Something went wrong."
            }
        }
    }

    private func exportFirst() async {
        guard let folder = await FolderPicker.choose(
            message: "Choose where to save \(profile.name)’s summaries", prompt: "Export Here")
        else { return }
        do {
            let result = try await model.export(to: folder)
            exportNote = "Exported \(Wording.summaries(result.count)) to \(result.folder)."
        } catch {
            exportNote = "Export failed: \((error as? CoreError)?.userMessage ?? "Something went wrong.")"
        }
    }
}

/// The second step of deleting a profile: the name has to be typed, so a
/// stray click can't delete someone's journal.
struct DeleteProfileSheet: View {
    let name: String
    let delete: () -> Void
    let cancel: () -> Void

    @State private var typed = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label {
                Text("Delete \(name) for good?")
                    .font(.title3.bold())
            } icon: {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.red)
            }
            .accessibilityAddTraits(.isHeader)
            Text("This can't be undone. All of \(name)’s summaries and dates will be gone.\n\nType “\(name)” to confirm.")
                .fixedSize(horizontal: false, vertical: true)
            TextField("Profile name", text: $typed, prompt: Text(name))
                .textFieldStyle(.roundedBorder)
                .accessibilityLabel("Profile name")
                .accessibilityIdentifier("delete.name")
            HStack {
                Spacer()
                Button("Cancel", role: .cancel, action: cancel)
                    .keyboardShortcut(.cancelAction)
                Button("Delete Forever", role: .destructive, action: delete)
                    .disabled(typed.trimmingCharacters(in: .whitespaces) != name.trimmingCharacters(in: .whitespaces))
                    .accessibilityIdentifier("delete.confirm")
            }
        }
        .padding(22)
        .frame(width: 420)
    }
}

// MARK: - Appearance

struct AppearanceSettingsTab: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    var body: some View {
        Form {
            Section {
                Picker("Theme", selection: model.setting(\.theme, fallback: settings)) {
                    Text("Match System").tag(Theme.system)
                    Text("Light").tag(Theme.light)
                    Text("Dark").tag(Theme.dark)
                }
                .pickerStyle(.segmented)
                LabeledContent("Accent Color") {
                    AccentPicker(settings: settings)
                }
                Picker(selection: model.setting(\.headingFont, fallback: settings)) {
                    Text("Serif").tag(HeadingFont.serif)
                    Text("Sans-serif").tag(HeadingFont.sans)
                } label: {
                    SettingLabel("Titles and summaries", detail: "Serif is bookish; sans-serif is clean.")
                }
                .pickerStyle(.segmented)
            }
            Section("Preview") {
                VStack(alignment: .leading, spacing: 6) {
                    Text("The Left Hand of Darkness")
                        .font(Typeface.title(model.headingFont, size: 22))
                    Text("Ursula K. Le Guin")
                        .font(.headline)
                        .foregroundStyle(.tint)
                    Text("Light is the left hand of darkness, and darkness the right hand of light.")
                        .font(Typeface.text(model.headingFont, size: 15))
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
            }
        }
        .formStyle(.grouped)
    }
}

/// The eight accent colors from the core, and any other color.
struct AccentPicker: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    var body: some View {
        let current = (model.settings ?? settings).accent.lowercased()
        HStack(spacing: 8) {
            ForEach(model.accentChoices) { accent in
                let selected = accent.hex.lowercased() == current
                Button {
                    model.updateSettings { $0.accent = accent.hex }
                } label: {
                    Circle()
                        .fill(Color(hex: accent.hex) ?? .accentColor)
                        .frame(width: 20, height: 20)
                        .overlay {
                            if selected {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 10, weight: .bold))
                                    .foregroundStyle(.white)
                            }
                        }
                        .overlay(Circle().strokeBorder(.black.opacity(0.15)))
                        .padding(2)
                        .overlay(Circle().strokeBorder(selected ? Color.primary.opacity(0.5) : .clear, lineWidth: 1.5))
                }
                .buttonStyle(.plain)
                .help(accent.name)
                .accessibilityLabel(accent.name)
                .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
            }
            ColorPicker(
                "Custom Color",
                selection: Binding(
                    get: { Color(hex: current) ?? .accentColor },
                    set: { color in
                        if let hex = color.hex { model.updateSettings { $0.accent = hex } }
                    }),
                supportsOpacity: false
            )
            .labelsHidden()
            .help("Any color you like")
            .accessibilityLabel("Custom color")
        }
    }
}

// MARK: - Writing

struct WritingSettingsTab: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    var body: some View {
        let live = model.settings ?? settings
        Form {
            Section {
                Picker("Font", selection: model.setting(\.writingFont, fallback: settings)) {
                    Text("iA Writer Duo").tag(WritingFont.iaDuo)
                    Text("Source Serif").tag(WritingFont.serif)
                    Text("System Sans-Serif").tag(WritingFont.sans)
                    Text("System Monospace").tag(WritingFont.mono)
                }
                Stepper(value: model.setting(\.writingSize, fallback: settings), in: 10...28) {
                    LabeledContent("Text Size", value: "\(live.writingSize) pt")
                }
                Picker("Line Spacing", selection: model.setting(\.lineSpacing, fallback: settings)) {
                    Text("Tight").tag(LineSpacing.tight)
                    Text("Comfortable").tag(LineSpacing.normal)
                    Text("Airy").tag(LineSpacing.airy)
                }
                Picker("Page Width", selection: model.setting(\.pageWidth, fallback: settings)) {
                    Text("Narrow").tag(PageWidth.narrow)
                    Text("Medium").tag(PageWidth.medium)
                    Text("Wide").tag(PageWidth.wide)
                }
                Toggle(isOn: model.setting(\.focusDefault, fallback: settings)) {
                    SettingLabel("Start in focus mode", detail: "Dim everything except the sentence you're writing.")
                }
            }
            Section("Preview") {
                Text("The quiet hours between chapters are where the best thoughts arrive.")
                    .font(Typeface.writing(live.writingFont, size: live.textSize))
                    .lineSpacing(model.layout?.wrapPoints ?? 4)
                    .padding(.vertical, 6)
                    .accessibilityLabel("Preview of your writing font")
            }
        }
        .formStyle(.grouped)
    }
}

// MARK: - Reading log

struct ReadingLogSettingsTab: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    private static let sample = "2026-09-20"

    var body: some View {
        Form {
            Section {
                Picker("Date Format", selection: model.setting(\.dateFormat, fallback: settings)) {
                    ForEach([DateFormat.long, .monthDayYear, .dayMonthYear, .yearMonthDay], id: \.self) { format in
                        Text(JournalDate.display(Self.sample, format: format)).tag(format)
                    }
                }
                Picker(selection: model.setting(\.weekStart, fallback: settings)) {
                    Text("Sunday").tag(WeekStart.sunday)
                    Text("Monday").tag(WeekStart.monday)
                } label: {
                    SettingLabel("Week starts on", detail: "For the calendar when you pick a date.")
                }
                Picker("Open To", selection: model.setting(\.startShelf, fallback: settings)) {
                    ForEach(Shelf.inOrder, id: \.self) { shelf in
                        Text(shelf.title).tag(shelf)
                    }
                }
            }
        }
        .formStyle(.grouped)
    }
}
