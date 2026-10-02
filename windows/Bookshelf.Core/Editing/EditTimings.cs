using System.Globalization;

namespace Bookshelf.Core.Editing;

/// <summary>
/// How long the writing page takes to handle each edit, kept so speed can be
/// checked (the UI tour reads <see cref="Summary"/> on demo journals). Small
/// edits (typing, a formatting button) restyle a line or two and must stay
/// well under a frame; a paste or opening a summary restyles many lines and
/// is counted apart.
/// </summary>
public sealed class EditTimings
{
    /// <summary>Edits touching more lines than this count as bulk.</summary>
    public const int SmallEditLines = 3;

    private const int Keep = 2000;
    private readonly List<double> _small = [];

    /// <summary>Small edits seen (all of them, even past the ones kept).</summary>
    public int Count { get; private set; }

    /// <summary>The slowest small edit, in milliseconds.</summary>
    public double Max { get; private set; }

    /// <summary>The text's length at the slowest small edit.</summary>
    public int MaxAtLength { get; private set; }

    /// <summary>The slowest bulk restyle, in milliseconds, and how many lines it took.</summary>
    public (double Ms, int Lines) Bulk { get; private set; }

    /// <summary>
    /// Time spent highlighting the rest of a long text while the editor was
    /// idle (see <see cref="PendingLines"/>), in milliseconds, since the last
    /// long text arrived.
    /// </summary>
    public double Background { get; private set; }

    /// <summary>Notes a bit of highlighting done while idle; <paramref name="restart"/> when a new long text arrived.</summary>
    public void AddBackground(double ms, bool restart = false) => Background = (restart ? 0 : Background) + ms;

    /// <summary>Notes one edit: <paramref name="ms"/> to handle it, restyling <paramref name="lines"/> lines of a <paramref name="length"/>-long text.</summary>
    public void Add(double ms, int lines, int length)
    {
        if (lines > SmallEditLines)
        {
            if (ms > Bulk.Ms) Bulk = (ms, lines);
            return;
        }
        Count++;
        if (_small.Count < Keep) _small.Add(ms);
        if (ms > Max)
        {
            Max = ms;
            MaxAtLength = length;
        }
    }

    /// <summary>The time below which <paramref name="fraction"/> of the small edits took (0.5: the median).</summary>
    public double Percentile(double fraction)
    {
        if (_small.Count == 0) return 0;
        var sorted = _small.Order().ToArray();
        var index = (int)Math.Ceiling(Math.Clamp(fraction, 0, 1) * sorted.Length) - 1;
        return sorted[Math.Clamp(index, 0, sorted.Length - 1)];
    }

    /// <summary>One line, <c>key=value</c> pairs with invariant numbers, easy for a script to read.</summary>
    public string Summary => string.Create(
        CultureInfo.InvariantCulture,
        $"edits={Count} median={Percentile(0.5):F2} p90={Percentile(0.9):F2} max={Max:F2} maxAtLength={MaxAtLength} bulk={Bulk.Ms:F1} bulkLines={Bulk.Lines} background={Background:F0}");
}
