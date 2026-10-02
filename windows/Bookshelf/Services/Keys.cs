using Bookshelf.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;

namespace Bookshelf.Services;

/// <summary>Keyboard accelerators from the core's Windows shortcuts table.</summary>
internal static class Keys
{
    /// <summary>
    /// Adds the shortcut titled <paramref name="title"/> (see
    /// <see cref="ShortcutKeys.Titles"/>) to <paramref name="owner"/>. With
    /// <paramref name="exceptWhileTyping"/>, the key goes to a text box that
    /// has the focus instead (Delete deletes text, Ctrl+Z undoes typing).
    /// </summary>
    public static KeyboardAccelerator? Add(UIElement owner, string title, Action action, bool exceptWhileTyping = false)
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
        return Add(owner, (VirtualKey)key.Value.Code, modifiers, action, exceptWhileTyping);
    }

    /// <summary>Adds a shortcut that isn't in the core's table (Delete, Ctrl+Z, Alt+Left).</summary>
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

    /// <summary>Whether a text box has the keyboard focus.</summary>
    public static bool IsTyping(XamlRoot? root) =>
        root is not null && FocusManager.GetFocusedElement(root) is TextBox or RichEditBox or PasswordBox;
}
