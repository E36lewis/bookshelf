using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Foundation;

namespace Bookshelf.Controls;

/// <summary>
/// The height of the tallest card on a shelf, shared by its rows so that
/// every card is as tall as the tallest (as in the GTK app). The shelf
/// forgets it when it's laid out again, or its width changes.
/// </summary>
public sealed partial class CardHeights
{
    private double _tallest;

    /// <summary>Raised when a taller card turns up, or the shelf starts again.</summary>
    public event Action? Changed;

    public double Tallest => _tallest;

    /// <summary>Notes a card's height; true if it's the tallest so far.</summary>
    public bool Offer(double height)
    {
        if (height <= _tallest + 0.5) return false;
        _tallest = height;
        Changed?.Invoke();
        return true;
    }

    public void Reset()
    {
        _tallest = 0;
        Changed?.Invoke();
    }
}

/// <summary>
/// One row of the shelf's grid of books, as the GTK app lays it out. Every
/// card is the same width, as many to a row as <see cref="Columns(double)"/>
/// says for the width, and each row is centred, so a short row (one or two
/// books, or the last row) sits in the middle. Every card is as tall as the
/// tallest on the shelf (<see cref="Heights"/>).
/// </summary>
public sealed partial class CardRowLayout : NonVirtualizingLayout
{
    /// <summary>Between cards, across and down.</summary>
    public const double Gap = 16;

    /// <summary>A card is never narrower than this: a column goes first.</summary>
    public const double MinCardWidth = 180;

    private CardHeights? _heights;

    /// <summary>Cards to a row for the grid's width: 3 from 600, 2 from 400, else 1; fewer if a card would be too narrow.</summary>
    public static int Columns(double width)
    {
        var columns = width >= 600 ? 3 : width >= 400 ? 2 : 1;
        while (columns > 1 && CardWidth(width, columns) < MinCardWidth) columns--;
        return columns;
    }

    /// <summary>A card's width when <paramref name="columns"/> fill <paramref name="width"/>.</summary>
    public static double CardWidth(double width, int columns) => Math.Max(0, (width - Gap * (columns - 1)) / columns);

    /// <summary>The shelf's tallest card, shared by all its rows.</summary>
    public CardHeights? Heights
    {
        get => _heights;
        set
        {
            if (_heights is not null) _heights.Changed -= InvalidateMeasure;
            _heights = value;
            if (_heights is not null) _heights.Changed += InvalidateMeasure;
            InvalidateMeasure();
        }
    }

    protected override Size MeasureOverride(NonVirtualizingLayoutContext context, Size availableSize)
    {
        var children = context.Children;
        if (children.Count == 0 || double.IsInfinity(availableSize.Width)) return new Size(0, 0);
        // A row made for more columns than fit now (while the window is
        // resized) squeezes in until the shelf lays itself out again.
        var width = CardWidth(availableSize.Width, Math.Max(Columns(availableSize.Width), children.Count));
        var height = 0.0;
        foreach (var child in children)
        {
            child.Measure(new Size(width, double.PositiveInfinity));
            height = Math.Max(height, child.DesiredSize.Height);
        }
        _heights?.Offer(height);
        return new Size(availableSize.Width, Math.Max(height, _heights?.Tallest ?? 0));
    }

    protected override Size ArrangeOverride(NonVirtualizingLayoutContext context, Size finalSize)
    {
        var children = context.Children;
        if (children.Count == 0) return finalSize;
        var width = CardWidth(finalSize.Width, Math.Max(Columns(finalSize.Width), children.Count));
        var used = children.Count * width + (children.Count - 1) * Gap;
        var x = Math.Max(0, (finalSize.Width - used) / 2);
        foreach (var child in children)
        {
            child.Arrange(new Rect(x, 0, width, finalSize.Height));
            x += width + Gap;
        }
        return finalSize;
    }
}
