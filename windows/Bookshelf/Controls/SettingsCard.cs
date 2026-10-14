using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Controls;

/// <summary>
/// One row of the settings page, as in the GTK app's settings lists: a
/// title and a line of explanation on the left, the control on the right.
/// (A small stand-in for the Community Toolkit's SettingsCard, to keep
/// the app free of extra packages.) The look is in Themes/Generic.xaml.
/// </summary>
public sealed class SettingsCard : ContentControl
{
    public static readonly DependencyProperty HeaderProperty = DependencyProperty.Register(
        nameof(Header), typeof(string), typeof(SettingsCard), new PropertyMetadata("", OnTextChanged));

    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(
        nameof(Description), typeof(string), typeof(SettingsCard), new PropertyMetadata("", OnTextChanged));

    private TextBlock? _description;

    public SettingsCard()
    {
        DefaultStyleKey = typeof(SettingsCard);
    }

    /// <summary>What the setting is ("Theme").</summary>
    public string Header
    {
        get => (string)GetValue(HeaderProperty);
        set => SetValue(HeaderProperty, value);
    }

    /// <summary>A line of explanation under the header; hidden when empty.</summary>
    public string Description
    {
        get => (string)GetValue(DescriptionProperty);
        set => SetValue(DescriptionProperty, value);
    }

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        _description = GetTemplateChild("PART_Description") as TextBlock;
        Update();
    }

    private static void OnTextChanged(DependencyObject d, DependencyPropertyChangedEventArgs e) => ((SettingsCard)d).Update();

    private void Update()
    {
        if (_description is not null)
        {
            _description.Visibility = string.IsNullOrEmpty(Description) ? Visibility.Collapsed : Visibility.Visible;
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(this, Header);
    }
}
