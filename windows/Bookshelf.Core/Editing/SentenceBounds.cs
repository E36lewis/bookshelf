namespace Bookshelf.Core.Editing;

/// <summary>
/// The sentence the caret is in, for focus mode, which dims everything
/// else. A sentence ends at <c>. ! ? …</c> (and any closing quotes or
/// brackets after it) followed by a space, or at the end of the line; a
/// line never holds part of another line's sentence, so a heading or a
/// list item is a sentence of its own. Offsets are UTF-16 code units.
/// </summary>
public static class SentenceBounds
{
    /// <summary>
    /// The sentence around <paramref name="caret"/>, without the spaces after
    /// it. The caret in those spaces belongs to the sentence before them;
    /// at the end of a line that ends in spaces it's an empty sentence (the
    /// next one, not written yet).
    /// </summary>
    public static (int Start, int End) Around(string text, int caret)
    {
        caret = Math.Clamp(caret, 0, text.Length);
        var (lineStart, lineEnd) = TextDiff.WholeLines(text, caret, caret);
        var start = lineStart;
        var i = lineStart;
        while (i < lineEnd)
        {
            if (!IsTerminator(text[i]))
            {
                i++;
                continue;
            }
            var end = i + 1;
            while (end < lineEnd && IsClosing(text[end])) end++;
            if (end < lineEnd && !char.IsWhiteSpace(text[end]))
            {
                i = end; // "3.5", "e.g.x": not the end of a sentence
                continue;
            }
            var next = end;
            while (next < lineEnd && char.IsWhiteSpace(text[next])) next++;
            if (caret < next || (next == lineEnd && caret == lineEnd && next == end))
            {
                return (start, end);
            }
            start = next;
            i = next;
        }
        return (start, TrimEnd(text, start, lineEnd));
    }

    private static int TrimEnd(string text, int start, int end)
    {
        while (end > start && char.IsWhiteSpace(text[end - 1])) end--;
        return end;
    }

    private static bool IsTerminator(char c) => c is '.' or '!' or '?' or '…' or '。' or '！' or '？';

    private static bool IsClosing(char c) =>
        c is '"' or '\'' or ')' or ']' or '”' or '’' or '»' or '*' or '_' or '`' or '~';
}
