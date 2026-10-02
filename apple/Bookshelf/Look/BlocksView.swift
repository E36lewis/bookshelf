import SwiftUI
import BookshelfKit

/// Markdown laid out natively from the core's blocks (`render_markdown`):
/// the summary on a book page, the reader, and the manual. HTML is never
/// interpreted; it shows as the text that was typed.
struct BlocksView: View {
    let blocks: [Block]
    /// The family for text and headings.
    var font: HeadingFont = .serif
    /// The body size in points.
    var size: CGFloat = 15
    /// Extra space between the wrapped rows of a paragraph.
    var lineSpacing: CGFloat = 4

    private var paragraphGap: CGFloat { size * 0.75 }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { index, block in
                view(for: block, first: index == 0)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func view(for block: Block, first: Bool) -> some View {
        switch block {
        case let .heading(level, runs):
            Self.text(runs, code: Typeface.code(size: size))
                .font(Typeface.title(font, size: headingSize(level)))
                .padding(.top, first ? 0 : paragraphGap * 0.8)
                .padding(.bottom, paragraphGap * 0.5)
                .accessibilityAddTraits(.isHeader)
                .accessibilityHeading(level <= 1 ? .h1 : level == 2 ? .h2 : level == 3 ? .h3 : .h4)
        case let .paragraph(runs):
            Self.text(runs, code: Typeface.code(size: size))
                .font(Typeface.text(font, size: size))
                .lineSpacing(lineSpacing)
                .padding(.bottom, paragraphGap)
        case let .listItem(depth, marker, runs, loose):
            HStack(alignment: .firstTextBaseline, spacing: size * 0.5) {
                Text(markerText(marker))
                    .font(Typeface.text(font, size: size))
                    .foregroundStyle(.secondary)
                    .frame(minWidth: size * 1.1, alignment: .trailing)
                    .accessibilityHidden(marker == .bullet)
                Self.text(runs, code: Typeface.code(size: size))
                    .font(Typeface.text(font, size: size))
                    .lineSpacing(lineSpacing)
            }
            .padding(.leading, CGFloat(depth) * size * 1.4)
            .padding(.bottom, loose ? paragraphGap : size * 0.3)
        case let .itemText(runs):
            Self.text(runs, code: Typeface.code(size: size))
                .font(Typeface.text(font, size: size))
                .lineSpacing(lineSpacing)
                .padding(.leading, size * 1.6)
                .padding(.bottom, size * 0.3)
        case let .listEnd(depth):
            Color.clear.frame(height: depth == 0 ? paragraphGap * 0.6 : 0)
        case let .codeBlock(text), let .html(text):
            Text(text.hasSuffix("\n") ? String(text.dropLast()) : text)
                .font(Typeface.code(size: size))
                .lineSpacing(2)
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 6))
                .padding(.bottom, paragraphGap)
        case .rule:
            Divider()
                .padding(.vertical, paragraphGap)
        }
    }

    private func headingSize(_ level: UInt8) -> CGFloat {
        switch level {
        case 1: size * 1.6
        case 2: size * 1.35
        case 3: size * 1.15
        default: size
        }
    }

    private func markerText(_ marker: ListMarker) -> String {
        switch marker {
        case .bullet: "•"
        case .number(let value): "\(value)."
        }
    }

    /// Runs as one `Text`, each with its styles.
    static func text(_ runs: [Run], code: Font) -> Text {
        runs.reduce(Text(verbatim: "")) { line, run in
            var piece = Text(verbatim: run.text)
            for style in run.styles {
                switch style {
                case .bold: piece = piece.bold()
                case .italic: piece = piece.italic()
                case .strike: piece = piece.strikethrough()
                case .code: piece = piece.font(code)
                }
            }
            return line + piece
        }
    }
}
