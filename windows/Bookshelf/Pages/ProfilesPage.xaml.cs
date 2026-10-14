using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace Bookshelf.Pages;

/// <summary>A profile in the list on the profiles page.</summary>
public sealed class ProfileRow(Profile profile, bool first)
{
    public string Id { get; } = profile.Id;
    public string Name { get; } = profile.Name;
    public Visibility RuleVisibility { get; } = first ? Visibility.Collapsed : Visibility.Visible;
}

/// <summary>
/// Who's reading? Every profile, to open one, and a new one: a name, and an
/// optional email that only goes to Open Library. Back from a shelf comes
/// here, as in the GTK app; on the first run it's the welcome.
/// </summary>
public sealed partial class ProfilesPage : BookshelfPage
{
    public ProfilesPage()
    {
        InitializeComponent();
        EmailWhy.Text = SettingsPage.WhyEmail;
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        Heading.FontFamily = Look.Heading;
        NewHeading.FontFamily = Look.Heading;
        IReadOnlyList<Profile> profiles;
        try
        {
            profiles = await Session.LoadProfilesAsync();
        }
        catch (CoreException ex)
        {
            // Not "Welcome": that would suggest the journal is empty.
            Shell.ShowNotice($"Couldn't read the profiles: {ex.UserMessage()}", InfoBarSeverity.Error, autoHide: false);
            return;
        }
        var any = profiles.Count > 0;
        // The first run reads as a welcome, not a "who's back?".
        Heading.Text = any ? "Who's reading?" : "Welcome";
        Lede.Text = any ? "Pick up where you left off." : "Make a profile to start your reading journal.";
        NewHeading.Text = any ? "New profile" : "Your profile";
        ProfilesList.ItemsSource = profiles.Select((p, i) => new ProfileRow(p, i == 0)).ToList();
        ProfilesPanel.Visibility = any ? Visibility.Visible : Visibility.Collapsed;
        if (!any) NameBox.Focus(FocusState.Programmatic);
    }

    private async void OnProfileClick(object sender, RoutedEventArgs e)
    {
        if (sender is not FrameworkElement { Tag: string id } || Session.Profiles.FirstOrDefault(p => p.Id == id) is not { } profile) return;
        try
        {
            await Shell.OpenProfileAsync(profile);
        }
        catch (CoreException ex)
        {
            Shell.ShowNotice($"Couldn't open {profile.Name}: {ex.UserMessage()}", InfoBarSeverity.Error, autoHide: false);
        }
    }

    private void OnNameChanged(object sender, TextChangedEventArgs e)
    {
        CreateButton.IsEnabled = NameBox.Text.Trim().Length > 0;
        Error.Visibility = Visibility.Collapsed;
    }

    private void OnEmailChanged(object sender, TextChangedEventArgs e) => Error.Visibility = Visibility.Collapsed;

    private void OnKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter && CreateButton.IsEnabled)
        {
            e.Handled = true;
            _ = CreateAsync();
        }
    }

    private void OnCreate(object sender, RoutedEventArgs e) => _ = CreateAsync();

    private async Task CreateAsync()
    {
        CreateButton.IsEnabled = false;
        try
        {
            // The email is checked first, so a bad one doesn't leave a half-made profile.
            var email = BookshelfFfiMethods.ValidateEmail(EmailBox.Text);
            var profile = await Session.Journal.CreateProfileAsync(NameBox.Text.Trim(), email);
            await Session.LoadProfilesAsync();
            await Shell.OpenProfileAsync(profile);
        }
        catch (CoreException ex)
        {
            Error.Text = ex.UserMessage();
            Error.Visibility = Visibility.Visible;
            ScreenReader.LiveRegionChanged(Error);
            CreateButton.IsEnabled = NameBox.Text.Trim().Length > 0;
        }
    }
}
