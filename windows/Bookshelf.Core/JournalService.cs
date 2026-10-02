using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// Runs calls into the Rust core off the UI thread. The core is synchronous;
/// this is what keeps the window responsive.
/// </summary>
/// <remarks>
/// Journal calls run one at a time, in the order they were made, so writes
/// (like autosaves) land in order. Calls that wait on the network or on a
/// slow drive (searching, saving a search result, backups, export) run
/// beside that queue instead: a slow Open Library or an unplugged USB stick
/// never holds up an autosave. The core's own lock keeps their database
/// steps safe, and it's never held while they wait.
/// </remarks>
public sealed class JournalService : IDisposable
{
    private readonly Journal _journal;
    private readonly SemaphoreSlim _queue = new(1, 1);

    private JournalService(Journal journal) => _journal = journal;

    /// <summary>Opens (or creates) the journal in <paramref name="directory"/>.</summary>
    public static Task<JournalService> OpenAsync(string directory) =>
        Task.Run(() =>
        {
            Directory.CreateDirectory(directory);
            return new JournalService(Journal.OpenAt(Path.GetFullPath(directory)));
        });

    /// <summary>Opens (or creates) this PC's journal in %LOCALAPPDATA%. A preview has its own.</summary>
    public static Task<JournalService> OpenDefaultAsync(Channel channel) =>
        Task.Run(() => new JournalService(Journal.OpenDefault(channel)));

    /// <summary>The folder the journal lives in.</summary>
    public string DataDirectory => _journal.DataDir();

    /// <summary>Call when the app quits. Safe to call twice; the journal stays usable.</summary>
    public Task CloseAsync() => Run(() => _journal.Close());

    /// <summary>Changes whenever the journal does: skip a refresh when it's the same.</summary>
    public Task<ulong> ChangeTokenAsync() => Run(() => _journal.ChangeToken());

    // ---- profiles -----------------------------------------------------------

    /// <summary>Every profile, by name.</summary>
    public Task<Profile[]> ProfilesAsync() => Run(() => _journal.Profiles());

    /// <summary>Makes a profile; a bad <paramref name="email"/> is refused before anything is made.</summary>
    public Task<Profile> CreateProfileAsync(string name, string? email = null) =>
        Run(() => _journal.CreateProfile(name, email));

    /// <summary>Renames a profile. Names are unique.</summary>
    public Task<Profile> RenameProfileAsync(string userId, string name) =>
        Run(() => _journal.RenameProfile(userId, name));

    /// <summary>Sets or clears (null or blank) a profile's contact email.</summary>
    public Task<Profile> SetProfileEmailAsync(string userId, string? email) =>
        Run(() => _journal.SetProfileEmail(userId, email));

    /// <summary>Deletes a profile with its settings and entries. No undo.</summary>
    public Task DeleteProfileAsync(string userId) => Run(() => _journal.DeleteProfile(userId));

    // ---- settings -----------------------------------------------------------

    /// <summary>A profile's settings.</summary>
    public Task<ProfileSettings> SettingsAsync(string userId) => Run(() => _journal.Settings(userId));

    /// <summary>Saves a profile's settings (the profile is <c>settings.UserId</c>).</summary>
    public Task UpdateSettingsAsync(ProfileSettings settings) =>
        Run(() => _journal.UpdateSettings(settings));

    // ---- shelves and entries ------------------------------------------------

    /// <summary>
    /// One shelf, laid out for <paramref name="now"/> (default: now) in the
    /// local time zone: "day 12" counts from that day, "Added" dates use its
    /// offset from UTC.
    /// </summary>
    public Task<ShelfView> ShelfAsync(string userId, Shelf shelf, DateTimeOffset? now = null)
    {
        var local = TimeZoneInfo.ConvertTime(now ?? DateTimeOffset.Now, TimeZoneInfo.Local);
        var today = JournalDate.ToText(DateOnly.FromDateTime(local.DateTime));
        var offset = (int)local.Offset.TotalMinutes;
        return Run(() => _journal.Shelf(userId, shelf, today, offset));
    }

    /// <summary>One entry with its book.</summary>
    public Task<EntryDetail> EntryAsync(string summaryId) => Run(() => _journal.Entry(summaryId));

