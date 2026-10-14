import SwiftUI
import BookshelfKit

/// Who's reading: every profile, and a form for a new one, as on Linux.
/// Back from the shelves comes here; the first time, it's the welcome.
struct ProfilesView: View {
    @Environment(AppModel.self) private var model

    private var welcome: Bool { model.profiles.isEmpty }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(welcome ? "Welcome" : "Who's reading?")
                        .font(Typeface.title(.serif, size: 30))
                        .foregroundStyle(Palette.ink)
                        .accessibilityAddTraits(.isHeader)
                        .accessibilityHeading(.h1)
                    Text(welcome
                         ? "A quiet place to keep track of the books you read, and to write down what they meant to you. Make a profile to start your reading journal."
                         : "Pick up where you left off.")
                        .font(.system(size: 14))
                        .foregroundStyle(Color.quietText)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("profiles.lede")
                }
                .padding(.bottom, 12)
                if !welcome {
                    VStack(spacing: 0) {
                        ForEach(model.profiles) { profile in
                            ProfileRow(profile: profile)
                            if profile.id != model.profiles.last?.id {
                                Rectangle().fill(Palette.rule).frame(height: 1)
                            }
                        }
                    }
                    .panel(padding: 0)
                    .accessibilityElement(children: .contain)
                    .accessibilityLabel("Profiles")
                }
                GroupHeading(title: welcome ? "Your Profile" : "New Profile")
                    .padding(.top, welcome ? 0 : 8)
                ProfileForm(autoFocus: welcome)
            }
            .frame(maxWidth: 480, alignment: .leading)
            .padding(.horizontal, 24)
            .padding(.top, 32)
            .padding(.bottom, 32)
            .frame(maxWidth: .infinity)
        }
        .pageBackground(Palette.paper)
        .navigationTitle(BuildChannel.appName(BuildChannel.current))
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text(BuildChannel.appName(BuildChannel.current))
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Palette.ink)
                    .accessibilityHidden(true)
            }
        }
        .frame(minWidth: 600, minHeight: 480)
    }
}

/// A profile in the list: its initial, its name, and a chevron.
private struct ProfileRow: View {
    let profile: Profile
    @Environment(AppModel.self) private var model
    @State private var hovering = false

    var body: some View {
        Button {
            model.switchTo(profile)
        } label: {
            HStack(spacing: 14) {
                Avatar(name: profile.name, size: 36)
                Text(profile.name)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Palette.ink)
                    .lineLimit(1)
                Spacer(minLength: 8)
                Image(systemName: "chevron.forward")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(Color.quietText)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(hovering ? Palette.cardHover : Color.clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .accessibilityLabel(profile.name)
        .accessibilityHint(profile.id == model.profile?.id ? "Back to your shelves" : "Opens their shelves")
        .accessibilityIdentifier("profile.\(profile.name)")
    }
}

/// A profile's initial in a circle of the accent color.
struct Avatar: View {
    let name: String
    var size: CGFloat = 28

    @Environment(AppModel.self) private var model
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Circle()
            .fill(.tint)
            .frame(width: size, height: size)
            .overlay {
                Text(String(name.prefix(1)).uppercased())
                    .font(.system(size: size * 0.48, weight: .semibold, design: .rounded))
                    // Near-black on light accents, white on the rest.
                    .foregroundStyle(Color(hex: model.accentForeground(for: colorScheme == .dark)) ?? .white)
            }
            .accessibilityHidden(true)
    }
}

/// A new profile: a name, and an optional email with why it's asked for.
struct ProfileForm: View {
    /// Whether the name takes the keyboard at once (the welcome).
    var autoFocus = false

    @Environment(AppModel.self) private var model
    @State private var name = ""
    @State private var email = ""
    @State private var problem: String?
    @State private var busy = false
    @FocusState private var focus: Field?

    private enum Field {
        case name, email
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(spacing: 18) {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Your name")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Palette.ink)
                        .accessibilityHidden(true)
                    TextField("Your name", text: $name, prompt: Text("Avery"))
                        .focused($focus, equals: .name)
                        .fieldChrome(focused: focus == .name)
                        .accessibilityLabel("Your name")
                        .accessibilityIdentifier("profile.name")
                }
                VStack(alignment: .leading, spacing: 6) {
                    Text("Email (optional)")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Palette.ink)
                        .accessibilityHidden(true)
                    TextField("Email (optional)", text: $email, prompt: Text("you@example.com"))
                        .focused($focus, equals: .email)
                        .fieldChrome(focused: focus == .email)
                        .accessibilityLabel("Email (optional)")
                        .accessibilityHint(Wording.emailWhy)
                        .accessibilityIdentifier("profile.email")
                    Text(Wording.emailWhy)
                        .font(.system(size: 12))
                        .foregroundStyle(Color.quietText)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityHidden(true)
                }
            }
            .panel(padding: 18)
            if let problem {
                Label(problem, systemImage: "exclamationmark.circle.fill")
                    .foregroundStyle(Color.errorText)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityIdentifier("profile.problem")
            }
            Button(action: create) {
                HStack(spacing: 8) {
                    Text("Create Profile")
                    Image(systemName: "arrow.right")
                        .imageScale(.small)
                        .accessibilityHidden(true)
                }
            }
            .buttonStyle(PillButtonStyle(kind: .primary))
            .keyboardShortcut(.defaultAction)
            .disabled(trimmedName.isEmpty || busy)
            .accessibilityLabel("Create Profile")
            .accessibilityIdentifier("profile.create")
        }
        .task(id: model.focusNewProfile) {
            guard autoFocus || model.focusNewProfile else { return }
            try? await Task.sleep(for: .milliseconds(50))
            focus = .name
            model.focusNewProfile = false
        }
        .onChange(of: name) { problem = nil }
        .onChange(of: email) { problem = nil }
    }

    private func create() {
        guard !trimmedName.isEmpty, !busy else { return }
        busy = true
        Task {
            defer { busy = false }
            do {
                try await model.createProfile(name: trimmedName, email: email)
                name = ""
                email = ""
            } catch {
                problem = (error as? CoreError)?.userMessage ?? "Something went wrong."
            }
        }
    }
}
