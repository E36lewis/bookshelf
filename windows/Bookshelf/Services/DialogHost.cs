using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Services;

/// <summary>
/// Shows ContentDialogs. WinUI allows only one open at a time, so flows
/// that need several (add a book: search, then "already logged?", then the
/// shelf) show them one after another, and a shortcut pressed while one is
/// open does nothing.
/// </summary>
internal static class DialogHost
{
    /// <summary>Whether a dialog is showing.</summary>
    public static bool IsOpen { get; private set; }

    /// <summary>Shows <paramref name="dialog"/> over the window; None if another is already open.</summary>
    public static async Task<ContentDialogResult> ShowAsync(ContentDialog dialog)
    {
        if (IsOpen || MainWindow.Current.Content is not FrameworkElement root) return ContentDialogResult.None;
        dialog.XamlRoot = root.XamlRoot;
        // Dialogs open in a layer of their own, outside the window's theme.
        dialog.RequestedTheme = root.ActualTheme;
        dialog.Style ??= (Style)Application.Current.Resources["DefaultContentDialogStyle"];
        IsOpen = true;
        try
        {
            return await dialog.ShowAsync();
        }
        finally
        {
            IsOpen = false;
        }
    }

    /// <summary>A message with one button.</summary>
    public static Task<ContentDialogResult> MessageAsync(string title, string message) =>
        ShowAsync(new ContentDialog
        {
            Title = title,
            Content = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true },
            CloseButtonText = "OK",
            DefaultButton = ContentDialogButton.Close,
        });
}
