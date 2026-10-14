using Bookshelf.Ffi;

namespace Bookshelf.Core.Editing;

/// <summary>
/// The list keys on the writing page, with the core's rules: Enter starts
/// the next item (or ends the list on an empty one), Tab nests the items
/// it covers and Shift+Tab un-nests them.
/// </summary>
public static class ListKeys
{
    /// <summary>
    /// What pressing <paramref name="key"/> does to <paramref name="text"/>
    /// with <c>[selStart, selEnd)</c> selected, or null when it isn't the
    /// list's key here (so it does what it always does). A result that
    /// changes nothing (<see cref="ChangesNothing"/>) means the key is the
    /// list's, with nothing to do.
    /// </summary>
    public static FormatResult? Apply(string text, int selStart, int selEnd, ListKey key) =>
        CoreValues.ListKeyEdit(text, selStart, selEnd, key) is { } edit ? FormatCommands.Result(text, edit) : null;

    /// <summary>Whether a result leaves the text as it was.</summary>
    public static bool ChangesNothing(FormatResult result) =>
        result.Change.Start == result.Change.OldEnd && result.Inserted.Length == 0;
}
