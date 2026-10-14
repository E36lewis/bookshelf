import SwiftUI
import BookshelfKit

/// Settings (⌘,), for the profile that's open. Every change is saved and
/// applied at once. A Mac Settings window with a tab for each part, drawn
/// like Linux's settings page: groups of rows in panels, pill buttons,
/// choices as pills and a switch.
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
        .background(Palette.paper)
        .tint(model.accent(for: colorScheme))
        .environment(\.journalAccent, model.journalAccent(for: colorScheme))
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

/// A tab's groups, one under another.
struct SettingsPage<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            content
        }
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .background(Palette.paper)
    }
}

/// A group: its heading in the title font, and its rows in a panel.
struct SettingsGroup<Content: View>: View {
    var title: String?
    @ViewBuilder let content: Content

    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let title {
                GroupHeading(title: title, font: model.headingFont)
            }
            VStack(alignment: .leading, spacing: 0) {
                content
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 4)
            .panel(padding: 0)
        }
    }
}

/// A row: the setting's name (with what it does under it), and its control.
struct SettingRow<Control: View>: View {
    let title: String
    var detail: String?
    @ViewBuilder let control: Control

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            SettingLabel(title, detail: detail)
            Spacer(minLength: 12)
            control
        }
        .padding(.vertical, 10)
    }
}

/// The line between two rows of a panel.
struct RowRule: View {
    var body: some View {
        Rectangle().fill(Palette.rule).frame(height: 1)
    }
}

/// A setting's name with a line under it explaining it.
struct SettingLabel: View {
    let title: String
    let detail: String?

    init(_ title: String, detail: String? = nil) {
        self.title = title
        self.detail = detail
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.system(size: 13))
                .foregroundStyle(Palette.ink)
            if let detail {
                Text(detail)
                    .font(.system(size: 12))
                    .foregroundStyle(Color.quietText)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A short note under a setting: done, or what went wrong.
struct SettingNote: View {
    let text: String
    var isProblem = false

    var body: some View {
        Label(text, systemImage: isProblem ? "exclamationmark.circle.fill" : "checkmark.circle.fill")
            .font(.system(size: 12))
            .foregroundStyle(isProblem ? Color.errorText : Color.quietText)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.bottom, 10)
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
    @FocusState private var focus: Field?

    private enum Field {
        case name, email
    }

    var body: some View {
        SettingsPage {
            SettingsGroup(title: "Profile") {
                VStack(alignment: .leading, spacing: 6) {
                    SettingLabel("Name")
                    HStack(spacing: 8) {
                        TextField("Name", text: $name)
                            .focused($focus, equals: .name)
                            .onSubmit(saveName)
                            .fieldChrome(focused: focus == .name, invalid: nameProblem != nil)
                            .accessibilityLabel("Name")
                            .accessibilityIdentifier("settings.name")
                        Button("Save", action: saveName)
                            .buttonStyle(PillButtonStyle())
                            .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || name == profile.name)
                    }
                }
                .padding(.top, 12)
                .padding(.bottom, 10)
                if let nameProblem {
                    SettingNote(text: nameProblem, isProblem: true)
                }
                VStack(alignment: .leading, spacing: 6) {
                    SettingLabel("Email (optional)")
                    HStack(spacing: 8) {
                        TextField("Email (optional)", text: $email, prompt: Text("you@example.com"))
                            .focused($focus, equals: .email)
                            .onSubmit(saveEmail)
                            .fieldChrome(focused: focus == .email, invalid: emailProblem != nil)
                            .accessibilityLabel("Email (optional)")
                            .accessibilityHint(Wording.emailWhy)
                            .accessibilityIdentifier("settings.email")
                        Button("Save", action: saveEmail)
                            .buttonStyle(PillButtonStyle())
                            .disabled(email == (profile.email ?? ""))
                    }
                    Text(Wording.emailWhy)
                        .font(.system(size: 12))
                        .foregroundStyle(Color.quietText)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityHidden(true)
                }
                .padding(.bottom, 12)
                if let emailProblem {
                    SettingNote(text: emailProblem, isProblem: true)
                }
                if let saved {
                    SettingNote(text: saved)
                }
            }
            SettingsGroup {
                SettingRow(
                    title: "Delete this profile",
                    detail: "Removes \(profile.name)’s summaries, dates and settings. The books themselves stay."
                ) {
                    Button("Delete Profile…", role: .destructive) { askingDelete = true }
                        .buttonStyle(PillButtonStyle(kind: .destructive))
                        .accessibilityIdentifier("settings.delete")
                }
                if let exportNote {
                    SettingNote(text: exportNote, isProblem: exportNote.hasPrefix("Export failed"))
                }
            }
        }
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
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label {
                Text("Delete \(name) for good?")
                    .font(.title3.bold())
                    .foregroundStyle(Palette.ink)
            } icon: {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Color.errorText)
            }
            .accessibilityAddTraits(.isHeader)
            Text("This can't be undone. All of \(name)’s summaries and dates will be gone.\n\nType “\(name)” to confirm.")
                .foregroundStyle(Palette.ink)
                .fixedSize(horizontal: false, vertical: true)
            TextField("Profile name", text: $typed, prompt: Text(name))
                .focused($focused)
                .fieldChrome(focused: focused)
                .accessibilityLabel("Profile name")
                .accessibilityIdentifier("delete.name")
            HStack(spacing: 8) {
                Spacer()
                Button("Cancel", role: .cancel, action: cancel)
                    .buttonStyle(PillButtonStyle())
                    .keyboardShortcut(.cancelAction)
                Button("Delete Forever", role: .destructive, action: delete)
                    .buttonStyle(PillButtonStyle(kind: .destructive))
                    .disabled(typed.trimmingCharacters(in: .whitespaces) != name.trimmingCharacters(in: .whitespaces))
                    .accessibilityIdentifier("delete.confirm")
            }
        }
        .padding(22)
        .frame(width: 420)
        .background(Palette.card)
    }
}

