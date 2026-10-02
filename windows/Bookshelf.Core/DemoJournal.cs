using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// A made-up journal for screenshots, UI tests and trying things out
/// (<c>Bookshelf.exe --demo-journal</c>): a few profiles with books on every
/// shelf, finished across two years, and Markdown summaries.
/// </summary>
/// <remarks>
/// Entirely offline. Each book is saved from a search result that already
/// has its description and no cover link, and the core asks Open Library
/// for neither then (see <c>Journal::save_search_result</c>). So the demo
/// books have no covers.
/// </remarks>
public static class DemoJournal
{
    /// <summary>The profiles the demo makes.</summary>
    public sealed record Profiles(Profile Avery, Profile Sam, Profile Jo);

    private sealed record Book(string Key, string Title, string Author, string Year, int Pages, string About);

    private static readonly Book Earthsea = new("/works/DEMO1W", "A Wizard of Earthsea", "Ursula K. Le Guin", "1968", 183,
        "A boy with a gift for magic grows into it the hard way, chasing the shadow he let into the world.");
    private static readonly Book Dune = new("/works/DEMO2W", "Dune", "Frank Herbert", "1965", 412,
        "On a desert planet where water is worth more than gold, a young heir is drawn into a war over the most precious substance in the universe.");
    private static readonly Book Middlemarch = new("/works/DEMO3W", "Middlemarch", "George Eliot", "1871", 880,
        "A provincial town, a handful of marriages and a great many ambitions, observed with patience and wit.");
    private static readonly Book Remains = new("/works/DEMO4W", "The Remains of the Day", "Kazuo Ishiguro", "1989", 245,
        "An English butler takes a motoring trip and, mile by mile, revisits a life of service.");
    private static readonly Book LeftHand = new("/works/DEMO5W", "The Left Hand of Darkness", "Ursula K. Le Guin", "1969", 304,
        "An envoy on a frozen world tries to understand a people who are neither men nor women.");
    private static readonly Book Piranesi = new("/works/DEMO6W", "Piranesi", "Susanna Clarke", "2020", 245,
        "A man lives in a house of endless halls, tides and statues, and keeps a careful journal of it.");
    private static readonly Book MobyDick = new("/works/DEMO7W", "Moby-Dick", "Herman Melville", "1851", 635,
        "A whaling voyage, a captain's obsession and a great deal about whales.");
    private static readonly Book Overstory = new("/works/DEMO8W", "The Overstory", "Richard Powers", "2018", 502,
        "Nine lives, and the trees that tie them together.");
    private static readonly Book Lighthouse = new("/works/DEMO9W", "To the Lighthouse", "Virginia Woolf", "1927", 209,
        "A family's summers by the sea, and the trip to the lighthouse that takes ten years.");

