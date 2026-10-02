using Bookshelf.Ffi;

namespace Bookshelf.Core.Editing;

/// <summary>One button on the writing page's formatting bar.</summary>
/// <param name="Tag">The button's tag in XAML, and the end of its automation id ("FormatBold").</param>
/// <param name="Name">What the button is called (its label, and what Narrator says).</param>
/// <param name="Action">What it does, through the core's <c>FormatEdit</c>.</param>
/// <param name="ShortcutTitle">Its row in the core's shortcuts table, if it has a key.</param>
public sealed record FormatCommand(string Tag, string Name, FormatAction Action, string? ShortcutTitle = null);

/// <summary>
/// A formatting button's effect: the new text, the one change that makes
/// it (applied to the editor as one undo step), and the selection after.
/// </summary>
public sealed record FormatResult(string Text, TextChange Change, string Inserted, int SelectionStart, int SelectionEnd);

/// <summary>
/// The formatting bar's buttons, in order, as the GTK app has them: inline
/// marks, then headings, then line marks and the link.
/// </summary>
public static class FormatCommands
{
    public static readonly IReadOnlyList<FormatCommand> All =
    [
        new("Bold", "Bold", new FormatAction.Bold(), ShortcutKeys.Titles.Bold),
        new("Italic", "Italic", new FormatAction.Italic(), ShortcutKeys.Titles.Italic),
        new("Strike", "Strikethrough", new FormatAction.Strike()),
        new("Code", "Code", new FormatAction.Code()),
        new("Heading1", "Heading 1", new FormatAction.Heading(1)),
        new("Heading2", "Heading 2", new FormatAction.Heading(2)),
        new("Heading3", "Heading 3", new FormatAction.Heading(3)),
        new("Quote", "Quote", new FormatAction.Quote()),
        new("Bullets", "Bulleted list", new FormatAction.Bullets()),
        new("Numbered", "Numbered list", new FormatAction.Numbered()),
        new("Link", "Link", new FormatAction.Link(), ShortcutKeys.Titles.Link),
    ];

    /// <summary>
    /// What pressing <paramref name="action"/> does to <paramref name="text"/>
    /// with <c>[selStart, selEnd)</c> selected, or null for nothing. With
    /// nothing selected, inline marks take the word around the caret
    /// (<see cref="WordBounds"/>); the rules themselves are the core's.
    /// </summary>
    public static FormatResult? Apply(string text, int selStart, int selEnd, FormatAction action)
    {
        var a = Math.Clamp(Math.Min(selStart, selEnd), 0, text.Length);
        var b = Math.Clamp(Math.Max(selStart, selEnd), 0, text.Length);
        if (a == b && BookshelfFfiMethods.FormatExpandsToWord(action)) (a, b) = WordBounds.Around(text, a);
        if (CoreValues.FormatEdit(text, a, b, action) is not { } edit) return null;
        var start = (int)Math.Min(edit.Start, (uint)text.Length);
        var end = (int)Math.Clamp(edit.End, (uint)start, (uint)text.Length);
        var (change, inserted) = TextDiff.Narrow(text, start, end, edit.Replacement);
        var after = string.Concat(text.AsSpan(0, change.Start), inserted, text.AsSpan(change.OldEnd));
        var selection = ((int)Math.Min(edit.NewSelStart, (uint)after.Length), (int)Math.Min(edit.NewSelEnd, (uint)after.Length));
        return new FormatResult(after, change, inserted, selection.Item1, selection.Item2);
    }

    /// <summary>The button with this tag, or null.</summary>
    public static FormatCommand? Find(string? tag) => All.FirstOrDefault(c => c.Tag == tag);

    /// <summary>The tooltip: the name, and the key if it has one ("Bold (Ctrl+B)").</summary>
    public static string Tooltip(FormatCommand command) =>
        KeyLabel(command.ShortcutTitle) is { } key ? $"{command.Name} ({key})" : command.Name;

    /// <summary>The key for a row of the core's shortcuts table ("Ctrl+B"), or null.</summary>
    public static string? KeyLabel(string? shortcutTitle) =>
        shortcutTitle is not null && ShortcutKeys.Find(shortcutTitle) is { } combo ? ShortcutKeys.Label(combo) : null;
}
