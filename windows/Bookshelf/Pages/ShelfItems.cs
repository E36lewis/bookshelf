using System.ComponentModel;
using System.Globalization;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Bookshelf.Pages;

/// <summary>One book on a shelf, as the shelf page's card shows it.</summary>
public sealed class EntryItem : INotifyPropertyChanged
{
    /// <summary>Card covers are 104 by 156 (2:3).</summary>
    public const int CoverWidth = 104;

    private readonly string? _coverPath;
    private ImageSource? _cover;
    private bool _coverAsked;

    public EntryItem(ShelfEntry entry)
    {
        SummaryId = entry.SummaryId;
        Title = entry.Title;
        Author = entry.Author ?? "";
        Meta = entry.Meta;
        MetaCaps = entry.Meta.ToUpper(CultureInfo.CurrentCulture);
        Excerpt = entry.Excerpt;
        EmptyNote = entry.Excerpt.Length == 0 ? entry.EmptyNote ?? "" : "";
        _coverPath = entry.CoverPath;
        Initial = Title.TrimStart().Length > 0 ? char.ToUpperInvariant(Title.TrimStart()[0]).ToString() : "";
        // "Dune. By Frank Herbert. Finished Sep 24, 2026 · 14 days. The spice…", as in the GTK app.
        var by = Author.Length > 0 ? $"By {Author}" : "";
        AutomationName = string.Join(". ", new[] { Title, by, Meta, Excerpt.Length > 0 ? Excerpt : EmptyNote }
            .Select(s => s.TrimEnd('.', ' ')).Where(s => s.Length > 0));
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    public string SummaryId { get; }
    public string Title { get; }
    public string Author { get; }
    public string Meta { get; }

    /// <summary>The dates in small capitals (Narrator reads <see cref="Meta"/>, in the card's name).</summary>
    public string MetaCaps { get; }

    public string Excerpt { get; }
    public string EmptyNote { get; }

    /// <summary>The title's first letter, on the stand-in cover.</summary>
    public string Initial { get; }

    /// <summary>What Narrator reads for the card.</summary>
    public string AutomationName { get; }

    /// <summary>Where the card is among the shelf's books ("3 of 12"), from 1.</summary>
    public int Position { get; set; }

    /// <summary>How many books the shelf shows.</summary>
    public int SetSize { get; set; }

    public FontFamily HeadingFont => Look.Heading;
    public Visibility AuthorVisibility => Visible(Author);
    public Visibility MetaVisibility => Visible(Meta);
    public Visibility ExcerptVisibility => Visible(Excerpt);
    public Visibility EmptyNoteVisibility => Visible(EmptyNote);

    /// <summary>
    /// The cover, once it's loaded; null shows the stand-in. Asking for it
    /// starts loading it, so only the cards on screen (and the grid's few
    /// ahead) read theirs.
    /// </summary>
    public ImageSource? Cover
    {
        get
        {
            if (!_coverAsked)
            {
                _coverAsked = true;
                LoadCover();
            }
            return _cover;
        }
    }

    public Visibility PlaceholderVisibility => _cover is null ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Loads the cover in the background (from the cache when it can).</summary>
    private async void LoadCover()
    {
        if (_coverPath is null) return;
        var cover = await Covers.LoadAsync(_coverPath, CoverWidth);
        if (cover is null) return;
        _cover = cover;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Cover)));
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(PlaceholderVisibility)));
    }

    /// <summary>What Narrator reads for the card.</summary>
    public override string ToString() => AutomationName;

    private static Visibility Visible(string text) => text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
}

/// <summary>The Finished shelf's books of one year, under "2026 · 12 books".</summary>
public sealed class YearRow(int year, string label)
{
    public string YearText { get; } = year.ToString(CultureInfo.InvariantCulture);
    public string Label { get; } = label;
    public FontFamily HeadingFont => Look.Heading;

    public override string ToString() => $"{YearText}, {Label}";
}

/// <summary>One row of the shelf's grid: up to three cards.</summary>
public sealed class CardRow(List<EntryItem> cards)
{
    public List<EntryItem> Cards { get; } = cards;
}

/// <summary>A year's heading, or a row of cards.</summary>
public sealed partial class ShelfRowTemplates : DataTemplateSelector
{
    public DataTemplate? Year { get; set; }
    public DataTemplate? Cards { get; set; }

    protected override DataTemplate SelectTemplateCore(object item) => (item is YearRow ? Year : Cards)!;

    protected override DataTemplate SelectTemplateCore(object item, DependencyObject container) => SelectTemplateCore(item);
}
