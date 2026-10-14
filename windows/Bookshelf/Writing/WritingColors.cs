using Microsoft.UI.Xaml;
using Windows.UI;
using Windows.UI.ViewManagement;

namespace Bookshelf.Writing;

/// <summary>
/// The writing page's text colors for the editor's theme, with the GTK
/// app's strengths: Markdown marks at 38 %, quotes at 72 %, code on a 9 %
/// wash and focus mode's dimmed text at 28 % of the ink.
/// </summary>
/// <remarks>
/// RichEdit ignores a color's alpha, so each one is mixed with the page's
/// background here, opaque. In High Contrast there are no colors of our
/// own: the text is Windows' text color, marks aren't dimmed, and focus
/// mode uses Windows' color for unavailable text. The text's background is
/// "none" (transparent, which RichEdit reads as none) except in High
/// Contrast, where RichEdit paints that white: there it's Windows' window
/// color.
/// </remarks>
internal sealed record WritingColors(Color Ink, Color Syntax, Color Quote, Color CodeBackground, Color Dim, Color Background)
{
    // WinUI's own: TextFillColorPrimary over SolidBackgroundFillColorBase
    // (which Mica tints only slightly).
    private static readonly Color LightInk = Color.FromArgb(0xE4, 0, 0, 0);
    private static readonly Color LightPage = Color.FromArgb(0xFF, 0xF3, 0xF3, 0xF3);
    private static readonly Color DarkInk = Color.FromArgb(0xFF, 0xFF, 0xFF, 0xFF);
    private static readonly Color DarkPage = Color.FromArgb(0xFF, 0x20, 0x20, 0x20);

    /// <summary>Whether Windows is in High Contrast right now.</summary>
    public static bool HighContrast => new AccessibilitySettings().HighContrast;

    /// <summary>The colors for <paramref name="editor"/> as it's themed now.</summary>
    public static WritingColors For(FrameworkElement editor)
    {
        if (HighContrast)
        {
            var text = SystemColor("SystemColorWindowTextColor", Microsoft.UI.Colors.Black);
            var gray = SystemColor("SystemColorGrayTextColor", text);
            var window = SystemColor("SystemColorWindowColor", Microsoft.UI.Colors.White);
            return new WritingColors(text, text, text, window, gray, window);
        }
        var dark = editor.ActualTheme == ElementTheme.Dark;
        var page = dark ? DarkPage : LightPage;
        var ink = Mix(dark ? DarkInk : LightInk, page, (dark ? DarkInk : LightInk).A / 255.0);
        return new WritingColors(
            ink, Mix(ink, page, 0.38), Mix(ink, page, 0.72), Mix(ink, page, 0.09), Mix(ink, page, 0.28), Microsoft.UI.Colors.Transparent);
    }

    /// <summary><paramref name="share"/> of <paramref name="ink"/> over <paramref name="page"/>, opaque.</summary>
    public static Color Mix(Color ink, Color page, double share)
    {
        byte Channel(byte i, byte p) => (byte)Math.Round(p + (i - p) * Math.Clamp(share, 0, 1));
        return Color.FromArgb(0xFF, Channel(ink.R, page.R), Channel(ink.G, page.G), Channel(ink.B, page.B));
    }

    private static Color SystemColor(string key, Color fallback) =>
        Application.Current.Resources.TryGetValue(key, out var value) && value is Color color ? color : fallback;
}
