using Bookshelf.Ffi;

namespace Bookshelf.Core.Tests;

/// <summary>
/// The journal as the Windows app uses it, end to end, without the network:
/// books come from a search result that already has its description and no
/// cover URL, so saving one asks Open Library for nothing. These need the
/// Rust core (bookshelf_ffi) next to the test binary; see the .csproj.
/// </summary>
public sealed class JournalServiceTests : IDisposable
{
    private readonly string _dir =
        Path.Combine(Path.GetTempPath(), "bookshelf-tests-" + Guid.NewGuid().ToString("N"));

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

    private static SearchResult OfflineBook(string key, string title, string? coverUrl = null) => new(
        ExternalId: key, Title: title, Subtitle: null, Author: "Frank Herbert", Isbn: null, Publisher: null,
        Description: "Sand, and the people of it.", PublishedDate: "1965", PageCount: 412, CoverUrl: coverUrl);

    private string Folder(string name) => Directory.CreateDirectory(Path.Combine(_dir, name)).FullName;

    private static DateOnly Today => DateOnly.FromDateTime(DateTime.Now);

    [Fact]
    public async Task TheWholeJourneyOffline()
    {
        // Open, and make a profile.
        using var journal = await JournalService.OpenAsync(Path.Combine(_dir, "journal"));
        var avery = await journal.CreateProfileAsync("Avery", " avery@example.com ");
        Assert.Equal("avery@example.com", avery.Email);

        // Add a book to the Reading shelf.
        var book = await journal.SaveSearchResultAsync(avery.Id, OfflineBook("/works/OL1W", "Dune"));
        Assert.Equal("Sand, and the people of it.", book.Description);
        Assert.Null(book.CoverPath);
        Assert.Null(await journal.ExistingEntryAsync(avery.Id, book.Id));
        var id = await journal.AddToShelfAsync(avery.Id, book.Id, Shelf.Reading);
        Assert.Equal(id, await journal.ExistingEntryAsync(avery.Id, book.Id));

        // Write, with line ends as RichEditBox gives them.
        var saved = await journal.SaveBodyAsync(id, "# Dune\rSpice **must** flow.");
        Assert.Equal(5u, saved.Words);
        var entry = await journal.EntryAsync(id);
        Assert.Equal("# Dune\nSpice **must** flow.", entry.Body);
        Assert.Equal(Shelf.Reading, entry.Shelf);
        Assert.Equal<DateOnly?>(Today, entry.StartedDate());
        Assert.True((DateTimeOffset.Now - entry.CreatedAt()).Duration() < TimeSpan.FromMinutes(10));

        var reading = await journal.ShelfAsync(avery.Id, Shelf.Reading);
        Assert.Equal("1 book", reading.CountLine);
        var item = Assert.IsType<ShelfRow.Entry>(reading.Rows[0]).Item;
        Assert.Equal("Dune", item.Title);
        Assert.Equal("Dune Spice must flow.", item.Excerpt);
        Assert.EndsWith("day 1", item.Meta);
        Assert.True(BookshelfFfiMethods.Matches(item.Haystack, BookshelfFfiMethods.QueryTerms("SPICE herbert")));

        // Finish it, with dates.
        var done = await journal.SetDatesAsync(id, new DateOnly(2026, 9, 1), new DateOnly(2026, 9, 11));
        Assert.Equal(Shelf.Finished, done.Shelf);
        Assert.Equal<long?>(10, done.Days);
        Assert.Equal<DateOnly?>(new DateOnly(2026, 9, 11), done.FinishedDate());
        var refused = await Assert.ThrowsAsync<CoreException.Invalid>(
            () => journal.SetDatesAsync(id, new DateOnly(2026, 9, 11), new DateOnly(2026, 9, 1)));
        Assert.Equal("Finished date is before started date.", refused.UserMessage());

        // Remove it, then Undo.
        var removed = await journal.RemoveEntryAsync(id);
        Assert.Equal("Dune", removed.Title());
        Assert.Equal(0u, (await journal.ShelfAsync(avery.Id, Shelf.Finished)).Total);
        await journal.RestoreEntryAsync(removed);
        Assert.Equal(1u, (await journal.ShelfAsync(avery.Id, Shelf.Finished)).Total);
        Assert.Equal(done, await journal.EntryAsync(id));

        // Read it again.
        var again = await journal.ReadAgainAsync(id);
        Assert.NotEqual(id, again);
        Assert.Equal(Shelf.Reading, (await journal.EntryAsync(again)).Shelf);

        // Back up into a folder of our own.
        var backups = Folder("chosen");
        await journal.SetBackupFolderAsync(backups);
        var made = await journal.BackUpNowAsync();
        Assert.NotNull(made);
        Assert.True(File.Exists(made));
        Assert.Null(await journal.BackUpNowAsync()); // one a day
        var status = await journal.BackupStatusAsync();
        Assert.True(status.IsCustom);
        Assert.True(status.Available);
        Assert.Equal(backups, status.Folder);
        Assert.Equal(JournalDate.ToText(Today), status.Latest);

        // Export.
        var exported = await journal.ExportMarkdownAsync(avery.Id, Folder("export"));
        Assert.Equal(2u, exported.Count);
        var dune = await File.ReadAllTextAsync(Path.Combine(exported.Folder, "dune.md"));
        Assert.Contains("Spice **must** flow.", dune);

        await journal.CloseAsync();
        await journal.CloseAsync();
    }

