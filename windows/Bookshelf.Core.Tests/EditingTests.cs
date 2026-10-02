using Bookshelf.Core.Editing;

namespace Bookshelf.Core.Tests;

public class TextDiffTests
{
    [Theory]
    [InlineData("abc", "abXc", 2, 2, 3)]    // insert
    [InlineData("abXc", "abc", 2, 3, 2)]    // delete
    [InlineData("abc", "aYc", 1, 2, 2)]     // replace
    [InlineData("aaa", "aaaa", 3, 3, 4)]    // repeats: the change is found at the end
    [InlineData("", "hi", 0, 0, 2)]
    [InlineData("same", "same", 4, 4, 4)]
    public void FindsTheSmallestChange(string before, string after, int start, int oldEnd, int newEnd)
    {
        Assert.Equal(new TextChange(start, oldEnd, newEnd), TextDiff.Between(before, after));
    }

    [Fact]
    public void GrowsToWholeLines()
    {
        const string text = "one\ntwo **b**\nthree";
        Assert.Equal((4, 13), TextDiff.WholeLines(text, 6, 7));   // inside "two **b**"
        Assert.Equal((0, 3), TextDiff.WholeLines(text, 0, 0));    // start of text
        Assert.Equal((14, 19), TextDiff.WholeLines(text, 19, 19)); // end of text
        Assert.Equal((0, 13), TextDiff.WholeLines(text, 2, 6));   // across a line break
        Assert.Equal((0, 0), TextDiff.WholeLines("", 0, 0));
    }
}

public class UndoHistoryTests
{
    private static readonly DateTime T0 = new(2026, 10, 2, 12, 0, 0, DateTimeKind.Utc);

    [Fact]
    public void TypingIsGroupedUntilAPause()
    {
        var h = new UndoHistory();
        h.Record(new("", 0, 0), "a", T0);
        h.Record(new("a", 1, 1), "b", T0.AddMilliseconds(200));
        h.Record(new("ab", 2, 2), "c", T0.AddMilliseconds(400));
        h.Record(new("abc", 3, 3), "d", T0.AddSeconds(3)); // after a pause

        Assert.Equal("abc", h.Undo(new("abcd", 4, 4))!.Text);
        Assert.Equal("", h.Undo(new("abc", 3, 3))!.Text);
        Assert.Null(h.Undo(new("", 0, 0)));
    }

    [Fact]
    public void NewLinesAndPastesStartNewSteps()
    {
        var h = new UndoHistory();
        h.Record(new("", 0, 0), "a", T0);
        h.Record(new("a", 1, 1), "\n", T0.AddMilliseconds(100));
        h.Record(new("a\n", 2, 2), "pasted words", T0.AddMilliseconds(200));

        Assert.Equal("a\n", h.Undo(new("a\npasted words", 14, 14))!.Text);
        Assert.Equal("a", h.Undo(new("a\n", 2, 2))!.Text);
    }

    [Fact]
    public void RedoReturnsAndNewTypingClearsIt()
    {
        var h = new UndoHistory();
        h.Record(new("", 0, 0), "a", T0);
        var back = h.Undo(new("a", 1, 1))!;
        Assert.Equal("a", h.Redo(back)!.Text);

        h.Undo(new("a", 1, 1));
        h.Record(new("", 0, 0), "z", T0.AddSeconds(5));
        Assert.False(h.CanRedo);
    }

    [Fact]
    public void KeepsAtMostTheLimit()
    {
        var h = new UndoHistory();
        for (var i = 0; i < UndoHistory.Limit + 50; i++)
            h.Record(new($"{i}", 0, 0), "\n", T0.AddSeconds(i));
        var steps = 0;
        var state = new EditorState("end", 0, 0);
        while (h.Undo(state) is { } previous)
        {
            state = previous;
            steps++;
        }
        Assert.Equal(UndoHistory.Limit, steps);
    }
}
