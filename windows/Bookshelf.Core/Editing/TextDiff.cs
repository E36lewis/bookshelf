namespace Bookshelf.Core.Editing;

/// <summary>
/// Where two versions of a text differ: <c>[Start, OldEnd)</c> in the old one
/// became <c>[Start, NewEnd)</c> in the new one. Offsets are UTF-16 code
/// units, like C# strings and RichEditBox.
/// </summary>
public readonly record struct TextChange(int Start, int OldEnd, int NewEnd)
{
    public bool IsEmpty => OldEnd == Start && NewEnd == Start;

    /// <summary>How much longer the text got (negative: shorter).</summary>
    public int Growth => NewEnd - OldEnd;
}

public static class TextDiff
{
    /// <summary>
    /// The smallest single change that turns <paramref name="before"/> into
    /// <paramref name="after"/>. It never starts or ends inside a character
    /// that takes two code units (an emoji, say), so the editor is never
    /// asked to replace half of one.
    /// </summary>
    public static TextChange Between(string before, string after)
    {
        var max = Math.Min(before.Length, after.Length);
        var prefix = 0;
        while (prefix < max && before[prefix] == after[prefix]) prefix++;
        if (prefix > 0 && char.IsHighSurrogate(before[prefix - 1])) prefix--;
        var suffix = 0;
        while (suffix < max - prefix && before[before.Length - 1 - suffix] == after[after.Length - 1 - suffix])
            suffix++;
        if (suffix > 0 && char.IsLowSurrogate(before[before.Length - suffix])) suffix--;
        return new TextChange(prefix, before.Length - suffix, after.Length - suffix);
    }

    /// <summary>
    /// Replacing <c>[start, end)</c> of <paramref name="text"/> with
    /// <paramref name="replacement"/>, narrowed to the part that really
    /// changes: Heading 1 on a line only adds "# " in front, it doesn't
    /// retype the line. Returns that change and the text it inserts.
    /// </summary>
    public static (TextChange Change, string Inserted) Narrow(string text, int start, int end, string replacement)
    {
        start = Math.Clamp(start, 0, text.Length);
        end = Math.Clamp(end, start, text.Length);
        var inner = Between(text.Substring(start, end - start), replacement);
        var change = new TextChange(start + inner.Start, start + inner.OldEnd, start + inner.NewEnd);
        return (change, replacement.Substring(inner.Start, inner.NewEnd - inner.Start));
    }

    /// <summary>
    /// Where <paramref name="range"/> (in the old text) is after
    /// <paramref name="change"/>: moved along if it's after the change,
    /// grown to cover the change if they overlap.
    /// </summary>
    public static (int Start, int End) Moved((int Start, int End) range, TextChange change)
    {
        if (range.End < change.Start) return range;
        if (range.Start > change.OldEnd) return (range.Start + change.Growth, range.End + change.Growth);
        var start = Math.Min(range.Start, change.Start);
        var end = Math.Max(range.End + change.Growth, change.NewEnd);
        return (start, Math.Max(start, end));
    }

    /// <summary>
    /// Grows <c>[start, end)</c> to the whole lines it touches, with lines
    /// separated by <c>'\n'</c>. Highlighting is per line, so this is all that
    /// needs restyling after an edit.
    /// </summary>
    public static (int Start, int End) WholeLines(string text, int start, int end)
    {
        start = Math.Clamp(start, 0, text.Length);
        end = Math.Clamp(end, start, text.Length);
        var lineStart = start == 0 ? 0 : text.LastIndexOf('\n', start - 1) + 1;
        var lineEnd = text.IndexOf('\n', end);
        return (lineStart, lineEnd < 0 ? text.Length : lineEnd);
    }
}
