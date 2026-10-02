using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace Bookshelf.Pages;

/// <summary>
/// One shelf: the open profile's books on it, newest first (finished books
/// grouped by year), filtered by the search. One instance serves all three
/// shelves and is kept while a book is open, so coming back keeps the place.
/// </summary>
public sealed partial class ShelfPage : BookshelfPage
{
    private bool _listening;
    private (ShelfView? View, string Query, Shelf Shelf, Microsoft.UI.Xaml.Media.FontFamily Font)? _shown;

    public ShelfPage()
    {
        InitializeComponent();
        // Undo a removal (also on the notice's button). A text box keeps its own Ctrl+Z.
        Keys.Add(this, VirtualKey.Z, VirtualKeyModifiers.Control, () => _ = Shell.UndoRemoveAsync(), exceptWhileTyping: true);
        // Just start typing to search.
        CharacterReceived += OnCharacterReceived;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (!_listening)
        {
            Session.ShelvesChanged += Show;
            Session.SearchChanged += Show;
            _listening = true;
        }
        Show();
        _ = Session.RefreshShelvesAsync();
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
        if (view is null)
        {
            CountLine.Text = "";
            Entries.ItemsSource = null;
            EmptyState.Visibility = Visibility.Collapsed;
            return;
        }

        var items = new List<EntryItem>();
        var groups = new List<YearGroup>();
        YearGroup? year = null;
        foreach (var row in view.Rows)
        {
            switch (row)
            {
                case ShelfRow.YearHeading heading:
                    year = new YearGroup(heading.Year, heading.Label);
                    groups.Add(year);
                    break;
                case ShelfRow.Entry entry when Session.Matches(entry.Item):
                    var item = new EntryItem(entry.Item);
                    items.Add(item);
                    year?.Add(item);
                    break;
            }
        }

        CountLine.Text = Session.IsSearching ? ShelfText.Matches(items.Count) : view.CountLine;
        Entries.ItemsSource = groups.Count > 0
            ? new CollectionViewSource { IsSourceGrouped = true, Source = groups }.View
            : items;
        foreach (var item in items) item.LoadCover();

        var empty = items.Count == 0;
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

    private void OnEntryClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is EntryItem item)
        {
            Frame.Navigate(typeof(BookPage), item.SummaryId, new DrillInNavigationTransitionInfo());
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
