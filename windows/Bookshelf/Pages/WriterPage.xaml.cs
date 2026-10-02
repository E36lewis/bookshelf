using Bookshelf.Core;
using Bookshelf.Core.Editing;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Windows.ApplicationModel.DataTransfer;
using Windows.Foundation;
using Windows.System;
using DispatcherQueuePriority = Microsoft.UI.Dispatching.DispatcherQueuePriority;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;

namespace Bookshelf.Pages;

/// <summary>A page that may need to save before the window closes.</summary>
internal interface IGuardsClose
{
    /// <summary>Saves; false means the person chose to keep the window open.</summary>
    Task<bool> CanCloseAsync();
}

/// <summary>
/// The writing page: the summary in the writing font, highlighted as
/// Markdown by <see cref="WritingBox"/>, with a formatting bar, focus mode
/// and full screen. It saves after a short pause in typing, on Ctrl+S, on
/// leaving and when the window closes; if a save fails, the text is rescued
/// to a file and the person is told where.
/// </summary>
public sealed partial class WriterPage : BookshelfPage, IGuardsClose
{
    /// <summary>The space above the first line, as in the GTK app.</summary>
    private const double TopMargin = 40;

    /// <summary>The least space either side of the column.</summary>
    private const double SideMargin = 32;

    private readonly DispatcherQueueTimer _autosave;
    private string _id = "";
    private string _title = "";
    private WritingBox? _writing;
    private PageLayout? _layout;
    private bool _dirty;
    private int _version; // counts edits, so a save knows whether newer typing is still unsaved
    private uint _words;
    private string? _problem; // why the last save failed, while it's unsaved

