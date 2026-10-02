import Foundation
import BookshelfFFI

/// A made-up journal for screenshots and UI tests: a few profiles, books on
/// every shelf across two years, and summaries with Markdown. It never goes
/// online: every book comes from a search result that already has its
/// description and no cover URL, so saving it asks Open Library for nothing.
public enum DemoJournal {
    /// One book on the demo shelves. Days count back from today.
    struct Book {
        var key: String
        var title: String
        var subtitle: String? = nil
        var author: String
        var publisher: String
        var year: String
        var pages: Int64
        var about: String
        var started: Int? = nil
        var finished: Int? = nil
        /// A date in last year, as (month, day), for books finished then.
        var lastYear: (started: (Int, Int), finished: (Int, Int))? = nil
        var body = ""
    }

    /// Fills the journal behind `worker` (it should be empty) and returns
    /// the profile to open. `theme` is set on every profile, so screenshots
    /// can be light or dark.
    @discardableResult
    public static func seed(
        _ worker: JournalWorker, today: Date = Date(), calendar: Calendar = .current, theme: Theme = .system
    ) async throws -> Profile {
        let avery = try await worker.createProfile(named: "Avery", email: "avery@example.com")
        let sam = try await worker.createProfile(named: "Sam")
        let jordan = try await worker.createProfile(named: "Jordan")

        for profile in [avery, sam, jordan] {
            var settings = try await worker.settings(for: profile.id)
            settings.theme = theme
            try await worker.updateSettings(settings)
        }

        for book in averysBooks {
            try await add(book, for: avery, worker: worker, today: today, calendar: calendar)
        }
        for book in samsBooks {
            try await add(book, for: sam, worker: worker, today: today, calendar: calendar)
        }
        return avery
    }

    private static func add(
        _ book: Book, for profile: Profile, worker: JournalWorker, today: Date, calendar: Calendar
    ) async throws {
        let result = SearchResult(
            externalId: book.key, title: book.title, subtitle: book.subtitle, author: book.author, isbn: nil,
            publisher: book.publisher, description: book.about, publishedDate: book.year, pageCount: book.pages,
            coverUrl: nil)
        let saved = try await worker.saveSearchResult(result, for: profile.id)
        let id = try await worker.addToShelf(.eventually, for: profile.id, book: saved.id)

        func daysAgo(_ n: Int?) -> Date? {
            n.flatMap { calendar.date(byAdding: .day, value: -$0, to: today) }
        }
        var started = daysAgo(book.started)
        var finished = daysAgo(book.finished)
        if let span = book.lastYear {
            let year = calendar.component(.year, from: today) - 1
            started = calendar.date(from: DateComponents(year: year, month: span.started.0, day: span.started.1))
            finished = calendar.date(from: DateComponents(year: year, month: span.finished.0, day: span.finished.1))
        }
        if started != nil || finished != nil {
            _ = try await worker.setDates(id, started: started, finished: finished, calendar: calendar)
        }
        if !book.body.isEmpty {
            _ = try await worker.saveBody(id, book.body)
        }
    }

