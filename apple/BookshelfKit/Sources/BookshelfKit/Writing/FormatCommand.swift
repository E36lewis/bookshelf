import BookshelfFFI

/// The writing page's formatting commands, in the order the Format menu
/// and the toolbar show them. Each is one of the core's `FormatAction`s.
public enum FormatCommand: CaseIterable, Hashable, Sendable {
    case bold, italic, strikethrough, code
    case heading1, heading2, heading3
    case quote, bulletedList, numberedList
    case link

    /// The menu item's title, also the toolbar button's label and the name
    /// Edit › Undo gives it.
    public var title: String {
        switch self {
        case .bold: "Bold"
        case .italic: "Italic"
        case .strikethrough: "Strikethrough"
        case .code: "Code"
        case .heading1: "Heading 1"
        case .heading2: "Heading 2"
        case .heading3: "Heading 3"
        case .quote: "Quote"
        case .bulletedList: "Bulleted List"
        case .numberedList: "Numbered List"
        case .link: "Link"
        }
    }

    /// What the core does for it.
    public var action: FormatAction {
        switch self {
        case .bold: .bold
        case .italic: .italic
        case .strikethrough: .strike
        case .code: .code
        case .heading1: .heading(level: 1)
        case .heading2: .heading(level: 2)
        case .heading3: .heading(level: 3)
        case .quote: .quote
        case .bulletedList: .bullets
        case .numberedList: .numbered
        case .link: .link
        }
    }

    /// The SF Symbol on its toolbar button.
    public var systemImage: String {
        switch self {
        case .bold: "bold"
        case .italic: "italic"
        case .strikethrough: "strikethrough"
        case .code: "chevron.left.forwardslash.chevron.right"
        case .heading1: "1.square"
        case .heading2: "2.square"
        case .heading3: "3.square"
        case .quote: "text.quote"
        case .bulletedList: "list.bullet"
        case .numberedList: "list.number"
        case .link: "link"
        }
    }

    /// The title of its keys in the core's shortcut table (`MenuShortcuts`),
    /// for the ones that have keys.
    public var shortcutTitle: String? {
        switch self {
        case .bold: MenuShortcuts.bold
        case .italic: MenuShortcuts.italic
        case .link: MenuShortcuts.link
        default: nil
        }
    }

    /// The menu's groups, with a divider between each.
    public static let groups: [[FormatCommand]] = [
        [.bold, .italic, .strikethrough, .code],
        [.heading1, .heading2, .heading3],
        [.quote, .bulletedList, .numberedList],
        [.link],
    ]

    /// The heading levels, in the toolbar's Heading menu.
    public static let headings: [FormatCommand] = [.heading1, .heading2, .heading3]
}
