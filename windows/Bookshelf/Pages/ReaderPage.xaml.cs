using Bookshelf.Controls;
using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;

namespace Bookshelf.Pages;

/// <summary>
/// Distraction-free reading of a summary, in the reading font and size, as
/// wide as the writing page. F11 or Ctrl+Shift+Enter (or the button) for
/// full screen, Esc to leave it.
/// </summary>
public sealed partial class ReaderPage : BookshelfPage
{
    private string _id = "";

    public ReaderPage()
    {
        InitializeComponent();
        Keys.Add(this, ShortcutKeys.Titles.FullScreen, () => SetFullScreen(!Shell.IsFullScreen));
        Keys.Add(this, ShortcutKeys.Titles.LeaveFullScreen, () => SetFullScreen(false));
        Shell.FullScreenChanged += OnFullScreenChanged;
        Unloaded += (_, _) => Shell.FullScreenChanged -= OnFullScreenChanged;
        OnFullScreenChanged();
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        _id = e.Parameter as string ?? "";
        try
        {
            var entry = await Session.Journal.EntryAsync(_id);
            var layout = Session.Settings is { } settings ? BookshelfFfiMethods.WriterLayout(settings) : null;
            Column.MaxWidth = (layout?.ColumnWidth ?? 720) + 64; // the column plus its padding
            BookTitle.Text = entry.Book.Title;
            BookTitle.FontFamily = Look.Heading;
            // The scroller has the focus (for the arrow keys), so it's what Narrator names first.
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(Scroller, $"Your summary of {entry.Book.Title}");
            Author.Text = entry.Book.Author ?? "";
            Author.Visibility = string.IsNullOrEmpty(entry.Book.Author) ? Visibility.Collapsed : Visibility.Visible;
            var empty = string.IsNullOrWhiteSpace(entry.Body);
            NothingYet.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
            if (!empty)
            {
                var blocks = await Task.Run(() => BookshelfFfiMethods.RenderMarkdown(entry.Body));
                MarkdownRenderer.Render(Body, blocks, layout?.FontPx ?? 18.67, Look.Reading, Look.Heading);
            }
            Scroller.Focus(FocusState.Programmatic); // arrow keys and Page Down scroll right away
        }
        catch (CoreException ex)
        {
            BookTitle.Text = "Couldn't open this summary";
            Author.Text = ex.UserMessage();
        }
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        SetFullScreen(false); // leaving the reader leaves full screen too
    }

    private void SetFullScreen(bool on) => Shell.SetFullScreen(on);

    private void OnFullScreenChanged()
    {
        var on = Shell.IsFullScreen;
        FullScreenIcon.Glyph = on ? "\uE73F" : "\uE740";
        var keys = on
            ? ShortcutKeys.Labels(ShortcutKeys.Titles.LeaveFullScreen, ShortcutKeys.Titles.FullScreen)
            : ShortcutKeys.Labels(ShortcutKeys.Titles.FullScreen);
        var label = on ? $"Leave full screen ({keys})" : $"Full screen ({keys})";
        ToolTipService.SetToolTip(FullScreenButton, label);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(FullScreenButton, label);
        EditButton.Visibility = on ? Visibility.Collapsed : Visibility.Visible;
    }

    private void OnFullScreen(object sender, RoutedEventArgs e) => SetFullScreen(!Shell.IsFullScreen);

    private void OnEdit(object sender, RoutedEventArgs e)
    {
        // The writer takes the reader's place, so Back returns to the book.
        Frame.Navigate(typeof(WriterPage), _id, new DrillInNavigationTransitionInfo());
        if (Frame.BackStack.Count > 0) Frame.BackStack.RemoveAt(Frame.BackStack.Count - 1);
    }
}
