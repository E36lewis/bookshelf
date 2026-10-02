using System.Globalization;
using Bookshelf.Core.Editing;
using Bookshelf.Ffi;

namespace Bookshelf.Core.Tests;

/// <summary>
/// The writing page's logic that doesn't need a window: words and sentences
/// around the caret, the formatting bar's edits, spacing, timings and the
/// status line. A caret is written ‸ and a selection «…» in the expected
/// texts, as in the core's own tests.
/// </summary>
public class WordBoundsTests
{
    [Theory]
    [InlineData("hello world", 2, 0, 5)]   // inside
    [InlineData("hello world", 0, 0, 5)]   // at the start
    [InlineData("hello world", 5, 0, 5)]   // just after
    [InlineData("hello world", 6, 6, 11)]
    [InlineData("hello world", 11, 6, 11)] // the end of the text
    [InlineData("a  b", 2, 2, 2)]          // between spaces: nothing
    [InlineData("", 0, 0, 0)]
    [InlineData("don't stop", 1, 0, 5)]    // an apostrophe between letters
    [InlineData("l’été", 4, 0, 5)]
    [InlineData("rock 'n' roll", 6, 6, 7)] // quotes around a word aren't part of it
    [InlineData("**bold** x", 4, 2, 6)]    // marks aren't words
    [InlineData("snake_case", 3, 0, 10)]
    [InlineData("3.14", 0, 0, 1)]
    [InlineData("one\ntwo", 3, 0, 3)]
    [InlineData("one\ntwo", 4, 4, 7)]
    [InlineData("word", 99, 0, 4)]         // past the end counts as the end
    public void FindsTheWordAroundTheCaret(string text, int caret, int start, int end)
    {
        Assert.Equal((start, end), WordBounds.Around(text, caret));
    }

    [Fact]
    public void KeepsCharactersWhole()
    {
        // "naïve" with a combining diaeresis, and a letter outside the BMP.
        Assert.Equal((0, 6), WordBounds.Around("naïve", 2));
        const string script = "𝒳y z"; // 𝒳 takes two code units
        Assert.Equal((0, 3), WordBounds.Around(script, 1)); // inside the pair: the whole word
        Assert.Equal((0, 3), WordBounds.Around(script, 3));
        // An emoji isn't a word.
        Assert.Equal((2, 2), WordBounds.Around("a 📚 b", 2));
        Assert.Equal((0, 1), WordBounds.Around("a📚", 1));
    }
}

public class SentenceBoundsTests
{
    [Theory]
    [InlineData("One. Two.", 0, 0, 4)]
    [InlineData("One. Two.", 4, 0, 4)]  // right after the full stop
    [InlineData("One. Two.", 5, 5, 9)]
    [InlineData("One. Two.", 9, 5, 9)]  // the end of the text
    [InlineData("One.  Two", 5, 0, 4)]  // in the spaces after a sentence
    [InlineData("One. ", 5, 5, 5)]      // about to start the next one
    [InlineData("No stop at all", 3, 0, 14)]
    [InlineData("Pi is 3.14 today. Yes", 2, 0, 17)]
    [InlineData("Really?! Yes.", 2, 0, 8)]
    [InlineData("\"Hi.\" She left.", 1, 0, 5)]
    [InlineData("**Bold.** Then", 11, 10, 14)]
    [InlineData("Wait… what", 7, 6, 10)]
    [InlineData("", 0, 0, 0)]
    public void FindsTheSentenceAroundTheCaret(string text, int caret, int start, int end)
    {
        Assert.Equal((start, end), SentenceBounds.Around(text, caret));
    }

    [Fact]
    public void ASentenceNeverCrossesALine()
    {
        const string text = "# A heading\nFirst line goes on\nand on. Done.";
        Assert.Equal((0, 11), SentenceBounds.Around(text, 4));
        Assert.Equal((12, 30), SentenceBounds.Around(text, 15));
        Assert.Equal((31, 38), SentenceBounds.Around(text, 33));
        Assert.Equal((39, 44), SentenceBounds.Around(text, 44));
        Assert.Equal((4, 4), SentenceBounds.Around("one\n\nthree", 4)); // an empty line
    }
}

