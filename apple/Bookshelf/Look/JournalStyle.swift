import AppKit
import SwiftUI
import BookshelfKit

// The journal's look, the same as Bookshelf's on Linux (bookshelf-app's
// style.css and palette-*.css): paper behind, white cards, ink text, calm
// pill buttons and round icon buttons. Colors are only the shared palette's
// and the profile's accent, with the same mixes as style.css.

/// One of the palette's colors, in a light window and in a dark one.
struct Shade: Sendable {
    let light: RGB
    let dark: RGB

    init(_ light: String, _ dark: String) {
        let black = RGB(red: 0, green: 0, blue: 0)
        self.light = RGB(hex: light) ?? black
        self.dark = RGB(hex: dark) ?? black
    }

    private init(light: RGB, dark: RGB) {
        self.light = light
        self.dark = dark
    }

    /// `t` of the way to `other` (style.css's `mix()`).
    func mixed(with other: Shade, _ t: Double) -> Shade {
        Shade(light: light.mixed(with: other.light, t), dark: dark.mixed(with: other.dark, t))
    }

    /// Follows the window's look, light or dark.
    var color: Color {
        let light = light, dark = dark
        return Color(nsColor: NSColor(name: nil) { appearance in
            let rgb = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: rgb.red, green: rgb.green, blue: rgb.blue, alpha: 1)
        })
    }
}

extension RGB {
    /// `t` of the way to `other`, per channel.
    func mixed(with other: RGB, _ t: Double) -> RGB {
        RGB(red: red + (other.red - red) * t,
            green: green + (other.green - green) * t,
            blue: blue + (other.blue - blue) * t)
    }
}

extension Color {
    init(rgb: RGB) {
        self.init(.sRGB, red: rgb.red, green: rgb.green, blue: rgb.blue)
    }
}

/// The shared palette (palette-light.css, palette-dark.css) and style.css's
/// mixes of it.
enum Palette {
    private static let paperShade = Shade("#f1f2f3", "#0d181d")
    private static let cardShade = Shade("#ffffff", "#1c222c")
    private static let inkShade = Shade("#17233c", "#f0f2f4")
    private static let mutedShade = Shade("#70757d", "#a7abb0")
    private static let ruleShade = Shade("#e3e5e6", "#292e37")
    private static let black = Shade("#000000", "#000000")
    private static let destructiveShade = Shade("#d54507", "#f38b61")

    /// The desk: behind everything.
    static let paper = paperShade.color
    /// The page: cards, panels, unselected tabs and buttons.
    static let card = cardShade.color
    /// Text.
    static let ink = inkShade.color
    /// Lines between rows in a panel.
    static let rule = ruleShade.color
    /// Light ink: the edges of cards, panels, buttons and tabs.
    static let edge = ruleShade.mixed(with: mutedShade, 0.35).color
    /// Medium ink: the edges of fields and choices, a switch that's off.
    static let field = mutedShade.mixed(with: cardShade, 0.15).color

    /// A card under the pointer, and pressed.
    static let cardHover = cardShade.mixed(with: inkShade, 0.035).color
    static let cardPressed = cardShade.mixed(with: inkShade, 0.07).color
    /// A button or tab under the pointer, and pressed.
    static let controlHover = cardShade.mixed(with: inkShade, 0.07).color
    static let controlPressed = cardShade.mixed(with: inkShade, 0.14).color
    /// Something switched on (the selected tab, the open search) under the pointer.
    static let inkHover = inkShade.mixed(with: cardShade, 0.15).color

    /// A destructive button: red, a shade deeper, with its words on it.
    static let destructive = destructiveShade.mixed(with: black, 0.1).color
    static let destructiveHover = destructiveShade.mixed(with: black, 0.16).color
    static let onDestructive = Shade("#ffffff", "#0d181d").color
}

/// The profile's accent as the controls use it.
struct JournalAccent {
    /// A primary button or a switch that's on: the accent a shade deeper,
    /// so the words on it keep 4.5:1 with every accent.
    var fill: Color
    var fillHover: Color
    /// Words and icons on `fill`.
    var onFill: Color
    /// The accent as text on the page, and the focus ring.
    var text: Color

