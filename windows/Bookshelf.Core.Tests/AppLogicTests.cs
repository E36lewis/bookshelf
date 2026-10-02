using Bookshelf.Ffi;

namespace Bookshelf.Core.Tests;

/// <summary>
/// The Windows app's own logic that doesn't need a window: what it
/// remembers, the demo journal, the words it shows and its shortcut keys.
/// </summary>
public sealed class AppLogicTests : IDisposable
{
    private readonly string _dir =
        Directory.CreateDirectory(Path.Combine(Path.GetTempPath(), "bookshelf-app-tests-" + Guid.NewGuid().ToString("N"))).FullName;

    public void Dispose()
    {
        try
        {
            Directory.Delete(_dir, recursive: true);
        }
        catch (IOException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }
    }

    [Fact]
    public void AppStateRoundTripsAndForgivesBadFiles()
    {
        Assert.Null(AppState.Load(_dir).LastProfileId);

        Assert.True(new AppState { LastProfileId = "7f1c8f0e-8a4b" }.Save(_dir));
        Assert.Equal("7f1c8f0e-8a4b", AppState.Load(_dir).LastProfileId);
        Assert.False(File.Exists(Path.Combine(_dir, AppState.FileName + ".tmp")));

        // Damaged, or not ours: a fresh start.
        var file = Path.Combine(_dir, AppState.FileName);
        File.WriteAllText(file, "{ not json");
        Assert.Null(AppState.Load(_dir).LastProfileId);
        File.WriteAllText(file, "null");
        Assert.Null(AppState.Load(_dir).LastProfileId);
        File.WriteAllText(file, new string(' ', 70 * 1024));
        Assert.Null(AppState.Load(_dir).LastProfileId);

        // A folder that isn't there can't be written to.
        Assert.False(new AppState { LastProfileId = "x" }.Save(Path.Combine(_dir, "missing")));
    }

    [Fact]
    public async Task TheDemoJournalFillsEveryShelf()
    {
        using var journal = await JournalService.OpenAsync(Path.Combine(_dir, "demo"));
        var today = new DateOnly(2026, 9, 20);
        var demo = await DemoJournal.SeedAsync(journal, today);
        Assert.Equal(new[] { "Avery", "Jo", "Sam" }, (await journal.ProfilesAsync()).Select(p => p.Name));

        var now = new DateTimeOffset(2026, 9, 20, 12, 0, 0, TimeSpan.Zero);
        var reading = await journal.ShelfAsync(demo.Avery.Id, Shelf.Reading, now);
        var finished = await journal.ShelfAsync(demo.Avery.Id, Shelf.Finished, now);
        var someday = await journal.ShelfAsync(demo.Avery.Id, Shelf.Eventually, now);
        Assert.Equal(2u, reading.Total);
        Assert.Equal(4u, finished.Total);
        Assert.Equal(2u, finished.ThisYear);
        Assert.Equal(2u, someday.Total);

        // Finished books are grouped under this year and last year.
        var years = finished.Rows.OfType<ShelfRow.YearHeading>().Select(y => y.Year).ToArray();
        Assert.Equal(new[] { 2026, 2025 }, years);
        var dune = finished.Rows.OfType<ShelfRow.Entry>().First().Item;
        Assert.Equal("Dune", dune.Title);
        Assert.Contains("water", dune.Excerpt);
        Assert.Null(dune.CoverPath); // offline: no covers

        // Sam has a look of their own, and Jo nothing yet.
        var sam = await journal.SettingsAsync(demo.Sam.Id);
        Assert.Equal(Theme.Dark, sam.Theme);
        Assert.Equal(WeekStart.Monday, sam.WeekStart);
        Assert.Equal(0u, (await journal.ShelfAsync(demo.Jo.Id, Shelf.Reading, now)).Total);
    }

    [Fact]
    public void ShelfWords()
    {
        Assert.Equal(new[] { "Reading", "Finished", "Eventually" }, ShelfText.All.Select(ShelfText.Name));
        Assert.Equal("Someday", ShelfText.Heading(Shelf.Eventually));
        Assert.Equal("Books you've started will live here.", ShelfText.Empty(Shelf.Reading));
        Assert.Equal("Nothing on this shelf matches “dune”.", ShelfText.NoMatches(" dune "));
        Assert.Equal("1 match", ShelfText.Matches(1));
        Assert.Equal("3 matches", ShelfText.Matches(3));
    }

