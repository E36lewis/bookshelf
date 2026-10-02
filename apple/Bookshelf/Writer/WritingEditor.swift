import AppKit
import SwiftUI
import BookshelfKit

/// The writing page's text, in a `WritingTextView`. The text view owns the
/// text while the page is open; the session hears about each change and
/// saves it.
struct WritingEditor: NSViewRepresentable {
    /// How the page sets its text: from the profile's settings.
    struct Look: Equatable {
        var font: WritingFont
        /// Points.
        var size: CGFloat
        /// The widest the column gets, in points.
        var column: CGFloat
        var lineSpacing: CGFloat
        var paragraphSpacing: CGFloat
        /// The caret, in the accent color (`#rrggbb`, light and dark).
        var caretLight: String?
        var caretDark: String?
    }

    let session: WritingSession
    let look: Look
    let focusMode: Bool
    let checking: WritingPreferences.Values
    /// The text view arrived (`true`) or left (`false`): menu commands go to it.
    let onEditor: (WritingTextView, Bool) -> Void
    /// Spelling or substitutions changed from the text view's own menus.
    let onCheckingChanged: (WritingPreferences.Values) -> Void
    /// UI tests only: an element that says what the styler set.
    var showsStyleProbe = false

    func makeCoordinator() -> Coordinator {
        Coordinator(session: session)
    }

    func makeNSView(context: Context) -> WritingPageView {
        let page = WritingPageView(text: session.text, prompts: session.prompts, look: look,
                                   styleProbe: showsStyleProbe)
        let coordinator = context.coordinator
        coordinator.page = page
        coordinator.onEditor = onEditor
        page.textView.delegate = coordinator
        page.textView.setAccessibilityLabel("Summary of \(session.title)")
        page.textView.setAccessibilityHelp("Markdown text. Saves as you type.")
        page.textView.applyChecking(checking)
        page.textView.isFocusing = focusMode
        page.textView.onCheckingChanged = { view in
            coordinator.onCheckingChanged?(WritingPreferences.Values(view))
        }
        onEditor(page.textView, true)
        return page
    }

    func updateNSView(_ page: WritingPageView, context: Context) {
        context.coordinator.onCheckingChanged = onCheckingChanged
        page.apply(look)
        page.textView.isFocusing = focusMode
        page.textView.applyChecking(checking)
    }

    static func dismantleNSView(_ page: WritingPageView, coordinator: Coordinator) {
        page.textView.onCheckingChanged = nil
        coordinator.onEditor?(page.textView, false)
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate {
        let session: WritingSession
        weak var page: WritingPageView?
        var onEditor: ((WritingTextView, Bool) -> Void)?
        var onCheckingChanged: ((WritingPreferences.Values) -> Void)?

        init(session: WritingSession) {
            self.session = session
        }

        func textDidChange(_ notification: Notification) {
            guard let view = notification.object as? NSTextView else { return }
            session.update(view.string)
            page?.updateStyleProbe()
        }
    }
}

/// The scroll view and text view, laid out as one centered column with
/// half a page of room below the last line.
final class WritingPageView: NSView {
    let scrollView = NSScrollView()
    let textView: WritingTextView
    private let highlighter: LiveHighlighter
    private let prompts: String
    private var look: WritingEditor.Look?
    private var column: CGFloat = 540
    private var styleProbe: StyleProbe?

