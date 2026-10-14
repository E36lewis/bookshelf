using Bookshelf.Controls;
using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace Bookshelf.Pages;

/// <summary>
/// One shelf: the open profile's books on it as a grid of cards, newest
/// first (finished books grouped by year), filtered by the search. One
/// instance serves all three shelves and is kept while a book is open, so
/// coming back keeps the place.
/// </summary>
public sealed partial class ShelfPage : BookshelfPage
{
    private bool _listening;
    private (ShelfView? View, string Query, Shelf Shelf, Microsoft.UI.Xaml.Media.FontFamily Font)? _shown;

    // What's shown, by year (or one group with no year), so a change in the
    // number of columns only regroups the rows.
    private List<(YearRow? Year, List<EntryItem> Items)> _sections = [];
    private int _columns = 3;
    private double _gridWidth;
    private string? _opened; // the book last opened from here, whose card gets the focus back

    public ShelfPage()
    {
        InitializeComponent();
        // Undo a removal (also on the notice's button). A text box keeps its own Ctrl+Z.
        Keys.Add(this, VirtualKey.Z, VirtualKeyModifiers.Control, () => _ = Shell.UndoRemoveAsync(), exceptWhileTyping: true);
        // Just start typing to search.
        CharacterReceived += OnCharacterReceived;
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (!_listening)
        {
            Session.ShelvesChanged += Show;
            Session.SearchChanged += Show;
            _listening = true;
        }
        Show();
        // Back from a book: the focus goes to its card (again, if the shelf
        // is laid out afresh because something changed meanwhile).
        var back = e.NavigationMode == NavigationMode.Back ? _opened : null;
        if (back is not null) FocusCard(back);
        try
        {
            await Session.RefreshShelvesAsync();
        }
        catch (CoreException)
        {
            return; // the shelf stays as it was
        }
        if (back is not null && Frame?.Content == this) FocusCard(back);
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        Session.ShelvesChanged -= Show;
        Session.SearchChanged -= Show;
        _listening = false;
    }

    /// <summary>Shows <see cref="Services.Session.CurrentShelf"/> (after a switch).</summary>
    internal void Show()
    {
        var shelf = Session.CurrentShelf;
        var view = Session.ViewOf(shelf);
        if (_shown == (view, Session.Query, shelf, Look.Heading)) return; // nothing new: keep the scroll position
        _shown = (view, Session.Query, shelf, Look.Heading);

        Heading.Text = ShelfText.Heading(shelf);
        Heading.FontFamily = Look.Heading;
        EmptyTitle.FontFamily = Look.Heading;
        if (view is null)
        {
            CountLine.Text = "";
            _sections = [];
            Entries.ItemsSource = null;
            EmptyState.Visibility = Visibility.Collapsed;
            return;
        }

        // Year headings step aside while searching, as in the GTK app.
        var sections = new List<(YearRow? Year, List<EntryItem> Items)>();
        var all = new List<EntryItem>();
        foreach (var row in view.Rows)
        {
            switch (row)
            {
                case ShelfRow.YearHeading heading when !Session.IsSearching:
                    sections.Add((new YearRow(heading.Year, heading.Label), new List<EntryItem>()));
                    break;
                case ShelfRow.Entry entry when Session.Matches(entry.Item):
                    var item = new EntryItem(entry.Item);
                    if (sections.Count == 0) sections.Add((null, new List<EntryItem>()));
                    sections[^1].Items.Add(item);
                    all.Add(item);
                    break;
            }
        }
        for (var i = 0; i < all.Count; i++)
        {
            all[i].Position = i + 1;
            all[i].SetSize = all.Count;
        }
        _sections = sections.Where(s => s.Items.Count > 0).ToList();

        var count = Session.IsSearching ? ShelfText.Matches(all.Count) : view.CountLine;
        if (count != CountLine.Text)
        {
            CountLine.Text = count;
            ScreenReader.LiveRegionChanged(CountLine); // "3 matches" as you search
        }
        LayOutRows();

        var empty = all.Count == 0;
        EmptyState.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        Entries.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
        if (empty && Session.IsSearching)
        {
            EmptyGlyph.Glyph = ""; // search
            EmptyTitle.Text = "No matches";
            EmptyText.Text = ShelfText.NoMatches(Session.Query);
            EmptyAddButton.Visibility = Visibility.Collapsed;
        }
        else if (empty)
        {
            EmptyGlyph.Glyph = ""; // library
            EmptyTitle.Text = "Nothing here yet";
            EmptyText.Text = ShelfText.Empty(shelf);
            EmptyAddButton.Visibility = Visibility.Visible;
        }
    }

    /// <summary>The year headings and rows of cards, as many to a row as fit.</summary>
    private void LayOutRows()
    {
        var rows = new List<object>();
        foreach (var (year, items) in _sections)
        {
            if (year is not null) rows.Add(year);
            for (var i = 0; i < items.Count; i += _columns)
            {
                rows.Add(new CardRow(items.GetRange(i, Math.Min(_columns, items.Count - i))));
            }
        }
        Heights.Reset();
        Entries.ItemsSource = rows;
    }

    /// <summary>The shelf's tallest card, which every card's height follows.</summary>
    private CardHeights Heights => (CardHeights)Resources["Heights"];

    private void OnEntriesSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var columns = CardRowLayout.Columns(e.NewSize.Width);
        var widthChanged = Math.Abs(e.NewSize.Width - _gridWidth) > 0.5;
        _gridWidth = e.NewSize.Width;
        if (columns != _columns)
        {
            _columns = columns;
            if (_sections.Count > 0) LayOutRows();
        }
        else if (widthChanged)
        {
            Heights.Reset(); // the cards wrap differently: measure them again
        }
    }

    /// <summary>Focuses a book's card, if it's on the shelf.</summary>
    private void FocusCard(string id)
    {
        if (Entries.ItemsSource is not List<object> rows) return;
        for (var r = 0; r < rows.Count; r++)
        {
            if (rows[r] is not CardRow row) continue;
            var i = row.Cards.FindIndex(c => c.SummaryId == id);
            if (i < 0) continue;
            if (Entries.GetOrCreateElement(r) is ItemsRepeater cards && cards.GetOrCreateElement(i) is Control card)
            {
                card.StartBringIntoView();
                if (!card.Focus(FocusState.Programmatic)) DispatcherQueue.TryEnqueue(() => card.Focus(FocusState.Programmatic));
            }
            return;
        }
    }

    private void OnCardClick(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            _opened = id;
            Frame.Navigate(typeof(BookPage), id, new DrillInNavigationTransitionInfo());
        }
    }

    private void OnAddBook(object sender, RoutedEventArgs e) => _ = Shell.AddBookAsync();

    private void OnCharacterReceived(UIElement sender, CharacterReceivedRoutedEventArgs args)
    {
        var c = args.Character;
        if (char.IsControl(c) || char.IsWhiteSpace(c) || Keys.IsTyping(XamlRoot) || DialogHost.IsOpen) return;
        args.Handled = true;
        Shell.StartSearch(c);
    }
}