    [Fact]
    public void DatesAndTheirRules()
    {
        Assert.Null(ReadingDates.DaysText(null));
        Assert.Equal("Finished the same day", ReadingDates.DaysText(0));
        Assert.Equal("Finished in 1 day", ReadingDates.DaysText(1));
        Assert.Equal("Finished in 10 days", ReadingDates.DaysText(10));

        var sep1 = new DateOnly(2026, 9, 1);
        var sep9 = new DateOnly(2026, 9, 9);
        Assert.True(ReadingDates.CanStart(sep1, sep9));
        Assert.True(ReadingDates.CanStart(sep9, sep9));
        Assert.False(ReadingDates.CanStart(sep9, sep1));
        Assert.True(ReadingDates.CanStart(sep9, null));
        Assert.True(ReadingDates.CanFinish(sep9, sep1));
        Assert.False(ReadingDates.CanFinish(sep1, sep9));
        Assert.True(ReadingDates.CanFinish(sep1, null));

        var day = new DateOnly(2026, 9, 5);
        Assert.Equal("Sep 5, 2026", ReadingDates.Format(day, DateFormat.Long));
        Assert.Equal("09/05/2026", ReadingDates.Format(day, DateFormat.MonthDayYear));
        Assert.Equal("05/09/2026", ReadingDates.Format(day, DateFormat.DayMonthYear));
        Assert.Equal("2026-09-05", ReadingDates.Format(day, DateFormat.YearMonthDay));
        Assert.Equal("{year.full}-{month.integer(2)}-{day.integer(2)}", ReadingDates.PickerPattern(DateFormat.YearMonthDay));
    }

    [Fact]
    public void SearchResultLines()
    {
        var full = new SearchResult("/works/OL1W", "Dune", null, "Frank Herbert", null, null, null, "1965", null, null);
        Assert.Equal("Frank Herbert · 1965", SearchText.Byline(full));
        Assert.Equal("1965", SearchText.Byline(full with { Author = null }));
        Assert.Equal("", SearchText.Byline(full with { Author = " ", PublishedDate = null }));

        var book = new BookInfo("b", "Dune", null, null, null, "Ace", null, "1965", 412, null);
        Assert.Equal("Ace · 1965 · 412 pages", SearchText.Details(book));
        Assert.Equal("", SearchText.Details(book with { Publisher = null, PublishedDate = null, PageCount = 0 }));
    }

    [Fact]
    public void EveryShortcutTheAppUsesIsInTheCoresTable()
    {
        foreach (var title in ShortcutKeys.Titles.All)
        {
            Assert.True(ShortcutKeys.Find(title) is not null, title);
        }
        Assert.Null(ShortcutKeys.Find("No such thing"));

        var add = ShortcutKeys.Find(ShortcutKeys.Titles.AddBook)!;
        Assert.Equal("Ctrl+N", ShortcutKeys.Label(add));
        Assert.Equal<(int, bool)?>(((int)'N', false), ShortcutKeys.VirtualKey(add.Key));

        var help = ShortcutKeys.Find(ShortcutKeys.Titles.Shortcuts)!;
        Assert.Equal("Ctrl+?", ShortcutKeys.Label(help));
        Assert.Equal<(int, bool)?>((0xBF, true), ShortcutKeys.VirtualKey(help.Key));

        var settings = ShortcutKeys.Find(ShortcutKeys.Titles.Settings)!;
        Assert.Equal(new[] { "Ctrl", "," }, ShortcutKeys.Keys(settings));
        Assert.Equal<(int, bool)?>((0xBC, false), ShortcutKeys.VirtualKey(settings.Key));

        Assert.Equal("F11", ShortcutKeys.Label(ShortcutKeys.Find(ShortcutKeys.Titles.FullScreen)!));
        Assert.Equal<(int, bool)?>((0x7A, false), ShortcutKeys.VirtualKey(new ShortcutKey.Function(11)));
        Assert.Equal("Esc", ShortcutKeys.Label(ShortcutKeys.Find(ShortcutKeys.Titles.LeaveFullScreen)!));
        Assert.Equal<(int, bool)?>(((int)'1', false), ShortcutKeys.VirtualKey(new ShortcutKey.Character("1")));
        Assert.Null(ShortcutKeys.VirtualKey(new ShortcutKey.Character("é")));
        Assert.Null(ShortcutKeys.VirtualKey(new ShortcutKey.Character("ab")));
    }

    [Fact]
    public void FullScreenAlsoHasAKeyOutsideTheFunctionRow()
    {
        var keys = ShortcutKeys.FindAll(ShortcutKeys.Titles.FullScreen);
        Assert.Equal(new[] { "F11", "Ctrl+Shift+Enter" }, keys.Select(ShortcutKeys.Label));
        Assert.Equal<(int, bool)?>((0x0D, false), ShortcutKeys.VirtualKey(keys[1].Key));
        Assert.Equal("F11 or Ctrl+Shift+Enter", ShortcutKeys.Labels(ShortcutKeys.Titles.FullScreen));
        Assert.Equal("Esc, F11 or Ctrl+Shift+Enter",
            ShortcutKeys.Labels(ShortcutKeys.Titles.LeaveFullScreen, ShortcutKeys.Titles.FullScreen));
        Assert.Null(ShortcutKeys.Labels("No such thing"));

        // No other shortcut has it.
        var others = BookshelfFfiMethods.Shortcuts(Platform.Windows)
            .SelectMany(g => g.Items)
            .Where(s => s.Title != ShortcutKeys.Titles.FullScreen)
            .SelectMany(s => ShortcutKeys.FindAll(s.Title));
        Assert.DoesNotContain(keys[1], others);
    }
}
