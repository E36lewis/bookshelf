import AppKit
import BookshelfKit

/// The writing page's text view (TextKit 1, plain text): one centered
/// column with room below the last line, the prompts on an empty page,
/// focus mode, typewriter scrolling, and the page's own undo history.
final class WritingTextView: NSTextView {
    /// Space above the first line.
    var topMargin: CGFloat = 36 {
        didSet { if oldValue != topMargin { invalidateTextContainerOrigin() } }
    }

    /// The questions drawn where the text starts while the page is empty.
    var prompts = NSAttributedString() {
        didSet {
            setAccessibilityPlaceholderValue(prompts.string)
            if isEmpty { needsDisplay = true }
        }
    }

    /// Dims everything but the sentence being written, and keeps the line
    /// being written in the middle of the page.
    var isFocusing = false {
        didSet {
            guard oldValue != isFocusing else { return }
            refreshFocus()
            if isFocusing { scheduleCentering() }
        }
    }

    /// The color of everything but the current sentence in focus mode.
    var dimmedColor: NSColor = WritingPalette().dimmed

    /// Called when a Substitutions or Spelling setting changes from the
    /// text view's own menus or panels, to keep the app's in step.
    var onCheckingChanged: ((WritingTextView) -> Void)?

    /// The page's own undo history: typing here never mixes with removing
    /// books from the shelves.
    let pageUndo = UndoManager()

    private var wasEmpty = true
    private var centeringScheduled = false

    private var isEmpty: Bool { (textStorage?.length ?? 0) == 0 }

    // MARK: Layout

    /// The text starts `topMargin` down; `textContainerInset.height` (twice
    /// over, as the text view sizes itself) is the room below the last line.
    override var textContainerOrigin: NSPoint {
        NSPoint(x: textContainerInset.width, y: topMargin)
    }

    override var undoManager: UndoManager? { pageUndo }

    // Edit › Undo and Redo reach the text view first while it has focus.
    @objc func undo(_ sender: Any?) {
        pageUndo.undo()
    }

    @objc func redo(_ sender: Any?) {
        pageUndo.redo()
    }

