using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;
using Microsoft.Windows.Storage.Pickers;
using Windows.System;

namespace Bookshelf.Pages;

/// <summary>
/// The open profile's settings, in Windows Settings style. Every change is
/// saved and applied at once.
/// </summary>
public sealed partial class SettingsPage : BookshelfPage
{
    /// <summary>Why the email is asked for, as the GTK app and the manual say it.</summary>
    internal const string WhyEmail =
        "Optional. Book details and covers come from Open Library, a free public library service run by the " +
        "Internet Archive. If you add an email, it's sent along with your book searches and cover downloads so " +
        "they can contact you if there's ever a problem. It goes nowhere else. Leave it blank to send nothing.";

    private bool _loading;
    private string? _backupFolder;

    public SettingsPage()
    {
        InitializeComponent();
        EmailWhy.Text = WhyEmail;
        AutomationProperties.SetHelpText(EmailBox, WhyEmail);
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        Load();
        _ = ShowBackupsAsync();
    }

    private void Load()
    {
        if (Session.Profile is not { } profile || Session.Settings is not { } s) return;
        _loading = true;
        try
        {
            NameBox.Text = profile.Name;
            EmailBox.Text = profile.Email ?? "";
            SaveName.IsEnabled = false;
            SaveEmail.IsEnabled = false;
            ThemeBox.SelectedIndex = s.Theme switch { Theme.Light => 1, Theme.Dark => 2, _ => 0 };
            HeadingFontBox.SelectedIndex = s.HeadingFont == HeadingFont.Sans ? 1 : 0;
            WritingFontBox.SelectedIndex = s.WritingFont switch
            {
                WritingFont.Serif => 1,
                WritingFont.Sans => 2,
                WritingFont.Mono => 3,
                _ => 0,
            };
            SizeBox.Value = s.WritingSize;
            SpacingBox.SelectedIndex = s.LineSpacing switch { LineSpacing.Tight => 0, LineSpacing.Airy => 2, _ => 1 };
            WidthBox.SelectedIndex = s.PageWidth switch { PageWidth.Narrow => 0, PageWidth.Wide => 2, _ => 1 };
            FocusSwitch.IsOn = s.FocusDefault;
            DateFormatBox.SelectedIndex = s.DateFormat switch
            {
                DateFormat.MonthDayYear => 1,
                DateFormat.DayMonthYear => 2,
                DateFormat.YearMonthDay => 3,
                _ => 0,
            };
            WeekBox.SelectedIndex = s.WeekStart == WeekStart.Monday ? 1 : 0;
            StartShelfBox.SelectedIndex = s.StartShelf switch { Shelf.Finished => 1, Shelf.Eventually => 2, _ => 0 };
            ShowSwatches(s.Accent);
            AccentPicker.Color = Look.ParseColor(s.Accent) ?? AccentPicker.Color;
            AboutCard.Description =
                $"Rust core {BookshelfFfiMethods.CoreVersion()}. Your journal is kept in {Session.Journal.DataDirectory}";
            ShowPreview();
        }
        finally
        {
            _loading = false;
        }
    }

    /// <summary>Saves a change to the settings and applies it.</summary>
    private async Task ChangeAsync(Func<ProfileSettings, ProfileSettings> change)
    {
        if (_loading || Session.Settings is not { } current) return;
        var next = change(current);
        if (next == current) return;
        try
        {
            await Session.UpdateSettingsAsync(next);
            Shell.ApplyLook();
            ShowPreview();
            Problem.IsOpen = false;
        }
        catch (CoreException ex)
        {
            ShowProblem("Couldn't save that setting", ex.UserMessage());
        }
    }

    private void ShowPreview()
    {
        if (Session.Settings is not { } s) return;
        var layout = BookshelfFfiMethods.WriterLayout(s);
        WritingPreview.FontFamily = Look.Writing;
        WritingPreview.FontSize = layout.FontPx;
        WritingPreview.LineHeight = layout.FontPx + layout.WrapGap;
    }

    private void ShowProblem(string title, string message)
    {
        Problem.Title = title;
        Problem.Message = message;
        Problem.IsOpen = true;
    }

    // ---- profile ----------------------------------------------------------------