public class TextDiffMoreTests
{
    [Fact]
    public void NeverSplitsATwoUnitCharacter()
    {
        // 😀 and 😁 share their first code unit; the change must take both.
        Assert.Equal(new TextChange(0, 2, 2), TextDiff.Between("😀", "😁"));
        Assert.Equal(new TextChange(0, 0, 2), TextDiff.Between("😀", "😁😀"));
        Assert.Equal(new TextChange(3, 3, 5), TextDiff.Between("a😀", "a😀😀"));
        Assert.Equal(new TextChange(2, 2, 4), TextDiff.Between("😀x", "😀😀x"));
    }

    [Fact]
    public void NarrowsAReplacementToWhatChanges()
    {
        // Heading 1 on "Title": only "# " goes in.
        var (change, inserted) = TextDiff.Narrow("a\nTitle\nb", 2, 7, "# Title");
        Assert.Equal(new TextChange(2, 2, 4), change);
        Assert.Equal("# ", inserted);

        // Bold around a word changes both ends, so the word goes in again.
        (change, inserted) = TextDiff.Narrow("one word", 4, 8, "**word**");
        Assert.Equal(new TextChange(4, 8, 12), change);
        Assert.Equal("**word**", inserted);

        // Nothing different: an empty change.
        (change, inserted) = TextDiff.Narrow("same", 0, 4, "same");
        Assert.True(change.IsEmpty);
        Assert.Equal("", inserted);
    }

    [Fact]
    public void MovesARangeAlongWithAnEdit()
    {
        var typedBefore = new TextChange(2, 2, 5);     // three characters typed at 2
        Assert.Equal((13, 17), TextDiff.Moved((10, 14), typedBefore));
        Assert.Equal((0, 1), TextDiff.Moved((0, 1), typedBefore)); // before the edit: unchanged
        var typedInside = new TextChange(12, 12, 13);
        Assert.Equal((10, 15), TextDiff.Moved((10, 14), typedInside));
        var deletedAcross = new TextChange(8, 12, 8);
        Assert.Equal((8, 10), TextDiff.Moved((10, 14), deletedAcross));
        Assert.Equal(-3, new TextChange(5, 8, 5).Growth);
    }
}

public class FormattingTests
{
    /// <summary>Formats <paramref name="marked"/> (‸ caret, «…» selection) and returns the result marked the same way.</summary>
    private static string Run(string marked, FormatAction action)
    {
        var (text, start, end) = Unmark(marked);
        var result = FormatCommands.Apply(text, start, end, action);
        if (result is null) return "(nothing)";
        // The change really turns the old text into the new one.
        Assert.Equal(result.Text, string.Concat(text.AsSpan(0, result.Change.Start), result.Inserted, text.AsSpan(result.Change.OldEnd)));
        Assert.Equal(result.Change.NewEnd - result.Change.Start, result.Inserted.Length);
        return Mark(result.Text, result.SelectionStart, result.SelectionEnd);
    }

    private static (string Text, int Start, int End) Unmark(string marked)
    {
        var caret = marked.IndexOf('‸');
        if (caret >= 0) return (marked.Remove(caret, 1), caret, caret);
        var start = marked.IndexOf('«');
        var end = marked.IndexOf('»') - 1;
        return (marked.Remove(start, 1).Remove(end, 1), start, end);
    }

    private static string Mark(string text, int start, int end) =>
        start == end ? text.Insert(start, "‸") : text.Insert(end, "»").Insert(start, "«");

