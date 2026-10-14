using System.Diagnostics;
using System.Runtime.InteropServices;
using Bookshelf.Controls;
using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Pages;
using Bookshelf.Services;
using Microsoft.UI.Input;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Navigation;
using Windows.Foundation;
using Windows.Graphics;
using Windows.System;
using WinRT.Interop;
using DispatcherQueuePriority = Microsoft.UI.Dispatching.DispatcherQueuePriority;
using DispatcherQueueTimer = Microsoft.UI.Dispatching.DispatcherQueueTimer;

namespace Bookshelf;

/// <summary>
/// The app's one window, laid out as the GTK app is: a row across the top
/// (Back and Add a book, the three shelves as tabs, Search and Settings)
/// that's also the title bar, the search under it when it's open, pages in
/// one frame (the profiles, then a shelf, a book, its writing and reading
/// pages), and notices.
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
    private bool _syncingTabs; // the tabs are being set from code, not by the person
    private bool _compactTabs; // icons only: the window is too narrow for the shelves' names
    private double _tabsFullWidth; // the tabs with their names
    private RectInt32[] _passthrough = [];

    internal MainWindow(LaunchOptions options)
    {
        Instance = this;
        _options = options;
        StartupLog.Step("Loading MainWindow.xaml");
        InitializeComponent();
        // The taskbar's name for the window. "Bookshelf" alone is a release;
        // anything else says it's a preview (in the header too).
        Title = AppFolders.AppName;
        if (AppFolders.BuildChannel == Channel.Preview) AppChannel.Visibility = Visibility.Visible;
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(Header);
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        Header.SizeChanged += (_, _) => LayOutHeader();
        LeftButtons.SizeChanged += (_, _) => LayOutHeader();
        RightButtons.SizeChanged += (_, _) => LayOutHeader();
        AppTitle.SizeChanged += (_, _) => LayOutHeader();
        Tabs.SizeChanged += OnTabsSizeChanged;
        Header.LayoutUpdated += (_, _) => UpdatePassthrough();
        _ = SetIconsAsync();
        PlaceWindow();
        AppWindow.Closing += OnClosing;
        Activated += OnActivated;
        _noticeTimer = DispatcherQueue.CreateTimer();
        _noticeTimer.IsRepeating = false;
        _noticeTimer.Tick += (_, _) => Notice.IsOpen = false;
        SetTips();
        UpdateChrome();
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
                : (await JournalService.OpenDefaultAsync(AppFolders.BuildChannel), null);
            if (_options.DemoJournal && _options.DemoSaveFails)
            {
                // After the demo's own notes are in.
                journal.FailBodySavesForTesting();
                StartupLog.Step("Demo journal: saving the writing page will fail (--demo-save-fails)");
            }
            _session = new Session(journal);

            var profiles = await _session.LoadProfilesAsync();
            var state = await Task.Run(() => AppState.Load(journal.DataDirectory));
            open ??= profiles.FirstOrDefault(p => p.Id == state.LastProfileId) ?? profiles.FirstOrDefault();
            if (open is null) ShowProfiles(); // the first run: Welcome
            else await OpenProfileAsync(open);

            Backups.Start(journal, DispatcherQueue);
            var showing = open is null ? "first run" : $"showing {ShelfText.Name(_session.CurrentShelf)}";
            StartupLog.Step($"Ready: Rust core {BookshelfFfiMethods.CoreVersion()} · {profiles.Count} profile(s) · {showing}");
        }
        catch (Exception e)
        {
            var message = e is CoreException core ? core.UserMessage() : e.Message;
            StartupLog.Step($"Couldn't open the journal: {message}");
            ContentFrame.Navigate(typeof(ProblemPage), message);
        }
    }

    /// <summary>
    /// A demo journal (never the real one) and the profile to open: a new one
    /// in a temporary folder, or in the folder given, where a second run
    /// finds it again and opens its last-used profile.
    /// </summary>
    private async Task<(JournalService, Profile?)> OpenDemoAsync()
    {
        var folder = _options.DemoFolder is { } chosen
            ? Path.GetFullPath(chosen)
            : Path.Combine(Path.GetTempPath(), $"Bookshelf demo {DateTime.Now:yyyyMMdd-HHmmss}-{Guid.NewGuid():N}"[..40]);
        var state = DemoJournal.Check(folder);
        StartupLog.Step($"Demo journal in {folder} ({state})");
        if (state == DemoJournal.FolderState.NotDemo)
        {
            throw new InvalidOperationException($"{folder} isn't empty and doesn't hold a demo journal, so it was left alone.");
        }
        var journal = await JournalService.OpenAsync(folder);
        if (state == DemoJournal.FolderState.Demo) return (journal, null);
        await Task.Run(() => DemoJournal.Mark(folder));
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

    /// <summary>
    /// The app icon on the window (taskbar, Alt+Tab) and beside its name in
    /// the header, from the copy of Bookshelf.ico unpacked next to
    /// Bookshelf.dll. If it can't be read, the header shows the name alone.
    /// </summary>
    private async Task SetIconsAsync()
    {
        var path = Path.Combine(AppContext.BaseDirectory, "Assets", "Bookshelf.ico");
        try
        {
            AppWindow.SetIcon(path);
            // From a stream: an unpackaged app's ms-appx and file URIs may not reach it.
            var icon = new BitmapImage { DecodePixelType = DecodePixelType.Logical, DecodePixelWidth = 16 };
            using (var file = File.OpenRead(path))
            {
                await icon.SetSourceAsync(file.AsRandomAccessStream());
            }
            AppIcon.Source = icon;
        }
        catch (Exception e)
        {
            StartupLog.Step($"No app icon on the window: {e.Message}");
        }
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
            presenter.PreferredMinimumWidth = (int)(420 * scale);
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

    /// <summary>Applies the open profile's theme, accent and fonts (with none open, the neutral look).</summary>
    internal void ApplyLook() => Look.Apply(_session?.Settings, Root, AppWindow);

    // ---- the header -------------------------------------------------------------

    /// <summary>
    /// What the header and the window show for the page and the profile:
    /// the shelf buttons only with a profile open (the app's name without
    /// one), Back when there's somewhere to go back to, the search when it's
    /// open, and nothing but the page in full screen.
    /// </summary>
    private void UpdateChrome()
    {
        var profile = _session?.Profile is not null;
        var fullScreen = IsFullScreen;
        Header.Visibility = fullScreen ? Visibility.Collapsed : Visibility.Visible;
        BackButton.Visibility = ContentFrame.CanGoBack ? Visibility.Visible : Visibility.Collapsed;
        var shelfButtons = profile ? Visibility.Visible : Visibility.Collapsed;
        AddButton.Visibility = shelfButtons;
        Tabs.Visibility = shelfButtons;
        RightButtons.Visibility = shelfButtons;
        AppTitle.Visibility = profile ? Visibility.Collapsed : Visibility.Visible;
        SearchBar.Visibility = profile && !fullScreen && SearchButton.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        PageBackdrop.Visibility = (ContentFrame.Content is ReaderPage or WriterPage) ? Visibility.Visible : Visibility.Collapsed;
        SyncTabs();
    }

    private (ShelfTab Tab, Shelf Shelf)[] TabShelves() =>
        [(ReadingTab, Shelf.Reading), (FinishedTab, Shelf.Finished), (EventuallyTab, Shelf.Eventually)];

    /// <summary>The tab of the shelf the page belongs to: its shelf, or none (Settings, the manual).</summary>
    private void SyncTabs()
    {
        Shelf? open = _session?.Profile is not null && (ContentFrame.Content is ShelfPage or BookPage or ReaderPage or WriterPage)
            ? Session.CurrentShelf
            : null;
        _syncingTabs = true;
        try
        {
            foreach (var (tab, shelf) in TabShelves()) tab.IsChecked = shelf == open;
        }
        finally
        {
            _syncingTabs = false;
        }
    }

    /// <summary>Tooltips with each button's keys, from the core's Windows table.</summary>
    private void SetTips()
    {
        foreach (var (tab, shelf) in TabShelves()) SetTip(tab, ShelfText.Name(shelf), ShelfText.Name(shelf));
        SetTip(AddButton, "Add a book", ShortcutKeys.Titles.AddBook);
        SetTip(SearchButton, "Search your shelves", ShortcutKeys.Titles.Search);
        SetTip(SettingsButton, "Settings", ShortcutKeys.Titles.Settings);
        ToolTipService.SetToolTip(BackButton, "Back (Alt+Left)");
        AutomationProperties.SetAcceleratorKey(BackButton, "Alt+Left");
    }

    private static void SetTip(FrameworkElement element, string what, string title)
    {
        var keys = ShortcutKeys.Labels(title);
        ToolTipService.SetToolTip(element, keys is null ? what : $"{what} ({keys})");
        if (keys is not null) AutomationProperties.SetAcceleratorKey(element, keys);
    }

    private void OnTabsSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (!_compactTabs && e.NewSize.Width > 0) _tabsFullWidth = e.NewSize.Width;
        LayOutHeader();
    }

    /// <summary>
    /// Keeps the window's own buttons clear, and the tabs (or the app's
    /// name) in the middle of the window when there's room: else in the
    /// middle of the room there is, and without the shelves' names when
    /// even that's too narrow.
    /// </summary>
    private void LayOutHeader()
    {
        if (_canClose || Root.XamlRoot is not { } xamlRoot) return;
        var scale = xamlRoot.RasterizationScale;
        var left = AppWindow.TitleBar.LeftInset / scale;
        var right = AppWindow.TitleBar.RightInset / scale;
        if (LeftInsetColumn.Width.Value != left) LeftInsetColumn.Width = new GridLength(left);
        if (RightInsetColumn.Width.Value != right) RightInsetColumn.Width = new GridLength(right);

        var leftSide = left + LeftButtons.Margin.Left + LeftButtons.ActualWidth;
        var rightSide = right + (RightButtons.Visibility == Visibility.Visible ? RightButtons.Margin.Right + RightButtons.ActualWidth : 0);
        var room = Header.ActualWidth - leftSide - rightSide;
        if (Tabs.Visibility == Visibility.Visible && _tabsFullWidth > 0) SetCompactTabs(room < _tabsFullWidth + 24);

        foreach (var middle in new FrameworkElement[] { Tabs, AppTitle })
        {
            var spare = Math.Max(0, room - middle.ActualWidth);
            var shift = Math.Clamp(rightSide - leftSide, -spare, spare);
            var margin = shift >= 0 ? new Thickness(shift, 0, 0, 0) : new Thickness(0, 0, -shift, 0);
            if (!middle.Margin.Equals(margin)) middle.Margin = margin;
        }
    }

    private void SetCompactTabs(bool compact)
    {
        if (compact == _compactTabs) return;
        _compactTabs = compact;
        var names = compact ? Visibility.Collapsed : Visibility.Visible;
        ReadingLabel.Visibility = names;
        FinishedLabel.Visibility = names;
        EventuallyLabel.Visibility = names;
        Tabs.Spacing = compact ? 4 : 6;
        foreach (var (tab, _) in TabShelves()) tab.Padding = compact ? new Thickness(9, 0, 9, 1) : new Thickness(15, 0, 16, 1);
    }

    /// <summary>
    /// The pointer left the header through the part that drags the window,
    /// where its buttons don't hear it go: they stop looking pointed at.
    /// </summary>
    private void OnBodyPointerEntered(object sender, PointerRoutedEventArgs e)
    {
        foreach (var control in new Control[] { BackButton, AddButton, SettingsButton, ReadingTab, FinishedTab, EventuallyTab })
        {
            VisualStateManager.GoToState(control, control.IsEnabled ? "Normal" : "Disabled", true);
        }
        VisualStateManager.GoToState(SearchButton, SearchButton.IsChecked == true ? "Checked" : "Normal", true);
    }

    /// <summary>
    /// The header is the title bar, which drags the window; its buttons
    /// take clicks instead. Checked after each layout, and only passed on
    /// to Windows when something moved.
    /// </summary>
    private void UpdatePassthrough()
    {
        if (_canClose || Root.XamlRoot is not { } xamlRoot || Header.Visibility != Visibility.Visible) return;
        var scale = xamlRoot.RasterizationScale;
        var rects = new List<RectInt32>(3);
        foreach (var element in new FrameworkElement[] { LeftButtons, Tabs, RightButtons })
        {
            if (element.Visibility != Visibility.Visible || element.ActualWidth <= 0) continue;
            var box = element.TransformToVisual(Root).TransformBounds(new Rect(0, 0, element.ActualWidth, element.ActualHeight));
            rects.Add(new RectInt32(
                (int)Math.Floor(box.X * scale),
                (int)Math.Floor(box.Y * scale),
                (int)Math.Ceiling(box.Width * scale),
                (int)Math.Ceiling(box.Height * scale)));
        }
        if (rects.SequenceEqual(_passthrough)) return;
        _passthrough = [.. rects];
        InputNonClientPointerSource.GetForWindowId(AppWindow.Id).SetRegionRects(NonClientRegionKind.Passthrough, _passthrough);
    }

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
        UpdateChrome(); // nothing but the page in full screen
        // Every key out, from the core's table: F11 does something else on many laptops.
        var keys = ShortcutKeys.Labels(ShortcutKeys.Titles.LeaveFullScreen, ShortcutKeys.Titles.FullScreen);
        if (on) ShowNotice($"Press {keys} to leave full screen", InfoBarSeverity.Informational, seconds: 4);
        else Announce("Left full screen");
        FullScreenChanged?.Invoke();
    }

    // ---- profiles ---------------------------------------------------------------

    /// <summary>Opens a profile on its start shelf, in its own look.</summary>
    internal async Task OpenProfileAsync(Profile profile)
    {
        await Session.OpenAsync(profile);
        ApplyLook();
        CloseSearch();
        ShowShelf(Session.CurrentShelf);
    }

    /// <summary>
    /// Who's reading? The profiles page, in the neutral look: where Back
    /// from a shelf goes, and the welcome on the first run.
    /// </summary>
    internal void ShowProfiles()
    {
        ContentFrame.Navigate(typeof(ProfilesPage), null, new EntranceNavigationTransitionInfo());
        ContentFrame.BackStack.Clear();
        ApplyLook();
        UpdateChrome();
    }

    /// <summary>Deletes a profile (already confirmed twice), then shows the profiles.</summary>
    internal async Task DeleteProfileAsync(Profile profile)
    {
        try
        {
            await Session.Journal.DeleteProfileAsync(profile.Id);
            Session.Close();
            CloseSearch();
            ShowProfiles();
            ShowNotice($"Deleted {profile.Name}", InfoBarSeverity.Success);
        }
        catch (CoreException e)
        {
            ShowNotice(e.UserMessage(), InfoBarSeverity.Error, autoHide: false);
        }
    }

    // ---- navigation -------------------------------------------------------------

    /// <summary>
    /// Shows a shelf. Shelves are the top of a profile, so Back goes from
    /// here to the profiles, as in the GTK app.
    /// </summary>
    internal void ShowShelf(Shelf shelf)
    {
        if (_session?.Profile is null) return;
        Session.CurrentShelf = shelf;
        if (ContentFrame.Content is ShelfPage page)
        {
            page.Show();
            // A focused card of the last shelf has gone with it.
            DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () => SettleFocus(page));
        }
        else
        {
            ContentFrame.Navigate(typeof(ShelfPage), null, new EntranceNavigationTransitionInfo());
        }
        ContentFrame.BackStack.Clear();
        ContentFrame.BackStack.Add(new PageStackEntry(typeof(ProfilesPage), null, new EntranceNavigationTransitionInfo()));
        UpdateChrome();
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

    private void OnBack(object sender, RoutedEventArgs e) => GoBack();

    private void OnAddBook(object sender, RoutedEventArgs e) => _ = AddBookAsync();

    private void OnSettings(object sender, RoutedEventArgs e) => OpenSettings();

    /// <summary>A tab clicked (or Space, or Enter): its shelf, even from a book of that shelf.</summary>
    private void OnTabClick(object sender, RoutedEventArgs e) => ShowTabShelf(sender);

    /// <summary>A tab selected some other way (Narrator).</summary>
    private void OnTabChecked(object sender, RoutedEventArgs e)
    {
        if (!_syncingTabs) ShowTabShelf(sender);
    }

    private void ShowTabShelf(object sender)
    {
        if (sender is not ShelfTab { Tag: string tag } || !Enum.TryParse<Shelf>(tag, out var shelf) || _session?.Profile is null) return;
        if (ContentFrame.Content is ShelfPage && Session.CurrentShelf == shelf) return;
        ShowShelf(shelf);
    }

    private void OnNavigated(object sender, NavigationEventArgs e)
    {
        if (e.Content is ProfilesPage && _session?.Profile is not null)
        {
            // Back to Who's reading?: nobody's shelves are open, and the look is neutral.
            _session.Close();
            CloseSearch();
            ApplyLook();
        }
        if (e.Content is not (ReaderPage or WriterPage)) SetFullScreen(false);
        UpdateChrome();
        // The writing and reading pages, Settings and the profiles focus what they need themselves.
        if (e.Content is ShelfPage or BookPage or ManualPage)
        {
            var page = (Page)e.Content;
            DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () => SettleFocus(page));
        }
    }

    /// <summary>
    /// A page that's just opened takes the focus (its first card or button),
    /// which would otherwise be lost with the last page, or fall back to the
    /// header's Back button. Typing on a shelf then searches it.
    /// </summary>
    private void SettleFocus(Page page)
    {
        if (ContentFrame.Content != page || Root.XamlRoot is not { } root) return;
        var focused = FocusManager.GetFocusedElement(root);
        if (focused is not null && !ReferenceEquals(focused, BackButton)) return;
        if (FocusManager.FindFirstFocusableElement(page) is Control first) first.Focus(FocusState.Programmatic);
    }

    private void OnNavigationFailed(object sender, NavigationFailedEventArgs e)
    {
        StartupLog.Step($"Couldn't open {e.SourcePageType?.Name}: {e.Exception?.Message}");
        e.Handled = true;
    }

    // ---- search -----------------------------------------------------------------

    private void OnSearchToggled(object sender, RoutedEventArgs e)
    {
        var open = SearchButton.IsChecked == true;
        // Closing the search shows everything again.
        if (!open && SearchBox.Text.Length > 0) SearchBox.Text = "";
        UpdateChrome();
        if (open) FocusSearchBox(FocusState.Programmatic);
    }

    private void CloseSearch()
    {
        SearchButton.IsChecked = false;
        SearchBox.Text = "";
        _session?.SetSearch("");
    }

    private void OnSearchTextChanged(object sender, TextChangedEventArgs e)
    {
        if (_session?.Profile is null) return;
        Session.SetSearch(SearchBox.Text);
        if (Session.IsSearching && ContentFrame.Content is not ShelfPage) ShowShelf(Session.CurrentShelf);
    }

    private void OnSearchKeyDown(object sender, KeyRoutedEventArgs e)
    {
        switch (e.Key)
        {
            case VirtualKey.Escape:
                // Esc clears the search, then closes it.
                e.Handled = true;
                if (SearchBox.Text.Length > 0)
                {
                    SearchBox.Text = "";
                }
                else
                {
                    SearchButton.IsChecked = false;
                    ContentFrame.Focus(FocusState.Programmatic);
                }
                break;
            case VirtualKey.Enter when _session?.Profile is not null && Session.IsSearching:
                // Nothing here but something on another shelf: go there.
                e.Handled = true;
                if (Session.MatchCount(Session.CurrentShelf) == 0
                    && ShelfText.All.FirstOrDefault(s => Session.MatchCount(s) > 0) is var other
                    && Session.MatchCount(other) > 0)
                {
                    ShowShelf(other);
                }
                break;
        }
    }

    private void FocusSearch()
    {
        if (_session?.Profile is null || IsFullScreen) return;
        SearchButton.IsChecked = true;
        FocusSearchBox(FocusState.Keyboard);
    }

    /// <summary>Focuses the search field, once it's laid out if it was only just opened.</summary>
    private void FocusSearchBox(FocusState how)
    {
        if (!SearchBox.Focus(how)) DispatcherQueue.TryEnqueue(() => SearchBox.Focus(how));
    }

    /// <summary>Starts a search with a character typed on the shelf page.</summary>
    internal void StartSearch(char c)
    {
        if (_session?.Profile is null) return;
        SearchButton.IsChecked = true;
        SearchBox.Text += c;
        SearchBox.SelectionStart = SearchBox.Text.Length;
        FocusSearchBox(FocusState.Keyboard);
    }

    /// <summary>
    /// Gives a search box's own text box the search box's name, if its
    /// template didn't, so Narrator never lands in a nameless field.
    /// </summary>
    internal static void NameTextBox(AutoSuggestBox search)
    {
        if (FindChild<TextBox>(search) is { } box && string.IsNullOrEmpty(AutomationProperties.GetName(box)))
        {
            AutomationProperties.SetName(box, AutomationProperties.GetName(search));
        }
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
        // None of these mean anything to a text box, so they work while typing too.
        Keys.AddGlobal(ShortcutKeys.Titles.Reading, () => ShowShelf(Shelf.Reading));
        Keys.AddGlobal(ShortcutKeys.Titles.Finished, () => ShowShelf(Shelf.Finished));
        Keys.AddGlobal(ShortcutKeys.Titles.Eventually, () => ShowShelf(Shelf.Eventually));
        Keys.AddGlobal(ShortcutKeys.Titles.AddBook, () => _ = AddBookAsync());
        Keys.AddGlobal(ShortcutKeys.Titles.Search, FocusSearch);
        Keys.AddGlobal(ShortcutKeys.Titles.Settings, OpenSettings);
        Keys.AddGlobal(ShortcutKeys.Titles.Manual, OpenManual);
        Keys.AddGlobal(ShortcutKeys.Titles.Shortcuts, () => _ = ShowShortcutsAsync());
        // Windows' own Back keys.
        Keys.AddGlobal(VirtualKey.Left, VirtualKeyModifiers.Menu, GoBack);
        Keys.AddGlobal(VirtualKey.GoBack, VirtualKeyModifiers.None, GoBack);
        Keys.Attach(Root);
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
            var button = new Button { Content = actionLabel, Style = (Style)Application.Current.Resources["PillButtonStyle"] };
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
        ScreenReader.Open(Notice);
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
