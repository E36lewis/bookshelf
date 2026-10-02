using System.Globalization;
using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// Calendar dates as the core writes them: <c>YYYY-MM-DD</c>, a day with no
/// time or time zone.
/// </summary>
public static class JournalDate
{
    private const string Format = "yyyy-MM-dd";

    /// <summary><paramref name="day"/> as <c>YYYY-MM-DD</c>.</summary>
    public static string ToText(DateOnly day) => day.ToString(Format, CultureInfo.InvariantCulture);

    /// <summary><paramref name="day"/> as <c>YYYY-MM-DD</c>, or null.</summary>
    public static string? ToText(DateOnly? day) => day is { } d ? ToText(d) : null;

    /// <summary>A <c>YYYY-MM-DD</c> date from the core, or null if it isn't one.</summary>
    public static DateOnly? Parse(string? text) =>
        DateOnly.TryParseExact(text, Format, CultureInfo.InvariantCulture, DateTimeStyles.None, out var day)
            ? day
            : null;

    /// <summary>A timestamp from the core (Unix milliseconds).</summary>
    public static DateTimeOffset FromUnixMilliseconds(long ms) => DateTimeOffset.FromUnixTimeMilliseconds(ms);
}

/// <summary>Conveniences on the core's types.</summary>
public static class CoreValues
{
    /// <summary>
    /// The error as a sentence to show people. (The exception's own
    /// <c>Message</c> is the generated "field=value" form.)
    /// </summary>
    public static string UserMessage(this CoreException e) => e switch
    {
        CoreException.Database x => x.message,
        CoreException.Network x => x.message,
        CoreException.Io x => x.message,
        CoreException.NotFound x => x.message,
        CoreException.Invalid x => x.message,
        CoreException.NewerJournal x => x.message,
        _ => e.Message,
    };

    /// <summary>When reading started.</summary>
    public static DateOnly? StartedDate(this EntryDetail e) => JournalDate.Parse(e.Started);

    /// <summary>When it was finished.</summary>
    public static DateOnly? FinishedDate(this EntryDetail e) => JournalDate.Parse(e.Finished);

    /// <summary>When the entry was made.</summary>
    public static DateTimeOffset CreatedAt(this EntryDetail e) => JournalDate.FromUnixMilliseconds(e.CreatedAtMs);

    /// <summary>When it last changed.</summary>
    public static DateTimeOffset UpdatedAt(this EntryDetail e) => JournalDate.FromUnixMilliseconds(e.UpdatedAtMs);

    /// <summary>
    /// What pressing a formatting button does with
    /// <paramref name="selStart"/>..<paramref name="selEnd"/> selected (C#
    /// string indices are the UTF-16 offsets the core counts in). Null means
    /// leave everything alone.
    /// </summary>
    public static TextEdit? FormatEdit(string text, int selStart, int selEnd, FormatAction action) =>
        BookshelfFfiMethods.FormatEdit(text, (uint)Math.Max(selStart, 0), (uint)Math.Max(selEnd, 0), action);
}
