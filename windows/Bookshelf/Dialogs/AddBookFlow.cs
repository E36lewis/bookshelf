using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Dialogs;

/// <summary>
/// Adding a book, one dialog after another (WinUI shows one at a time):
/// search and pick, then "you've logged this before" if so, then which
/// shelf. The new entry's page opens at the end.
/// </summary>
internal static class AddBookFlow
{
    public static async Task RunAsync(MainWindow shell)
    {
        var session = shell.Session;
        if (session.Profile is not { } profile || DialogHost.IsOpen) return;

        var search = new AddBookDialog { UserId = profile.Id };
        await DialogHost.ShowAsync(search);
        if (search.Saved is not { } book) return;

        if (search.ExistingEntry is { } existing)
        {
            var again = new ContentDialog
            {
                Title = "You've logged this book before",
                Content = Wrapped($"Open your entry for “{book.Title}”, or start a new one for this reading?"),
                PrimaryButtonText = "Open my entry",
                SecondaryButtonText = "Read it again",
                CloseButtonText = "Cancel",
                DefaultButton = ContentDialogButton.Primary,
            };
            switch (await DialogHost.ShowAsync(again))
            {
                case ContentDialogResult.Primary:
                    shell.OpenEntry(existing, session.CurrentShelf);
                    return;
                case ContentDialogResult.None:
                    return;
            }
        }

        if (await AskShelfAsync(book.Title) is not { } shelf) return;
        try
        {
            var id = await session.Journal.AddToShelfAsync(profile.Id, book.Id, shelf);
            await session.RefreshShelvesAsync();
            shell.OpenEntry(id, shelf);
        }
        catch (CoreException ex)
        {
            shell.ShowNotice($"Couldn't add “{book.Title}”: {ex.UserMessage()}", InfoBarSeverity.Error, autoHide: false);
        }
    }

    /// <summary>Which shelf a new book goes on (its dates start as today); null if cancelled.</summary>
    private static async Task<Shelf?> AskShelfAsync(string title)
    {
        var choices = new RadioButtons
        {
            Header = "Which shelf does it go on? You can change the dates afterwards.",
            Items = { "Someday", "Reading now", "Finished" },
            SelectedIndex = 1,
        };
        var dialog = new ContentDialog
        {
            Title = $"Add “{title}”",
            Content = choices,
            PrimaryButtonText = "Add",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
        };
        if (await DialogHost.ShowAsync(dialog) != ContentDialogResult.Primary) return null;
        return choices.SelectedIndex switch
        {
            0 => Shelf.Eventually,
            2 => Shelf.Finished,
            _ => Shelf.Reading,
        };
    }

    private static TextBlock Wrapped(string text) => new() { Text = text, TextWrapping = TextWrapping.Wrap };
}
