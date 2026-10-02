using Bookshelf.Core;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Dialogs;

/// <summary>Every keyboard shortcut (Ctrl+?), from the core's Windows table, with the keys drawn as keys.</summary>
internal static class ShortcutsDialog
{
    public static async Task ShowAsync()
    {
        var list = new StackPanel { Spacing = 2, Padding = new Thickness(0, 0, 16, 0) };
        foreach (var group in BookshelfFfiMethods.Shortcuts(Platform.Windows))
        {
            var heading = new TextBlock
            {
                Text = group.Title,
                Style = Resource<Style>("BodyStrongTextBlockStyle"),
                Margin = new Thickness(0, list.Children.Count == 0 ? 0 : 16, 0, 4),
            };
            AutomationProperties.SetHeadingLevel(heading, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
            list.Children.Add(heading);
            foreach (var shortcut in group.Items)
            {
                if (shortcut.Accel is not Accel.Keys { Combo: var combo }) continue;
                list.Children.Add(Row(shortcut.Title, combo));
            }
        }

        var dialog = new ContentDialog
        {
            Title = "Keyboard shortcuts",
            Content = new ScrollViewer { Content = list, MaxHeight = 520 },
            CloseButtonText = "Close",
            DefaultButton = ContentDialogButton.Close,
        };
        await DialogHost.ShowAsync(dialog);
    }

    private static Grid Row(string title, KeyCombo combo)
    {
        var row = new Grid { ColumnSpacing = 24, MinHeight = 32 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(new TextBlock { Text = title, VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.Wrap });

        var keys = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4, VerticalAlignment = VerticalAlignment.Center };
        foreach (var key in ShortcutKeys.Keys(combo))
        {
            keys.Children.Add(new Border
            {
                Style = Resource<Style>("KeyCapStyle"),
                Child = new TextBlock
                {
                    Text = key,
                    Style = Resource<Style>("CaptionTextBlockStyle"),
                    HorizontalAlignment = HorizontalAlignment.Center,
                },
            });
        }
        Grid.SetColumn(keys, 1);
        row.Children.Add(keys);
        AutomationProperties.SetName(row, $"{title}: {ShortcutKeys.Label(combo)}");
        return row;
    }

    private static T Resource<T>(string key) => (T)Application.Current.Resources[key];
}
