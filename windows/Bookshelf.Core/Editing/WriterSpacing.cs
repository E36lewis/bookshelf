namespace Bookshelf.Core.Editing;

/// <summary>
/// The writing page's line spacing, as RichEditBox paragraph settings, from
/// the core's page layout (the same numbers the GTK app uses).
/// </summary>
/// <remarks>
/// GTK adds <c>wrap_gap</c> pixels only between the wrapped rows of a line,
/// and <c>line_gap</c> below each line. RichEdit's line spacing applies to
/// every row, the last one too, so a one-row line got both gaps (the
/// "generous" spacing of the first Windows build). Here each row is the
/// font's own height plus the wrap gap, and the space after a line is only
/// what's left of the line gap. A line then ends exactly where GTK's does.
/// RichEdit counts in points: 3/4 of a pixel at 96 to the inch.
/// </remarks>
/// <param name="RowPitch">Pixels from one wrapped row to the next.</param>
/// <param name="LinePitch">Pixels from a one-row line to the next line.</param>
public readonly record struct WriterSpacing(double RowPitch, double LinePitch)
{
    private const double PointsPerPixel = 72.0 / 96.0;

    /// <summary>
    /// The spacing for text whose rows are <paramref name="naturalRow"/>
    /// pixels high on their own (the font's line height at the writing size).
    /// </summary>
    public static WriterSpacing For(double naturalRow, int wrapGap, int lineGap)
    {
        naturalRow = double.IsFinite(naturalRow) && naturalRow > 1 ? naturalRow : 1;
        wrapGap = Math.Max(0, wrapGap);
        lineGap = Math.Max(wrapGap, lineGap);
        return new WriterSpacing(naturalRow + wrapGap, naturalRow + lineGap);
    }

    /// <summary>Each row's height, for <c>LineSpacingRule.AtLeast</c> (taller rows, like headings, keep theirs).</summary>
    public double RowPoints => RowPitch * PointsPerPixel;

    /// <summary>The space after each line, for <c>ParagraphFormat.SpaceAfter</c>.</summary>
    public double SpaceAfterPoints => (LinePitch - RowPitch) * PointsPerPixel;
}
