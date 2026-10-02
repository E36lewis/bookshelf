using System.Globalization;
using System.Text;

namespace Bookshelf.Core.Editing;

/// <summary>
/// The word around the caret, for the formatting buttons that act on a word
/// when nothing is selected (Bold with the caret in "wo|rd" makes
/// "**word**"). The same idea of a word as the GTK app's: letters, digits
/// and combining marks, with an apostrophe between letters ("don't") kept
/// inside. Offsets are UTF-16 code units, like C# strings and RichEditBox.
/// </summary>
/// <remarks>
/// RichEdit's own <c>Expand(Word)</c> takes the spaces after a word too, and
/// stops at apostrophes, which is why the app uses this instead.
/// </remarks>
public static class WordBounds
{
    /// <summary>
    /// The word the caret is in or just after, or an empty range at the
    /// caret when it's next to no word (between spaces, say).
    /// </summary>
    public static (int Start, int End) Around(string text, int caret)
    {
        caret = Math.Clamp(caret, 0, text.Length);
        if (caret > 0 && caret < text.Length && char.IsLowSurrogate(text[caret]) && char.IsHighSurrogate(text[caret - 1]))
        {
            caret--; // never split a character in two
        }
        var start = caret;
        while (start > 0 && IsWordAt(text, Previous(text, start))) start = Previous(text, start);
        var end = caret;
        while (end < text.Length && IsWordAt(text, end)) end = Next(text, end);
        return (start, end);
    }

    /// <summary>Whether the character starting at <paramref name="i"/> is part of a word.</summary>
    private static bool IsWordAt(string text, int i)
    {
        if (i < 0 || i >= text.Length) return false;
        if (IsWordCharacter(text, i)) return true;
        // "don't", "l’été": an apostrophe with a letter on each side.
        return text[i] is '\'' or '’'
            && i > 0 && IsWordCharacter(text, Previous(text, i))
            && i + 1 < text.Length && IsWordCharacter(text, i + 1);
    }

    private static bool IsWordCharacter(string text, int i)
    {
        if (!Rune.TryGetRuneAt(text, i, out var rune)) return false; // half a character
        return Rune.IsLetterOrDigit(rune) || Rune.GetUnicodeCategory(rune) is
            UnicodeCategory.NonSpacingMark or UnicodeCategory.SpacingCombiningMark or
            UnicodeCategory.EnclosingMark or UnicodeCategory.ConnectorPunctuation;
    }

    /// <summary>Where the character before <paramref name="i"/> starts.</summary>
    private static int Previous(string text, int i) =>
        i >= 2 && char.IsLowSurrogate(text[i - 1]) && char.IsHighSurrogate(text[i - 2]) ? i - 2 : i - 1;

    /// <summary>Where the character after the one at <paramref name="i"/> starts.</summary>
    private static int Next(string text, int i) =>
        i + 1 < text.Length && char.IsHighSurrogate(text[i]) && char.IsLowSurrogate(text[i + 1]) ? i + 2 : i + 1;
}