    /// <summary>
    /// Fills an empty journal. <paramref name="today"/> anchors every date, so
    /// "day 12" and "this year" always look the same.
    /// </summary>
    public static async Task<Profiles> SeedAsync(JournalService journal, DateOnly today)
    {
        var avery = await journal.CreateProfileAsync("Avery", "avery@example.com");
        var sam = await journal.CreateProfileAsync("Sam");
        var jo = await journal.CreateProfileAsync("Jo");
        var lastYear = today.Year - 1;

        // Avery: a bit of everything, in the default look.
        await AddAsync(journal, avery, LeftHand, today.AddDays(-11), null, LeftHandNotes);
        await AddAsync(journal, avery, Piranesi, today.AddDays(-2), null, "");
        await AddAsync(journal, avery, Dune, today.AddDays(-40), today.AddDays(-9), DuneNotes);
        await AddAsync(journal, avery, Earthsea, today.AddDays(-70), today.AddDays(-58), EarthseaNotes);
        await AddAsync(journal, avery, Middlemarch, new DateOnly(lastYear, 8, 2), new DateOnly(lastYear, 10, 19), MiddlemarchNotes);
        await AddAsync(journal, avery, Remains, new DateOnly(lastYear, 3, 6), new DateOnly(lastYear, 3, 15), RemainsNotes);
        await AddAsync(journal, avery, MobyDick, null, null, "Everyone says to skip the chapters about whales. I won't.");
        await AddAsync(journal, avery, Overstory, null, null, "");

        // Sam: the dark theme, another accent and sans-serif headings.
        await AddAsync(journal, sam, Lighthouse, today.AddDays(-5), null, LighthouseNotes);
        await AddAsync(journal, sam, Dune, new DateOnly(lastYear, 11, 1), new DateOnly(lastYear, 11, 30), "Read it for the second time. Slower, *better*.");
        await AddAsync(journal, sam, Piranesi, null, null, "");
        var teal = BookshelfFfiMethods.Accents().FirstOrDefault(a => a.Name == "Teal")?.Hex ?? "#2a7f7a";
        var settings = await journal.SettingsAsync(sam.Id);
        await journal.UpdateSettingsAsync(settings with
        {
            Theme = Theme.Dark,
            Accent = teal,
            HeadingFont = HeadingFont.Sans,
            WeekStart = WeekStart.Monday,
            DateFormat = DateFormat.YearMonthDay,
        });

        // Jo has nothing yet: the empty shelves.
        return new Profiles(avery, sam, jo);
    }

    private static async Task AddAsync(
        JournalService journal, Profile who, Book b, DateOnly? started, DateOnly? finished, string notes)
    {
        var saved = await journal.SaveSearchResultAsync(who.Id, new SearchResult(
            ExternalId: b.Key, Title: b.Title, Subtitle: null, Author: b.Author, Isbn: null, Publisher: null,
            Description: b.About, PublishedDate: b.Year, PageCount: b.Pages, CoverUrl: null));
        var shelf = finished is not null ? Shelf.Finished : started is not null ? Shelf.Reading : Shelf.Eventually;
        var id = await journal.AddToShelfAsync(who.Id, saved.Id, shelf);
        if (shelf != Shelf.Eventually) await journal.SetDatesAsync(id, started, finished);
        if (notes.Length > 0) await journal.SaveBodyAsync(id, notes);
    }

    private const string DuneNotes = """
        # What stayed with me

        The desert is the real main character. Every scene comes back to **water**: who has it, who wastes it, and who would die for it.

        - Paul's visions felt less like a gift and more like a trap.
        - The glossary at the back is worth reading *after* the ending.
        - Politics first, sand worms second. I didn't expect that.

        > Big ideas, told through small, careful rituals.

        I want to reread the first hundred pages now that I know where it all goes.
        """;

    private const string EarthseaNotes = """
        Short, and it never wastes a word. The idea that knowing something's **true name** gives you power over it stayed with me for days.

        Ged's mistake isn't a villain's. It's pride, and the whole book is him learning to own it.
        """;

    private const string MiddlemarchNotes = """
        ## Slow, then all at once

        It took me a month to get into it and a weekend to finish it. Dorothea and Lydgate both marry the wrong person for the *right* reasons, which is somehow worse.

        1. The narrator is wise without being smug.
        2. Nobody is a villain, not even Casaubon.
        3. The last paragraph is perfect.
        """;

    private const string RemainsNotes = """
        Stevens never says what he feels, and the book makes you hear it anyway. The pier scene at the end: I had to put it down for a minute.
        """;

    private const string LeftHandNotes = """
        Halfway through. The journey across the ice is the best thing I've read this year. Genly and Estraven are only now beginning to *see* each other.

        Questions for later:
        - Why does Genly keep getting Estraven wrong?
        - What does "shifgrethor" really protect?
        """;

    private const string LighthouseNotes = """
        Almost nothing happens and it's completely absorbing. The middle section, "Time Passes", covers ten years in a few pages, and it hits hard.
        """;
}
