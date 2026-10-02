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
/// A book's page: what the book is, when it was read (dates saved as soon
/// as they're picked), and the summary laid out as a finished page.
/// </summary>
public sealed partial class BookPage : BookshelfPage
{
    private string _id = "";
    private EntryDetail? _entry;
    private bool _updating; // the pickers are being set from code, not by the person

    public BookPage()
    {
        InitializeComponent();
        Keys.Add(WriteButton, ShortcutKeys.Titles.Write, Write);
        Keys.Add(ReadButton, ShortcutKeys.Titles.Read, Read);
        Keys.Add(this, VirtualKey.Delete, VirtualKeyModifiers.None, () => _ = RemoveAsync(), exceptWhileTyping: true);
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        _id = e.Parameter as string ?? "";
        try
        {
            Show(await Session.Journal.EntryAsync(_id));
        }
        catch (CoreException ex)
        {
            ShowProblem("Couldn't open this book", ex.UserMessage());
        }
    }

    private void Show(EntryDetail entry)
    {
        _entry = entry;
        var book = entry.Book;
        BookTitle.Text = book.Title;
        BookTitle.FontFamily = Look.Heading;
        SetText(Subtitle, book.Subtitle);
        Subtitle.FontFamily = Look.Heading;
        SetText(Author, book.Author);
        SetText(Details, SearchText.Details(book));
        CoverInitial.Text = book.Title.TrimStart().Length > 0 ? char.ToUpperInvariant(book.Title.TrimStart()[0]).ToString() : "";
        CoverInitial.FontFamily = Look.Heading;
        _ = ShowCoverAsync(book.CoverPath);

        Description.Text = book.Description ?? "";
        About.Visibility = string.IsNullOrWhiteSpace(book.Description) ? Visibility.Collapsed : Visibility.Visible;

        ShowDates(entry);
        _ = ShowSummaryAsync(entry.Body);
    }

    private static void SetText(TextBlock block, string? text)
    {
        block.Text = text ?? "";
        block.Visibility = string.IsNullOrWhiteSpace(text) ? Visibility.Collapsed : Visibility.Visible;
    }

    private async Task ShowCoverAsync(string? path)
    {
        var cover = await Covers.LoadAsync(path, (int)CoverFrame.Width);
        CoverImage.Source = cover;
        CoverStandIn.Visibility = cover is null ? Visibility.Visible : Visibility.Collapsed;
    }

    // ---- reading dates ---------------------------------------------------------

    private static DateOnly Today => DateOnly.FromDateTime(DateTime.Now);

    private static DateTimeOffset At(DateOnly day) => new(day.ToDateTime(TimeOnly.MinValue));

    private static DateOnly? Day(DateTimeOffset? at) => at is { } d ? DateOnly.FromDateTime(d.Date) : null;

    /// <summary>Shows the entry's dates; days that would contradict the other date can't be picked.</summary>
    private void ShowDates(EntryDetail entry)
    {
        _updating = true;
        try
        {
            var settings = Session.Settings;
            var started = entry.StartedDate();
            var finished = entry.FinishedDate();
            var earliest = At(Today.AddYears(-120));
            var latest = At(Today.AddYears(20));
            foreach (var picker in new[] { StartedPicker, FinishedPicker })
            {
                picker.FirstDayOfWeek = settings?.WeekStart == WeekStart.Monday
                    ? Windows.Globalization.DayOfWeek.Monday
                    : Windows.Globalization.DayOfWeek.Sunday;
                picker.DateFormat = ReadingDates.PickerPattern(settings?.DateFormat ?? DateFormat.Long);
                picker.MinDate = earliest;
                picker.MaxDate = latest;
            }
            StartedPicker.Date = started is { } s ? At(s) : null;
            FinishedPicker.Date = finished is { } f ? At(f) : null;
            if (finished is { } end) StartedPicker.MaxDate = At(end);
            if (started is { } start) FinishedPicker.MinDate = At(start);

            StartedToday.IsEnabled = ReadingDates.CanStart(Today, finished) && started != Today;
            FinishedToday.IsEnabled = ReadingDates.CanFinish(Today, started) && finished != Today;
            StartedClear.IsEnabled = started is not null;
            FinishedClear.IsEnabled = finished is not null;
            SetText(DaysText, ReadingDates.DaysText(entry.Days));
        }
        finally
        {
            _updating = false;
        }
    }

    private void OnStartedChanged(CalendarDatePicker sender, CalendarDatePickerDateChangedEventArgs args)
    {
        if (!_updating && _entry is { } entry) _ = SaveDatesAsync(Day(args.NewDate), entry.FinishedDate());
    }

