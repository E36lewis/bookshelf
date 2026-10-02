using System.ComponentModel;
using Bookshelf.Ffi;
using Bookshelf.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace Bookshelf.Pages;

/// <summary>One book on a shelf, as the shelf page's list shows it.</summary>
public sealed class EntryItem : INotifyPropertyChanged
{
    /// <summary>List covers are 64 by 96.</summary>
    public const int CoverWidth = 64;

    private readonly string? _coverPath;
    private ImageSource? _cover;

    public EntryItem(ShelfEntry entry)
    {
        SummaryId = entry.SummaryId;
        Title = entry.Title;
        Author = entry.Author ?? "";
        Meta = entry.Meta;
        Excerpt = entry.Excerpt;
        EmptyNote = entry.Excerpt.Length == 0 ? entry.EmptyNote ?? "" : "";
        _coverPath = entry.CoverPath;
        Initial = Title.TrimStart().Length > 0 ? char.ToUpperInvariant(Title.TrimStart()[0]).ToString() : "";
        AutomationName = string.Join(". ", new[] { Title, Author, Meta }.Where(s => s.Length > 0));
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    public string SummaryId { get; }
    public string Title { get; }
    public string Author { get; }
    public string Meta { get; }
    public string Excerpt { get; }
    public string EmptyNote { get; }

    /// <summary>The title's first letter, on the stand-in cover.</summary>
    public string Initial { get; }

    /// <summary>What Narrator reads for the row.</summary>
    public string AutomationName { get; }

    public FontFamily HeadingFont => Look.Heading;
    public Visibility AuthorVisibility => Visible(Author);
    public Visibility MetaVisibility => Visible(Meta);
    public Visibility ExcerptVisibility => Visible(Excerpt);
    public Visibility EmptyNoteVisibility => Visible(EmptyNote);

    /// <summary>The cover, once it's loaded; null shows the stand-in.</summary>
    public ImageSource? Cover
    {
        get => _cover;
        private set
        {
            _cover = value;
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Cover)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(PlaceholderVisibility)));
        }
    }

    public Visibility PlaceholderVisibility => _cover is null ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>Loads the cover in the background (from the cache when it can).</summary>
    public async void LoadCover()
    {
        if (_coverPath is null || _cover is not null) return;
        Cover = await Covers.LoadAsync(_coverPath, CoverWidth);
    }

    /// <summary>What Narrator reads for the list item.</summary>
    public override string ToString() => AutomationName;

    private static Visibility Visible(string text) => text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
}

/// <summary>The Finished shelf's books of one year, under "2026 · 12 books".</summary>
public sealed class YearGroup : List<EntryItem>
{
    public YearGroup(int year, string label)
    {
        YearText = year.ToString(System.Globalization.CultureInfo.InvariantCulture);
        Label = label;
    }

    public string YearText { get; }
    public string Label { get; }

    public override string ToString() => $"{YearText}, {Label}";
    public FontFamily HeadingFont => Look.Heading;
}
