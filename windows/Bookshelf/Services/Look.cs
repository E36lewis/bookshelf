using Bookshelf.Ffi;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Foundation;
using Windows.UI;
using Windows.UI.ViewManagement;

namespace Bookshelf.Services;

/// <summary>
/// The profile's look: light or dark, the accent color and the fonts.
/// Colors come from theme resources everywhere; this only changes which
/// accent they're made from, and never in High Contrast, where Windows'
/// own colors win.
/// </summary>
internal static class Look
{
    private const string FontFolder = "Assets/Fonts";
    private const string SerifFile = "SourceSerif4-Regular.ttf";
    private const string SerifName = "Source Serif 4";
    private const string DuoFile = "iAWriterDuoS-Regular.ttf";
    private const string DuoName = "iA Writer Duo S";

    /// <summary>Source Serif 4, bundled.</summary>
    public static FontFamily Serif { get; private set; } = new("Georgia");

    /// <summary>iA Writer Duo, bundled.</summary>
    public static FontFamily Duo { get; private set; } = new("Cascadia Mono, Consolas");

    /// <summary>The system's sans-serif for text.</summary>
    public static readonly FontFamily Sans = new("Segoe UI Variable Text, Segoe UI");

    /// <summary>The system's sans-serif for headings.</summary>
    public static readonly FontFamily SansDisplay = new("Segoe UI Variable Display, Segoe UI");

    /// <summary>The system's monospace.</summary>
    public static readonly FontFamily Mono = new("Cascadia Mono, Consolas");

    /// <summary>Titles and headings, and the summaries on the book and reading pages.</summary>
    public static FontFamily Heading { get; private set; } = SansDisplay;

    /// <summary>Running text of a summary (book page, reader): the heading font in its text cut.</summary>
    public static FontFamily Reading { get; private set; } = Sans;

    /// <summary>The writing page's font.</summary>
    public static FontFamily Writing { get; private set; } = Mono;

    /// <summary>The settings the look was last made from.</summary>
    public static ProfileSettings? Current { get; private set; }

    /// <summary>
    /// Finds out how to load the bundled fonts in this build. An unpackaged,
    /// single-file app unpacks them next to Bookshelf.dll; ms-appx should
    /// reach them there, and a plain file path is the fallback. Each way is
    /// checked by measuring text in it: a font that didn't load falls back
    /// to the system's, which measures differently. Logged to startup.log.
    /// </summary>
    public static void LoadFonts()
    {
        var folder = Path.Combine(AppContext.BaseDirectory, "Assets", "Fonts");
        var missing = new FontFamily($"ms-appx:///{FontFolder}/missing.ttf#Missing Font");
        Serif = Pick(SerifFile, SerifName, folder, missing) ?? new FontFamily("Georgia");
        Duo = Pick(DuoFile, DuoName, folder, missing) ?? Mono;
    }

    private static FontFamily? Pick(string file, string name, string folder, FontFamily missing)
    {
        var path = Path.Combine(folder, file);
        if (!File.Exists(path))
        {
            StartupLog.Step($"Fonts: {file} isn't in {folder}");
            return null;
        }
        var fallback = Width(missing);
        var candidates = new[]
        {
            $"ms-appx:///{FontFolder}/{file}#{name}",
            $"{path}#{name}",
            $"{new Uri(path).AbsoluteUri}#{name}",
        };
        foreach (var source in candidates)
        {
            var family = new FontFamily(source);
            var width = Width(family);
            if (Math.Abs(width - fallback) > 1.5)
            {
                StartupLog.Step($"Fonts: {name} loads from {source.Split('#')[0]} ({width:F0} px, not {fallback:F0})");
                return family;
            }
        }
        StartupLog.Step($"Fonts: {name} didn't load ({fallback:F0} px each way); using a system font");
        return null;
    }

    private static double Width(FontFamily family)
    {
        var probe = new TextBlock { FontFamily = family, FontSize = 40, Text = "iiiiiiiiiiWWWWW" };
        probe.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        return probe.DesiredSize.Width;
    }

    /// <summary>
    /// Applies a profile's look to the window (or the neutral look, for
    /// null: no profile open). Cheap when nothing changed.
    /// </summary>
    public static void Apply(ProfileSettings? settings, FrameworkElement root, AppWindow window)
    {
        Current = settings;
        Heading = settings?.HeadingFont == HeadingFont.Sans ? SansDisplay : Serif;
        Reading = settings?.HeadingFont == HeadingFont.Sans ? Sans : Serif;
        Writing = settings?.WritingFont switch
        {
            WritingFont.Serif => Serif,
            WritingFont.Sans => Sans,
            WritingFont.Mono => Mono,
            _ => Duo,
        };

        var theme = settings?.Theme switch
        {
            Theme.Light => ElementTheme.Light,
            Theme.Dark => ElementTheme.Dark,
            _ => ElementTheme.Default,
        };
        window.TitleBar.PreferredTheme = theme switch
        {
            ElementTheme.Light => TitleBarTheme.Light,
            ElementTheme.Dark => TitleBarTheme.Dark,
            _ => TitleBarTheme.UseDefaultAppMode,
        };

        var accentChanged = SetAccent(settings?.Accent ?? BookshelfFfiMethods.Accents()[0].Hex);
        if (accentChanged && root.RequestedTheme == theme)
        {
            // Theme resources are looked up again when the theme changes, so
            // flip it and back to repaint everything in the new accent.
            root.RequestedTheme = root.ActualTheme == ElementTheme.Dark ? ElementTheme.Light : ElementTheme.Dark;
        }
        root.RequestedTheme = theme;
    }

    private static string? _accent;

    /// <summary>Points WinUI's accent colors at <paramref name="hex"/>. True if anything changed.</summary>
    private static bool SetAccent(string hex)
    {
        if (new AccessibilitySettings().HighContrast)
        {
            // High Contrast has its own colors; put Windows' accent back.
            if (_accent is null) return false;
            foreach (var key in AccentKeys) Application.Current.Resources.Remove(key);
            _accent = null;
            return true;
        }
        if (string.Equals(_accent, hex, StringComparison.OrdinalIgnoreCase)) return false;
        // WinUI's shades don't depend on light or dark (see accent_colors).
        var p = BookshelfFfiMethods.AccentColors(hex, false);
        var colors = new[] { p.Bg, p.Light1, p.Light2, p.Light3, p.Dark1, p.Dark2, p.Dark3 };
        for (var i = 0; i < AccentKeys.Length; i++)
        {
            if (ParseColor(colors[i]) is { } color) Application.Current.Resources[AccentKeys[i]] = color;
        }
        _accent = hex;
        return true;
    }

    private static readonly string[] AccentKeys =
    [
        "SystemAccentColor",
        "SystemAccentColorLight1", "SystemAccentColorLight2", "SystemAccentColorLight3",
        "SystemAccentColorDark1", "SystemAccentColorDark2", "SystemAccentColorDark3",
    ];

    /// <summary><c>#rrggbb</c> as a color, or null if it isn't one.</summary>
    public static Color? ParseColor(string hex)
    {
        if (hex.Length != 7 || hex[0] != '#') return null;
        if (!uint.TryParse(hex.AsSpan(1), System.Globalization.NumberStyles.HexNumber, null, out var rgb)) return null;
        return Color.FromArgb(255, (byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb);
    }

    /// <summary>A color as <c>#rrggbb</c>.</summary>
    public static string ToHex(Color c) => $"#{c.R:x2}{c.G:x2}{c.B:x2}";
}
