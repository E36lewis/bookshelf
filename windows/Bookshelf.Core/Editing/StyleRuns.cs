using Bookshelf.Ffi;

namespace Bookshelf.Core.Editing;

/// <summary>How a run of text looks: the core's highlight kinds that cover it, and focus mode's dimming.</summary>
[Flags]
public enum TextStyle
{
    Plain = 0,
    Heading = 1 << 0,
    Bold = 1 << 1,
    Italic = 1 << 2,
    Quote = 1 << 3,
    Code = 1 << 4,
    Strike = 1 << 5,
    /// <summary>The Markdown marks themselves.</summary>
    Syntax = 1 << 6,
    /// <summary>Outside the sentence focus mode keeps bright.</summary>
    Dim = 1 << 7,
}

/// <summary><c>[Start, End)</c> of one look.</summary>
public readonly record struct StyledRun(int Start, int End, TextStyle Style);

/// <summary>
/// Cuts text into runs that each look one way, from the core's highlight
/// spans. The editor then makes one formatting call per run instead of
/// resetting everything and painting each span over it: each call into
/// RichEdit costs about a millisecond on a slow PC, so this is what keeps
/// typing quick.
/// </summary>
public static class StyleRuns
{
    /// <summary>
    /// Runs covering <c>[0, length)</c>, every character in exactly one, with
    /// neighbours always different. <paramref name="spans"/> use the same
    /// offsets (spans reaching past <paramref name="length"/> are cut there).
    /// With <paramref name="bright"/>, everything outside it is
    /// <see cref="TextStyle.Dim"/>.
    /// </summary>
    public static List<StyledRun> For(int length, IReadOnlyList<StyleSpan> spans, (int Start, int End)? bright = null)
    {
        var runs = new List<StyledRun>();
        if (length <= 0) return runs;

        // Every place the look can change, then the look between each two.
        var cuts = new List<int>(spans.Count * 2 + 4) { 0, length };
        foreach (var s in spans)
        {
            cuts.Add(Clamp(s.Start, length));
            cuts.Add(Clamp(s.End, length));
        }
        if (bright is { } b)
        {
            cuts.Add(Math.Clamp(b.Start, 0, length));
            cuts.Add(Math.Clamp(b.End, 0, length));
        }
        cuts.Sort();

        // Spans by start, walked once alongside the cuts.
        var bySt = spans.Where(s => s.End > s.Start).OrderBy(s => s.Start).ToArray();
        var open = new List<StyleSpan>();
        var next = 0;
        for (var i = 0; i + 1 < cuts.Count; i++)
        {
            var (from, to) = (cuts[i], cuts[i + 1]);
            if (to <= from) continue;
            while (next < bySt.Length && Clamp(bySt[next].Start, length) <= from) open.Add(bySt[next++]);
            open.RemoveAll(s => Clamp(s.End, length) <= from);
            var style = TextStyle.Plain;
            foreach (var s in open) style |= Of(s.Kind);
            if (bright is { } lit && (from < lit.Start || to > lit.End)) style |= TextStyle.Dim;
            if (runs.Count > 0 && runs[^1].Style == style && runs[^1].End == from) runs[^1] = runs[^1] with { End = to };
            else runs.Add(new StyledRun(from, to, style));
        }
        return runs;
    }

    /// <summary>The look of one highlight kind.</summary>
    public static TextStyle Of(StyleKind kind) => kind switch
    {
        StyleKind.Heading => TextStyle.Heading,
        StyleKind.Bold => TextStyle.Bold,
        StyleKind.Italic => TextStyle.Italic,
        StyleKind.Quote => TextStyle.Quote,
        StyleKind.Code => TextStyle.Code,
        StyleKind.Strike => TextStyle.Strike,
        StyleKind.Syntax => TextStyle.Syntax,
        _ => TextStyle.Plain,
    };

    private static int Clamp(uint offset, int length) => (int)Math.Min(offset, (uint)length);
}
