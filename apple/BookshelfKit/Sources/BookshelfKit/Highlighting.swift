import Foundation

/// Markdown highlighting from the shared core, as ranges in a Swift string.
/// The core counts in UTF-16, like NSString and NSTextView.
public func highlightRanges(in text: String) -> [(kind: StyleKind, range: NSRange)] {
    markdownSpans(text: text).map { span in
        (span.kind, NSRange(location: Int(span.start), length: Int(span.end - span.start)))
    }
}
