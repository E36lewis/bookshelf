using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI.ViewManagement;

namespace Bookshelf.Writing;

/// <summary>
/// Typewriter scrolling for focus mode: keeps the line being written in the
/// middle of the page, by scrolling the RichEditBox's own ScrollViewer.
/// Without Windows' animation effects, it jumps instead of gliding. Also
/// gives the text room above and below inside that ScrollViewer
/// (<see cref="SetRoom"/>).
/// </summary>
/// <remarks>
/// RichEdit reports where the caret is in "client coordinates", and it isn't
/// documented whether those move with the scrolling. So the first time the
/// view has scrolled with the caret still, this measures again and learns
/// which it is (and notes it in startup.log); until then it assumes they
/// move with the view.
/// </remarks>
internal sealed class Typewriter(RichEditBox box)
{
    private readonly UISettings _ui = new();
    private ScrollViewer? _scroller;
    private (int Caret, double Offset, double Y)? _lastSeen;
    private Thickness _room;
    private static bool? _yIsInView; // learned once per run
    private static bool _described; // startup.log says once what holds the text

    /// <summary>
    /// Space above the first line and below the last, scrolling with the
    /// text. (The RichEditBox's own Padding sits outside its scrolling area,
    /// so text would vanish into it.) The room below lets the last line come
    /// up to the middle of the page.
    /// </summary>
    public void SetRoom(double above, double below)
    {
        _room = new Thickness(0, above, 0, below);
        if (Scroller?.Content is FrameworkElement text) text.Margin = _room;
    }

    /// <summary>Scrolls so the caret's line sits mid-page (if it isn't already).</summary>
    public void CenterCaret()
    {
        if (Scroller is not { ViewportHeight: > 0 } scroller) return;
        var caretY = CaretY(scroller);
        var target = Math.Clamp(caretY - scroller.ViewportHeight / 2, 0, scroller.ScrollableHeight);
        if (Math.Abs(target - scroller.VerticalOffset) < 4) return;
        scroller.ChangeView(null, target, null, disableAnimation: !_ui.AnimationsEnabled);
    }

    /// <summary>Where the middle of the caret's line is, from the top of everything there is to scroll.</summary>
    private double CaretY(ScrollViewer scroller)
    {
        var selection = box.Document.Selection;
        selection.GetRect(PointOptions.ClientCoordinates, out var rect, out _);
        var y = rect.Y + rect.Height / 2;
        var offset = scroller.VerticalOffset;
        Learn(selection.StartPosition, offset, y);
        return _yIsInView == false ? y + _room.Top : y + offset;
    }

    private void Learn(int caret, double offset, double y)
    {
        if (_yIsInView is null && _lastSeen is { } seen && seen.Caret == caret && Math.Abs(seen.Offset - offset) > 20)
        {
            var scrolled = offset - seen.Offset;
            if (Math.Abs(y - seen.Y) < 2) _yIsInView = false;
            else if (Math.Abs(y - seen.Y + scrolled) < 2) _yIsInView = true;
            if (_yIsInView is { } inView)
                StartupLog.Step($"Typewriter: the caret's position is measured from the {(inView ? "visible page" : "top of the text")}");
        }
        _lastSeen = (caret, offset, y);
    }

    /// <summary>Measures again once a scroll has settled, so <see cref="Learn"/> sees the view move under a still caret.</summary>
    private void OnViewChanged(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        if (!e.IsIntermediate && _yIsInView is null && _scroller is { } scroller) CaretY(scroller);
    }

    private ScrollViewer? Scroller
    {
        get
        {
            if (_scroller is not null) return _scroller;
            _scroller = Find<ScrollViewer>(box);
            if (_scroller is null) return null;
            _scroller.ViewChanged += OnViewChanged;
            if (_scroller.Content is FrameworkElement text) text.Margin = _room;
            if (!_described)
            {
                _described = true;
                StartupLog.Step($"Writer: the text scrolls in a {_scroller.Content?.GetType().Name ?? "nothing"}");
            }
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
