namespace Bookshelf.Core.Editing;

/// <summary>
/// Lines still to be highlighted. Opening or pasting a long text highlights
/// the lines around the caret at once and leaves the rest here, to be done
/// a few lines at a time while the editor is idle, so the page is never
/// frozen. Ranges follow the text as it's edited in the meantime.
/// Offsets are UTF-16 code units; lines are separated by <c>'\n'</c>.
/// </summary>
public sealed class PendingLines
{
    private readonly List<(int Start, int End)> _ranges = [];

    /// <summary>Whether everything has been highlighted.</summary>
    public bool IsEmpty => _ranges.Count == 0;

    /// <summary>Notes <c>[start, end)</c> as still to do (nothing if it's empty).</summary>
    public void Add(int start, int end)
    {
        if (end > start) _ranges.Add((start, end));
    }

    public void Clear() => _ranges.Clear();

    /// <summary>Moves the ranges along after <paramref name="change"/>, which left a text <paramref name="length"/> long.</summary>
    public void Shift(TextChange change, int length)
    {
        for (var i = _ranges.Count - 1; i >= 0; i--)
        {
            var (start, end) = TextDiff.Moved(_ranges[i], change);
            start = Math.Clamp(start, 0, length);
            end = Math.Clamp(end, start, length);
            if (end > start) _ranges[i] = (start, end);
            else _ranges.RemoveAt(i);
        }
    }

    /// <summary>
    /// Takes the next few lines off the list: at most <paramref name="maxLines"/>
    /// whole lines of <paramref name="text"/>, as <c>[Start, End)</c> without the
    /// last line break. Null when there's nothing left.
    /// </summary>
    public (int Start, int End)? Take(string text, int maxLines)
    {
        while (_ranges.Count > 0)
        {
            var (start, end) = _ranges[0];
            start = Math.Clamp(start, 0, text.Length);
            end = Math.Clamp(end, start, text.Length);
            if (end <= start)
            {
                _ranges.RemoveAt(0);
                continue;
            }
            // Begin at the start of a line, so lines are whole.
            start = start == 0 ? 0 : text.LastIndexOf('\n', start - 1) + 1;
            var lineEnd = start;
            var next = start;
            for (var lines = 0; lines < Math.Max(1, maxLines) && next < end; lines++)
            {
                var newline = text.IndexOf('\n', next);
                lineEnd = newline < 0 ? text.Length : newline;
                next = newline < 0 ? text.Length : newline + 1;
            }
            if (next >= end) _ranges.RemoveAt(0);
            else _ranges[0] = (next, end);
            return (start, lineEnd);
        }
        return null;
    }

    /// <summary>
    /// The whole lines from <paramref name="lines"/> before the line holding
    /// <paramref name="at"/> to <paramref name="lines"/> after it, as
    /// <c>[Start, End)</c> without the last line break.
    /// </summary>
    public static (int Start, int End) Around(string text, int at, int lines)
    {
        var (start, end) = TextDiff.WholeLines(text, at, at);
        // start - 1 is the line break ending the line before; that line starts after the break before it.
        for (var i = 0; i < lines && start > 0; i++) start = start == 1 ? 0 : text.LastIndexOf('\n', start - 2) + 1;
        for (var i = 0; i < lines && end < text.Length; i++)
        {
            var newline = text.IndexOf('\n', end + 1);
            end = newline < 0 ? text.Length : newline;
        }
        return (start, end);
    }

    /// <summary>How many lines <c>[start, end)</c> touches.</summary>
    public static int LineCount(string text, int start, int end) =>
        end <= start ? 1 : text.AsSpan(start, end - start).Count('\n') + 1;
}
