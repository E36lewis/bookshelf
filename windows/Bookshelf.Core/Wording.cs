using System.Globalization;
using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// The words the Windows app shows for the shelves, the same as the GTK
/// app's (bookshelf-app/src/main.rs), kept here so they're tested.
/// </summary>
public static class ShelfText
{
    /// <summary>The shelves in the order of the navigation pane and Ctrl+1/2/3.</summary>
    public static readonly Shelf[] All = [Shelf.Reading, Shelf.Finished, Shelf.Eventually];

    /// <summary>The shelf's name in the navigation pane (and the shortcuts list).</summary>
    public static string Name(Shelf shelf) => shelf switch
    {
        Shelf.Reading => "Reading",
        Shelf.Finished => "Finished",
        _ => "Eventually",
    };

    /// <summary>The heading at the top of the shelf.</summary>
    public static string Heading(Shelf shelf) => shelf switch
    {
        Shelf.Reading => "Reading now",
        Shelf.Finished => "Finished",
        _ => "Someday",
    };

    /// <summary>What an empty shelf says.</summary>
    public static string Empty(Shelf shelf) => shelf switch
    {
        Shelf.Reading => "Books you've started will live here.",
        Shelf.Finished => "Every book you finish gets its own page here.",
        _ => "Books you want to read someday wait here.",
    };

    /// <summary>What a shelf says when nothing on it matches a search.</summary>
    public static string NoMatches(string query) => $"Nothing on this shelf matches “{query.Trim()}”.";

    /// <summary>"3 matches", under the heading while searching.</summary>
    public static string Matches(int count) => count == 1 ? "1 match" : $"{count:N0} matches";
}

/// <summary>Dates as the profile chose to see them, and the rules for reading dates.</summary>
public static class ReadingDates
{
    /// <summary>"Finished in 10 days" under the dates, when both are set.</summary>
    public static string? DaysText(long? days) => days switch
    {
        null => null,
        0 => "Finished the same day",
        1 => "Finished in 1 day",
        var n => $"Finished in {n:N0} days",
    };

    /// <summary>Whether <paramref name="day"/> can be the start: not after the finish.</summary>
    public static bool CanStart(DateOnly day, DateOnly? finished) => finished is not { } f || day <= f;

    /// <summary>Whether <paramref name="day"/> can be the finish: not before the start.</summary>
    public static bool CanFinish(DateOnly day, DateOnly? started) => started is not { } s || day >= s;

    /// <summary>
    /// The date as the core writes it in meta lines: "Sep 20, 2026",
    /// "09/20/2026", "20/09/2026" or "2026-09-20". English month names, as
    /// the rest of the app's words are.
    /// </summary>
    public static string Format(DateOnly day, DateFormat format) => day.ToString(format switch
    {
        DateFormat.MonthDayYear => "MM'/'dd'/'yyyy",
        DateFormat.DayMonthYear => "dd'/'MM'/'yyyy",
        DateFormat.YearMonthDay => "yyyy'-'MM'-'dd",
        _ => "MMM d, yyyy",
    }, CultureInfo.InvariantCulture);

    /// <summary>
    /// The same format as a WinUI date picker pattern
    /// (<c>CalendarDatePicker.DateFormat</c>, Windows.Globalization's
    /// DateTimeFormatter syntax).
    /// </summary>
    public static string PickerPattern(DateFormat format) => format switch
    {
        DateFormat.MonthDayYear => "{month.integer(2)}/{day.integer(2)}/{year.full}",
        DateFormat.DayMonthYear => "{day.integer(2)}/{month.integer(2)}/{year.full}",
        DateFormat.YearMonthDay => "{year.full}-{month.integer(2)}-{day.integer(2)}",
        _ => "{month.abbreviated} {day.integer}, {year.full}",
    };
}

/// <summary>Search results, as the Add a book dialog lists them.</summary>
public static class SearchText
{
    /// <summary>"Frank Herbert · 1965", or whichever of the two is known.</summary>
    public static string Byline(SearchResult result) =>
        string.Join(" · ", new[] { result.Author, result.PublishedDate }.Where(s => !string.IsNullOrWhiteSpace(s)));

    /// <summary>"Ace · 1965 · 412 pages" under a book's title, or empty.</summary>
    public static string Details(BookInfo book) =>
        string.Join(" · ", new[]
        {
            book.Publisher,
            book.PublishedDate,
            book.PageCount is { } n && n > 0 ? $"{n:N0} pages" : null,
        }.Where(s => !string.IsNullOrWhiteSpace(s)));
}

/// <summary>The writing page's status line.</summary>
public static class WriterText
{
    /// <summary>"1 word", "1,204 words".</summary>
    public static string Words(uint words) =>
        words == 1 ? "1 word" : string.Format(CultureInfo.CurrentCulture, "{0:N0} words", words);

    /// <summary>"89 words · Saved", or "· Saving…" while there's typing still to save.</summary>
    public static string Status(uint words, bool saved) => $"{Words(words)} · {(saved ? "Saved" : "Saving…")}";

    /// <summary>What the status line says when a save failed.</summary>
    public static string NotSaved(string reason) => $"Not saved: {reason}";

    /// <summary>The focus mode button's tooltip.</summary>
    public static string FocusTooltip(string? key) =>
        $"Focus mode{(key is null ? "" : $" ({key})")}: dim everything except the sentence you're writing";
}