    override func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(undo(_:)):
            item.title = pageUndo.undoMenuItemTitle
            return pageUndo.canUndo
        case #selector(redo(_:)):
            item.title = pageUndo.redoMenuItemTitle
            return pageUndo.canRedo
        default:
            return super.validateMenuItem(item)
        }
    }

    // MARK: Prompts

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard isEmpty, prompts.length > 0, let container = textContainer else { return }
        let padding = container.lineFragmentPadding
        let origin = textContainerOrigin
        let rect = NSRect(
            x: origin.x + padding, y: origin.y,
            width: max(0, container.size.width - padding * 2), height: bounds.height - origin.y)
        prompts.draw(with: rect, options: [.usesLineFragmentOrigin, .usesFontLeading])
    }

    override func didChangeText() {
        super.didChangeText()
        // The prompts go on the first keystroke, and come back on an empty page.
        if isEmpty != wasEmpty {
            wasEmpty = isEmpty
            needsDisplay = true
        }
        if isFocusing { scheduleCentering() }
    }

    // MARK: Focus mode

    override func setSelectedRanges(
        _ ranges: [NSValue], affinity: NSSelectionAffinity, stillSelecting: Bool
    ) {
        super.setSelectedRanges(ranges, affinity: affinity, stillSelecting: stillSelecting)
        guard isFocusing, !stillSelecting else { return }
        refreshFocus()
        scheduleCentering()
    }

    /// Dims all but the current sentence, with the layout manager's
    /// temporary attributes: they change only how the text is drawn, so the
    /// text itself, its undo and what VoiceOver reads are untouched.
    func refreshFocus() {
        guard let layout = layoutManager else { return }
        let length = textStorage?.length ?? 0
        let all = NSRange(location: 0, length: length)
        layout.removeTemporaryAttribute(.foregroundColor, forCharacterRange: all)
        guard isFocusing, length > 0 else { return }
        let lit = Sentences.around(selectedRange().location, in: string as NSString)
        if lit.location > 0 {
            layout.addTemporaryAttribute(
                .foregroundColor, value: dimmedColor, forCharacterRange: NSRange(location: 0, length: lit.location))
        }
        let end = NSMaxRange(lit)
        if end < length {
            layout.addTemporaryAttribute(
                .foregroundColor, value: dimmedColor, forCharacterRange: NSRange(location: end, length: length - end))
        }
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        // The colors are dynamic; this just redraws with the new look.
        needsDisplay = true
    }

    /// Typewriter scrolling, once the text view has finished its own
    /// scrolling and layout for this change.
    private func scheduleCentering() {
        guard !centeringScheduled else { return }
        centeringScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            centeringScheduled = false
            if isFocusing { centerCaretLine() }
        }
    }

    /// Scrolls so the line with the caret sits mid-page (as near as the
    /// top of the text allows). Instant when Reduce Motion is on.
    func centerCaretLine() {
        guard let scrollView = enclosingScrollView, let line = caretLineRect() else { return }
        let clip = scrollView.contentView
        let visible = clip.bounds
        let top = max(0, min(line.midY - visible.height / 2, frame.height - visible.height))
        guard abs(top - visible.origin.y) > 0.5 else { return }
        let target = NSPoint(x: visible.origin.x, y: top)
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            clip.scroll(to: target)
            scrollView.reflectScrolledClipView(clip)
        } else {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.18
                context.allowsImplicitAnimation = true
                clip.animator().setBoundsOrigin(target)
            } completionHandler: {
                scrollView.reflectScrolledClipView(clip)
            }
        }
    }

    /// The line the caret is on, in the text view's coordinates.
    private func caretLineRect() -> NSRect? {
        guard let layout = layoutManager, let container = textContainer else { return nil }
        let length = textStorage?.length ?? 0
        let caret = min(selectedRange().location, length)
        layout.ensureLayout(forCharacterRange: NSRange(location: 0, length: min(caret + 1, length)))
        var rect: NSRect
        if caret == length, layout.extraLineFragmentTextContainer === container {
            // After a final newline, or on an empty page.
            rect = layout.extraLineFragmentRect
        } else if length > 0 {
            let glyph = layout.glyphIndexForCharacter(at: min(caret, length - 1))
            rect = layout.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        } else {
            return nil
        }
        rect.origin.y += textContainerOrigin.y
        return rect
    }

    // MARK: Keys

    /// Esc leaves full screen first; otherwise it does what it always does
    /// in text (offers completions).
    override func cancelOperation(_ sender: Any?) {
        if let window, window.styleMask.contains(.fullScreen) {
            window.toggleFullScreen(nil)
        } else {
            super.cancelOperation(sender)
        }
    }

    // MARK: Menu commands

    /// Edit › Find: the find bar's actions.
    func performFind(_ action: NSTextFinder.Action) {
        let sender = NSMenuItem()
        sender.tag = action.rawValue
        window?.makeFirstResponder(self)
        performTextFinderAction(sender)
    }

    /// A standard text command (Check Document Now, Make Upper Case…).
    func perform(command: Selector) {
        window?.makeFirstResponder(self)
        _ = tryToPerform(command, with: nil)
    }

    /// A formatting command, as one undo step named after it.
    func format(_ action: FormatAction, named name: String) {
        window?.makeFirstResponder(self)
        applyFormat(action, undoName: name)
    }

    // MARK: Checking and substitutions

    /// Off while the app's own settings are being applied.
    private var reportsChecking = true

    /// Applies the remembered Spelling and Substitutions settings.
    func applyChecking(_ values: WritingPreferences.Values) {
        reportsChecking = false
        values.apply(to: self)
        reportsChecking = true
    }

    // Changes made from the text view's own context menu or the system's
    // Substitutions panel come back here, to be remembered.
    override var isContinuousSpellCheckingEnabled: Bool {
        didSet { if reportsChecking, oldValue != isContinuousSpellCheckingEnabled { onCheckingChanged?(self) } }
    }

    override var isGrammarCheckingEnabled: Bool {
        didSet { if reportsChecking, oldValue != isGrammarCheckingEnabled { onCheckingChanged?(self) } }
    }

    override var isAutomaticSpellingCorrectionEnabled: Bool {
        didSet { if reportsChecking, oldValue != isAutomaticSpellingCorrectionEnabled { onCheckingChanged?(self) } }
    }

    override var isAutomaticQuoteSubstitutionEnabled: Bool {
        didSet { if reportsChecking, oldValue != isAutomaticQuoteSubstitutionEnabled { onCheckingChanged?(self) } }
    }

    override var isAutomaticDashSubstitutionEnabled: Bool {
        didSet { if reportsChecking, oldValue != isAutomaticDashSubstitutionEnabled { onCheckingChanged?(self) } }
    }

    override var isAutomaticTextReplacementEnabled: Bool {
        didSet { if reportsChecking, oldValue != isAutomaticTextReplacementEnabled { onCheckingChanged?(self) } }
    }

    override var smartInsertDeleteEnabled: Bool {
        didSet { if reportsChecking, oldValue != smartInsertDeleteEnabled { onCheckingChanged?(self) } }
    }
}
