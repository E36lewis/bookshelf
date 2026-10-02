using Bookshelf.Core;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;
using Windows.UI.Core;

namespace Bookshelf.Services;

/// <summary>Keyboard shortcuts from the core's Windows shortcuts table.</summary>
internal static class Keys
{
    private static readonly List<(VirtualKey Key, VirtualKeyModifiers Modifiers, Action Action)> Global = [];

    /// <summary>
    /// Adds the shortcut titled <paramref name="title"/> (see
    /// <see cref="ShortcutKeys.Titles"/>) to <paramref name="owner"/> as a
    /// keyboard accelerator, which also shows in its tooltip. With
    /// <paramref name="exceptWhileTyping"/>, the key goes to a text box that
    /// has the focus instead (Delete deletes text, Ctrl+Z undoes typing).
    /// </summary>
    public static KeyboardAccelerator? Add(UIElement owner, string title, Action action, bool exceptWhileTyping = false) =>
        Combo(title) is { } c ? Add(owner, c.Key, c.Modifiers, action, exceptWhileTyping) : null;

    /// <summary>Adds a shortcut that isn't in the core's table (Delete, Ctrl+Z).</summary>
    public static KeyboardAccelerator Add(
        UIElement owner, VirtualKey key, VirtualKeyModifiers modifiers, Action action, bool exceptWhileTyping = false)
    {
        var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
        accelerator.Invoked += (_, args) =>
        {
            if (exceptWhileTyping && IsTyping(owner.XamlRoot)) return; // not handled: the text box gets it
            if (DialogHost.IsOpen) return;
            args.Handled = true;
            action();
        };
        owner.KeyboardAccelerators.Add(accelerator);
        return accelerator;
    }

    /// <summary>
    /// Adds a shortcut that works anywhere in the window, even in the writing
    /// page's RichEditBox (which keeps keys from accelerators): handled on the
    /// way down to the focused element (see <see cref="Attach"/>).
    /// </summary>
    public static void AddGlobal(string title, Action action)
    {
        if (Combo(title) is { } c) Global.Add((c.Key, c.Modifiers, action));
    }

    /// <summary>Adds a window-wide shortcut that isn't in the core's table (Alt+Left).</summary>
    public static void AddGlobal(VirtualKey key, VirtualKeyModifiers modifiers, Action action) =>
        Global.Add((key, modifiers, action));

    /// <summary>Handles the window-wide shortcuts for everything inside <paramref name="root"/>.</summary>
    public static void Attach(UIElement root) => root.PreviewKeyDown += (_, e) =>
    {
        if (DialogHost.IsOpen) return;
        var modifiers = Modifiers();
        foreach (var (key, mods, action) in Global)
        {
            if (key != e.Key || mods != modifiers) continue;
            e.Handled = true;
            action();
            return;
        }
    };

    /// <summary>The modifier keys held down right now.</summary>
    public static VirtualKeyModifiers Modifiers()
    {
        var modifiers = VirtualKeyModifiers.None;
        if (IsDown(VirtualKey.Control)) modifiers |= VirtualKeyModifiers.Control;
        if (IsDown(VirtualKey.Shift)) modifiers |= VirtualKeyModifiers.Shift;
        if (IsDown(VirtualKey.Menu)) modifiers |= VirtualKeyModifiers.Menu;
        return modifiers;
    }

    private static bool IsDown(VirtualKey key) =>
        InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(CoreVirtualKeyStates.Down);

    /// <summary>
    /// Whether <paramref name="key"/> with exactly <paramref name="modifiers"/>
    /// is the shortcut titled <paramref name="title"/>: for pages that handle
    /// keys themselves because the RichEditBox keeps them from accelerators.
    /// </summary>
    public static bool Is(string title, VirtualKey key, VirtualKeyModifiers modifiers)
    {
        if (!Looked.TryGetValue(title, out var combo)) Looked[title] = combo = Combo(title);
        return combo is { } c && c.Key == key && c.Modifiers == modifiers;
    }

    // UI thread only, like everything here.
    private static readonly Dictionary<string, (VirtualKey Key, VirtualKeyModifiers Modifiers)?> Looked = [];

    /// <summary>The key and modifiers for a title in the core's Windows table.</summary>
    private static (VirtualKey Key, VirtualKeyModifiers Modifiers)? Combo(string title)
    {
        var combo = ShortcutKeys.Find(title);
        var key = combo is null ? null : ShortcutKeys.VirtualKey(combo.Key);
        if (combo is null || key is null)
        {
            StartupLog.Step($"No Windows shortcut for “{title}” in the core's table");
            return null;
        }
        var modifiers = VirtualKeyModifiers.None;
        if (combo.Control) modifiers |= VirtualKeyModifiers.Control;
        if (combo.Shift || key.Value.ImpliedShift) modifiers |= VirtualKeyModifiers.Shift;
        return ((VirtualKey)key.Value.Code, modifiers);
    }

    /// <summary>Whether a text box has the keyboard focus.</summary>
    public static bool IsTyping(XamlRoot? root) =>
        root is not null && FocusManager.GetFocusedElement(root) is TextBox or RichEditBox or PasswordBox;
}