    static let averysBooks: [Book] = [
        Book(
            key: "/works/DEMO1W", title: "The Left Hand of Darkness", author: "Ursula K. Le Guin",
            publisher: "Ace Books", year: "1969", pages: 304,
            about: "An envoy arrives alone on a frozen world whose people have no fixed sex, and has to learn "
                + "who to trust before the winter closes in.",
            started: 12,
            body: """
                ## Where I am

                Just crossed the **Gobrin Ice**. The pace has slowed right down and I don't mind at all.

                - Estraven is the real centre of the book
                - The weather is a character of its own

                > Light is the left hand of darkness.

                Next time: the *Handdara* chapters, and words like `shifgrethor`. ~~Skim~~ Savour the ice.
                """),
        Book(
            key: "/works/DEMO2W", title: "Middlemarch", subtitle: "A Study of Provincial Life",
            author: "George Eliot", publisher: "William Blackwood and Sons", year: "1871", pages: 880,
            about: "A whole town seen from the inside: marriages, ambitions and a new hospital, and the quiet "
                + "ways people's choices bend one another's lives.",
            started: 3),
        Book(
            key: "/works/DEMO3W", title: "Piranesi", author: "Susanna Clarke", publisher: "Bloomsbury",
            year: "2020", pages: 272,
            about: "A man lives in a house of endless halls and tides, keeping careful journals, until the "
                + "notes he finds start to contradict what he remembers.",
            started: 34, finished: 20,
            body: """
                # What stayed with me

                The House is *beautiful and kind*, and I believed it completely. Reading the journals \
                alongside the narrator felt like solving the mystery **with** him rather than ahead of him.

                ## Moments

                1. The first time the tides flood the lower halls
                2. Finding the numbered pages
                3. The last chapter, all of it

                ---

                I'd read it again in winter.
                """),
        Book(
            key: "/works/DEMO4W", title: "The Remains of the Day", author: "Kazuo Ishiguro", publisher: "Faber & Faber",
            year: "1989", pages: 245,
            about: "An English butler drives across the country in 1956 and looks back on decades of service, "
                + "and on what loyalty cost him.",
            started: 60, finished: 48,
            body: "A careful, *restrained* voice that slowly lets you see everything he can't say. The pier "
                + "scene near the end undid me."),
        Book(
            key: "/works/DEMO5W", title: "A Wizard of Earthsea", author: "Ursula K. Le Guin", publisher: "Parnassus Press",
            year: "1968", pages: 183,
            about: "A gifted, proud boy at a school for wizards unleashes a shadow on the world, and has to "
                + "follow it to the edge of the sea.",
            started: 9, finished: 2,
            body: "Short and *wise*. Names have power; so does knowing your own."),
        Book(
            key: "/works/DEMO6W", title: "Dune", author: "Frank Herbert", publisher: "Chilton Books",
            year: "1965", pages: 412,
            about: "On the desert planet Arrakis, the only source of the most valuable substance in the "
                + "universe, a young heir is drawn into a war for its future.",
            lastYear: ((9, 2), (10, 14)),
            body: """
                ## Big ideas

                - Ecology as politics
                - The danger of **heroes**

                The appendices are worth it. `Spice must flow.`
                """),
        Book(
            key: "/works/DEMO7W", title: "Pride and Prejudice", author: "Jane Austen", publisher: "T. Egerton",
            year: "1813", pages: 279,
            about: "Elizabeth Bennet and Mr Darcy misjudge each other at every turn in a sharp, funny novel of "
                + "manners and money.",
            lastYear: ((5, 1), (5, 19)),
            body: "Funnier than I remembered. ~~Darcy is awful~~ Darcy is *shy*."),
        Book(
            key: "/works/DEMO8W", title: "Moby-Dick", subtitle: "or, The Whale", author: "Herman Melville",
            publisher: "Harper & Brothers", year: "1851", pages: 635,
            about: "Captain Ahab's obsessive hunt for the white whale, told by Ishmael, a sailor who signs on "
                + "to the Pequod."),
        Book(
            key: "/works/DEMO9W", title: "The Overstory", author: "Richard Powers", publisher: "W. W. Norton",
            year: "2018", pages: 502,
            about: "Nine strangers, each changed by a tree, are drawn together to try to save the last of the "
                + "old forests.",
            body: "Recommended by Sam. Why do I want to read it? *Trees*, mostly."),
    ]

    static let samsBooks: [Book] = [
        Book(
            key: "/works/DEMO6W", title: "Dune", author: "Frank Herbert", publisher: "Chilton Books",
            year: "1965", pages: 412,
            about: "On the desert planet Arrakis, the only source of the most valuable substance in the "
                + "universe, a young heir is drawn into a war for its future.",
            started: 5),
    ]
}