    [Fact]
    public async Task ProfilesAndSettings()
    {
        using var journal = await JournalService.OpenAsync(Path.Combine(_dir, "journal"));
        Assert.Empty(await journal.ProfilesAsync());
        var avery = await journal.CreateProfileAsync("Avery");
        Assert.Equal(new[] { "Avery" }, (await journal.ProfilesAsync()).Select(p => p.Name));

        var taken = await Assert.ThrowsAsync<CoreException.Invalid>(() => journal.CreateProfileAsync("Avery"));
        Assert.Equal("That name is already taken.", taken.UserMessage());
        var email = await Assert.ThrowsAsync<CoreException.Invalid>(() => journal.CreateProfileAsync("Sam", "nope"));
        Assert.Equal("That doesn't look like an email address.", email.UserMessage());

        var settings = await journal.SettingsAsync(avery.Id);
        Assert.Equal(Theme.System, settings.Theme);
        Assert.Equal(WritingFont.IaDuo, settings.WritingFont);
        Assert.Equal(Shelf.Reading, settings.StartShelf);
        var changed = settings with { Theme = Theme.Dark, DateFormat = DateFormat.YearMonthDay, WritingSize = 18 };
        await journal.UpdateSettingsAsync(changed);
        Assert.Equal(changed, await journal.SettingsAsync(avery.Id));
        Assert.Equal(24.0, BookshelfFfiMethods.WriterLayout(changed).FontPx);
        await Assert.ThrowsAsync<CoreException.Invalid>(
            () => journal.UpdateSettingsAsync(changed with { WritingSize = 40 }));

        Assert.Equal("Ave", (await journal.RenameProfileAsync(avery.Id, "Ave")).Name);
        Assert.Equal("ave@example.com", (await journal.SetProfileEmailAsync(avery.Id, "ave@example.com")).Email);
        await journal.DeleteProfileAsync(avery.Id);
        Assert.Empty(await journal.ProfilesAsync());
        await Assert.ThrowsAsync<CoreException.NotFound>(() => journal.SettingsAsync(avery.Id));
        await Assert.ThrowsAsync<CoreException.Invalid>(() => journal.EntryAsync("not an id!"));
    }

    [Fact]
    public async Task CoreMessagesGoToTheLogFile()
    {
        using var journal = await JournalService.OpenAsync(Path.Combine(_dir, "journal"));
        var log = CoreLog.Start(Folder("logs"));
        var avery = await journal.CreateProfileAsync("Avery");
        // Not https, so the cover is refused (and logged) before any request.
        var book = await journal.SaveSearchResultAsync(
            avery.Id, OfflineBook("/works/OL2W", "Emma", coverUrl: "http://example.invalid/cover.jpg"));
        Assert.Null(book.CoverPath);
        Assert.Contains("cover download failed", await File.ReadAllTextAsync(log.FilePath));
    }

    [Fact]
    public void DatesAndEdits()
    {
        Assert.Equal("2024-02-29", JournalDate.ToText(new DateOnly(2024, 2, 29)));
        Assert.Null(JournalDate.ToText((DateOnly?)null));
        Assert.Equal<DateOnly?>(new DateOnly(2026, 9, 5), JournalDate.Parse("2026-09-05"));
        Assert.Null(JournalDate.Parse("2026-02-30"));
        Assert.Null(JournalDate.Parse("09/05/2026"));
        Assert.Null(JournalDate.Parse(null));

        // "📚 word": the emoji is two UTF-16 units, as C# strings count.
        var edit = CoreValues.FormatEdit("📚 word", 3, 7, new FormatAction.Bold());
        Assert.NotNull(edit);
        Assert.Equal("**word**", edit.Replacement);
        Assert.Equal((3u, 7u, 5u, 9u), (edit.Start, edit.End, edit.NewSelStart, edit.NewSelEnd));
        Assert.Null(CoreValues.FormatEdit("a   b", 1, 4, new FormatAction.Italic()));
    }

    [Fact]
    public void ReadingPageAndHelp()
    {
        var blocks = BookshelfFfiMethods.RenderMarkdown("# Hi\n\n**bold**");
        var heading = Assert.IsType<Block.Heading>(blocks[0]);
        Assert.Equal(1, heading.Level);
        Assert.Equal("Hi", heading.Runs[0].Text);
        var para = Assert.IsType<Block.Paragraph>(blocks[1]);
        Assert.Equal(new[] { RunStyle.Bold }, para.Runs[0].Styles);

        Assert.StartsWith("# Bookshelf", BookshelfFfiMethods.Manual().Intro);
        var bold = BookshelfFfiMethods.Shortcuts(Platform.Windows).SelectMany(g => g.Items).First(s => s.Title == "Bold");
        var combo = Assert.IsType<Accel.Keys>(bold.Accel).Combo;
        Assert.Equal(new ShortcutKey.Character("b"), combo.Key);
        Assert.True(combo.Control);
        Assert.False(combo.Command);

        Assert.Equal("#2d71e5", BookshelfFfiMethods.Accents()[0].Hex);
        Assert.Equal("#2d71e5", BookshelfFfiMethods.AccentColors("#2d71e5", false).Bg);
        Assert.Equal(700u, BookshelfFfiMethods.AutosaveMs());
        Assert.Equal(2u, BookshelfFfiMethods.WordCount("two words"));
        Assert.StartsWith("What stayed with you", BookshelfFfiMethods.Prompts(Shelf.Finished));
        Assert.Equal("~/notes", BookshelfFfiMethods.DisplayPath("/home/ann/notes", "/home/ann").Replace('\\', '/'));
    }
}