// MARK: - Appearance

struct AppearanceSettingsTab: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    var body: some View {
        SettingsPage {
            SettingsGroup(title: "Appearance") {
                SettingRow(title: "Theme") {
                    ChoicePicker(
                        title: "Theme",
                        choices: [Choice(Theme.system, "Match System"), Choice(.light, "Light"), Choice(.dark, "Dark")],
                        selection: model.setting(\.theme, fallback: settings))
                }
                RowRule()
                SettingRow(title: "Accent color") {
                    AccentPicker(settings: settings)
                }
                RowRule()
                SettingRow(title: "Titles and summaries", detail: "Serif is bookish; sans-serif is clean.") {
                    ChoicePicker(
                        title: "Titles and summaries",
                        choices: [Choice(HeadingFont.serif, "Serif"), Choice(.sans, "Sans-serif")],
                        selection: model.setting(\.headingFont, fallback: settings))
                }
            }
            SettingsGroup(title: "Preview") {
                VStack(alignment: .leading, spacing: 6) {
                    Text("The Left Hand of Darkness")
                        .font(Typeface.title(model.headingFont, size: 22))
                        .foregroundStyle(Palette.ink)
                    Text("Ursula K. Le Guin")
                        .font(.system(size: 14, weight: .bold))
                        .foregroundStyle(.tint)
                    Text("Light is the left hand of darkness, and darkness the right hand of light.")
                        .font(Typeface.text(model.headingFont, size: 15))
                        .foregroundStyle(Color.quietText)
                }
                .padding(.vertical, 12)
            }
        }
    }
}

