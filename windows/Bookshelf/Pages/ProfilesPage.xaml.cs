using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace Bookshelf.Pages;

/// <summary>
/// Making a profile: the welcome on first run, or New profile from the
/// profile menu. A name, and an optional email that only goes to Open Library.
/// </summary>
public sealed partial class WelcomePage : BookshelfPage
{
    private bool _firstRun;

    public WelcomePage()
    {
        InitializeComponent();
        EmailWhy.Text = SettingsPage.WhyEmail;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        _firstRun = e.Parameter is true;
        Heading.Text = _firstRun ? "Welcome" : "New profile";
        Heading.FontFamily = Look.Heading;
        Lede.Text = _firstRun
            ? "Make a profile to start your reading journal."
            : "Each person gets their own shelves, summaries and settings.";
        CancelButton.Visibility = _firstRun ? Visibility.Collapsed : Visibility.Visible;
        NameBox.Focus(FocusState.Programmatic);
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

    private void OnCancel(object sender, RoutedEventArgs e)
    {
        if (Frame.CanGoBack) Frame.GoBack();
    }

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
