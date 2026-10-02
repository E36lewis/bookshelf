import SwiftUI
import BookshelfKit

/// Shelves on the left, the shelf's books in the middle, the book's page
/// on the right.
struct MainSplitView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        NavigationSplitView {
            SidebarView()
                .navigationSplitViewColumnWidth(min: 190, ideal: 220, max: 300)
        } content: {
            EntryListView()
                .navigationSplitViewColumnWidth(min: 300, ideal: 370, max: 520)
        } detail: {
            DetailView()
        }
        .searchable(text: $model.searchText, placement: .toolbar, prompt: "Title, author, or something you wrote")
        .task(id: model.searchText) { await model.runSearch() }
        .sheet(isPresented: $model.isAddingBook) {
            AddBookSheet()
        }
        .sheet(isPresented: $model.isCreatingProfile) {
            NewProfileSheet()
        }
        .frame(minWidth: 860, minHeight: 520)
    }
}

/// The profile at the top, and the three shelves with their counts.
struct SidebarView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        List(selection: Binding(get: { model.isSearching ? nil : model.shelf }, set: { if let shelf = $0 { model.show(shelf) } })) {
            Section("Shelves") {
                ForEach(Shelf.inOrder, id: \.self) { shelf in
                    Label(shelf.title, systemImage: shelf.systemImage)
                        .badge(Int(model.shelves[shelf]?.total ?? 0))
                        .tag(shelf)
                        .accessibilityIdentifier("shelf.\(shelf.title)")
                }
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .top, spacing: 0) {
            ProfileMenu()
                .padding(.horizontal, 12)
                .padding(.top, 4)
                .padding(.bottom, 8)
        }
    }
}

/// Who's reading: switch profile, or make a new one.
struct ProfileMenu: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        HStack(spacing: 10) {
            Avatar(name: model.profile?.name ?? "")
            Menu {
                Section("Who's reading?") {
                    ForEach(model.profiles) { profile in
                        Toggle(profile.name, isOn: Binding(
                            get: { profile.id == model.profile?.id },
                            set: { if $0 { model.switchTo(profile) } }))
                    }
                }
                Divider()
                Button("New Profile…") { model.isCreatingProfile = true }
                Button("Profile Settings…") { openSettings() }
            } label: {
                Text(model.profile?.name ?? "Profile")
                    .font(.headline)
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .help("Switch profile")
            .accessibilityLabel("Profile: \(model.profile?.name ?? "")")
            .accessibilityHint("Switch profile, or make a new one")
            .accessibilityIdentifier("profile.menu")
            Spacer(minLength: 0)
        }
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
