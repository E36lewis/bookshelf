using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI.ViewManagement;

namespace Bookshelf.Writing;

/// <summary>
/// Typewriter scrolling for focus mode: keeps the line being written in the
/// middle of the page, by scrolling the RichEditBox's own ScrollViewer.
/// Without Windows' animation effects, it jumps instead of gliding. Out of
/// focus mode, it keeps a few lines clear around the caret instead
/// (<see cref="KeepClear"/>).
/// </summary>
/// <remarks>
/// RichEdit gives the caret's place measured from the top of the whole text,
/// in the same pixels as the ScrollViewer's offset (seen in CI: unchanged as
/// the view scrolls), so the target offset is that less half a page.
/// </remarks>
internal sealed class Typewriter(RichEditBox box)
{
    private readonly UISettings _ui = new();
    private ScrollViewer? _scroller;
    private int _logged;

    /// <summary>Notes the numbers behind the first few scrolls (demo journals: startup.log).</summary>
    public bool LogScrolls { get; set; }

    /// <summary>Scrolls so the caret's line sits mid-page (if it isn't already).</summary>
    public void CenterCaret()
    {
        if (Scroller is not { ViewportHeight: > 0 } scroller || CaretY() is not { } y) return;
        var target = Math.Clamp(y - scroller.ViewportHeight / 2, 0, scroller.ScrollableHeight);
        if (Math.Abs(target - scroller.VerticalOffset) < 4) return;
        Note($"caret {y:F0} px into the text, the page shows {scroller.VerticalOffset:F0} to " +
            $"{scroller.VerticalOffset + scroller.ViewportHeight:F0} (of {scroller.ScrollableHeight + scroller.ViewportHeight:F0}); scrolling to {target:F0}");
        scroller.ChangeView(null, target, null, disableAnimation: !_ui.AnimationsEnabled);
    }

    /// <summary>
    /// Scrolls just enough that <paramref name="lines"/> lines of the page
    /// show above and below the caret's line, as far as the page allows (a
    /// window too short for them just keeps the caret where it is).
    /// </summary>
    public void KeepClear(double lines, double linePitch)
    {
        if (Scroller is not { ViewportHeight: > 0 } scroller || CaretLine() is not { } line) return;
        var room = lines * Math.Max(line.Height, linePitch); // the caret is a row high; a line has its space after too
        var (top, bottom) = (line.Top - room, line.Top + line.Height + room);
        if (bottom - top > scroller.ViewportHeight) return;
        var target = scroller.VerticalOffset;
        if (bottom > target + scroller.ViewportHeight) target = bottom - scroller.ViewportHeight;
        else if (top < target) target = top;
        target = Math.Clamp(target, 0, scroller.ScrollableHeight);
        if (Math.Abs(target - scroller.VerticalOffset) < 1) return;
        scroller.ChangeView(null, target, null, disableAnimation: true);
    }

    /// <summary>
    /// How many lines of the page show above and below the caret's line, in
    /// lines of <paramref name="linePitch"/>; null if it can't be told. For
    /// the UI tour, whose UI Automation rectangles lag the box's scrolling.
    /// </summary>
    public (double Above, double Below)? Room(double linePitch)
    {
        if (Scroller is not { ViewportHeight: > 0 } scroller || CaretLine() is not { } line) return null;
        var pitch = Math.Max(line.Height, linePitch);
        var above = (line.Top - scroller.VerticalOffset) / pitch;
        var below = (scroller.VerticalOffset + scroller.ViewportHeight - line.Top - line.Height) / pitch;
        return (above, below);
    }

    private void Note(string what)
    {
        if (!LogScrolls || _logged >= 12) return;
        _logged++;
        StartupLog.Step($"Typewriter: {what}");
    }

    private void OnViewChanged(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        if (e.IsIntermediate || !LogScrolls || _logged >= 12 || _scroller is not { } scroller) return;
        Note($"settled at {scroller.VerticalOffset:F0}; caret {CaretY():F0} px into the text");
    }

    /// <summary>The middle of the caret's line, in pixels from the top of the text; null if it can't be told.</summary>
    private double? CaretY() => CaretLine() is { } line ? line.Top + line.Height / 2 : null;

    /// <summary>The top and height of the caret's line, in pixels from the top of the text; null if it can't be told.</summary>
    private (double Top, double Height)? CaretLine()
    {
        box.Document.Selection.GetRect(PointOptions.ClientCoordinates, out var rect, out _);
        return rect.Height > 0 ? (rect.Y, rect.Height) : null;
    }

    private ScrollViewer? Scroller
    {
        get
        {
            if (_scroller is not null) return _scroller;
            _scroller = Find<ScrollViewer>(box);
            if (_scroller is not null) _scroller.ViewChanged += OnViewChanged;
            return _scroller;
        }
    }

    private static T? Find<T>(DependencyObject parent) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is T found) return found;
            if (Find<T>(child) is { } deeper) return deeper;
        }
        return null;
    }
}
