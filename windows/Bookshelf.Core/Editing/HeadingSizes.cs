namespace Bookshelf.Core.Editing;

/// <summary>
/// How much bigger a heading is on the writing page, by level, as on the
/// Mac: <c>#</c> 1.4 times the text, <c>##</c> 1.22, <c>###</c> 1.1, and
/// deeper ones the text's size (bold, like every heading).
/// </summary>
public static class HeadingSizes
{
    /// <summary>
    /// The level of the heading the core's Heading span
    /// <c>[start, end)</c> of <paramref name="text"/> is: the number of
    /// <c>#</c>s it starts with (after any spaces), 1 to 6.
    /// </summary>
    public static int Level(string text, int start, int end)
    {
        end = Math.Min(end, text.Length);
        var i = Math.Max(start, 0);
        while (i < end && text[i] == ' ') i++;
        var level = 0;
        while (i < end && text[i] == '#' && level < 6)
        {
            level++;
            i++;
        }
        return Math.Max(level, 1);
    }

    /// <summary>How many times the text's size a heading of <paramref name="level"/> is.</summary>
    public static float Scale(int level) => level switch
    {
        1 => 1.4f,
        2 => 1.22f,
        3 => 1.1f,
        _ => 1f,
    };
}
