using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Controls;

/// <summary>
/// One row of the settings page in the style of Windows Settings: an icon,
/// a title and a line of explanation on the left, the control on the right.
/// (A small stand-in for the Community Toolkit's SettingsCard, to keep
/// the app free of extra packages.) The look is in Themes/Generic.xaml.
/// </summary>
public sealed class SettingsCard : ContentControl
{
    public static readonly DependencyProperty HeaderProperty = DependencyProperty.Register(
        nameof(Header), typeof(string), typeof(SettingsCard), new PropertyMetadata("", OnTextChanged));

    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(
        nameof(Description), typeof(string), typeof(SettingsCard), new PropertyMetadata("", OnTextChanged));

    public static readonly DependencyProperty GlyphProperty = DependencyProperty.Register(
        nameof(Glyph), typeof(string), typeof(SettingsCard), new PropertyMetadata("", OnTextChanged));

    private TextBlock? _description;
    private FontIcon? _icon;

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

    /// <summary>A Segoe Fluent Icons glyph; no icon when empty.</summary>
    public string Glyph
    {
        get => (string)GetValue(GlyphProperty);
        set => SetValue(GlyphProperty, value);
    }

    protected override void OnApplyTemplate()
    {
        base.OnApplyTemplate();
        _description = GetTemplateChild("PART_Description") as TextBlock;
        _icon = GetTemplateChild("PART_Icon") as FontIcon;
        Update();
    }

    private static void OnTextChanged(DependencyObject d, DependencyPropertyChangedEventArgs e) => ((SettingsCard)d).Update();

    private void Update()
    {
        if (_description is not null)
        {
            _description.Visibility = string.IsNullOrEmpty(Description) ? Visibility.Collapsed : Visibility.Visible;
        }
        if (_icon is not null)
        {
            _icon.Visibility = string.IsNullOrEmpty(Glyph) ? Visibility.Collapsed : Visibility.Visible;
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(this, Header);
    }
}
