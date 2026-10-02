namespace Bookshelf.Core.Editing;

/// <summary>The text and selection at one moment.</summary>
public sealed record EditorState(string Text, int SelectionStart, int SelectionEnd);

/// <summary>
/// Undo and redo for the writing page, for text only. The editor's own undo
/// is switched off because it also records the highlighting, which would
/// make Ctrl+Z undo colors instead of typing.
///
/// Typing is grouped into steps the way people expect: a new step starts
/// after a pause, at a new line, or with a paste.
/// </summary>
public sealed class UndoHistory
{
    private readonly LinkedList<EditorState> _undo = new();
    private readonly Stack<EditorState> _redo = new();
    private DateTime _lastEdit = DateTime.MinValue;

    public static readonly TimeSpan Pause = TimeSpan.FromSeconds(1);
    public const int Limit = 200;

    public bool CanUndo => _undo.Count > 0;
    public bool CanRedo => _redo.Count > 0;

    /// <summary>
    /// Call after every edit the person made, with the state just before it
    /// and the text it inserted (empty for a deletion).
    /// </summary>
    public void Record(EditorState before, string inserted, DateTime now)
    {
        var newStep = _undo.Count == 0
            || now - _lastEdit >= Pause
            || inserted.Contains('\n')
            || inserted.Length > 1; // a paste, or text replaced in one go
        if (newStep)
        {
            _undo.AddLast(before);
            if (_undo.Count > Limit) _undo.RemoveFirst();
        }
        _redo.Clear();
        _lastEdit = now;
    }

    /// <summary>The state to go back to, or null if there's nothing to undo.</summary>
    public EditorState? Undo(EditorState current)
    {
        if (_undo.Last is not { } last) return null;
        _undo.RemoveLast();
        _redo.Push(current);
        _lastEdit = DateTime.MinValue; // typing after an undo starts a new step
        return last.Value;
    }

    /// <summary>The state to go forward to, or null if there's nothing to redo.</summary>
    public EditorState? Redo(EditorState current)
    {
        if (_redo.Count == 0) return null;
        _undo.AddLast(current);
        _lastEdit = DateTime.MinValue;
        return _redo.Pop();
    }

    public void Clear()
    {
        _undo.Clear();
        _redo.Clear();
        _lastEdit = DateTime.MinValue;
    }
}
