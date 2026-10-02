using System.Diagnostics;
using Bookshelf.Core.Editing;
using Bookshelf.Ffi;
using Microsoft.UI.Input;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;
using Windows.UI.Core;

namespace Bookshelf;

/// <summary>
/// Turns a RichEditBox into the writing page's editor:
/// <list type="bullet">
/// <item>Markdown highlighting from the shared Rust core, redone only for the
/// lines an edit touched, so long texts stay fast.</item>
/// <item>Bookshelf's own text-only undo (Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z): the
/// box's built-in undo also records the highlighting, so it's switched off.</item>
/// <item>Paste as plain text, so pasted words take the editor's look.</item>
/// </list>
/// RichEditBox separates lines with '\r'; this class works with '\n' (one
/// character for one, so offsets match) because that's what the core expects.
/// </summary>
public sealed class WritingBox
{
    private readonly RichEditBox _box;
    private readonly UndoHistory _history = new();
    private readonly Windows.UI.Color _ink;
    private string _text = "";
    private (int Start, int End) _selection;
    private bool _applying; // true while we change the box ourselves

    public bool Highlight { get; set; } = true;

    /// <summary>The text size in pixels; headings are a quarter bigger. Call <see cref="RestyleAll"/> after changing it.</summary>
    public float TextSize { get; set; } = 15;

    /// <summary>The text, with <c>\n</c> line ends.</summary>
    public string Text => _text;

    /// <summary>Raised after each restyle, with a short description of the work done.</summary>
    public event Action<string>? Restyled;

    /// <summary>Raised when the person changed the text (typing, pasting, undo or redo); not by <see cref="SetText"/>.</summary>
    public event Action? Edited;

    public WritingBox(RichEditBox box)
    {
        _box = box;
        _box.Document.UndoLimit = 0; // our own undo instead (see UndoHistory)
        _ink = _box.Document.GetDefaultCharacterFormat().ForegroundColor;
        _box.TextChanged += (_, _) => OnTextChanged();
        _box.SelectionChanged += (_, _) =>
        {
            if (!_applying) _selection = CurrentSelection();
        };
        _box.PreviewKeyDown += OnPreviewKeyDown;
        _box.Paste += OnPaste;
    }

    /// <summary>Replaces everything (e.g. when a summary is opened); clears undo.</summary>
    public void SetText(string text)
    {
        _applying = true;
        _box.Document.SetText(TextSetOptions.None, text.Replace("\r\n", "\r").Replace('\n', '\r'));
        _applying = false;
        _text = Read();
        _history.Clear();
        _selection = CurrentSelection();
        RestyleAll();
    }

    public void RestyleAll() => Restyle(0, _text.Length);

    private string Read()
    {
        _box.Document.GetText(TextGetOptions.None, out var raw);
        return raw.Replace('\r', '\n');
    }

    private (int, int) CurrentSelection()
    {
        var s = _box.Document.Selection;
        return (s.StartPosition, s.EndPosition);
    }

    private void OnTextChanged()
    {
        if (_applying) return;
        var now = Read();
        if (now == _text) return; // only formatting changed (our highlighting)

        var change = TextDiff.Between(_text, now);
        var inserted = now.Substring(change.Start, change.NewEnd - change.Start);
        _history.Record(new EditorState(_text, _selection.Start, _selection.End), inserted, DateTime.UtcNow);
        _text = now;
        Restyle(change.Start, change.NewEnd);
        _selection = CurrentSelection();
        Edited?.Invoke();
    }

    private void OnPreviewKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (!IsDown(VirtualKey.Control)) return;
        var shift = IsDown(VirtualKey.Shift);
        if (e.Key == VirtualKey.Z && !shift)
        {
            e.Handled = true;
            Apply(_history.Undo(Snapshot()));
        }
        else if (e.Key == VirtualKey.Y || (e.Key == VirtualKey.Z && shift))
        {
            e.Handled = true;
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
        var change = TextDiff.Between(_text, target.Text);
        _applying = true;
        try
        {
            var replacement = target.Text.Substring(change.Start, change.NewEnd - change.Start).Replace('\n', '\r');
            _box.Document.GetRange(change.Start, change.OldEnd).SetText(TextSetOptions.None, replacement);
        }
        finally
        {
            _applying = false;
        }
        _text = Read();
        if (_text != target.Text)
        {
            // Shouldn't happen; if RichEdit adjusted something, fall back to replacing everything.
            _applying = true;
            _box.Document.SetText(TextSetOptions.None, target.Text.Replace('\n', '\r'));
            _applying = false;
            _text = Read();
            change = new TextChange(0, 0, _text.Length);
        }
        Restyle(change.Start, change.NewEnd);
        _box.Document.Selection.SetRange(target.SelectionStart, target.SelectionEnd);
        _selection = (target.SelectionStart, target.SelectionEnd);
        Edited?.Invoke();
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

    /// <summary>Restyles the whole lines around <c>[start, end)</c>.</summary>
    private void Restyle(int start, int end)
    {
        var (from, to) = TextDiff.WholeLines(_text, start, end);
        var clock = Stopwatch.StartNew();
        var doc = _box.Document;
        _applying = true;
        doc.BatchDisplayUpdates();
        try
        {
            var all = doc.GetRange(from, to).CharacterFormat;
            all.Bold = FormatEffect.Off;
            all.Italic = FormatEffect.Off;
            all.Strikethrough = FormatEffect.Off;
            all.Size = TextSize;
            all.ForegroundColor = _ink;
            all.BackgroundColor = Microsoft.UI.Colors.Transparent;

            if (Highlight && to > from)
            {
                var dim = Windows.UI.Color.FromArgb(110, _ink.R, _ink.G, _ink.B);
                var codeBack = Windows.UI.Color.FromArgb(28, _ink.R, _ink.G, _ink.B);
                foreach (var span in BookshelfFfiMethods.MarkdownSpans(_text.Substring(from, to - from)))
                {
                    var f = doc.GetRange(from + (int)span.Start, from + (int)span.End).CharacterFormat;
                    switch (span.Kind)
                    {
                        case StyleKind.Heading: f.Bold = FormatEffect.On; f.Size = TextSize * 1.25f; break;
                        case StyleKind.Bold: f.Bold = FormatEffect.On; break;
                        case StyleKind.Italic:
                        case StyleKind.Quote: f.Italic = FormatEffect.On; break;
                        case StyleKind.Code: f.BackgroundColor = codeBack; break;
                        case StyleKind.Strike: f.Strikethrough = FormatEffect.On; break;
                        case StyleKind.Syntax: f.ForegroundColor = dim; break;
                    }
                }
            }
        }
        finally
        {
            doc.ApplyDisplayUpdates();
            _applying = false;
        }
        var lines = to - from == 0 ? 1 : _text.AsSpan(from, to - from).Count('\n') + 1;
        Restyled?.Invoke($"{_text.Length:N0} characters · restyled {lines:N0} line{(lines == 1 ? "" : "s")} in {clock.Elapsed.TotalMilliseconds:F1} ms");
    }
}
