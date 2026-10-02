using Bookshelf.Controls;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Bookshelf.Pages;

/// <summary>A line of the manual's contents.</summary>
public sealed class ContentsItem(ManualEntry entry)
{
    public string Title { get; } = entry.Title;
    public string Anchor { get; } = entry.Anchor;
    public Thickness Indent { get; } = new(entry.Nested ? 16 : 0, 0, 0, 0);

    public override string ToString() => Title;
}

/// <summary>The user manual (F1), from the same MANUAL.md as every app, with contents that jump to each section.</summary>
public sealed partial class ManualPage : BookshelfPage
{
    private readonly Dictionary<string, FrameworkElement> _sections = new();
    private bool _loaded;

    public ManualPage()
    {
        InitializeComponent();
    }

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (_loaded) return;
        _loaded = true;
        var (manual, intro, sections) = await Task.Run(() =>
        {
            var m = BookshelfFfiMethods.ManualFor(Platform.Windows); // Windows' keys, menus and folders
            var laidOut = m.Sections.Select(s => (s.Anchor, Blocks: BookshelfFfiMethods.RenderMarkdown(s.Markdown))).ToList();
            return (m, BookshelfFfiMethods.RenderMarkdown(m.Intro), laidOut);
        });
        Add(intro, null);
        foreach (var (anchor, blocks) in sections) Add(blocks, anchor);
        Contents.ItemsSource = manual.Contents.Select(c => new ContentsItem(c)).ToList();
    }

    private void Add(Block[] blocks, string? anchor)
    {
        var text = new RichTextBlock { IsTextSelectionEnabled = true, Margin = new Thickness(0, 0, 0, 24) };
        MarkdownRenderer.Render(text, blocks, 15, Look.Sans, Look.SansDisplay);
        if (anchor is not null)
        {
            _sections[anchor] = text;
            AutomationProperties.SetAutomationId(text, anchor);
        }
        Sections.Children.Add(text);
    }

    private void OnContentsClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is ContentsItem item && _sections.TryGetValue(item.Anchor, out var section))
        {
            section.StartBringIntoView(new BringIntoViewOptions
            {
                VerticalAlignmentRatio = 0,
                VerticalOffset = -16,
                AnimationDesired = true,
            });
        }
    }
}
