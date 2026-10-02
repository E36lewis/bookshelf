using System.Diagnostics;
using Bookshelf.Core.Editing;
using Bookshelf.Ffi;
using Bookshelf.Writing;
using DispatcherQueuePriority = Microsoft.UI.Dispatching.DispatcherQueuePriority;
using Microsoft.UI.Input;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;
using Windows.UI.Core;
using Windows.UI.ViewManagement;

namespace Bookshelf;

/// <summary>
/// Turns a RichEditBox into the writing page's editor:
/// <list type="bullet">
/// <item>Markdown highlighting from the shared Rust core, redone only for the
/// lines an edit touched, so long texts stay fast; recolored when the theme
/// changes, and plain in High Contrast.</item>
/// <item>The formatting bar's edits (<see cref="Format"/>), each one step
/// of undo.</item>
/// <item>Focus mode: everything but the current sentence dimmed, and the
/// caret's line kept mid-page (<see cref="Typewriter"/>).</item>
/// <item>Bookshelf's own text-only undo (Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z): the
/// box's built-in undo also records the highlighting, so it's switched off.
/// Highlighting and dimming never touch it.</item>
/// <item>Paste as plain text, so pasted words take the editor's look.</item>
/// </list>
/// RichEditBox separates lines with '\r'; this class works with '\n' (one
/// character for one, so offsets match) because that's what the core expects.
/// </summary>
public sealed class WritingBox
{
    private readonly RichEditBox _box;
    private readonly UndoHistory _history = new();
    private readonly Typewriter _typewriter;
    private readonly UISettings _system = new(); // kept: its event lives as long as it does
    private WritingColors _colors;
    private WriterSpacing? _spacing;
    private string _text = "";
    private (int Start, int End) _selection;
    private (int Start, int End) _bright; // focus mode: the sentence shown at full strength
    private readonly PendingLines _pending = new(); // lines of a long text still to highlight
    private bool _applying; // true while we change the box ourselves
    private bool _caretUpdateQueued;
    private bool _backgroundQueued;
    private bool _endsWithOwnBreak; // RichEdit's GetText gives its final paragraph mark too

    /// <summary>A long text's lines around the caret are highlighted at once, this many each way; the rest when idle.</summary>
    private const int LinesNow = 60;

    /// <summary>Lines highlighted per idle moment, and the time an idle moment may take.</summary>
    private const int LinesPerChunk = 8;
    private const double ChunkMs = 12;

    public bool Highlight { get; set; } = true;

    /// <summary>The text size in points; headings are a quarter bigger. Call <see cref="RestyleAll"/> after changing it.</summary>
    public float TextSize { get; set; } = 14;

    /// <summary>The text, with <c>\n</c> line ends.</summary>
    public string Text => _text;

    /// <summary>Whether focus mode is on (see <see cref="SetFocusMode"/>).</summary>
    public bool IsFocusMode { get; private set; }

    /// <summary>How long each edit took to handle; see <see cref="Timed"/>.</summary>
    public EditTimings Timings { get; } = new();

    /// <summary>Raised after each edit or restyle is timed (for the demo journal's speed check).</summary>
    public event Action? Timed;

    /// <summary>Notes where the time goes in big restyles, in startup.log (demo journals: nobody's text is logged).</summary>
    public bool LogTimings { get; set; }

    /// <summary>Raised when the person changed the text (typing, pasting, formatting, undo or redo); not by <see cref="SetText"/>.</summary>
    public event Action? Edited;

    public WritingBox(RichEditBox box)
    {
        _box = box;
        _box.Document.UndoLimit = 0; // our own undo instead (see UndoHistory)
        _colors = WritingColors.For(box);
        _typewriter = new Typewriter(box);
        _box.TextChanged += (_, _) => OnTextChanged();
        _box.SelectionChanged += (_, _) =>
        {
            if (_applying) return;
            _selection = CurrentSelection();
            QueueCaretUpdate();
        };
        _box.PreviewKeyDown += OnPreviewKeyDown;
        _box.Paste += OnPaste;
        // Light, dark and High Contrast each have their own colors. (A
        // desktop app can't watch AccessibilitySettings; Windows' colors
        // changing covers High Contrast going on or off.)
        _box.ActualThemeChanged += (_, _) => Recolor();
        _system.ColorValuesChanged += OnSystemColorsChanged;
        _box.Unloaded += (_, _) => _system.ColorValuesChanged -= OnSystemColorsChanged;
    }