    [Fact]
    public void InlineMarksTakeTheWordAroundTheCaret()
    {
        Assert.Equal("one **«word»** here", Run("one wo‸rd here", new FormatAction.Bold()));
        Assert.Equal("one *«word»* here", Run("one word‸ here", new FormatAction.Italic()));
        Assert.Equal("~~«don't»~~ go", Run("do‸n't go", new FormatAction.Strike()));
        Assert.Equal("`«x»`", Run("‸x", new FormatAction.Code()));
        Assert.Equal("see [here](‸)", Run("see he‸re", new FormatAction.Link()));
        // Next to no word: an empty pair, with the caret in it.
        Assert.Equal("a **‸** b", Run("a ‸ b", new FormatAction.Bold()));
    }

    [Fact]
    public void PressingAgainTakesTheMarksOff()
    {
        Assert.Equal("one «word» here", Run("one **wo‸rd** here", new FormatAction.Bold()));
        Assert.Equal("«word»", Run("**«word»**", new FormatAction.Bold()));
    }

    [Fact]
    public void ASelectionIsUsedAsItIs()
    {
        Assert.Equal("**«two words»** end", Run("«two words» end", new FormatAction.Bold()));
        // Stray spaces stay outside the marks.
        Assert.Equal("a **«b»**   c", Run("a« b »  c", new FormatAction.Bold()));
        Assert.Equal("(nothing)", Run("a«   »b", new FormatAction.Italic()));
    }

    [Fact]
    public void LineMarksTakeTheCaretsLine()
    {
        Assert.Equal("x\n# title‸\ny", Run("x\nti‸tle\ny", new FormatAction.Heading(1)));
        Assert.Equal("x\ntitle‸", Run("x\n# ti‸tle", new FormatAction.Heading(1)));
        Assert.Equal("> a‸", Run("a‸", new FormatAction.Quote()));
        Assert.Equal("«- a\n- b»", Run("«a\nb»", new FormatAction.Bullets()));
        Assert.Equal("«1. a\n2. b»", Run("«a\nb»", new FormatAction.Numbered()));
    }

    [Fact]
    public void WorksInUtf16LikeTheEditor()
    {
        Assert.Equal("📚 **«word»**", Run("📚 wo‸rd", new FormatAction.Bold()));
        Assert.Equal("📚 x\n## 📖‸", Run("📚 x\n📖‸", new FormatAction.Heading(2)));
    }

    [Fact]
    public void TheBarsButtons()
    {
        Assert.Equal(
            new[] { "Bold", "Italic", "Strike", "Code", "Heading1", "Heading2", "Heading3", "Quote", "Bullets", "Numbered", "Link" },
            FormatCommands.All.Select(c => c.Tag));
        Assert.Equal(FormatCommands.All.Count, FormatCommands.All.Select(c => c.Tag).Distinct().Count());
        Assert.Equal("Bold (Ctrl+B)", FormatCommands.Tooltip(FormatCommands.Find("Bold")!));
        Assert.Equal("Italic (Ctrl+I)", FormatCommands.Tooltip(FormatCommands.Find("Italic")!));
        Assert.Equal("Link (Ctrl+K)", FormatCommands.Tooltip(FormatCommands.Find("Link")!));
        Assert.Equal("Strikethrough", FormatCommands.Tooltip(FormatCommands.Find("Strike")!));
        Assert.Equal(new FormatAction.Heading(2), FormatCommands.Find("Heading2")!.Action);
        Assert.Null(FormatCommands.Find("Underline"));
        Assert.Null(FormatCommands.Find(null));
        Assert.Equal("Ctrl+Shift+F", FormatCommands.KeyLabel(ShortcutKeys.Titles.FocusMode));
    }
}

public class FormattingUndoTests
{
    private static readonly DateTime T0 = new(2026, 10, 2, 12, 0, 0, DateTimeKind.Utc);

