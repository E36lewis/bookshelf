using System.Runtime.InteropServices;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Foundation;
using Windows.UI.ViewManagement;
using WinRT.Interop;

namespace Bookshelf.Writing;

/// <summary>
/// Typewriter scrolling for focus mode: keeps the line being written in the
/// middle of the page, by scrolling the RichEditBox's own ScrollViewer.
/// Without Windows' animation effects, it jumps instead of gliding.
/// </summary>
/// <remarks>
/// RichEdit scrolls its text itself, and where its "client coordinates"
/// start isn't documented. Its screen coordinates are what UI Automation
/// (and so Narrator) uses, so the caret is found on screen and then placed
/// in the ScrollViewer's view.
/// </remarks>
internal sealed class Typewriter(RichEditBox box)
{
    private readonly UISettings _ui = new();
    private ScrollViewer? _scroller;

    /// <summary>Scrolls so the caret's line sits mid-page (if it isn't already).</summary>
    public void CenterCaret()
    {
        if (Scroller is not { ViewportHeight: > 0 } scroller || CaretInView(scroller) is not { } y) return;
        var target = Math.Clamp(scroller.VerticalOffset + y - scroller.ViewportHeight / 2, 0, scroller.ScrollableHeight);
        if (Math.Abs(target - scroller.VerticalOffset) < 4) return;
        scroller.ChangeView(null, target, null, disableAnimation: !_ui.AnimationsEnabled);
    }

    /// <summary>The middle of the caret's line, in pixels from the top of the visible page; null if it can't be told.</summary>
    private double? CaretInView(ScrollViewer scroller)
    {
        if (box.XamlRoot is not { } root) return null;
        box.Document.Selection.GetRect(PointOptions.None, out var rect, out _);
        if (rect.Height <= 0) return null;
        var origin = new Win32Point();
        if (!ClientToScreen(WindowNative.GetWindowHandle(MainWindow.Instance), ref origin)) return null;
        var scale = root.RasterizationScale > 0 ? root.RasterizationScale : 1;
        var viewTop = scroller.TransformToVisual(null).TransformPoint(new Point(0, 0)).Y;
        return (rect.Y + rect.Height / 2 - origin.Y) / scale - viewTop;
    }

    private ScrollViewer? Scroller => _scroller ??= Find<ScrollViewer>(box);

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

    [StructLayout(LayoutKind.Sequential)]
    private struct Win32Point
    {
        public int X;
        public int Y;
    }

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool ClientToScreen(IntPtr window, ref Win32Point point);
}
