using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;
using Windows.UI.Core;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;

namespace Bookshelf.Pages;

/// <summary>A page that may need to save before the window closes.</summary>
internal interface IGuardsClose
{
    /// <summary>Saves; false means the person chose to keep the window open.</summary>
    Task<bool> CanCloseAsync();
}

/// <summary>
/// The writing page, in its first form: the summary in the writing font,
/// highlighted as Markdown by <see cref="WritingBox"/>, saved after a short
/// pause in typing, on Ctrl+S, on leaving and when the window closes. If a
/// save fails, the text is rescued to a file and the person is told where.
/// </summary>
public sealed partial class WriterPage : BookshelfPage, IGuardsClose
{
    private readonly DispatcherQueueTimer _autosave;
    private string _id = "";
    private string _title = "";
    private WritingBox? _writing;
    private bool _dirty;
    private int _version; // counts edits, so a save knows whether newer typing is still unsaved
    private uint _words;
    private string? _problem;

    public WriterPage()
    {
        InitializeComponent();
        _autosave = DispatcherQueue.CreateTimer();
        _autosave.Interval = TimeSpan.FromMilliseconds(BookshelfFfiMethods.AutosaveMs());
        _autosave.IsRepeating = false;
        _autosave.Tick += (_, _) => _ = SaveAsync();
        Keys.Add(SaveButton, ShortcutKeys.Titles.Save, () => _ = SaveAsync());
        Keys.Add(this, ShortcutKeys.Titles.FullScreen, () => Shell.SetFullScreen(!Shell.IsFullScreen));
        Keys.Add(this, ShortcutKeys.Titles.LeaveFullScreen, () => Shell.SetFullScreen(false));
        Editor.PreviewKeyDown += OnEditorKeyDown;
        Shell.FullScreenChanged += OnFullScreenChanged;
        Unloaded += (_, _) => Shell.FullScreenChanged -= OnFullScreenChanged;
        OnFullScreenChanged();
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        _id = e.Parameter as string ?? "";
        try
        {
            var entry = await Session.Journal.EntryAsync(_id);
            var settings = Session.Settings ?? await Session.Journal.SettingsAsync(entry.UserId);
            var layout = BookshelfFfiMethods.WriterLayout(settings);
            _title = entry.Book.Title;
            WriterTitle.Text = _title;
            Column.MaxWidth = layout.ColumnWidth + 64; // the column plus its padding

            Editor.FontFamily = Look.Writing;
            Editor.FontSize = layout.FontPx;
            Prompt.FontFamily = Look.Writing;
            Prompt.FontSize = layout.FontPx;
            Prompt.LineHeight = layout.FontPx + layout.WrapGap + layout.LineGap;
            Prompt.Text = BookshelfFfiMethods.Prompts(entry.Shelf);
            var paragraph = Editor.Document.GetDefaultParagraphFormat();
            // Space between wrapped rows, and more below each line, from the core's layout.
            paragraph.SetLineSpacing(LineSpacingRule.Multiple, (float)(1 + layout.WrapGap / layout.FontPx));
            paragraph.SpaceAfter = (float)(layout.LineGap * 0.75); // pixels to points
            Editor.Document.SetDefaultParagraphFormat(paragraph);

            _writing = new WritingBox(Editor) { TextSize = settings.WritingSize };
            _writing.SetText(entry.Body);
            _writing.Edited += OnEdited;
            _words = BookshelfFfiMethods.WordCount(entry.Body);
            Prompt.Visibility = entry.Body.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
            ShowStatus();
            Editor.Document.Selection.SetRange(_writing.Text.Length, _writing.Text.Length);
            // Ready to type. After layout: focus asked for during navigation can be lost.
            DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () => Editor.Focus(FocusState.Keyboard));
        }
        catch (CoreException ex)
        {
            Editor.IsEnabled = false;
            ShowProblem($"Couldn't open this summary: {ex.UserMessage()}");
        }
    }

    protected override void OnNavigatingFrom(NavigatingCancelEventArgs e)
    {
        base.OnNavigatingFrom(e);
        _autosave.Stop();
        // Started now, before the next page reads the entry: the journal
        // runs calls in order, so it sees what was just written.
        if (_dirty) _ = SaveOnLeavingAsync();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        Shell.SetFullScreen(false);
    }

    private void OnEdited()
    {
        _dirty = true;
        _version++;
        Prompt.Visibility = _writing?.Text.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        ShowStatus();
        _autosave.Stop();
        _autosave.Start();
    }

    /// <summary>Saves what's unsaved. False (and the reason in the status line) if it couldn't be.</summary>
    private async Task<bool> SaveAsync()
    {
        _autosave.Stop();
        if (_writing is null || !_dirty) return true;
        var version = _version;
        try
        {
            var saved = await Session.Journal.SaveBodyAsync(_id, _writing.Text);
            _words = saved.Words;
            if (version == _version) _dirty = false;
            _problem = null;
            ShowStatus();
            return true;
        }
        catch (CoreException ex)
        {
            ShowProblem($"Not saved: {ex.UserMessage()}");
            return false;
        }
    }

    private async Task SaveOnLeavingAsync()
    {
        var text = _writing?.Text ?? "";
        if (await SaveAsync()) return;
        // Already leaving, so keep the text somewhere and say where.
        var rescued = await RescueAsync(text);
        Shell.ShowNotice($"“{_title}” couldn't be saved: {_problem} {rescued}", InfoBarSeverity.Error, autoHide: false);
    }

    /// <summary>When the window closes: save, or say where the text went and ask.</summary>
    public async Task<bool> CanCloseAsync()
    {
        if (await SaveAsync()) return true;
        var rescued = await RescueAsync(_writing?.Text ?? "");
        var dialog = new ContentDialog
        {
            Title = "Your writing couldn't be saved",
            Content = new TextBlock
            {
                Text = $"{_problem}\n\n{rescued}",
                TextWrapping = TextWrapping.Wrap,
                IsTextSelectionEnabled = true,
            },
            PrimaryButtonText = "Close Anyway",
            CloseButtonText = "Keep Open",
            DefaultButton = ContentDialogButton.Close,
        };
        return await DialogHost.ShowAsync(dialog) == ContentDialogResult.Primary;
    }

    /// <summary>
    /// The last resort: a recovery file, or failing that the clipboard.
    /// Returns a sentence saying where the text is.
    /// </summary>
    private async Task<string> RescueAsync(string text)
    {
        try
        {
            var file = await Session.Journal.RescueBodyAsync(_title, text);
            return $"A copy of your text is in {file}.";
        }
        catch (CoreException ex)
        {
            var package = new DataPackage();
            package.SetText(text);
            Clipboard.SetContent(package);
            return $"A recovery copy couldn't be written either ({ex.UserMessage()}), so your text was copied " +
                "to the clipboard. Paste it somewhere safe before closing Bookshelf.";
        }
    }

    private void ShowStatus()
    {
        Problem.Visibility = Visibility.Collapsed;
        Status.Visibility = Visibility.Visible;
        var words = _words == 1 ? "1 word" : $"{_words:N0} words";
        Status.Text = _dirty ? $"{words} · Editing" : $"{words} · Saved";
    }

    private void ShowProblem(string message)
    {
        _problem = message;
        Problem.Text = message;
        Problem.Visibility = Visibility.Visible;
        Status.Visibility = Visibility.Collapsed;
    }

    /// <summary>
    /// The RichEditBox keeps keys from keyboard accelerators, so the writing
    /// page's own keys are handled here: Ctrl+S saves, F11 and Esc go in
    /// and out of full screen. It also has Ctrl shortcuts for alignment and
    /// line spacing (Ctrl+E, R, L, J, 1, 2, 5) that would format the text
    /// behind the Markdown's back; those do nothing. (Ctrl+1/2/3 switch
    /// shelves, as everywhere: see MainWindow.)
    /// </summary>
    private void OnEditorKeyDown(object sender, KeyRoutedEventArgs e)
    {
        var modifiers = Keys.Modifiers();
        if (modifiers == VirtualKeyModifiers.None && e.Key == VirtualKey.F11)
        {
            e.Handled = true;
            Shell.SetFullScreen(!Shell.IsFullScreen);
        }
        else if (modifiers == VirtualKeyModifiers.None && e.Key == VirtualKey.Escape && Shell.IsFullScreen)
        {
            e.Handled = true;
            Shell.SetFullScreen(false);
        }
        else if (modifiers == VirtualKeyModifiers.Control)
        {
            switch (e.Key)
            {
                case VirtualKey.S:
                    e.Handled = true;
                    _ = SaveAsync();
                    break;
                case VirtualKey.E or VirtualKey.R or VirtualKey.L or VirtualKey.J or VirtualKey.Number5:
                    e.Handled = true;
                    break;
            }
        }
    }


    private void OnSave(object sender, RoutedEventArgs e) => _ = SaveAsync();

    private void OnFullScreen(object sender, RoutedEventArgs e) => Shell.SetFullScreen(!Shell.IsFullScreen);

    private void OnFullScreenChanged()
    {
        var on = Shell.IsFullScreen;
        FullScreenIcon.Glyph = on ? "\uE73F" : "\uE740";
        var label = on ? "Leave full screen (Esc)" : "Full screen (F11)";
        ToolTipService.SetToolTip(FullScreenButton, label);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(FullScreenButton, label);
    }
}