/// The eight accent colors from the core, and any other color.
struct AccentPicker: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    var body: some View {
        let current = (model.settings ?? settings).accent.lowercased()
        HStack(spacing: 6) {
            ForEach(model.accentChoices) { accent in
                let selected = accent.hex.lowercased() == current
                Button {
                    model.updateSettings { $0.accent = accent.hex }
                } label: {
                    Circle()
                        .fill(Color(hex: accent.hex) ?? .accentColor)
                        .frame(width: 22, height: 22)
                        .overlay {
                            if selected {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 10, weight: .bold))
                                    .foregroundStyle(.white)
                            }
                        }
                        .overlay(Circle().strokeBorder(.black.opacity(0.2)))
                        .padding(2)
                        .overlay(Circle().strokeBorder(selected ? Palette.ink : .clear, lineWidth: 1.5))
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
        SettingsPage {
            SettingsGroup(title: "Writing") {
                SettingRow(title: "Font") {
                    ChoicePicker(
                        title: "Font",
                        choices: [
                            Choice(WritingFont.iaDuo, "iA Writer Duo"), Choice(.serif, "Source Serif"),
                            Choice(.sans, "System Sans-Serif"), Choice(.mono, "System Monospace"),
                        ],
                        selection: model.setting(\.writingFont, fallback: settings))
                }
                RowRule()
                SettingRow(title: "Text size", detail: "Points") {
                    Stepper(value: model.setting(\.writingSize, fallback: settings), in: 10...28) {
                        Text("\(live.writingSize) pt")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(Palette.ink)
                            .monospacedDigit()
                    }
                    .accessibilityLabel("Text size")
                    .accessibilityValue("\(live.writingSize) points")
                }
                RowRule()
                SettingRow(title: "Line spacing") {
                    ChoicePicker(
                        title: "Line spacing",
                        choices: [Choice(LineSpacing.tight, "Tight"), Choice(.normal, "Comfortable"), Choice(.airy, "Airy")],
                        selection: model.setting(\.lineSpacing, fallback: settings))
                }
                RowRule()
                SettingRow(title: "Page width") {
                    ChoicePicker(
                        title: "Page width",
                        choices: [Choice(PageWidth.narrow, "Narrow"), Choice(.medium, "Medium"), Choice(.wide, "Wide")],
                        selection: model.setting(\.pageWidth, fallback: settings))
                }
                RowRule()
                Toggle(isOn: model.setting(\.focusDefault, fallback: settings)) {
                    SettingLabel("Start in focus mode", detail: "Dim everything except the sentence you're writing.")
                }
                .toggleStyle(JournalSwitchStyle())
                .padding(.vertical, 10)
            }
            SettingsGroup(title: "Preview") {
                Text("The quiet hours between chapters are where the best thoughts arrive.")
                    .font(Typeface.writing(live.writingFont, size: live.textSize))
                    .foregroundStyle(Palette.ink)
                    .lineSpacing(model.layout?.wrapPoints ?? 4)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.vertical, 12)
                    .accessibilityLabel("Preview of your writing font")
            }
        }
    }
}

// MARK: - Reading log

struct ReadingLogSettingsTab: View {
    let settings: ProfileSettings
    @Environment(AppModel.self) private var model

    private static let sample = "2026-09-20"

    var body: some View {
        SettingsPage {
            SettingsGroup(title: "Reading Log") {
                SettingRow(title: "Date format") {
                    ChoicePicker(
                        title: "Date format",
                        choices: [DateFormat.long, .monthDayYear, .dayMonthYear, .yearMonthDay].map {
                            Choice($0, JournalDate.display(Self.sample, format: $0))
                        },
                        selection: model.setting(\.dateFormat, fallback: settings))
                }
                RowRule()
                SettingRow(title: "Week starts on", detail: "For the calendar when you pick a date.") {
                    ChoicePicker(
                        title: "Week starts on",
                        choices: [Choice(WeekStart.sunday, "Sunday"), Choice(.monday, "Monday")],
                        selection: model.setting(\.weekStart, fallback: settings))
                }
                RowRule()
                SettingRow(title: "Open to") {
                    ChoicePicker(
                        title: "Open to",
                        choices: Shelf.inOrder.map { Choice($0, $0.title) },
                        selection: model.setting(\.startShelf, fallback: settings))
                }
            }
        }
    }
}