    [Fact]
    public void AFormattingButtonIsOneStepOfItsOwn()
    {
        var h = new UndoHistory();
        // "plain" typed quickly, then Bold at once, then more typing at once.
        h.Record(new("", 0, 0), "p", T0);
        h.Record(new("p", 1, 1), "l", T0.AddMilliseconds(100));
        h.Record(new("pl", 2, 2), "a", T0.AddMilliseconds(200));
        h.RecordStep(new("pla", 3, 3));
        h.Record(new("**pla**", 5, 5), "x", T0.AddMilliseconds(300));

        Assert.Equal("**pla**", h.Undo(new("**pla**x", 6, 6))!.Text); // the typing after
        Assert.Equal("pla", h.Undo(new("**pla**", 2, 5))!.Text);      // the button, alone
        Assert.Equal("", h.Undo(new("pla", 3, 3))!.Text);             // the typing before
        Assert.Equal("pla", h.Redo(new("", 0, 0))!.Text);
    }

    [Fact]
    public void AFormattingStepClearsRedo()
    {
        var h = new UndoHistory();
        h.Record(new("", 0, 0), "a", T0);
        h.Undo(new("a", 1, 1));
        Assert.True(h.CanRedo);
        h.RecordStep(new("", 0, 0));
        Assert.False(h.CanRedo);
    }
}

public class WriterLayoutTests
{
    [Fact]
    public void RowsAndLinesMatchTheGtkApp()
    {
        // The default layout: 18.67 px text with rows 24 px high on their own.
        var s = WriterSpacing.For(24, wrapGap: 6, lineGap: 12);
        Assert.Equal(30.0, s.RowPitch);        // wrapped rows: the font plus the wrap gap
        Assert.Equal(36.0, s.LinePitch);       // one-row lines: the font plus the line gap
        Assert.Equal(22.5, s.RowPoints, 3);   // RichEdit counts points
        Assert.Equal(4.5, s.SpaceAfterPoints, 3);

        var airy = WriterSpacing.For(24, 12, 20);
        Assert.Equal((36.0, 44.0), (airy.RowPitch, airy.LinePitch));
        var tight = WriterSpacing.For(24, 2, 6);
        Assert.Equal(3.0, tight.SpaceAfterPoints, 3);
    }

    [Fact]
    public void OddNumbersStaySensible()
    {
        Assert.Equal(0.0, WriterSpacing.For(20, 8, 4).SpaceAfterPoints); // never negative
        Assert.Equal(1.0, WriterSpacing.For(double.NaN, 0, 0).RowPitch);
        Assert.Equal(1.0, WriterSpacing.For(-5, -2, 0).RowPitch);
    }

    [Fact]
    public void TheCoresLayoutForEachSetting()
    {
        var settings = new ProfileSettings(
            "u", Theme.System, "#2d71e5", HeadingFont.Serif, WritingFont.IaDuo, 14,
            LineSpacing.Normal, PageWidth.Medium, false, DateFormat.Long, WeekStart.Sunday, Shelf.Reading);
        var normal = BookshelfFfiMethods.WriterLayout(settings);
        Assert.Equal((720, 6, 12), (normal.ColumnWidth, normal.WrapGap, normal.LineGap));
        Assert.Equal(14 * 96.0 / 72.0, normal.FontPx, 6);
        var airy = BookshelfFfiMethods.WriterLayout(settings with { LineSpacing = LineSpacing.Airy, PageWidth = PageWidth.Wide });
        Assert.Equal((900, 12, 20), (airy.ColumnWidth, airy.WrapGap, airy.LineGap));
    }

    [Fact]
    public void HeadingsAreBiggerByLevel()
    {
        Assert.True(HeadingSizes.Scale(1) > HeadingSizes.Scale(2));
        Assert.True(HeadingSizes.Scale(2) > HeadingSizes.Scale(3));
        Assert.True(HeadingSizes.Scale(3) > 1);
        Assert.Equal(1f, HeadingSizes.Scale(4));
        Assert.Equal(1f, HeadingSizes.Scale(0));
    }

