import SwiftUI
import BookshelfKit

/// The first page anyone sees: what Bookshelf is, and a profile to start.
struct WelcomeView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        ScrollView {
            VStack(spacing: 28) {
                VStack(spacing: 12) {
                    Image(systemName: "books.vertical.fill")
                        .font(.system(size: 52))
                        .foregroundStyle(.tint)
                        .accessibilityHidden(true)
                    Text("Welcome to Bookshelf")
                        .font(Typeface.title(.serif, size: 34))
                        .accessibilityAddTraits(.isHeader)
                    Text("A quiet place to keep track of the books you read, and to write down what they meant to you. Make a profile to start your reading journal.")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 460)
                        .fixedSize(horizontal: false, vertical: true)
                }
                ProfileForm(actionTitle: "Create Profile")
                    .padding(24)
                    .frame(width: 460)
                    .background(.background, in: RoundedRectangle(cornerRadius: 14))
                    .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(.separator))
                    .shadow(color: .black.opacity(0.06), radius: 12, y: 4)
            }
            .padding(48)
            .frame(maxWidth: .infinity)
        }
        .background(.background.secondary)
        .frame(minWidth: 760, minHeight: 560)
        .navigationTitle("Bookshelf")
    }
}

/// A new profile: a name, and an optional email with why it's asked for.
struct ProfileForm: View {
    let actionTitle: String
    var onCancel: (() -> Void)?

    @Environment(AppModel.self) private var model
    @State private var name = ""
    @State private var email = ""
    @State private var problem: String?
    @State private var busy = false
    @FocusState private var nameFocused: Bool

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Your name")
                    .font(.headline)
                TextField("Your name", text: $name, prompt: Text("Avery"))
                    .textFieldStyle(.roundedBorder)
                    .controlSize(.large)
                    .focused($nameFocused)
                    .labelsHidden()
                    .accessibilityLabel("Your name")
                    .accessibilityIdentifier("profile.name")
            }
            VStack(alignment: .leading, spacing: 6) {
                Text("Email (optional)")
                    .font(.headline)
                TextField("Email (optional)", text: $email, prompt: Text("you@example.com"))
                    .textFieldStyle(.roundedBorder)
                    .controlSize(.large)
                    .labelsHidden()
                    .accessibilityLabel("Email (optional)")
                    .accessibilityHint(Wording.emailWhy)
                    .accessibilityIdentifier("profile.email")
                Text(Wording.emailWhy)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityHidden(true)
            }
            if let problem {
                Label(problem, systemImage: "exclamationmark.circle.fill")
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("profile.problem")
            }
            HStack {
                Spacer()
                if let onCancel {
                    Button("Cancel", role: .cancel, action: onCancel)
                        .keyboardShortcut(.cancelAction)
                        .controlSize(.large)
                }
                Button(actionTitle, action: create)
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .disabled(trimmedName.isEmpty || busy)
                    .accessibilityIdentifier("profile.create")
            }
        }
        .onAppear { nameFocused = true }
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
            } catch {
                problem = (error as? CoreError)?.userMessage ?? "Something went wrong."
            }
        }
    }
}

/// File › New Profile…: the same form, in a sheet.
struct NewProfileSheet: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Text("New Profile")
                .font(.title2.bold())
                .accessibilityAddTraits(.isHeader)
            Text("Everyone who reads in your home can have their own shelves, summaries and settings.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            ProfileForm(actionTitle: "Create Profile") { model.isCreatingProfile = false }
        }
        .padding(24)
        .frame(width: 460)
    }
}