    /// For an accent `base` (`#rrggbb`), with `onFill` and `text` from the core.
    init(base: String, onFill: String, text: String) {
        let black = RGB(red: 0, green: 0, blue: 0)
        let rgb = RGB(hex: base) ?? RGB(red: 0x2d / 255, green: 0x71 / 255, blue: 0xe5 / 255)
        fill = Color(rgb: rgb.mixed(with: black, 0.15))
        fillHover = Color(rgb: rgb.mixed(with: black, 0.27))
        self.onFill = Color(hex: onFill) ?? .white
        self.text = Color(hex: text) ?? .accentColor
    }

    /// The default blue, until a profile's accent is known.
    static let standard = JournalAccent(base: "#2d71e5", onFill: "#ffffff", text: "#2660c3")
}

private struct JournalAccentKey: EnvironmentKey {
    static let defaultValue = JournalAccent.standard
}

extension EnvironmentValues {
    var journalAccent: JournalAccent {
        get { self[JournalAccentKey.self] }
        set { self[JournalAccentKey.self] = newValue }
    }
}

// MARK: - Buttons

/// A calm pill led by its words (and an icon, if it has one): the card
/// with a light ink edge; `primary` is the accent, `destructive` red.
struct PillButtonStyle: ButtonStyle {
    enum Kind {
        case plain, primary, destructive
    }

    var kind: Kind = .plain

    func makeBody(configuration: Configuration) -> some View {
        PillButton(configuration: configuration, kind: kind)
    }

    private struct PillButton: View {
        let configuration: Configuration
        let kind: Kind

        @Environment(\.isEnabled) private var isEnabled
        @Environment(\.journalAccent) private var accent
        @State private var hovering = false

        var body: some View {
            let active = isEnabled && (hovering || configuration.isPressed)
            configuration.label
                .labelStyle(IconThenTitle())
                .font(.system(size: 13, weight: .semibold))
                .lineLimit(1)
                .foregroundStyle(foreground)
                .padding(.horizontal, 14)
                .padding(.vertical, 6)
                .frame(minHeight: 28)
                .background(Capsule().fill(fill(active: active)))
                .overlay(Capsule().strokeBorder(edge(active: active), lineWidth: 1))
                .contentShape(Capsule())
                .contentShape(.focusEffect, Capsule())
                .opacity(isEnabled ? 1 : 0.3)
                .onHover { hovering = $0 }
                .animation(.easeOut(duration: 0.1), value: active)
        }

        private var foreground: Color {
            switch kind {
            case .plain: Palette.ink
            case .primary: accent.onFill
            case .destructive: Palette.onDestructive
            }
        }

        private func fill(active: Bool) -> Color {
            switch kind {
            case .plain:
                configuration.isPressed ? Palette.controlPressed : active ? Palette.controlHover : Palette.card
            case .primary: active ? accent.fillHover : accent.fill
            case .destructive: active ? Palette.destructiveHover : Palette.destructive
            }
        }

        private func edge(active: Bool) -> Color {
            kind == .plain ? Palette.edge : fill(active: active)
        }
    }
}

/// A label's icon, then its title, a little apart.
struct IconThenTitle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 7) {
            configuration.icon
                .imageScale(.small)
            configuration.title
        }
    }
}

/// The header's round icon buttons, outlined. `isOn` (the open search)
/// fills them with ink.
struct CircleButtonStyle: ButtonStyle {
    var isOn = false

    func makeBody(configuration: Configuration) -> some View {
        CircleButton(configuration: configuration, isOn: isOn)
    }

    private struct CircleButton: View {
        let configuration: Configuration
        let isOn: Bool

        @Environment(\.isEnabled) private var isEnabled
        @State private var hovering = false

        var body: some View {
            configuration.label
                .labelStyle(.iconOnly)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(isOn ? Palette.card : Palette.ink)
                .frame(width: CircleChrome.size, height: CircleChrome.size)
                .background(Circle().fill(fill))
                .overlay(Circle().strokeBorder(isOn ? Palette.ink : Palette.edge, lineWidth: 1))
                .contentShape(Circle())
                .contentShape(.focusEffect, Circle())
                .opacity(isEnabled ? 1 : 0.3)
                .onHover { hovering = $0 }
        }

