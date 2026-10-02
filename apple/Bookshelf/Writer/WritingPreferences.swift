import AppKit
import SwiftUI

/// Edit › Spelling and Grammar and Edit › Substitutions for the writing
/// page, remembered across launches. Smart quotes, smart dashes and text
/// replacement start off: they quietly turn Markdown's `"`, `--` and
/// `(c)` into other characters.
@MainActor
@Observable
final class WritingPreferences {
    /// The settings as one value, to hand to the text view.
    struct Values: Equatable {
        var checkSpelling: Bool
        var checkGrammar: Bool
        var correctSpelling: Bool
        var smartCopyPaste: Bool
        var smartQuotes: Bool
        var smartDashes: Bool
        var textReplacement: Bool
    }

    var values: Values {
        didSet { if values != oldValue { store() } }
    }

    private let defaults: UserDefaults
    private static let prefix = "writing."

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        func flag(_ key: String, _ fallback: Bool) -> Bool {
            defaults.object(forKey: Self.prefix + key) as? Bool ?? fallback
        }
        values = Values(
            checkSpelling: flag("checkSpelling", true),
            checkGrammar: flag("checkGrammar", false),
            // Follows System Settings › Keyboard until changed here.
            correctSpelling: flag("correctSpelling", NSSpellChecker.isAutomaticSpellingCorrectionEnabled),
            smartCopyPaste: flag("smartCopyPaste", true),
            smartQuotes: flag("smartQuotes", false),
            smartDashes: flag("smartDashes", false),
            textReplacement: flag("textReplacement", false))
    }

    private func store() {
        let all: [(String, Bool)] = [
            ("checkSpelling", values.checkSpelling), ("checkGrammar", values.checkGrammar),
            ("correctSpelling", values.correctSpelling), ("smartCopyPaste", values.smartCopyPaste),
            ("smartQuotes", values.smartQuotes), ("smartDashes", values.smartDashes),
            ("textReplacement", values.textReplacement),
        ]
        for (key, value) in all { defaults.set(value, forKey: Self.prefix + key) }
    }

    /// A menu toggle's binding for one setting.
    func binding(_ key: WritableKeyPath<Values, Bool>) -> Binding<Bool> {
        Binding(get: { self.values[keyPath: key] }, set: { self.values[keyPath: key] = $0 })
    }
}

extension WritingPreferences.Values {
    /// What `view` has now (changed from its own menus or panels).
    @MainActor
    init(_ view: NSTextView) {
        self.init(
            checkSpelling: view.isContinuousSpellCheckingEnabled,
            checkGrammar: view.isGrammarCheckingEnabled,
            correctSpelling: view.isAutomaticSpellingCorrectionEnabled,
            smartCopyPaste: view.smartInsertDeleteEnabled,
            smartQuotes: view.isAutomaticQuoteSubstitutionEnabled,
            smartDashes: view.isAutomaticDashSubstitutionEnabled,
            textReplacement: view.isAutomaticTextReplacementEnabled)
    }

    /// Sets them on `view`, touching only what differs.
    @MainActor
    func apply(to view: NSTextView) {
        if view.isContinuousSpellCheckingEnabled != checkSpelling { view.isContinuousSpellCheckingEnabled = checkSpelling }
        if view.isGrammarCheckingEnabled != checkGrammar { view.isGrammarCheckingEnabled = checkGrammar }
        if view.isAutomaticSpellingCorrectionEnabled != correctSpelling {
            view.isAutomaticSpellingCorrectionEnabled = correctSpelling
        }
        if view.smartInsertDeleteEnabled != smartCopyPaste { view.smartInsertDeleteEnabled = smartCopyPaste }
        if view.isAutomaticQuoteSubstitutionEnabled != smartQuotes { view.isAutomaticQuoteSubstitutionEnabled = smartQuotes }
        if view.isAutomaticDashSubstitutionEnabled != smartDashes { view.isAutomaticDashSubstitutionEnabled = smartDashes }
        if view.isAutomaticTextReplacementEnabled != textReplacement {
            view.isAutomaticTextReplacementEnabled = textReplacement
        }
    }
}