    private void OnFinishedChanged(CalendarDatePicker sender, CalendarDatePickerDateChangedEventArgs args)
    {
        if (!_updating && _entry is { } entry) _ = SaveDatesAsync(entry.StartedDate(), Day(args.NewDate));
    }

    private void OnStartedToday(object sender, RoutedEventArgs e)
    {
        if (_entry is { } entry) _ = SaveDatesAsync(Today, entry.FinishedDate());
    }

    private void OnFinishedToday(object sender, RoutedEventArgs e)
    {
        if (_entry is { } entry) _ = SaveDatesAsync(entry.StartedDate(), Today);
    }

    private void OnStartedClear(object sender, RoutedEventArgs e)
    {
        if (_entry is { } entry) _ = SaveDatesAsync(null, entry.FinishedDate());
    }

    private void OnFinishedClear(object sender, RoutedEventArgs e)
    {
        if (_entry is { } entry) _ = SaveDatesAsync(entry.StartedDate(), null);
    }

    private async Task SaveDatesAsync(DateOnly? started, DateOnly? finished)
    {
        if (_entry is null) return;
        try
        {
            var saved = await Session.Journal.SetDatesAsync(_id, started, finished);
            _entry = saved;
            Problem.IsOpen = false;
            ShowDates(saved);
            Shell.Announce(DaysText.Visibility == Visibility.Visible ? $"Dates saved. {DaysText.Text}" : "Dates saved");
            await Session.RefreshShelvesAsync();
        }
        catch (CoreException ex)
        {
            ShowProblem("Couldn't save the dates", ex.UserMessage());
            ShowDates(_entry);
        }
    }

    // ---- the summary -----------------------------------------------------------

    private async Task ShowSummaryAsync(string body)
    {
        var empty = string.IsNullOrWhiteSpace(body);
        WriteLabel.Text = empty ? "Write" : "Edit";
        ReadButton.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
        NothingYet.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        Preview.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
        if (empty) return;
        var blocks = await Task.Run(() => BookshelfFfiMethods.RenderMarkdown(body));
        MarkdownRenderer.Render(Preview, blocks, 16, Look.Reading, Look.Heading);
    }

    private void Write()
    {
        if (_entry is not null) Frame.Navigate(typeof(WriterPage), _id, new DrillInNavigationTransitionInfo());
    }

    private void Read()
    {
        if (_entry is { } entry && !string.IsNullOrWhiteSpace(entry.Body))
        {
            Frame.Navigate(typeof(ReaderPage), _id, new DrillInNavigationTransitionInfo());
        }
    }

    private void OnWrite(object sender, RoutedEventArgs e) => Write();

    private void OnRead(object sender, RoutedEventArgs e) => Read();

    private void OnSummaryTapped(object sender, TappedRoutedEventArgs e) => Write();

    // ---- read again, remove ----------------------------------------------------

    private async void OnReadAgain(object sender, RoutedEventArgs e)
    {
        if (_entry is null) return;
        try
        {
            var again = await Session.Journal.ReadAgainAsync(_id);
            await Session.RefreshShelvesAsync();
            // The new reading takes this page's place.
            Frame.Navigate(typeof(BookPage), again, new SuppressNavigationTransitionInfo());
            if (Frame.BackStack.Count > 0) Frame.BackStack.RemoveAt(Frame.BackStack.Count - 1);
            Shell.ShowNotice("New reading started today", InfoBarSeverity.Success);
        }
        catch (CoreException ex)
        {
            ShowProblem("Couldn't start a new reading", ex.UserMessage());
        }
    }

    private void OnRemove(object sender, RoutedEventArgs e) => _ = RemoveAsync();

    /// <summary>Removes the entry right away; the notice offers Undo.</summary>
    private async Task RemoveAsync()
    {
        if (_entry is null) return;
        try
        {
            var removed = await Session.Journal.RemoveEntryAsync(_id);
            _entry = null;
            if (Frame.CanGoBack) Frame.GoBack();
            else Shell.ShowShelf(Session.CurrentShelf);
            Shell.ShowRemoved(removed);
            await Session.RefreshShelvesAsync();
        }
        catch (CoreException ex)
        {
            ShowProblem("Couldn't remove it", ex.UserMessage());
        }
    }

    private void ShowProblem(string title, string message)
    {
        Problem.Title = title;
        Problem.Message = message;
        ScreenReader.Open(Problem);
    }
}