    /// <summary>Replaces everything (e.g. when a summary is opened); clears undo.</summary>
    public void SetText(string text)
    {
        var plain = text.Replace("\r\n", "\r").Replace('\n', '\r');
        _applying = true;
        _box.Document.SetText(TextSetOptions.None, plain);
        _applying = false;
        // A RichEdit text always ends with a paragraph mark of its own, and
        // GetText may include it. Learn which, from text we know, so an
        // empty page reads as empty and a save never gains a line break.
        _box.Document.GetText(TextGetOptions.None, out var raw);
        _endsWithOwnBreak = raw.Length == plain.Length + 1 && raw[^1] == '\r' && raw.StartsWith(plain, StringComparison.Ordinal);
        _text = Read();
        _history.Clear();
        _pending.Clear();
        _selection = CurrentSelection();
        ApplySpacing();
        RestyleAll();
    }

    /// <summary>
    /// Takes in any typing the box has but hasn't reported yet (RichEditBox
    /// raises TextChanged a moment later), so a save right now has every
    /// key. Saving calls this first.
    /// </summary>
    public void CatchUp() => OnTextChanged();

    /// <summary>Room above the first line and below the last, inside the scrolling area (see <see cref="Typewriter.SetRoom"/>).</summary>
    public void SetRoom(double above, double below) => _typewriter.SetRoom(above, below);

    /// <summary>The page's line spacing (see <see cref="WriterSpacing"/>), for the text there is and all that's typed later.</summary>
    public void SetSpacing(WriterSpacing spacing)
    {
        _spacing = spacing;
        ApplySpacing();
    }

    public void RestyleAll()
    {
        var clock = Stopwatch.StartNew();
        _pending.Clear();
        var lines = RestyleSoon(0, _text.Length);
        Note(clock, lines);
    }

    /// <summary>
    /// Runs a formatting button (or Ctrl+B, I, K) on the selection, or on the
    /// word around the caret. One replacement, one step of undo. False if it
    /// did nothing (Italic on a selection of only spaces, say).
    /// </summary>
    public bool Format(FormatAction action)
    {
        CatchUp(); // work on the text as it is, the last key included
        var clock = Stopwatch.StartNew();
        var (start, end) = CurrentSelection();
        if (FormatCommands.Apply(_text, start, end, action) is not { } result) return false;
        var before = new EditorState(_text, start, end);
        var oldBright = _bright;
        var change = Replace(result.Change, result.Inserted, result.Text);
        _history.RecordStep(before);
        Select(result.SelectionStart, result.SelectionEnd);
        var lines = RestyleEdit(change, oldBright);
        Note(clock, lines);
        Edited?.Invoke();
        QueueCaretUpdate();
        return true;
    }

    /// <summary>
    /// Focus mode on or off. On: everything but the sentence being written is
    /// dimmed, and typing keeps its line mid-page. Only colors change, never
    /// the text, so undo doesn't see it.
    /// </summary>
    public void SetFocusMode(bool on)
    {
        if (on == IsFocusMode) return;
        IsFocusMode = on;
        if (!on)
        {
            RestyleAll();
            return;
        }
        CatchUp();
        _bright = SentenceBounds.Around(_text, CurrentSelection().Start);
        var doc = _box.Document;
        _applying = true;
        doc.BatchDisplayUpdates();
        try
        {
            doc.GetRange(0, _text.Length).CharacterFormat.ForegroundColor = _colors.Dim;
        }
        finally
        {
            doc.ApplyDisplayUpdates();
            _applying = false;
        }
        Restyle(_bright.Start, _bright.End);
        QueueCaretUpdate();
    }

    private string Read()
    {
        _box.Document.GetText(TextGetOptions.None, out var raw);
        if (_endsWithOwnBreak && raw.Length > 0 && raw[^1] == '\r') raw = raw[..^1];
        return raw.Replace('\r', '\n');
    }

    private (int Start, int End) CurrentSelection()
    {
        var s = _box.Document.Selection;
        return (s.StartPosition, s.EndPosition);
    }

    private void Select(int start, int end)
    {
        _box.Document.Selection.SetRange(start, end);
        _selection = (start, end);
    }