    public WriterPage()
    {
        InitializeComponent();
        _autosave = DispatcherQueue.CreateTimer();
        _autosave.Interval = TimeSpan.FromMilliseconds(BookshelfFfiMethods.AutosaveMs());
        _autosave.IsRepeating = false;
        _autosave.Tick += (_, _) => _ = SaveAsync();

        // While typing, the editor's own key handler (OnEditorKeyDown) has
        // these keys; the accelerators are for the rest of the page (the
        // formatting bar, say).
        Keys.Add(SaveButton, ShortcutKeys.Titles.Save, () => _ = SaveAsync(), exceptWhileTyping: true);
        Keys.Add(this, ShortcutKeys.Titles.FullScreen, ToggleFullScreen, exceptWhileTyping: true);
        Keys.Add(this, ShortcutKeys.Titles.LeaveFullScreen, () => Shell.SetFullScreen(false), exceptWhileTyping: true);
        Keys.Add(FocusButton, ShortcutKeys.Titles.FocusMode, ToggleFocusMode, exceptWhileTyping: true);
        foreach (var command in FormatCommands.All.Where(c => c.ShortcutTitle is not null))
        {
            Keys.Add(this, command.ShortcutTitle!, () => Format(command), exceptWhileTyping: true);
        }
        DescribeButtons();

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
            _layout = layout;
            _title = entry.Book.Title;
            WriterTitle.Text = _title;

            Editor.FontFamily = Look.Writing;
            Editor.FontSize = layout.FontPx;
            var spacing = WriterSpacing.For(RowHeight(Look.Writing, layout.FontPx), layout.WrapGap, layout.LineGap);
            Prompt.FontFamily = Look.Writing;
            Prompt.FontSize = layout.FontPx;
            Prompt.LineHeight = spacing.LinePitch;
            Prompt.Text = BookshelfFfiMethods.Prompts(entry.Shelf);
            LayOut();

            var writing = new WritingBox(Editor) { TextSize = (float)(layout.FontPx * 0.75), LogTimings = App.Options.DemoJournal };
            _writing = writing;
            writing.SetSpacing(spacing);
            writing.SetText(entry.Body);
            writing.Edited += OnEdited;
            if (App.Options.DemoJournal)
            {
                // Demo journals only: the timings, for the UI tour's speed check.
                Timing.Visibility = Visibility.Visible;
                writing.Timed += () => Timing.Text = writing.Timings.Summary;
                Timing.Text = writing.Timings.Summary;
                LookProbe.Visibility = Visibility.Visible;
                writing.Looked += look => LookProbe.Text = look;
                writing.ProbeLook();
            }
            LayOut();
            _words = BookshelfFfiMethods.WordCount(entry.Body);
            UpdatePrompt();
            ShowStatus(announce: false);
            Editor.Document.Selection.SetRange(writing.Text.Length, writing.Text.Length);
            if (settings.FocusDefault) SetFocusMode(true, announce: false);
            // Ready to type. After layout: focus asked for during navigation can be lost.
            DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () => Editor.Focus(FocusState.Keyboard));
        }
        catch (CoreException ex)
        {
            Editor.IsEnabled = false;
            FormatBar.IsEnabled = false;
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

    // ---- layout -----------------------------------------------------------------

    /// <summary>How tall a row of text in <paramref name="font"/> is on its own, in pixels.</summary>
    private static double RowHeight(FontFamily font, double size)
    {
        var probe = new TextBlock { FontFamily = font, FontSize = size, Text = "Ag" };
        probe.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        return probe.DesiredSize.Height;
    }

    private void OnPageSizeChanged(object sender, SizeChangedEventArgs e) => LayOut();

    /// <summary>
    /// Centers the column (at most the layout's width) with the editor's
    /// padding, and leaves room below the last line to bring it up to the
    /// middle of the page. The prompts sit exactly where typing starts.
    /// </summary>
    private void LayOut()
    {
        if (_layout is not { } layout || PageArea.ActualWidth <= 0) return;
        var side = Math.Max(SideMargin, (PageArea.ActualWidth - layout.ColumnWidth) / 2);
        var below = Math.Max(120, PageArea.ActualHeight / 2);
        Editor.Padding = new Thickness(side, 0, side, 0);
        _writing?.SetRoom(TopMargin, below);
        Prompt.Margin = new Thickness(side, TopMargin, side, 0);
    }

    // ---- editing ----------------------------------------------------------------

    private void OnEdited()
    {
        _dirty = true;
        _version++;
        UpdatePrompt();
        ShowStatus(announce: false);
        _autosave.Stop();
        _autosave.Start();
    }

    /// <summary>The prompts show only on an empty page; screen readers get them as the editor's help text.</summary>
    private void UpdatePrompt()
    {
        var empty = _writing?.Text.Length == 0;
        Prompt.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        AutomationProperties.SetHelpText(Editor, empty ? Prompt.Text.Replace('\n', ' ') : "");
    }

    private void Format(FormatCommand command)
    {
        if (_writing is null || !Editor.IsEnabled) return;
        _writing.Format(command.Action);
        Editor.Focus(FocusState.Programmatic);
    }

    private void OnFormat(object sender, RoutedEventArgs e)
    {
        if (sender is FrameworkElement { Tag: string tag } && FormatCommands.Find(tag) is { } command) Format(command);
    }

    /// <summary>Tooltips with keys, from the core's shortcuts table, and the same for Narrator.</summary>
    private void DescribeButtons()
    {
        foreach (var button in FormatBar.PrimaryCommands.OfType<AppBarButton>())
        {
            if (FormatCommands.Find(button.Tag as string) is not { } command) continue;
            ToolTipService.SetToolTip(button, FormatCommands.Tooltip(command));
            if (FormatCommands.KeyLabel(command.ShortcutTitle) is { } key) AutomationProperties.SetAcceleratorKey(button, key);
        }
        var focusKey = FormatCommands.KeyLabel(ShortcutKeys.Titles.FocusMode);
        ToolTipService.SetToolTip(FocusButton, WriterText.FocusTooltip(focusKey));
        if (focusKey is not null) AutomationProperties.SetAcceleratorKey(FocusButton, focusKey);
    }

    // ---- focus mode and full screen ------------------------------------------------

    private void ToggleFocusMode() => SetFocusMode(!(_writing?.IsFocusMode ?? false), announce: true);

    private void SetFocusMode(bool on, bool announce)
    {
        if (_writing is null) return;
        _writing.SetFocusMode(on);
        FocusButton.IsChecked = on;
        if (announce) Shell.Announce(on ? "Focus mode on" : "Focus mode off");
    }

    private void OnFocusClicked(object sender, RoutedEventArgs e)
    {
        SetFocusMode(FocusButton.IsChecked == true, announce: false);
        Editor.Focus(FocusState.Programmatic);
    }

    private void ToggleFullScreen() => Shell.SetFullScreen(!Shell.IsFullScreen);

    private void OnFullScreen(object sender, RoutedEventArgs e) => ToggleFullScreen();

    private void OnFullScreenChanged()
    {
        var on = Shell.IsFullScreen;
        FullScreenIcon.Glyph = on ? "" : "";
        // Every key: F11 is a hardware key on many laptops, so Ctrl+Shift+Enter too.
        var key = on
            ? ShortcutKeys.Labels(ShortcutKeys.Titles.LeaveFullScreen, ShortcutKeys.Titles.FullScreen)
            : ShortcutKeys.Labels(ShortcutKeys.Titles.FullScreen);
        var label = on ? "Leave full screen" : "Full screen";
        FullScreenButton.Label = label;
        ToolTipService.SetToolTip(FullScreenButton, key is null ? label : $"{label} ({key})");
        AutomationProperties.SetAcceleratorKey(FullScreenButton, key ?? "");
    }

    // ---- saving -----------------------------------------------------------------

    /// <summary>Saves what's unsaved. False (and the reason in the status line) if it couldn't be.</summary>
    private async Task<bool> SaveAsync()
    {
        _writing?.CatchUp();
        _autosave.Stop();
        if (_writing is null || !_dirty) return true;
        var version = _version;
        try
        {
            var saved = await Session.Journal.SaveBodyAsync(_id, _writing.Text);
            _words = saved.Words;
            if (version == _version) _dirty = false;
            _problem = null;
            ShowStatus(announce: true);
            return true;
        }
        catch (CoreException ex)
        {
            _problem = ex.UserMessage();
            ShowProblem(WriterText.NotSaved(_problem));
            return false;
        }
    }

    private async Task SaveOnLeavingAsync()
    {
        if (await SaveAsync()) return;
        var text = _writing?.Text ?? "";
        // Already leaving, so keep the text somewhere and say where.
        var rescued = await RescueAsync(text);
        Shell.ShowNotice($"“{_title}” couldn't be saved: {_problem} {rescued}", InfoBarSeverity.Error, autoHide: false);
    }

    /// <summary>When the window closes: save, or say where the text went and ask.</summary>
    public async Task<bool> CanCloseAsync()
    {
        if (await SaveAsync()) return true;
        var rescued = await RescueAsync(_writing?.Text ?? "");
        var message = new TextBlock
        {
            Text = $"{_problem}\n\n{rescued}",
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
        };
        AutomationProperties.SetAutomationId(message, "RescueMessage");
        var dialog = new ContentDialog
        {
            Title = "Your writing couldn't be saved",
            Content = message,
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

    /// <summary>"89 words · Saved". Read out (politely) only when <paramref name="announce"/>: after a save, not per key.</summary>
    private void ShowStatus(bool announce)
    {
        Problem.Visibility = Visibility.Collapsed;
        Status.Visibility = Visibility.Visible;
        Status.Text = WriterText.Status(_words, saved: !_dirty);
        if (announce) RaiseLiveRegionChanged(Status);
    }

    private void ShowProblem(string message)
    {
        Problem.Text = message;
        Problem.Visibility = Visibility.Visible;
        Status.Visibility = Visibility.Collapsed;
        RaiseLiveRegionChanged(Problem);
    }

    private static void RaiseLiveRegionChanged(UIElement element)
    {
        var peer = FrameworkElementAutomationPeer.FromElement(element) ?? FrameworkElementAutomationPeer.CreatePeerForElement(element);
        peer?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
    }

    private void OnSave(object sender, RoutedEventArgs e) => _ = SaveAsync();

    // ---- keys ---------------------------------------------------------------------

    /// <summary>
    /// The RichEditBox keeps keys from keyboard accelerators, so the writing
    /// page's own keys are handled here, from the core's shortcuts table:
    /// Ctrl+S saves, Ctrl+B, I and K format, Ctrl+Shift+F is focus mode, F11
    /// or Ctrl+Shift+Enter and Esc go in and out of full screen. Shift+Tab
    /// goes to the formatting bar (Tab types a tab, for nested lists). The
    /// box also has Ctrl shortcuts for alignment and line spacing (Ctrl+E,
    /// R, L, J, 1, 2, 5) that would format the text behind the Markdown's
    /// back; those do nothing. (Ctrl+1/2/3 switch shelves, as everywhere:
    /// see MainWindow.)
    /// </summary>
    private void OnEditorKeyDown(object sender, KeyRoutedEventArgs e)
    {
        var modifiers = Keys.Modifiers();
        var key = e.Key;
        if (Keys.Is(ShortcutKeys.Titles.FullScreen, key, modifiers))
        {
            e.Handled = true;
            ToggleFullScreen();
        }
        else if (Keys.Is(ShortcutKeys.Titles.LeaveFullScreen, key, modifiers) && Shell.IsFullScreen)
        {
            e.Handled = true;
            Shell.SetFullScreen(false);
        }
        else if (Keys.Is(ShortcutKeys.Titles.FocusMode, key, modifiers))
        {
            e.Handled = true;
            ToggleFocusMode();
        }
        else if (Keys.Is(ShortcutKeys.Titles.Save, key, modifiers))
        {
            e.Handled = true;
            _ = SaveAsync();
        }
        else if (FormatCommands.All.FirstOrDefault(c => c.ShortcutTitle is { } t && Keys.Is(t, key, modifiers)) is { } command)
        {
            e.Handled = true;
            Format(command);
        }
        else if (modifiers == VirtualKeyModifiers.Shift && key == VirtualKey.Tab)
        {
            e.Handled = true;
            BoldButton.Focus(FocusState.Keyboard);
        }
        else if (modifiers == VirtualKeyModifiers.Control
            && key is VirtualKey.E or VirtualKey.R or VirtualKey.L or VirtualKey.J or VirtualKey.Number5)
        {
            e.Handled = true;
        }
    }

    /// <summary>Esc on the formatting bar goes back to the text.</summary>
    private void OnFormatBarKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Escape || FormatBar.IsOpen || Shell.IsFullScreen) return;
        e.Handled = true;
        Editor.Focus(FocusState.Keyboard);
    }
}
