namespace Bookshelf.Core.Editing;

/// <summary>
/// Where two versions of a text differ: <c>[Start, OldEnd)</c> in the old one
/// became <c>[Start, NewEnd)</c> in the new one. Offsets are UTF-16 code
/// units, like C# strings and RichEditBox.
/// </summary>
public readonly record struct TextChange(int Start, int OldEnd, int NewEnd)
{
    public bool IsEmpty => OldEnd == Start && NewEnd == Start;
}

public static class TextDiff
{
    /// <summary>The smallest single change that turns <paramref name="before"/> into <paramref name="after"/>.</summary>
    public static TextChange Between(string before, string after)
    {
        var max = Math.Min(before.Length, after.Length);
        var prefix = 0;
        while (prefix < max && before[prefix] == after[prefix]) prefix++;
        var suffix = 0;
        while (suffix < max - prefix && before[before.Length - 1 - suffix] == after[after.Length - 1 - suffix])
            suffix++;
        return new TextChange(prefix, before.Length - suffix, after.Length - suffix);
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
