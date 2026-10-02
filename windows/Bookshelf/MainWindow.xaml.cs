using System.Diagnostics;
using System.Runtime.InteropServices;
using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Pages;
using Bookshelf.Services;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using Windows.Graphics;
using Windows.System;
using WinRT.Interop;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;

namespace Bookshelf;

/// <summary>
/// The app's one window: the title bar with the search, the shelves and
/// the profile in the navigation pane, pages in the frame, and notices.
/// </summary>
public sealed partial class MainWindow : Window
{
    private readonly LaunchOptions _options;
    private readonly DispatcherQueueTimer _noticeTimer;
    private Session? _session;
    private OverlappedPresenter? _windowed;
    private RemovedEntry? _removed; // the last removal, while its Undo is offered
    private bool _canClose;
    private bool _closing;

    internal MainWindow(LaunchOptions options)
    {
        Instance = this;
        _options = options;
        StartupLog.Step("Loading MainWindow.xaml");
        InitializeComponent();
        Title = AppFolders.AppName;
        // The taskbar's name for the window, whatever the title bar shows.
        Root.Loaded += (_, _) => Title = AppFolders.AppName;
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        SetBackdrop();
        PlaceWindow();
        AppWindow.Closing += OnClosing;
        Activated += OnActivated;
        _noticeTimer = DispatcherQueue.CreateTimer();
        _noticeTimer.IsRepeating = false;
        _noticeTimer.Tick += (_, _) => Notice.IsOpen = false;
        SetShellVisible(false);
        AddKeyboardShortcuts();
        StartupLog.Step("Checking the bundled fonts");
        Look.LoadFonts();
        Look.Apply(null, Root, AppWindow);
        _ = StartAsync();
    }

    /// <summary>The app's window (there's only one).</summary>
    internal static MainWindow Instance { get; private set; } = null!;

    /// <summary>The open journal. Pages exist only once it's open.</summary>
    internal Session Session => _session ?? throw new InvalidOperationException("The journal isn't open yet.");

    // ---- startup ----------------------------------------------------------------

    private async Task StartAsync()
    {
        try
        {
            try
            {
                CoreLog.Start(AppFolders.Logs);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                StartupLog.Step($"No core.log: {e.Message}");
            }
            StartupLog.Step("Opening the journal");
            var (journal, open) = _options.DemoJournal
                ? await OpenDemoAsync()
                : (await JournalService.OpenDefaultAsync(Channel.Preview), null);
            _session = new Session(journal);
            _session.ShelvesChanged += UpdateBadges;
            _session.SearchChanged += UpdateBadges;
            _session.ProfileChanged += UpdateProfileItem;

            var profiles = await _session.LoadProfilesAsync();
            var state = await Task.Run(() => AppState.Load(journal.DataDirectory));
            open ??= profiles.FirstOrDefault(p => p.Id == state.LastProfileId) ?? profiles.FirstOrDefault();
            if (open is null) ShowWelcome(firstRun: true);
            else await OpenProfileAsync(open);

            Backups.Start(journal, DispatcherQueue);
            var showing = open is null ? "first run" : $"showing {ShelfText.Name(_session.CurrentShelf)}";
            StartupLog.Step($"Ready: Rust core {BookshelfFfiMethods.CoreVersion()} · {profiles.Count} profile(s) · {showing}");
        }
        catch (Exception e)
        {
            var message = e is CoreException core ? core.UserMessage() : e.Message;
            StartupLog.Step($"Couldn't open the journal: {message}");
            SetShellVisible(false);
            ContentFrame.Navigate(typeof(ProblemPage), message);
        }
    }

    /// <summary>A new demo journal in a temporary folder (never the real one), and the profile to open.</summary>
    private async Task<(JournalService, Profile?)> OpenDemoAsync()
    {
        var folder = Path.Combine(Path.GetTempPath(), $"Bookshelf demo {DateTime.Now:yyyyMMdd-HHmmss}-{Guid.NewGuid():N}"[..40]);
        StartupLog.Step($"Demo journal in {folder}");
        var journal = await JournalService.OpenAsync(folder);
        if (_options.DemoEmpty) return (journal, null);
        var demo = await DemoJournal.SeedAsync(journal, DateOnly.FromDateTime(DateTime.Now));
        var open = _options.DemoProfile?.ToLowerInvariant() switch
        {
            "sam" => demo.Sam,
            "jo" => demo.Jo,
            _ => demo.Avery,
        };
        return (journal, open);
    }

    // ---- window ---------------------------------------------------------------

