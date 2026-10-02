using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI.ViewManagement;

namespace Bookshelf.Writing;

/// <summary>
/// Typewriter scrolling for focus mode: keeps the line being written in the
/// middle of the page, by scrolling the RichEditBox's own ScrollViewer.
/// Without Windows' animation effects, it jumps instead of gliding.
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
    private double? CaretY()
    {
        box.Document.Selection.GetRect(PointOptions.ClientCoordinates, out var rect, out _);
        return rect.Height > 0 ? rect.Y + rect.Height / 2 : null;
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