    private void OnNameChanged(object sender, TextChangedEventArgs e)
    {
        if (_loading) return;
        SaveName.IsEnabled = NameBox.Text.Trim().Length > 0 && NameBox.Text.Trim() != Session.Profile?.Name;
        ProfileError.Visibility = Visibility.Collapsed;
    }

    private void OnEmailChanged(object sender, TextChangedEventArgs e)
    {
        if (_loading) return;
        SaveEmail.IsEnabled = EmailBox.Text.Trim() != (Session.Profile?.Email ?? "");
        ProfileError.Visibility = Visibility.Collapsed;
    }

    private void OnNameKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter && SaveName.IsEnabled) _ = SaveNameAsync();
    }

    private void OnEmailKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter && SaveEmail.IsEnabled) _ = SaveEmailAsync();
    }

    private void OnSaveName(object sender, RoutedEventArgs e) => _ = SaveNameAsync();

    private void OnSaveEmail(object sender, RoutedEventArgs e) => _ = SaveEmailAsync();

    private async Task SaveNameAsync()
    {
        var name = NameBox.Text;
        try
        {
            await Session.UpdateProfileAsync((journal, id) => journal.RenameProfileAsync(id, name));
            SaveName.IsEnabled = false;
            Shell.ShowNotice("Name saved", InfoBarSeverity.Success);
        }
        catch (CoreException ex)
        {
            ShowProfileError(ex.UserMessage());
        }
    }

    private async Task SaveEmailAsync()
    {
        try
        {
            var email = BookshelfFfiMethods.ValidateEmail(EmailBox.Text);
            await Session.UpdateProfileAsync((journal, id) => journal.SetProfileEmailAsync(id, email));
            _loading = true;
            EmailBox.Text = email ?? "";
            _loading = false;
            SaveEmail.IsEnabled = false;
            Shell.ShowNotice(email is null ? "Email removed" : "Email saved", InfoBarSeverity.Success);
        }
        catch (CoreException ex)
        {
            ShowProfileError(ex.UserMessage());
        }
    }

    private void ShowProfileError(string message)
    {
        ProfileError.Text = message;
        ProfileError.Visibility = Visibility.Visible;
    }

    private async void OnDeleteProfile(object sender, RoutedEventArgs e)
    {
        if (Session.Profile is not { } profile) return;
        var name = profile.Name;
        // Two steps, the second needing the name typed, so a stray click can't delete a journal.
        var first = new ContentDialog
        {
            Title = $"Delete {name}?",
            Content = new TextBlock
            {
                Text = "Their summaries, dates and settings will be deleted. The books themselves stay.\n\n" +
                    "Export first to keep a copy of everything they wrote.",
                TextWrapping = TextWrapping.Wrap,
            },
            PrimaryButtonText = "Delete",
            SecondaryButtonText = "Export first…",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        var answer = await DialogHost.ShowAsync(first);
        if (answer == ContentDialogResult.Secondary)
        {
            await ExportAsync();
            return;
        }
        if (answer != ContentDialogResult.Primary) return;

        var typed = new TextBox { PlaceholderText = name };
        AutomationProperties.SetName(typed, "Profile name");
        var second = new ContentDialog
        {
            Title = $"Delete {name} for good?",
            Content = new StackPanel
            {
                Spacing = 12,
                Children =
                {
                    new TextBlock
                    {
                        Text = $"This can't be undone. All of {name}'s summaries and dates will be gone.\n\nType “{name}” to confirm.",
                        TextWrapping = TextWrapping.Wrap,
                    },
                    typed,
                },
            },
            PrimaryButtonText = "Delete Forever",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
            IsPrimaryButtonEnabled = false,
        };
        typed.TextChanged += (_, _) => second.IsPrimaryButtonEnabled = typed.Text.Trim() == name.Trim();
        second.Opened += (_, _) => typed.Focus(FocusState.Programmatic);
        if (await DialogHost.ShowAsync(second) != ContentDialogResult.Primary) return;
        await Shell.DeleteProfileAsync(profile);
    }

    // ---- appearance -------------------------------------------------------------

    private void OnThemeChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with
        {
            Theme = ThemeBox.SelectedIndex switch { 1 => Theme.Light, 2 => Theme.Dark, _ => Theme.System },
        });

    private void OnHeadingFontChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with { HeadingFont = HeadingFontBox.SelectedIndex == 1 ? HeadingFont.Sans : HeadingFont.Serif });

    /// <summary>The eight accents as round swatches, the chosen one ticked.</summary>
    private void ShowSwatches(string current)
    {
        Swatches.Children.Clear();
        foreach (var accent in BookshelfFfiMethods.Accents())
        {
            if (Look.ParseColor(accent.Hex) is not { } color) continue;
            var chosen = string.Equals(accent.Hex, current, StringComparison.OrdinalIgnoreCase);
            var tick = Look.ParseColor(BookshelfFfiMethods.AccentColors(accent.Hex, false).Fg) ?? color;
            var swatch = new Grid { Width = 28, Height = 28 };
            swatch.Children.Add(new Ellipse { Fill = new SolidColorBrush(color) });
            if (chosen)
            {
                swatch.Children.Add(new FontIcon
                {
                    Glyph = "",
                    FontSize = 14,
                    Foreground = new SolidColorBrush(tick),
                });
            }
            var button = new Button
            {
                Content = swatch,
                Padding = new Thickness(3),
                CornerRadius = new CornerRadius(17),
                Style = (Style)Application.Current.Resources["SubtleButtonStyle"],
            };
            AutomationProperties.SetName(button, chosen ? $"{accent.Name}, chosen" : accent.Name);
            ToolTipService.SetToolTip(button, accent.Name);
            var hex = accent.Hex;
            button.Click += (_, _) => _ = ChooseAccentAsync(hex);
            Swatches.Children.Add(button);
        }
    }

    private async Task ChooseAccentAsync(string hex)
    {
        await ChangeAsync(s => s with { Accent = hex });
        if (Session.Settings is { } s) ShowSwatches(s.Accent);
    }

    private void OnUseCustomAccent(object sender, RoutedEventArgs e)
    {
        CustomFlyout.Hide();
        _ = ChooseAccentAsync(Look.ToHex(AccentPicker.Color));
    }

    // ---- writing ----------------------------------------------------------------

    private void OnWritingFontChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with
        {
            WritingFont = WritingFontBox.SelectedIndex switch
            {
                1 => WritingFont.Serif,
                2 => WritingFont.Sans,
                3 => WritingFont.Mono,
                _ => WritingFont.IaDuo,
            },
        });

    private void OnSizeChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        if (double.IsNaN(args.NewValue)) return;
        var size = (uint)Math.Clamp(Math.Round(args.NewValue), 10, 28);
        _ = ChangeAsync(s => s with { WritingSize = size });
    }

    private void OnSpacingChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with
        {
            LineSpacing = SpacingBox.SelectedIndex switch { 0 => LineSpacing.Tight, 2 => LineSpacing.Airy, _ => LineSpacing.Normal },
        });

    private void OnWidthChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with
        {
            PageWidth = WidthBox.SelectedIndex switch { 0 => PageWidth.Narrow, 2 => PageWidth.Wide, _ => PageWidth.Medium },
        });

    private void OnFocusToggled(object sender, RoutedEventArgs e) =>
        _ = ChangeAsync(s => s with { FocusDefault = FocusSwitch.IsOn });

    // ---- reading log ------------------------------------------------------------

    private void OnDateFormatChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with
        {
            DateFormat = DateFormatBox.SelectedIndex switch
            {
                1 => DateFormat.MonthDayYear,
                2 => DateFormat.DayMonthYear,
                3 => DateFormat.YearMonthDay,
                _ => DateFormat.Long,
            },
        });

    private void OnWeekChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with { WeekStart = WeekBox.SelectedIndex == 1 ? WeekStart.Monday : WeekStart.Sunday });

    private void OnStartShelfChanged(object sender, SelectionChangedEventArgs e) =>
        _ = ChangeAsync(s => s with
        {
            StartShelf = StartShelfBox.SelectedIndex switch { 1 => Shelf.Finished, 2 => Shelf.Eventually, _ => Shelf.Reading },
        });

    // ---- your data --------------------------------------------------------------

    private void OnExport(object sender, RoutedEventArgs e) => _ = ExportAsync();

    private async Task ExportAsync()
    {
        if (Session.Profile is not { } profile) return;
        var folder = await PickFolderAsync("Export here");
        if (folder is null) return;
        try
        {
            ExportButton.IsEnabled = false;
            var result = await Session.Journal.ExportMarkdownAsync(profile.Id, folder);
            var what = result.Count == 1 ? "1 summary" : $"{result.Count:N0} summaries";
            Shell.ShowNotice($"Exported {what} to {result.Folder}", InfoBarSeverity.Success, "Open folder",
                () => Shell.OpenFolder(result.Folder));
        }
        catch (CoreException ex)
        {
            ShowProblem("Export failed", ex.UserMessage());
        }
        finally
        {
            ExportButton.IsEnabled = true;
        }
    }

    /// <summary>Asks for a folder; null if the person cancelled.</summary>
    private static async Task<string?> PickFolderAsync(string commit)
    {
        var picker = new FolderPicker(Shell.AppWindow.Id)
        {
            CommitButtonText = commit,
            SuggestedStartLocation = PickerLocationId.DocumentsLibrary,
        };
        var picked = await picker.PickSingleFolderAsync();
        return picked?.Path;
    }

    private async Task ShowBackupsAsync()
    {
        try
        {
            var status = await Session.Journal.BackupStatusAsync();
            _backupFolder = status.Available ? status.Folder : null;
            ResetBackups.Visibility = status.IsCustom ? Visibility.Visible : Visibility.Collapsed;
            OpenBackups.IsEnabled = status.Available;
            string text;
            if (!status.Available)
            {
                text = $"Saved to {status.Folder}\nThat folder isn't available right now (is the drive plugged in?). " +
                    "Backups are paused until it's back, or until you choose another folder.";
            }
            else
            {
                var latest = JournalDate.Parse(status.Latest) is { } day
                    ? $"Latest: {ReadingDates.Format(day, Session.Settings?.DateFormat ?? DateFormat.Long)}"
                    : "The first one is made the next time Bookshelf starts.";
                text = $"Saved to {status.Folder}\nOne copy a day of every profile; the last {status.Keep} are kept. {latest}";
            }
            BackupsCard.Description = text;
            ToolTipService.SetToolTip(OpenBackups,
                "To restore a backup: quit Bookshelf, delete bookshelf.sqlite3-wal and bookshelf.sqlite3-shm from " +
                $"{Session.Journal.DataDirectory} if they're there, then copy the backup over bookshelf.sqlite3 in that same folder.");
        }
        catch (CoreException ex)
        {
            BackupsCard.Description = $"Couldn't check the backups: {ex.UserMessage()}";
        }
    }

    private async void OnChangeBackups(object sender, RoutedEventArgs e)
    {
        var folder = await PickFolderAsync("Keep backups here");
        if (folder is null) return;
        await MoveBackupsAsync(folder, $"Backups now go to {folder}");
    }

    private void OnResetBackups(object sender, RoutedEventArgs e) =>
        _ = MoveBackupsAsync(null, "Backups are back in the default folder");

    /// <summary>Keeps backups in <paramref name="folder"/> (null: the default) and makes one there now.</summary>
    private async Task MoveBackupsAsync(string? folder, string done)
    {
        try
        {
            await Session.Journal.SetBackupFolderAsync(folder);
            await Session.Journal.BackUpNowAsync();
            Shell.ShowNotice(done, InfoBarSeverity.Success);
        }
        catch (CoreException ex)
        {
            ShowProblem("Couldn't make a backup there", ex.UserMessage());
        }
        await ShowBackupsAsync();
    }

    private void OnOpenBackups(object sender, RoutedEventArgs e)
    {
        if (_backupFolder is { } folder) _ = Shell.OpenFolder(folder);
    }

    private void OnOpenDataFolder(object sender, RoutedEventArgs e) => _ = Shell.OpenFolder(Session.Journal.DataDirectory);

    // ---- help -------------------------------------------------------------------

    private void OnShortcuts(object sender, RoutedEventArgs e) => _ = Shell.ShowShortcutsAsync();

    private void OnManual(object sender, RoutedEventArgs e) => Shell.OpenManual();
}