    [Fact]
    public void AHeadingSpansLevelIsItsHashes()
    {
        const string text = "# One\n\n## Two\n###### Six\n####### Seven";
        var levels = BookshelfFfiMethods.MarkdownSpans(text)
            .Where(s => s.Kind == StyleKind.Heading)
            .Select(s => HeadingSizes.Level(text, (int)s.Start, (int)s.End))
            .ToList();
        Assert.Equal(new[] { 1, 2, 6 }, levels.Take(3));
        Assert.Equal(3, HeadingSizes.Level("ab ### c", 2, 8)); // a span inside a longer text
        Assert.Equal(1, HeadingSizes.Level("x", 0, 1));        // never below 1
        Assert.Equal(6, HeadingSizes.Level("#######", 0, 7));  // never above 6
    }
}

public class EditTimingsTests
{
    [Fact]
    public void SmallEditsAndBulkAreKeptApart()
    {
        var t = new EditTimings();
        Assert.Equal(0.0, t.Percentile(0.5));
        foreach (var ms in new[] { 1.0, 2, 3, 4, 10 }) t.Add(ms, lines: 1, length: 100);
        t.Add(4.5, lines: 2, length: 60_000);
        t.Add(250, lines: 1000, length: 60_000); // a paste

        Assert.Equal(6, t.Count);
        Assert.Equal(3.0, t.Percentile(0.5));
        Assert.Equal(10.0, t.Percentile(0.9));
        Assert.Equal(10.0, t.Max);
        Assert.Equal(100, t.MaxAtLength);
        Assert.Equal((250.0, 1000), t.Bulk);
        Assert.Equal("edits=6 median=3.00 p90=10.00 max=10.00 maxAtLength=100 bulk=250.0 bulkLines=1000 background=0", t.Summary);
    }
}

public class PendingLinesTests
{
    //                     0 1 2345 678 9
    private const string Text = "a\nbb\nccc\nd";

    [Fact]
    public void HandsOutAFewWholeLinesAtATime()
    {
        var p = new PendingLines();
        Assert.True(p.IsEmpty);
        p.Add(0, Text.Length);
        Assert.Equal((0, 4), p.Take(Text, 2));   // "a", "bb"
        Assert.Equal((5, 10), p.Take(Text, 2));  // "ccc", "d"
        Assert.Null(p.Take(Text, 2));
        Assert.True(p.IsEmpty);

        p.Add(6, 7);                             // part of a line: the whole line
        Assert.Equal((5, 8), p.Take(Text, 5));
        p.Add(3, 3);                             // nothing
        Assert.True(p.IsEmpty);
    }

    [Fact]
    public void FollowsTheTextAsItsEdited()
    {
        var p = new PendingLines();
        p.Add(5, 10);
        p.Shift(new TextChange(0, 0, 3), 13);    // three characters typed before
        Assert.Equal((8, 13), p.Take("xyz" + Text, 9));

        p.Add(5, 10);
        p.Shift(new TextChange(4, 10, 4), 4);    // all of it deleted
        Assert.True(p.IsEmpty);
    }

    [Fact]
    public void LinesAroundAPlace()
    {
        Assert.Equal((2, 10), PendingLines.Around(Text, 6, 1));
        Assert.Equal((0, 8), PendingLines.Around(Text, 0, 2));
        Assert.Equal((0, 10), PendingLines.Around(Text, 9, 99));
        Assert.Equal((0, 2), PendingLines.Around("\nx", 1, 1)); // an empty line before
        Assert.Equal((0, 0), PendingLines.Around("", 0, 3));
        Assert.Equal(4, PendingLines.LineCount(Text, 0, Text.Length));
        Assert.Equal(1, PendingLines.LineCount(Text, 3, 3));
    }

    [Fact]
    public void BackgroundTimeAddsUpUntilANewText()
    {
        var t = new EditTimings();
        t.AddBackground(10, restart: true);
        t.AddBackground(5);
        Assert.Equal(15.0, t.Background);
        t.AddBackground(2, restart: true);
        Assert.Equal(2.0, t.Background);
    }
}