        private var fill: Color {
            let active = isEnabled && (hovering || configuration.isPressed)
            if isOn { return active ? Palette.inkHover : Palette.ink }
            if configuration.isPressed { return Palette.controlPressed }
            return active ? Palette.controlHover : Palette.card
        }
    }
}

/// The round outline of a header button, for a menu (which draws its own
/// button inside it).
struct CircleChrome: ViewModifier {
    static let size: CGFloat = 30

    func body(content: Content) -> some View {
        content
            .frame(width: Self.size, height: Self.size)
            .background(Circle().fill(Palette.card))
            .overlay(Circle().strokeBorder(Palette.edge, lineWidth: 1))
            .contentShape(Circle())
    }
}

/// Back: plain and muted, darker under the pointer.
struct BackButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        BackButton(configuration: configuration)
    }

    private struct BackButton: View {
        let configuration: Configuration
        @State private var hovering = false

        var body: some View {
            let active = hovering || configuration.isPressed
            configuration.label
                .labelStyle(.iconOnly)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(active ? Palette.ink : Color.quietText)
                .frame(width: CircleChrome.size, height: CircleChrome.size)
                .background(Circle().fill(Palette.ink.opacity(active ? 0.07 : 0)))
                .contentShape(Circle())
                .contentShape(.focusEffect, Circle())
                .onHover { hovering = $0 }
        }
    }
}

/// A shelf tab: a pill with its icon and name; ink when it's the shelf
/// showing.
struct ShelfTabStyle: ButtonStyle {
    let isSelected: Bool

    func makeBody(configuration: Configuration) -> some View {
        ShelfTab(configuration: configuration, isSelected: isSelected)
    }

    private struct ShelfTab: View {
        let configuration: Configuration
        let isSelected: Bool
        @State private var hovering = false

        var body: some View {
            configuration.label
                .labelStyle(IconThenTitle())
                .font(.system(size: 13, weight: .semibold))
                .lineLimit(1)
                .foregroundStyle(isSelected ? Palette.card : Palette.ink)
                .padding(.horizontal, 14)
                .padding(.vertical, 6)
                .background(Capsule().fill(fill))
                .overlay(Capsule().strokeBorder(isSelected ? Palette.ink : Palette.edge, lineWidth: 1))
                .contentShape(Capsule())
                .contentShape(.focusEffect, Capsule())
                .onHover { hovering = $0 }
                .animation(.easeOut(duration: 0.1), value: hovering)
        }

        private var fill: Color {
            let active = hovering || configuration.isPressed
            if isSelected { return active ? Palette.inkHover : Palette.ink }
            if configuration.isPressed { return Palette.controlPressed }
            return active ? Palette.controlHover : Palette.card
        }
    }
}

/// A book on the shelf: a card, a touch darker under the pointer.
struct CardButtonStyle: ButtonStyle {
    static let radius: CGFloat = 12

    func makeBody(configuration: Configuration) -> some View {
        Card(configuration: configuration)
    }

    private struct Card: View {
        let configuration: Configuration
        @State private var hovering = false

        var body: some View {
            let shape = RoundedRectangle(cornerRadius: CardButtonStyle.radius, style: .continuous)
            configuration.label
                .padding(16)
                .background(shape.fill(fill))
                .overlay(shape.strokeBorder(hovering ? Palette.field : Palette.edge, lineWidth: 1))
                .contentShape(shape)
                .contentShape(.focusEffect, shape)
                .onHover { hovering = $0 }
                .animation(.easeOut(duration: 0.12), value: hovering)
        }

        private var fill: Color {
            configuration.isPressed ? Palette.cardPressed : hovering ? Palette.cardHover : Palette.card
        }
    }
}

// MARK: - Pages, panels, fields, choices, switches

extension View {
    /// A page's background, with its header (the window's toolbar) the
    /// same color and no line between them, as on Linux.
    func pageBackground(_ color: Color) -> some View {
        background(color)
            .toolbarBackground(color, for: .windowToolbar)
            .toolbarBackground(.visible, for: .windowToolbar)
    }
}

