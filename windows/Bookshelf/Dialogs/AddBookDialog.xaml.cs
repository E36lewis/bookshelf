using Bookshelf.Core;
using Bookshelf.Ffi;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Dialogs;

/// <summary>A search result, as the Add a book dialog lists it.</summary>
public sealed class ResultItem(SearchResult result)
{
    public SearchResult Result { get; } = result;
    public string Title => Result.Title;
    public string Byline { get; } = SearchText.Byline(result);
    public Visibility BylineVisibility => Byline.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    public string AutomationName => Byline.Length > 0 ? $"{Title}, {Byline}" : Title;

    /// <summary>What Narrator reads for the list item.</summary>
    public override string ToString() => AutomationName;
}

/// <summary>
/// The first step of adding a book: search Open Library as you type
/// (after a short pause, and only the latest search's results are shown),
/// then save the one picked. The flow goes on in <see cref="AddBookFlow"/>.
/// </summary>
public sealed partial class AddBookDialog : ContentDialog
{
    private readonly DispatcherQueueTimer _debounce;
    private int _generation; // bumped by every keystroke and search; stale results are dropped

    public AddBookDialog()
    {
        InitializeComponent();
        _debounce = DispatcherQueue.CreateTimer();
        _debounce.Interval = TimeSpan.FromMilliseconds(400);
        _debounce.IsRepeating = false;
        _debounce.Tick += (_, _) => _ = SearchAsync();
        Opened += (_, _) => Query.Focus(FocusState.Programmatic);
        Closed += (_, _) => _debounce.Stop();
    }

    /// <summary>Whose shelves the book is for (their email goes with the search).</summary>
    internal string UserId { get; init; } = "";

    /// <summary>The book that was picked and saved, if any.</summary>
    internal BookInfo? Saved { get; private set; }

    /// <summary>The profile's entry for that book, if it already has one.</summary>
    internal string? ExistingEntry { get; private set; }

    private static Services.Session Session => MainWindow.Instance.Session;

    private void OnQueryChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        _generation++;
        _debounce.Stop();
        if (Query.Text.Trim().Length == 0)
        {
            Busy.Visibility = Visibility.Collapsed;
            ShowHint("\uE721", "Find a book", "Results come from Open Library.");
            return;
        }
        _debounce.Start();
    }

    /// <summary>Esc cancels the dialog (the search box would otherwise keep it).</summary>
    private void OnQueryKeyDown(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        if (e.Key != Windows.System.VirtualKey.Escape) return;
        e.Handled = true;
        Hide();
    }

    private void OnQuerySubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        _debounce.Stop();
        _ = SearchAsync();
    }

    private async Task SearchAsync()
    {
        var query = Query.Text.Trim();
        if (query.Length == 0) return;
        var generation = ++_generation;
        Busy.Visibility = Visibility.Visible;
        Problem.IsOpen = false;
        try
        {
            var found = await Session.Journal.SearchBooksAsync(UserId, query);
            if (generation != _generation) return; // a newer search is on its way
            if (found.Length == 0)
            {
                ShowHint("\uE721", "No books found", "Try another title, an author's name or an ISBN.");
                return;
            }
            Results.ItemsSource = found.Select(r => new ResultItem(r)).ToList();
            Results.Visibility = Visibility.Visible;
            Hint.Visibility = Visibility.Collapsed;
        }
        catch (CoreException ex)
        {
            if (generation != _generation) return;
            ShowProblem("Search failed", ex.UserMessage());
        }
        finally
        {
            if (generation == _generation) Busy.Visibility = Visibility.Collapsed;
        }
    }

    private async void OnResultClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not ResultItem item) return;
        _generation++; // nothing else lands while this saves
        _debounce.Stop();
        Results.IsEnabled = false;
        Query.IsEnabled = false;
        Busy.Visibility = Visibility.Visible;
        Problem.IsOpen = false;
        try
        {
            // Fetches the description and the cover, then checks for an entry.
            var book = await Session.Journal.SaveSearchResultAsync(UserId, item.Result);
            ExistingEntry = await Session.Journal.ExistingEntryAsync(UserId, book.Id);
            Saved = book;
            Hide();
        }
        catch (CoreException ex)
        {
            ShowProblem("Couldn't add it", ex.UserMessage());
        }
        finally
        {
            Results.IsEnabled = true;
            Query.IsEnabled = true;
            Busy.Visibility = Visibility.Collapsed;
        }
    }

    private void ShowHint(string glyph, string title, string text)
    {
        Results.ItemsSource = null;
        Results.Visibility = Visibility.Collapsed;
        HintGlyph.Glyph = glyph;
        HintTitle.Text = title;
        HintText.Text = text;
        Hint.Visibility = Visibility.Visible;
    }

    private void ShowProblem(string title, string message)
    {
        Problem.Title = title;
        Problem.Message = message;
        Problem.IsOpen = true;
    }
}