public class WriterTextTests
{
    [Fact]
    public void TheStatusLine()
    {
        var saved = CultureInfo.CurrentCulture;
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("en-US");
        try
        {
            Assert.Equal("1 word · Saved", WriterText.Status(1, saved: true));
            Assert.Equal("0 words · Saving…", WriterText.Status(0, saved: false));
            Assert.Equal("10,240 words · Saved", WriterText.Status(10_240, saved: true));
            Assert.Equal("Not saved: The disk is full.", WriterText.NotSaved("The disk is full."));
            Assert.StartsWith("Focus mode (Ctrl+Shift+F): dim", WriterText.FocusTooltip("Ctrl+Shift+F"));
            Assert.StartsWith("Focus mode: dim", WriterText.FocusTooltip(null));
        }
        finally
        {
            CultureInfo.CurrentCulture = saved;
        }
    }

    [Fact]
    public void TheWritingKeysComeFromTheCoresTable()
    {
        Assert.Equal("Ctrl+B", ShortcutKeys.Label(ShortcutKeys.Find(ShortcutKeys.Titles.Bold)!));
        Assert.Equal("Ctrl+I", ShortcutKeys.Label(ShortcutKeys.Find(ShortcutKeys.Titles.Italic)!));
        Assert.Equal("Ctrl+K", ShortcutKeys.Label(ShortcutKeys.Find(ShortcutKeys.Titles.Link)!));
        var focus = ShortcutKeys.Find(ShortcutKeys.Titles.FocusMode)!;
        Assert.Equal("Ctrl+Shift+F", ShortcutKeys.Label(focus));
        Assert.Equal((0x46, false), ShortcutKeys.VirtualKey(focus.Key)); // VK_F
    }
}

/// <summary>The test hooks the UI tour uses, and that they can't touch a real journal.</summary>
public sealed class WriterTestHookTests : IDisposable
{
    private readonly string _dir =
        Directory.CreateDirectory(Path.Combine(Path.GetTempPath(), "bookshelf-writer-tests-" + Guid.NewGuid().ToString("N"))).FullName;

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
    public async Task AForcedSaveFailureLeavesTheTextAndTheRescueAlone()
    {
        using var journal = await JournalService.OpenAsync(Path.Combine(_dir, "journal"));
        var demo = await DemoJournal.SeedAsync(journal, new DateOnly(2026, 9, 20));
        var reading = await journal.ShelfAsync(demo.Avery.Id, Shelf.Reading);
        var id = reading.Rows.OfType<ShelfRow.Entry>().First().Item.SummaryId;
        var before = (await journal.EntryAsync(id)).Body;

        journal.FailBodySavesForTesting();
        var error = await Assert.ThrowsAsync<CoreException.Io>(() => journal.SaveBodyAsync(id, "lost?"));
        Assert.Contains("--demo-save-fails", error.UserMessage());
        Assert.Equal(before, (await journal.EntryAsync(id)).Body);

        var file = await journal.RescueBodyAsync("A Wizard of Earthsea", "kept\r\nsafe");
        Assert.Equal("kept\nsafe", await File.ReadAllTextAsync(file));
    }

    [Fact]
    public void ADemoFolderIsNewMarkedOrLeftAlone()
    {
        var missing = Path.Combine(_dir, "missing");
        Assert.Equal(DemoJournal.FolderState.New, DemoJournal.Check(missing));
        var empty = Directory.CreateDirectory(Path.Combine(_dir, "empty")).FullName;
        Assert.Equal(DemoJournal.FolderState.New, DemoJournal.Check(empty));

        DemoJournal.Mark(empty);
        File.WriteAllText(Path.Combine(empty, "bookshelf.db"), "");
        Assert.Equal(DemoJournal.FolderState.Demo, DemoJournal.Check(empty));

        // A folder with anything else in it (a real journal, say) is never used.
        var real = Directory.CreateDirectory(Path.Combine(_dir, "real")).FullName;
        File.WriteAllText(Path.Combine(real, "bookshelf.db"), "");
        Assert.Equal(DemoJournal.FolderState.NotDemo, DemoJournal.Check(real));
    }
}
