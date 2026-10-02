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
    /// keyboard accelerator for each of its keys, which also show in its
    /// tooltip. With <paramref name="exceptWhileTyping"/>, the key goes to a
    /// text box that has the focus instead (Delete deletes text, Ctrl+Z
    /// undoes typing).
    /// </summary>
    public static void Add(UIElement owner, string title, Action action, bool exceptWhileTyping = false)
    {
        foreach (var (key, modifiers) in Combos(title)) Add(owner, key, modifiers, action, exceptWhileTyping);
    }

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
        foreach (var (key, modifiers) in Combos(title)) Global.Add((key, modifiers, action));
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
        if (!Looked.TryGetValue(title, out var combos)) Looked[title] = combos = Combos(title);
        return combos.Contains((key, modifiers));
    }

    // UI thread only, like everything here.
    private static readonly Dictionary<string, List<(VirtualKey Key, VirtualKeyModifiers Modifiers)>> Looked = [];

    /// <summary>The keys and modifiers for a title: the core's Windows table's, and any the app takes as well.</summary>
    private static List<(VirtualKey Key, VirtualKeyModifiers Modifiers)> Combos(string title)
    {
        var combos = new List<(VirtualKey Key, VirtualKeyModifiers Modifiers)>();
        foreach (var combo in ShortcutKeys.FindAll(title))
        {
            if (ShortcutKeys.VirtualKey(combo.Key) is not { } key) continue;
            var modifiers = VirtualKeyModifiers.None;
            if (combo.Control) modifiers |= VirtualKeyModifiers.Control;
            if (combo.Shift || key.ImpliedShift) modifiers |= VirtualKeyModifiers.Shift;
            combos.Add(((VirtualKey)key.Code, modifiers));
        }
        if (combos.Count == 0) StartupLog.Step($"No Windows shortcut for “{title}” in the core's table");
        return combos;
    }

    /// <summary>Whether a text box has the keyboard focus.</summary>
    public static bool IsTyping(XamlRoot? root) =>
        root is not null && FocusManager.GetFocusedElement(root) is TextBox or RichEditBox or PasswordBox;
}