/// A panel: the card's color, a light ink edge and round corners, for
/// Settings' groups and the book page's sections.
struct Panel: ViewModifier {
    var padding: CGFloat = 16

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: CardButtonStyle.radius, style: .continuous)
        content
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(shape.fill(Palette.card))
            .overlay(shape.strokeBorder(Palette.edge, lineWidth: 1))
    }
}

extension View {
    func panel(padding: CGFloat = 16) -> some View {
        modifier(Panel(padding: padding))
    }
}

/// A group's heading, in the title font, over its panel.
struct GroupHeading: View {
    let title: String
    var font: HeadingFont = .serif

    var body: some View {
        Text(title)
            .font(Typeface.title(font, size: 19))
            .foregroundStyle(Palette.ink)
            .accessibilityAddTraits(.isHeader)
    }
}

/// A text field drawn on the page: a medium ink edge, 16 pt text, and the
/// focus ring around it while it's being typed in.
struct FieldChrome: ViewModifier {
    let isFocused: Bool
    var isInvalid = false

    @Environment(\.journalAccent) private var accent

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: 8, style: .continuous)
        content
            .textFieldStyle(.plain)
            .font(.system(size: 16))
            .foregroundStyle(Palette.ink)
            .focusEffectDisabled()
            .padding(.horizontal, 12)
            .frame(minHeight: 38)
            .contentShape(shape)
            .overlay(shape.strokeBorder(
                isFocused ? Palette.ink : isInvalid ? Color.errorText : Palette.field, lineWidth: 1))
            .overlay(
                RoundedRectangle(cornerRadius: 11, style: .continuous)
                    .strokeBorder(accent.text, lineWidth: 2)
                    .padding(-4)
                    .opacity(isFocused ? 1 : 0))
    }
}

extension View {
    func fieldChrome(focused: Bool, invalid: Bool = false) -> some View {
        modifier(FieldChrome(isFocused: focused, isInvalid: invalid))
    }
}

/// One choice in a `ChoicePicker`.
struct Choice<Value: Hashable>: Identifiable {
    let value: Value
    let name: String
    var id: Value { value }

    init(_ value: Value, _ name: String) {
        self.value = value
        self.name = name
    }
}

/// A choice: its value in a pill with a small chevron, opening a menu of
/// the others.
struct ChoicePicker<Value: Hashable>: View {
    let title: String
    let choices: [Choice<Value>]
    @Binding var selection: Value

    var body: some View {
        let current = choices.first { $0.value == selection }?.name ?? ""
        Menu {
            Picker(title, selection: $selection) {
                ForEach(choices) { choice in
                    Text(choice.name).tag(choice.value)
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            Text(current)
                .font(.system(size: 13, weight: .semibold))
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.visible)
        .tint(Palette.ink)
        .fixedSize()
        .padding(.leading, 14)
        .padding(.trailing, 10)
        .padding(.vertical, 5)
        .overlay(Capsule().strokeBorder(Palette.field, lineWidth: 1))
        .contentShape(Capsule())
        .accessibilityLabel(title)
        .accessibilityValue(current)
    }
}

/// A switch: a pill track, medium ink when off and the accent when on,
/// with a round knob that slides across.
struct JournalSwitchStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        JournalSwitch(configuration: configuration)
    }

    private struct JournalSwitch: View {
        let configuration: Configuration
        @Environment(\.journalAccent) private var accent

        var body: some View {
            let on = configuration.isOn
            HStack(alignment: .center, spacing: 12) {
                configuration.label
                Spacer(minLength: 8)
                Capsule()
                    .fill(on ? accent.fill : Palette.field)
                    .frame(width: 46, height: 26)
                    .overlay(alignment: on ? .trailing : .leading) {
                        Circle()
                            .fill(on ? accent.onFill : Palette.card)
                            .frame(width: 20, height: 20)
                            .padding(3)
                    }
                    .animation(.easeOut(duration: 0.15), value: on)
            }
            .contentShape(Rectangle())
            .onTapGesture { configuration.isOn.toggle() }
        }
    }
}