    /// <summary>The profile's latest entry for a book, if any.</summary>
    public Task<string?> ExistingEntryAsync(string userId, string bookId) =>
        Run(() => _journal.ExistingEntry(userId, bookId));

    /// <summary>Puts a saved book on a shelf, dated today. Returns the entry's id.</summary>
    public Task<string> AddToShelfAsync(string userId, string bookId, Shelf shelf) =>
        Run(() => _journal.AddToShelf(userId, bookId, shelf));

    /// <summary>Sets or clears the reading dates. Finishing before starting is refused.</summary>
    public Task<EntryDetail> SetDatesAsync(string summaryId, DateOnly? started, DateOnly? finished) =>
        Run(() => _journal.SetDates(summaryId, JournalDate.ToText(started), JournalDate.ToText(finished)));

    /// <summary>Saves the writing page's text. Line ends are stored as <c>\n</c>.</summary>
    public Task<SaveResult> SaveBodyAsync(string summaryId, string body) =>
        Run(() => _journal.SaveBody(summaryId, body));

    /// <summary>
    /// When saving fails: writes the text to the recovery folder instead and
    /// returns the file, to tell the user where it is.
    /// </summary>
    public Task<string> RescueBodyAsync(string title, string body) =>
        Run(() => _journal.RescueBody(title, body));

    /// <summary>Starts another reading of the entry's book today. Returns the new id.</summary>
    public Task<string> ReadAgainAsync(string summaryId) => Run(() => _journal.ReadAgain(summaryId));

    /// <summary>Removes an entry; keep the result for Undo.</summary>
    public Task<RemovedEntry> RemoveEntryAsync(string summaryId) =>
        Run(() => _journal.RemoveEntry(summaryId));

    /// <summary>Puts a removed entry back exactly as it was (Undo).</summary>
    public Task RestoreEntryAsync(RemovedEntry removed) => Run(() => _journal.RestoreEntry(removed));

    // ---- Open Library (beside the queue) -------------------------------------

    /// <summary>Searches Open Library. Nothing is saved.</summary>
    public Task<SearchResult[]> SearchBooksAsync(string userId, string query) =>
        Beside(() => _journal.SearchBooks(userId, query));

    /// <summary>Saves a picked result as a book, with its description and cover.</summary>
    public Task<BookInfo> SaveSearchResultAsync(string userId, SearchResult result) =>
        Beside(() => _journal.SaveSearchResult(userId, result));

    // ---- backups and export (beside the queue) -------------------------------

    /// <summary>Where backups go and the newest one's date.</summary>
    public Task<BackupStatus> BackupStatusAsync() => Beside(() => _journal.BackupStatus());

    /// <summary>Keeps backups in <paramref name="folder"/> from now on, or the default folder for null.</summary>
    public Task SetBackupFolderAsync(string? folder) => Beside(() =>
    {
        _journal.SetBackupFolder(folder);
        return true;
    });

    /// <summary>Makes today's backup if there isn't one yet. Returns the new file.</summary>
    public Task<string?> BackUpNowAsync() => Beside(() => _journal.BackUpNow());

    /// <summary>
    /// Writes the profile's entries as Markdown into a "Bookshelf summaries"
    /// folder inside <paramref name="folder"/>.
    /// </summary>
    public Task<ExportResult> ExportMarkdownAsync(string userId, string folder) =>
        Beside(() => _journal.ExportMarkdown(userId, folder));

    // ---- plumbing ---------------------------------------------------------------

    private Task Run(Action call) => Run(() =>
    {
        call();
        return true;
    });

    private async Task<T> Run<T>(Func<T> call)
    {
        await _queue.WaitAsync().ConfigureAwait(false);
        try
        {
            return await Task.Run(call).ConfigureAwait(false);
        }
        finally
        {
            _queue.Release();
        }
    }

    /// <summary>Off the UI thread, but not in the queue: for calls that wait on the network or a slow drive.</summary>
    private static Task<T> Beside<T>(Func<T> call) => Task.Run(call);

    /// <summary>Lets go of the journal. Call <see cref="CloseAsync"/> first when quitting.</summary>
    public void Dispose()
    {
        _journal.Dispose();
        _queue.Dispose();
    }
}
