using System.ComponentModel;
using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Bookshelf.Dialogs;

/// <summary>A search result, as the Add a book dialog lists it.</summary>
public sealed class ResultItem(SearchResult result) : INotifyPropertyChanged
{
    /// <summary>Result covers are 40 by 60.</summary>
    public const int CoverWidth = 40;

    private ImageSource? _cover;

    public event PropertyChangedEventHandler? PropertyChanged;

    public SearchResult Result { get; } = result;
    public string Title => Result.Title;
    public string Subtitle { get; } = result.Subtitle?.Trim() ?? "";
    public Visibility SubtitleVisibility => Subtitle.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    public string Byline { get; } = SearchText.Byline(result);
    public Visibility BylineVisibility => Byline.Length > 0 ? Visibility.Visible : Visibility.Collapsed;

    public string AutomationName =>
        string.Join(", ", new[] { Subtitle.Length > 0 ? $"{Title}: {Subtitle}" : Title, Byline }.Where(s => s.Length > 0));

    /// <summary>The title's first letter, on the stand-in cover.</summary>
    public string Initial { get; } =
        result.Title.TrimStart().Length > 0 ? char.ToUpperInvariant(result.Title.TrimStart()[0]).ToString() : "";

    public FontFamily HeadingFont => Look.Heading;

    /// <summary>The cover, once it's loaded; null shows the stand-in.</summary>
    public ImageSource? Cover
    {
        get => _cover;
        set
        {
            _cover = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Cover)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(PlaceholderVisibility)));
        }
    }

    public Visibility PlaceholderVisibility => _cover is null ? Visibility.Visible : Visibility.Collapsed;

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
    /// <summary>How many result covers load at once (the core keeps to a few as well).</summary>
    private static readonly SemaphoreSlim CoverLoads = new(3, 3);

    private readonly DispatcherQueueTimer _debounce;
    private int _generation; // bumped by every keystroke and search; stale results are dropped
    private int _shown; // bumped whenever the list changes; covers for a list that's gone aren't loaded

    public AddBookDialog()
    {
        InitializeComponent();
        _debounce = DispatcherQueue.CreateTimer();
        _debounce.Interval = TimeSpan.FromMilliseconds(400);
        _debounce.IsRepeating = false;
        _debounce.Tick += (_, _) => _ = SearchAsync();
        Opened += (_, _) => Query.Focus(FocusState.Programmatic);
        Query.Loaded += (_, _) => MainWindow.NameTextBox(Query);
        Closed += (_, _) =>
        {
            _debounce.Stop();
            _shown++;
        };
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
            // A demo journal never goes online.
            var found = App.Options.DemoJournal
                ? DemoJournal.SearchResults.ToArray()
                : await Session.Journal.SearchBooksAsync(UserId, query);
            if (generation != _generation) return; // a newer search is on its way
            if (found.Length == 0)
            {
                ShowHint("\uE721", "No books found", "Try another title, an author's name or an ISBN.");
                return;
            }
            var items = found.Select(r => new ResultItem(r)).ToList();
            Results.ItemsSource = items;
            var shown = ++_shown;
            foreach (var item in items) _ = LoadCoverAsync(item, shown);
            Results.Visibility = Visibility.Visible;
            Hint.Visibility = Visibility.Collapsed;
            MainWindow.Instance.Announce(found.Length == 1 ? "1 book found" : $"{found.Length} books found");
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

    /// <summary>
    /// Loads a result's cover through the core, a few at a time, without
    /// holding up the list. Any trouble leaves the stand-in: covers are
    /// decoration.
    /// </summary>
    private async Task LoadCoverAsync(ResultItem item, int shown)
    {
        if (item.Result.CoverId is not { } id) return;
        await CoverLoads.WaitAsync();
        try
        {
            if (shown != _shown) return; // the list has moved on
            var bytes = App.Options.DemoJournal
                ? DemoJournal.SearchCover(id)
                : await Session.Journal.SearchCoverThumbnailAsync(UserId, id);
            if (bytes is null || shown != _shown) return;
            item.Cover = await Covers.FromBytesAsync(bytes, ResultItem.CoverWidth);
        }
        catch (CoreException ex)
        {
            StartupLog.Step($"No cover for a search result: {ex.UserMessage()}");
        }
        finally
        {
            CoverLoads.Release();
        }
    }

    private void ShowHint(string glyph, string title, string text)
    {
        _shown++;
        Results.ItemsSource = null;
        Results.Visibility = Visibility.Collapsed;
        HintGlyph.Glyph = glyph;
        HintTitle.Text = title;
        HintText.Text = text;
        Hint.Visibility = Visibility.Visible;
        ScreenReader.LiveRegionChanged(HintTitle);
    }

    private void ShowProblem(string title, string message)
    {
        Problem.Title = title;
        Problem.Message = message;
        ScreenReader.Open(Problem);
    }
}