    /// <summary>Mica on Windows 11; Windows 10 keeps the root's solid theme background.</summary>
    private void SetBackdrop()
    {
        if (!MicaController.IsSupported()) return;
        SystemBackdrop = new MicaBackdrop();
        Root.ClearValue(Panel.BackgroundProperty);
        StartupLog.Step("Mica backdrop");
    }

    /// <summary>A comfortable size, centered, never bigger than the screen; and a minimum size.</summary>
    private void PlaceWindow()
    {
        var scale = GetDpiForWindow(WindowNative.GetWindowHandle(this)) / 96.0;
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var width = (int)(1200 * scale);
        var height = (int)(820 * scale);
        if (!_options.DemoJournal)
        {
            // Demo windows keep their size, so screenshots match on any screen.
            width = Math.Min(width, area.Width);
            height = Math.Min(height, area.Height);
        }
        AppWindow.MoveAndResize(new RectInt32(
            area.X + Math.Max(0, (area.Width - width) / 2),
            area.Y + Math.Max(0, (area.Height - height) / 2),
            width,
            height));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            _windowed = presenter;
            presenter.PreferredMinimumWidth = (int)(560 * scale);
            presenter.PreferredMinimumHeight = (int)(480 * scale);
        }
    }

    /// <summary>Comes to the front (another launch handed over to this one).</summary>
    internal void BringToFront()
    {
        if (AppWindow.Presenter is OverlappedPresenter { State: OverlappedPresenterState.Minimized } presenter)
        {
            presenter.Restore();
        }
        Activate();
        SetForegroundWindow(WindowNative.GetWindowHandle(this));
    }

    private void OnActivated(object sender, WindowActivatedEventArgs args)
    {
        // Back from another app: the date may have moved on ("day 12").
        if (args.WindowActivationState != WindowActivationState.Deactivated && _session?.Profile is not null)
        {
            _ = _session.RefreshShelvesAsync();
        }
    }

    /// <summary>The navigation pane and search, hidden until there's a profile.</summary>
    private void SetShellVisible(bool visible)
    {
        Nav.IsPaneVisible = visible && !IsFullScreen;
        AppTitleBar.IsPaneToggleButtonVisible = visible;
        AppTitleBar.IsBackButtonVisible = visible;
        SearchBox.Visibility = visible ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>Applies the open profile's theme, accent and fonts.</summary>
    internal void ApplyLook() => Look.Apply(_session?.Settings, Root, AppWindow);

    // ---- full screen ------------------------------------------------------------

    /// <summary>Whether the window fills the screen (reading and writing only).</summary>
    internal bool IsFullScreen => AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;

    /// <summary>Raised when full screen starts or ends.</summary>
    internal event Action? FullScreenChanged;

    internal void SetFullScreen(bool on)
    {
        if (on == IsFullScreen) return;
        if (on) AppWindow.SetPresenter(AppWindowPresenterKind.FullScreen);
        else if (_windowed is not null) AppWindow.SetPresenter(_windowed);
        else AppWindow.SetPresenter(AppWindowPresenterKind.Overlapped);
        // Nothing but the page in full screen.
        AppTitleBar.Visibility = on ? Visibility.Collapsed : Visibility.Visible;
        Nav.IsPaneVisible = !on && _session?.Profile is not null;
        if (on) ShowNotice("Press Esc to leave full screen", InfoBarSeverity.Informational, seconds: 3);
        FullScreenChanged?.Invoke();
    }

    // ---- profiles ---------------------------------------------------------------

    /// <summary>Opens a profile on its start shelf, in its own look.</summary>
    internal async Task OpenProfileAsync(Profile profile)
    {
        await Session.OpenAsync(profile);
        ApplyLook();
        SearchBox.Text = "";
        Session.SetSearch("");
        SetShellVisible(true);
        ShowShelf(Session.CurrentShelf);
    }

    /// <summary>The first-run page, or New profile.</summary>
    internal void ShowWelcome(bool firstRun)
    {
        if (firstRun)
        {
            SetShellVisible(false);
            ApplyLook();
        }
        ContentFrame.Navigate(typeof(WelcomePage), firstRun, new EntranceNavigationTransitionInfo());
        if (firstRun) ContentFrame.BackStack.Clear();
        UpdateBackButton();
    }

    /// <summary>Deletes a profile (already confirmed twice) and opens another, or the welcome page.</summary>
    internal async Task DeleteProfileAsync(Profile profile)
    {
        try
        {
            await Session.Journal.DeleteProfileAsync(profile.Id);
            Session.Close();
            var others = await Session.LoadProfilesAsync();
            if (others.FirstOrDefault() is { } next) await OpenProfileAsync(next);
            else ShowWelcome(firstRun: true);
            ShowNotice($"Deleted {profile.Name}", InfoBarSeverity.Success);
        }
        catch (CoreException e)
        {
            ShowNotice(e.UserMessage(), InfoBarSeverity.Error, autoHide: false);
        }
    }

    private void UpdateProfileItem()
    {
        var profile = _session?.Profile;
        ProfilePicture.DisplayName = profile?.Name ?? "";
        ProfileName.Text = profile?.Name ?? "";
        var label = profile is null ? "Profiles" : $"{profile.Name}: switch profile";
        AutomationProperties.SetName(ProfileItem, label);
        ToolTipService.SetToolTip(ProfileItem, "Switch profile");
    }

    private void ShowProfileMenu()
    {
        ProfileMenu.Items.Clear();
        foreach (var profile in Session.Profiles)
        {
            var item = new RadioMenuFlyoutItem
            {
                Text = profile.Name,
                GroupName = "profiles",
                IsChecked = profile.Id == Session.Profile?.Id,
            };
            item.Click += async (_, _) =>
            {
                if (profile.Id != Session.Profile?.Id) await OpenProfileAsync(profile);
            };
            ProfileMenu.Items.Add(item);
        }
        ProfileMenu.Items.Add(new MenuFlyoutSeparator());
        var add = new MenuFlyoutItem { Text = "New profile…", Icon = new FontIcon { Glyph = "" } };
        add.Click += (_, _) => ShowWelcome(firstRun: false);
        ProfileMenu.Items.Add(add);
        FlyoutBase.ShowAttachedFlyout(ProfileItem);
    }

    // ---- navigation -------------------------------------------------------------

    /// <summary>Shows a shelf. Shelves are the top of the app, so Back starts again from here.</summary>
    internal void ShowShelf(Shelf shelf)
    {
        if (_session?.Profile is null) return;
        Session.CurrentShelf = shelf;
        if (ContentFrame.Content is ShelfPage page) page.Show();
        else ContentFrame.Navigate(typeof(ShelfPage), null, new EntranceNavigationTransitionInfo());
        ContentFrame.BackStack.Clear();
        UpdateBackButton();
        SelectNavItem();
    }

    /// <summary>Opens an entry's page from its shelf.</summary>
    internal void OpenEntry(string summaryId, Shelf shelf)
    {
        ShowShelf(shelf);
        ContentFrame.Navigate(typeof(BookPage), summaryId, new DrillInNavigationTransitionInfo());
    }

    internal void OpenSettings()
    {
        if (_session?.Profile is null || ContentFrame.Content is SettingsPage) return;
        ContentFrame.Navigate(typeof(SettingsPage), null, new EntranceNavigationTransitionInfo());
    }

    internal void OpenManual()
    {
        if (ContentFrame.Content is ManualPage) return;
        ContentFrame.Navigate(typeof(ManualPage), null, new EntranceNavigationTransitionInfo());
    }

    internal Task ShowShortcutsAsync() => Dialogs.ShortcutsDialog.ShowAsync();

    internal Task AddBookAsync() => Dialogs.AddBookFlow.RunAsync(this);

    private void GoBack()
    {
        if (ContentFrame.CanGoBack) ContentFrame.GoBack();
    }

    private void OnBackRequested(TitleBar sender, object args) => GoBack();

    private void OnPaneToggleRequested(TitleBar sender, object args) => Nav.IsPaneOpen = !Nav.IsPaneOpen;

    private void OnNavItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        if (args.IsSettingsInvoked)
        {
            OpenSettings();
        }
        else if (args.InvokedItemContainer == ProfileItem)
        {
            ShowProfileMenu();
        }
        else if (args.InvokedItemContainer?.Tag is string tag && Enum.TryParse<Shelf>(tag, out var shelf))
        {
            ShowShelf(shelf);
        }
    }

    private void OnNavigated(object sender, NavigationEventArgs e)
    {
        UpdateBackButton();
        SelectNavItem();
        if (e.Content is not (ReaderPage or WriterPage)) SetFullScreen(false);
    }

    private void OnNavigationFailed(object sender, NavigationFailedEventArgs e)
    {
        StartupLog.Step($"Couldn't open {e.SourcePageType?.Name}: {e.Exception?.Message}");
        e.Handled = true;
    }

    private void UpdateBackButton() => AppTitleBar.IsBackButtonEnabled = ContentFrame.CanGoBack;

    /// <summary>Highlights where the page belongs: its shelf, Settings, or nothing.</summary>
    private void SelectNavItem()
    {
        var item = ContentFrame.Content switch
        {
            SettingsPage => Nav.SettingsItem,
            ShelfPage or BookPage or ReaderPage or WriterPage when _session?.Profile is not null => ItemFor(Session.CurrentShelf),
            _ => null,
        };
        Nav.SelectedItem = item;
        // A null selection leaves the Settings item highlighted, so unselect it by hand.
        if (item is null && Nav.SettingsItem is NavigationViewItem settings) settings.IsSelected = false;
    }

    private NavigationViewItem ItemFor(Shelf shelf) => shelf switch
    {
        Shelf.Finished => FinishedItem,
        Shelf.Eventually => EventuallyItem,
        _ => ReadingItem,
    };

    private InfoBadge BadgeFor(Shelf shelf) => shelf switch
    {
        Shelf.Finished => FinishedBadge,
        Shelf.Eventually => EventuallyBadge,
        _ => ReadingBadge,
    };

    /// <summary>Books per shelf beside its name; while searching, the matches on each.</summary>
    private void UpdateBadges()
    {
        foreach (var shelf in ShelfText.All)
        {
            var badge = BadgeFor(shelf);
            var view = _session?.ViewOf(shelf);
            if (view is null)
            {
                badge.Visibility = Visibility.Collapsed;
                continue;
            }
            var count = Session.IsSearching ? Session.MatchCount(shelf) : (int)view.Total;
            badge.Value = count;
            badge.Visibility = count > 0 ? Visibility.Visible : Visibility.Collapsed;
            var what = Session.IsSearching ? (count == 1 ? "1 match" : $"{count} matches") : (count == 1 ? "1 book" : $"{count} books");
            AutomationProperties.SetName(ItemFor(shelf), $"{ShelfText.Name(shelf)}, {what}");
        }
    }

    // ---- search -----------------------------------------------------------------

    private void OnSearchTextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (_session?.Profile is null) return;
        Session.SetSearch(sender.Text);
        if (Session.IsSearching && ContentFrame.Content is not ShelfPage) ShowShelf(Session.CurrentShelf);
    }

    private void OnSearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        if (_session?.Profile is null || !Session.IsSearching) return;
        // Nothing here but something on another shelf: go there.
        if (Session.MatchCount(Session.CurrentShelf) == 0
            && ShelfText.All.FirstOrDefault(s => Session.MatchCount(s) > 0) is var other
            && Session.MatchCount(other) > 0)
        {
            ShowShelf(other);
        }
    }

    private void OnSearchKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Escape) return;
        e.Handled = true;
        if (SearchBox.Text.Length > 0) SearchBox.Text = "";
        else ContentFrame.Focus(FocusState.Programmatic);
    }

    private void FocusSearch()
    {
        if (_session?.Profile is null || IsFullScreen) return;
        SearchBox.Focus(FocusState.Keyboard);
    }

    /// <summary>Starts a search with a character typed on the shelf page.</summary>
    internal void StartSearch(char c)
    {
        SearchBox.Text += c;
        SearchBox.Focus(FocusState.Keyboard);
        if (FindChild<TextBox>(SearchBox) is { } box) box.SelectionStart = box.Text.Length;
    }

    private static T? FindChild<T>(DependencyObject parent) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is T found) return found;
            if (FindChild<T>(child) is { } deeper) return deeper;
        }
        return null;
    }

    // ---- keyboard ---------------------------------------------------------------

    /// <summary>The shortcuts that work everywhere, from the core's Windows table.</summary>
    private void AddKeyboardShortcuts()
    {
        Root.KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
        Keys.Add(Root, ShortcutKeys.Titles.Reading, () => ShowShelf(Shelf.Reading));
        Keys.Add(Root, ShortcutKeys.Titles.Finished, () => ShowShelf(Shelf.Finished));
        Keys.Add(Root, ShortcutKeys.Titles.Eventually, () => ShowShelf(Shelf.Eventually));
        Keys.Add(Root, ShortcutKeys.Titles.AddBook, () => _ = AddBookAsync());
        Keys.Add(Root, ShortcutKeys.Titles.Search, FocusSearch);
        Keys.Add(Root, ShortcutKeys.Titles.Settings, OpenSettings);
        Keys.Add(Root, ShortcutKeys.Titles.Manual, OpenManual);
        Keys.Add(Root, ShortcutKeys.Titles.Shortcuts, () => _ = ShowShortcutsAsync());
        // Windows' own Back keys.
        Keys.Add(Root, VirtualKey.Left, VirtualKeyModifiers.Menu, GoBack);
        Keys.Add(Root, VirtualKey.GoBack, VirtualKeyModifiers.None, GoBack);
    }

    // ---- notices ----------------------------------------------------------------

    /// <summary>
    /// A notice at the bottom of the window, which outlives the page that
    /// raised it. Narrator reads it out when it opens.
    /// </summary>
    internal void ShowNotice(
        string message,
        InfoBarSeverity severity = InfoBarSeverity.Informational,
        string? actionLabel = null,
        Func<Task>? action = null,
        bool autoHide = true,
        int seconds = 10)
    {
        ForgetRemoved();
        Notice.IsOpen = false;
        Notice.Title = "";
        Notice.Message = message;
        Notice.Severity = severity;
        if (actionLabel is not null && action is not null)
        {
            var button = new Button { Content = actionLabel };
            button.Click += async (_, _) =>
            {
                // Started before the notice closes: closing it forgets a removal's Undo.
                var running = action();
                Notice.IsOpen = false;
                await running;
            };
            Notice.ActionButton = button;
        }
        else
        {
            Notice.ActionButton = null;
        }
        NoticeHost.Visibility = Visibility.Visible;
        Notice.IsOpen = true;
        _noticeTimer.Stop();
        if (autoHide)
        {
            _noticeTimer.Interval = TimeSpan.FromSeconds(seconds);
            _noticeTimer.Start();
        }
    }

    private void OnNoticeClosed(InfoBar sender, InfoBarClosedEventArgs args)
    {
        if (Notice.IsOpen) return; // a new notice took its place
        NoticeHost.Visibility = Visibility.Collapsed;
        _noticeTimer.Stop();
        ForgetRemoved();
    }

    /// <summary>"Removed “Dune”", with Undo for ten seconds (and Ctrl+Z on the shelf).</summary>
    internal void ShowRemoved(RemovedEntry removed)
    {
        ShowNotice($"Removed “{removed.Title()}”", InfoBarSeverity.Informational, "Undo", UndoRemoveAsync);
        _removed = removed; // after ShowNotice, which forgets the previous one
    }

    /// <summary>Puts the last removed entry back, if its Undo is still offered.</summary>
    internal async Task UndoRemoveAsync()
    {
        if (_removed is not { } removed) return;
        _removed = null;
        Notice.IsOpen = false;
        try
        {
            await Session.Journal.RestoreEntryAsync(removed);
            await Session.RefreshShelvesAsync();
            Announce($"Restored “{removed.Title()}”");
        }
        catch (CoreException e)
        {
            ShowNotice($"Couldn't undo: {e.UserMessage()}", InfoBarSeverity.Error, autoHide: false);
        }
        finally
        {
            removed.Dispose();
        }
    }

    private void ForgetRemoved()
    {
        _removed?.Dispose();
        _removed = null;
    }

    /// <summary>Says something to Narrator (and other screen readers) without showing it.</summary>
    internal void Announce(string text)
    {
        var peer = FrameworkElementAutomationPeer.FromElement(Root) ?? FrameworkElementAutomationPeer.CreatePeerForElement(Root);
        peer?.RaiseNotificationEvent(
            AutomationNotificationKind.ActionCompleted,
            AutomationNotificationProcessing.ImportantMostRecent,
            text,
            "BookshelfAnnouncement");
    }

    /// <summary>Opens a folder in File Explorer.</summary>
    internal Task OpenFolder(string folder)
    {
        if (!Directory.Exists(folder))
        {
            ShowNotice("That folder isn't available right now.", InfoBarSeverity.Warning);
            return Task.CompletedTask;
        }
        try
        {
            Process.Start(new ProcessStartInfo("explorer.exe") { ArgumentList = { folder }, UseShellExecute = false })?.Dispose();
        }
        catch (Exception e)
        {
            ShowNotice($"Couldn't open the folder: {e.Message}", InfoBarSeverity.Error);
        }
        return Task.CompletedTask;
    }

    // ---- closing ----------------------------------------------------------------

    /// <summary>
    /// Saves the writing page, if one is open, before closing (asking what
    /// to do if that fails), then tidies the journal up.
    /// </summary>
    private async void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (_canClose) return;
        args.Cancel = true;
        if (_closing) return;
        _closing = true;
        try
        {
            if (ContentFrame.Content is IGuardsClose guard && !await guard.CanCloseAsync()) return; // Keep Open
            if (_session is not null) await _session.CloseJournalAsync();
        }
        catch (Exception e)
        {
            StartupLog.Step($"While closing: {e.Message}");
        }
        finally
        {
            _closing = false;
        }
        _canClose = true;
        Close();
    }

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr window);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetForegroundWindow(IntPtr window);
}