    init(text: String, prompts: String, look: WritingEditor.Look, styleProbe: Bool) {
        // TextKit 1, built by hand: predictable, and what the highlighter,
        // focus mode and typewriter scrolling are written against.
        let storage = NSTextStorage(string: text)
        let layoutManager = NSLayoutManager()
        storage.addLayoutManager(layoutManager)
        let container = NSTextContainer(
            containerSize: NSSize(width: look.column, height: CGFloat.greatestFiniteMagnitude))
        container.widthTracksTextView = true
        container.lineFragmentPadding = 0
        layoutManager.addTextContainer(container)
        textView = WritingTextView(frame: NSRect(x: 0, y: 0, width: look.column, height: 400), textContainer: container)
        highlighter = LiveHighlighter(styler: MarkdownStyler(look: look.writingLook))
        self.prompts = prompts
        super.init(frame: .zero)
        configure()
        apply(look)
        highlighter.textView = textView
        storage.delegate = highlighter
        textView.setSelectedRange(NSRange(location: storage.length, length: 0))
        if styleProbe {
            let probe = StyleProbe(frame: NSRect(x: 0, y: 0, width: 1, height: 1))
            addSubview(probe)
            self.styleProbe = probe
            updateStyleProbe()
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("not used")
    }

    private func configure() {
        scrollView.frame = bounds
        scrollView.autoresizingMask = [.width, .height]
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = true
        scrollView.backgroundColor = .textBackgroundColor
        scrollView.findBarPosition = .aboveContent
        scrollView.documentView = textView
        addSubview(scrollView)

        textView.minSize = .zero
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        textView.drawsBackground = true
        textView.backgroundColor = .textBackgroundColor
        // Plain text: pasting brings words, not someone else's fonts.
        textView.isRichText = false
        textView.importsGraphics = false
        textView.allowsImageEditing = false
        textView.usesFontPanel = false
        textView.usesRuler = false
        textView.allowsUndo = true
        textView.usesFindBar = true
        textView.isIncrementalSearchingEnabled = true
        // Links and data detectors mean nothing in plain text.
        textView.isAutomaticLinkDetectionEnabled = false
        textView.isAutomaticDataDetectionEnabled = false
        textView.setAccessibilityIdentifier("writer.text")
    }

    /// Restyles everything for a new look (a settings change); does nothing
    /// if it's the same.
    func apply(_ look: WritingEditor.Look) {
        guard look != self.look else { return }
        self.look = look
        let writing = look.writingLook
        let styler = MarkdownStyler(look: writing)
        highlighter.styler = styler
        textView.defaultParagraphStyle = styler.paragraphStyle
        textView.typingAttributes = styler.plainAttributes
        if let storage = textView.textStorage { styler.restyleAll(storage) }
        textView.prompts = NSAttributedString(string: prompts, attributes: [
            .font: writing.fonts.regular,
            .foregroundColor: styler.palette.prompt,
            .paragraphStyle: styler.paragraphStyle,
        ])
        textView.dimmedColor = styler.palette.dimmed
        textView.insertionPointColor = Self.caret(light: look.caretLight, dark: look.caretDark)
        textView.refreshFocus()
        column = look.column
        updateMargins()
    }

    /// The accent for the caret, following the appearance.
    private static func caret(light: String?, dark: String?) -> NSColor {
        guard let light = light.flatMap(RGB.init(hex:)), let dark = dark.flatMap(RGB.init(hex:)) else {
            return .textColor
        }
        return NSColor(name: nil) { appearance in
            let rgb = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
            return NSColor(srgbRed: rgb.red, green: rgb.green, blue: rgb.blue, alpha: 1)
        }
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateMargins()
    }

    override func layout() {
        super.layout()
        updateMargins()
    }

    /// Centers the column, and leaves half a page below the last line so
    /// it can be written mid-page rather than at the bottom edge.
    private func updateMargins() {
        let size = scrollView.contentSize
        guard size.width > 0, size.height > 0 else { return }
        let side = max(28, floor((size.width - column) / 2))
        let below = max(120, floor(size.height / 2))
        let inset = NSSize(width: side, height: ceil((below + textView.topMargin) / 2))
        if textView.textContainerInset != inset { textView.textContainerInset = inset }
        if textView.minSize.height != size.height { textView.minSize = NSSize(width: 0, height: size.height) }
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard window != nil else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, let window else { return }
            window.makeFirstResponder(textView)
            textView.scrollRangeToVisible(textView.selectedRange())
            if textView.isFocusing { textView.centerCaretLine() }
        }
    }

    /// UI tests: what the styler set, for the probe to report.
    func updateStyleProbe() {
        guard let styleProbe, let storage = textView.textStorage else { return }
        styleProbe.text = MarkdownStyler.describe(storage)
    }
}

extension WritingEditor.Look {
    /// The styler's look, with the writing font's faces.
    @MainActor
    var writingLook: WritingLook {
        WritingLook(fonts: Typeface.writingFonts(font, size: size), lineSpacing: lineSpacing,
                    paragraphSpacing: paragraphSpacing)
    }
}

/// For UI tests (`-BookshelfStyleProbe`): an invisible element whose value
/// is what the styler set (`MarkdownStyler.describe`), since a UI test
/// can't read a text view's attributes.
final class StyleProbe: NSView {
    var text = ""

    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .staticText }
    override func accessibilityValue() -> Any? { text }
    override func accessibilityIdentifier() -> String { "writer.styles" }
}
