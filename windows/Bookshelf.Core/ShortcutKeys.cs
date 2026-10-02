using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// The Windows column of the core's shortcuts table, as key labels for the
/// shortcuts list and as virtual-key codes for the app's keyboard
/// accelerators, so the keys that work and the keys that are listed come
/// from one table.
/// </summary>
public static class ShortcutKeys
{
    /// <summary>The titles the app looks its accelerators up by.</summary>
    public static class Titles
    {
        public const string Reading = "Reading";
        public const string Finished = "Finished";
        public const string Eventually = "Eventually";
        public const string AddBook = "Add a book";
        public const string Search = "Search your shelves";
        public const string Settings = "Settings";
        public const string Write = "Write or edit your summary";
        public const string Read = "Read your summary";
        public const string Save = "Save now (it also saves as you type)";
        public const string Bold = "Bold";
        public const string Italic = "Italic";
        public const string Link = "Link";
        public const string FocusMode = "Focus mode";
        public const string FullScreen = "Full screen";
        public const string LeaveFullScreen = "Leave full screen";
        public const string Manual = "User manual";
        public const string Shortcuts = "Keyboard shortcuts";

        /// <summary>Every title the Windows app wires up.</summary>
        public static readonly string[] All =
        [
            Reading, Finished, Eventually, AddBook, Search, Settings, Write, Read, Save,
            Bold, Italic, Link, FocusMode, FullScreen, LeaveFullScreen, Manual, Shortcuts,
        ];
    }

    /// <summary>The core's Windows table, read once: it never changes while the app runs.</summary>
    private static readonly Lazy<Shortcut[]> Table =
        new(() => BookshelfFfiMethods.Shortcuts(Platform.Windows).SelectMany(g => g.Items).ToArray());

    /// <summary>The Windows shortcut titled <paramref name="title"/>, if the table has one.</summary>
    public static KeyCombo? Find(string title) => FindAll(title).FirstOrDefault();

    /// <summary>
    /// Every Windows key for <paramref name="title"/>: the table's main one,
    /// then any others it lists for the same action (full screen is F11 or
    /// Ctrl+Shift+Enter, because many laptops keep F11 for a hardware key
    /// unless Fn is held).
    /// </summary>
    public static IReadOnlyList<KeyCombo> FindAll(string title) =>
        Table.Value
            .Where(s => s.Title == title)
            .SelectMany(s => s.Also.Prepend(s.Accel))
            .OfType<Accel.Keys>()
            .Select(k => k.Combo)
            .Distinct()
            .ToList();

    /// <summary>
    /// All the keys for these shortcuts as one label, for a tooltip:
    /// "F11 or Ctrl+Shift+Enter", "Esc, F11 or Ctrl+Shift+Enter". Null if none.
    /// </summary>
    public static string? Labels(params string[] titles)
    {
        var labels = titles.SelectMany(FindAll).Select(Label).Distinct().ToList();
        return labels.Count switch
        {
            0 => null,
            1 => labels[0],
            _ => $"{string.Join(", ", labels.Take(labels.Count - 1))} or {labels[^1]}",
        };
    }

    /// <summary>The keys one at a time, for key caps: ["Ctrl", "N"].</summary>
    public static IReadOnlyList<string> Keys(KeyCombo combo)
    {
        var keys = new List<string>();
        if (combo.Control) keys.Add("Ctrl");
        if (combo.Shift) keys.Add("Shift");
        keys.Add(KeyName(combo.Key));
        return keys;
    }

    /// <summary>The whole combination as one label: "Ctrl+N", "F11", "Ctrl+?".</summary>
    public static string Label(KeyCombo combo) => string.Join("+", Keys(combo));

    /// <summary>A key's name as printed on the keyboard.</summary>
    public static string KeyName(ShortcutKey key) => key switch
    {
        ShortcutKey.Character c => c.Text.ToUpperInvariant(),
        ShortcutKey.Return => "Enter",
        ShortcutKey.Escape => "Esc",
        ShortcutKey.Function f => $"F{f.Number}",
        _ => "?",
    };

    /// <summary>
    /// The Windows virtual-key code for <paramref name="key"/>, and whether
    /// typing it takes Shift that the table leaves implied ("?" is Shift + the
    /// "/" key on most layouts). Null for a key Windows has no code for.
    /// </summary>
    public static (int Code, bool ImpliedShift)? VirtualKey(ShortcutKey key)
    {
        switch (key)
        {
            case ShortcutKey.Return:
                return (0x0D, false);
            case ShortcutKey.Escape:
                return (0x1B, false);
            case ShortcutKey.Function { Number: >= 1 and <= 24 } f:
                return (0x70 + f.Number - 1, false);
            case ShortcutKey.Character { Text.Length: 1 } c:
                var ch = c.Text[0];
                if (ch is >= 'a' and <= 'z') return (char.ToUpperInvariant(ch), false);
                if (ch is >= 'A' and <= 'Z' or >= '0' and <= '9') return (ch, false);
                return ch switch
                {
                    ',' => (0xBC, false), // VK_OEM_COMMA
                    '.' => (0xBE, false), // VK_OEM_PERIOD
                    '-' => (0xBD, false), // VK_OEM_MINUS
                    '/' => (0xBF, false), // VK_OEM_2
                    '?' => (0xBF, true),
                    _ => null,
                };
            default:
                return null;
        }
    }
}