    private void OnTextChanged()
    {
        if (_applying) return;
        var clock = Stopwatch.StartNew();
        var now = Read();
        if (now == _text) return; // only formatting changed (our highlighting)

        var change = TextDiff.Between(_text, now);
        var inserted = now.Substring(change.Start, change.NewEnd - change.Start);
        _history.Record(new EditorState(_text, _selection.Start, _selection.End), inserted, DateTime.UtcNow);
        _text = now;
        _pending.Shift(change, _text.Length);
        var oldBright = _bright;
        _selection = CurrentSelection();
        var lines = RestyleEdit(change, oldBright);
        Note(clock, lines);
        Edited?.Invoke();
        QueueCaretUpdate();
    }

    private void OnPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (!IsDown(VirtualKey.Control) || IsDown(VirtualKey.Menu)) return;
        var shift = IsDown(VirtualKey.Shift);
        if (e.Key == VirtualKey.Z && !shift)
        {
            e.Handled = true;
            CatchUp(); // so undo sees the last key typed
            Apply(_history.Undo(Snapshot()));
        }
        else if (e.Key == VirtualKey.Y || (e.Key == VirtualKey.Z && shift))
        {
            e.Handled = true;
            CatchUp();
            Apply(_history.Redo(Snapshot()));
        }
    }

    private static bool IsDown(VirtualKey key) =>
        InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(CoreVirtualKeyStates.Down);

    private EditorState Snapshot() => new(_text, _selection.Start, _selection.End);

    /// <summary>
    /// Goes to an undo/redo state by replacing only the part that differs, so
    /// it's fast on long texts and the view doesn't jump.
    /// </summary>
    private void Apply(EditorState? target)
    {
        if (target is null) return;
        var clock = Stopwatch.StartNew();
        var diff = TextDiff.Between(_text, target.Text);
        var oldBright = _bright;
        var change = Replace(diff, target.Text.Substring(diff.Start, diff.NewEnd - diff.Start), target.Text);
        Select(target.SelectionStart, target.SelectionEnd);
        var lines = RestyleEdit(change, oldBright);
        Note(clock, lines);
        Edited?.Invoke();
        QueueCaretUpdate();
    }

    /// <summary>
    /// Makes the box hold <paramref name="expected"/> by putting
    /// <paramref name="inserted"/> in place of <c>[change.Start, change.OldEnd)</c>.
    /// Returns the change made: all of it, if RichEdit adjusted something and
    /// everything had to be replaced (it shouldn't).
    /// </summary>
    private TextChange Replace(TextChange change, string inserted, string expected)
    {
        var oldLength = _text.Length;
        _applying = true;
        try
        {
            _box.Document.GetRange(change.Start, change.OldEnd).SetText(TextSetOptions.None, inserted.Replace('\n', '\r'));
        }
        finally
        {
            _applying = false;
        }
        _text = Read();
        if (_text == expected)
        {
            _pending.Shift(change, _text.Length);
            return change;
        }

        _applying = true;
        try
        {
            _box.Document.SetText(TextSetOptions.None, expected.Replace('\n', '\r'));
        }
        finally
        {
            _applying = false;
        }
        _text = Read();
        _pending.Clear();
        ApplySpacing();
        return new TextChange(0, oldLength, _text.Length);
    }

    private async void OnPaste(object sender, TextControlPasteEventArgs e)
    {
        e.Handled = true; // never bring in another app's fonts and colors
        var content = Clipboard.GetContent();
        if (!content.Contains(StandardDataFormats.Text)) return;
        var plain = await content.GetTextAsync();
        var selection = _box.Document.Selection;
        selection.SetText(TextSetOptions.None, plain.Replace("\r\n", "\r").Replace('\n', '\r'));
        selection.Collapse(false); // caret after the pasted text
    }

    // ---- focus mode -------------------------------------------------------------

    /// <summary>
    /// After the caret moves (and any typing has landed): in focus mode,
    /// brightens the sentence it's now in and keeps its line mid-page. Done
    /// once per burst of events, when the dispatcher is idle.
    /// </summary>
    private void QueueCaretUpdate()
    {
        if (!IsFocusMode || _caretUpdateQueued) return;
        _caretUpdateQueued = true;
        _box.DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
        {
            _caretUpdateQueued = false;
            if (!IsFocusMode) return;
            CatchUp();
            var next = SentenceBounds.Around(_text, CurrentSelection().Start);
            if (next != _bright)
            {
                var old = _bright;
                _bright = next;
                RestyleBoth(old, next);
            }
            _typewriter.CenterCaret();
        });
    }

    /// <summary>
    /// Restyles after an edit: the lines it touched, and in focus mode the
    /// lines of the sentence that was bright before (moved along by the edit)
    /// and of the one that's bright now. Returns how many lines it restyled.
    /// </summary>
    private int RestyleEdit(TextChange change, (int Start, int End) oldBright)
    {
        if (!IsFocusMode) return RestyleSoon(change.Start, change.NewEnd);
        _bright = SentenceBounds.Around(_text, _selection.Start);
        var edited = (Math.Min(change.Start, _bright.Start), Math.Max(change.NewEnd, _bright.End));
        return RestyleBoth(TextDiff.Moved(oldBright, change), edited);
    }

    /// <summary>Restyles the lines of two ranges, once if they share lines.</summary>
    private int RestyleBoth((int Start, int End) a, (int Start, int End) b)
    {
        var la = TextDiff.WholeLines(_text, a.Start, a.End);
        var lb = TextDiff.WholeLines(_text, b.Start, b.End);
        if (la.End < lb.Start || lb.End < la.Start) return RestyleSoon(la.Start, la.End) + RestyleSoon(lb.Start, lb.End);
        return RestyleSoon(Math.Min(la.Start, lb.Start), Math.Max(la.End, lb.End));
    }

    /// <summary>
    /// Restyles the lines of <c>[start, end)</c>: all of them if there are
    /// few, else those around the caret now and the others while the editor
    /// is idle, a few at a time. Returns how many lines it restyled now.
    /// </summary>
    private int RestyleSoon(int start, int end)
    {
        var (from, to) = TextDiff.WholeLines(_text, start, end);
        if (PendingLines.LineCount(_text, from, to) <= 3 * LinesNow) return Restyle(from, to);
        var caret = Math.Clamp(_selection.Start, from, to);
        var (nearFrom, nearTo) = PendingLines.Around(_text, caret, LinesNow);
        nearFrom = Math.Max(nearFrom, from);
        nearTo = Math.Min(nearTo, to);
        var lines = Restyle(nearFrom, nearTo);
        _pending.Add(from, nearFrom);
        _pending.Add(nearTo + 1, to);
        Timings.AddBackground(0, restart: true);
        QueueBackground();
        return lines;
    }

    private void QueueBackground()
    {
        if (_backgroundQueued || _pending.IsEmpty) return;
        _backgroundQueued = true;
        _box.DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, RestyleWhileIdle);
    }

    /// <summary>A few more lines of a long text, when nothing else is waiting.</summary>
    private void RestyleWhileIdle()
    {
        _backgroundQueued = false;
        CatchUp(); // the pending lines follow the text, the last key included
        var clock = Stopwatch.StartNew();
        while (clock.Elapsed.TotalMilliseconds < ChunkMs && _pending.Take(_text, LinesPerChunk) is { } lines)
        {
            Restyle(lines.Start, lines.End);
        }
        Timings.AddBackground(clock.Elapsed.TotalMilliseconds);
        if (_pending.IsEmpty)
        {
            if (LogTimings) StartupLog.Step($"Writer: highlighting the rest took {Timings.Background:F0} ms while idle");
            Timed?.Invoke();
        }
        QueueBackground();
    }

    // ---- styling ----------------------------------------------------------------

    /// <summary>Raised off the UI thread.</summary>
    private void OnSystemColorsChanged(UISettings sender, object args) =>
        _box.DispatcherQueue.TryEnqueue(Recolor);

    /// <summary>Picks the colors for the theme the editor has now, and restyles everything in them.</summary>
    private void Recolor()
    {
        _colors = WritingColors.For(_box);
        RestyleAll(); // focus mode's dimming included
    }

    /// <summary>The page's spacing on every line there is, and as the default for new ones.</summary>
    private void ApplySpacing()
    {
        if (_spacing is not { } spacing) return;
        var doc = _box.Document;
        var lineSpacing = (float)spacing.RowPoints;
        var after = (float)spacing.SpaceAfterPoints;
        var defaults = doc.GetDefaultParagraphFormat();
        defaults.SetLineSpacing(LineSpacingRule.AtLeast, lineSpacing);
        defaults.SpaceBefore = 0;
        defaults.SpaceAfter = after;
        doc.SetDefaultParagraphFormat(defaults);
        _applying = true;
        try
        {
            var all = doc.GetRange(0, _text.Length + 1).ParagraphFormat;
            all.SetLineSpacing(LineSpacingRule.AtLeast, lineSpacing);
            all.SpaceBefore = 0;
            all.SpaceAfter = after;
        }
        finally
        {
            _applying = false;
        }
    }

    /// <summary>Restyles the whole lines around <c>[start, end)</c>. Returns how many lines.</summary>
    private int Restyle(int start, int end)
    {
        var (from, to) = TextDiff.WholeLines(_text, start, end);
        var doc = _box.Document;
        var c = _colors;
        var clock = LogTimings ? Stopwatch.StartNew() : null;
        double spansAt = 0, resetAt = 0, marksAt = 0, spanCount = 0;
        _applying = true;
        doc.BatchDisplayUpdates();
        try
        {
            // The line break too, so a blank line left by a heading isn't heading-sized.
            var all = doc.GetRange(from, to + 1).CharacterFormat;
            all.Bold = FormatEffect.Off;
            all.Italic = FormatEffect.Off;
            all.Strikethrough = FormatEffect.Off;
            all.Size = TextSize;
            all.ForegroundColor = c.Ink;
            all.BackgroundColor = Microsoft.UI.Colors.Transparent;
            resetAt = clock?.Elapsed.TotalMilliseconds ?? 0;

            if (Highlight && to > from)
            {
                var spans = BookshelfFfiMethods.MarkdownSpans(_text.Substring(from, to - from));
                spansAt = clock?.Elapsed.TotalMilliseconds ?? 0;
                spanCount = spans.Length;
                // Marks last, so they stay dimmed inside a quote or a heading.
                foreach (var span in spans.Where(s => s.Kind != StyleKind.Syntax).Concat(spans.Where(s => s.Kind == StyleKind.Syntax)))
                {
                    var f = doc.GetRange(from + (int)span.Start, from + (int)span.End).CharacterFormat;
                    switch (span.Kind)
                    {
                        case StyleKind.Heading: f.Bold = FormatEffect.On; f.Size = TextSize * 1.25f; break;
                        case StyleKind.Bold: f.Bold = FormatEffect.On; break;
                        case StyleKind.Italic: f.Italic = FormatEffect.On; break;
                        case StyleKind.Quote: f.Italic = FormatEffect.On; f.ForegroundColor = c.Quote; break;
                        case StyleKind.Code: f.BackgroundColor = c.CodeBackground; break;
                        case StyleKind.Strike: f.Strikethrough = FormatEffect.On; break;
                        case StyleKind.Syntax: f.ForegroundColor = c.Syntax; break;
                    }
                }
            }

            marksAt = clock?.Elapsed.TotalMilliseconds ?? 0;
            if (IsFocusMode)
            {
                // Everything on these lines but the bright sentence.
                var brightStart = Math.Clamp(_bright.Start, from, to);
                var brightEnd = Math.Clamp(_bright.End, brightStart, to);
                if (from < brightStart) doc.GetRange(from, brightStart).CharacterFormat.ForegroundColor = c.Dim;
                if (brightEnd < to) doc.GetRange(brightEnd, to).CharacterFormat.ForegroundColor = c.Dim;
            }
        }
        finally
        {
            doc.ApplyDisplayUpdates();
            _applying = false;
        }
        var lineCount = PendingLines.LineCount(_text, from, to);
        if (clock is not null && lineCount > LinesPerChunk)
        {
            StartupLog.Step(
                $"Writer: restyled {lineCount} lines ({spanCount} spans) in {clock.Elapsed.TotalMilliseconds:F0} ms: " +
                $"reset {resetAt:F1}, spans {spansAt - resetAt:F1}, marks {marksAt - spansAt:F1}, display {clock.Elapsed.TotalMilliseconds - marksAt:F1}");
        }
        return lineCount;
    }

    private void Note(Stopwatch clock, int lines)
    {
        Timings.Add(clock.Elapsed.TotalMilliseconds, lines, _text.Length);
        Timed?.Invoke();
    }
}
