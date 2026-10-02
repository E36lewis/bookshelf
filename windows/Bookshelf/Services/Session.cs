using Bookshelf.Core;
using Bookshelf.Ffi;

namespace Bookshelf.Services;

/// <summary>
/// The open journal and who's using it: the profile, their settings, their
/// three shelves (kept laid out, so switching shelves is instant) and the
/// search. Lives on the UI thread; every journal call goes through
/// <see cref="JournalService"/>, which runs it off that thread.
/// </summary>
internal sealed class Session
{
    private readonly ShelfView?[] _shelves = new ShelfView?[3];
    private (ulong Token, DateOnly Day, string UserId)? _builtFrom;

    public Session(JournalService journal) => Journal = journal;

    /// <summary>The journal, for the calls the pages make themselves.</summary>
    public JournalService Journal { get; }

    /// <summary>Every profile, by name.</summary>
    public IReadOnlyList<Profile> Profiles { get; private set; } = [];

    /// <summary>The open profile, if one is.</summary>
    public Profile? Profile { get; private set; }

    /// <summary>The open profile's settings.</summary>
    public ProfileSettings? Settings { get; private set; }

    /// <summary>The shelf the shelf page shows.</summary>
    public Shelf CurrentShelf { get; set; } = Shelf.Reading;

    /// <summary>The search as typed (trimmed); empty when not searching.</summary>
    public string Query { get; private set; } = "";

    /// <summary>The search's words, for <see cref="Matches"/>.</summary>
    public string[] Terms { get; private set; } = [];

    /// <summary>Whether there's a search.</summary>
    public bool IsSearching => Terms.Length > 0;

    /// <summary>The shelves were laid out again (something changed).</summary>
    public event Action? ShelvesChanged;

    /// <summary>The search changed.</summary>
    public event Action? SearchChanged;

    /// <summary>The open profile, its name or the list of profiles changed.</summary>
    public event Action? ProfileChanged;

    /// <summary>One shelf as last laid out, or null before the first refresh.</summary>
    public ShelfView? ViewOf(Shelf shelf) => _shelves[Index(shelf)];

    /// <summary>Whether a shelf entry matches the search (always, with no search).</summary>
    public bool Matches(ShelfEntry entry) => !IsSearching || BookshelfFfiMethods.Matches(entry.Haystack, Terms);

    /// <summary>How many entries on a shelf match the search.</summary>
    public int MatchCount(Shelf shelf) =>
        ViewOf(shelf)?.Rows.OfType<ShelfRow.Entry>().Count(e => Matches(e.Item)) ?? 0;

    /// <summary>Reads the list of profiles again.</summary>
    public async Task<IReadOnlyList<Profile>> LoadProfilesAsync()
    {
        Profiles = await Journal.ProfilesAsync();
        if (Profile is { } open) Profile = Profiles.FirstOrDefault(p => p.Id == open.Id);
        ProfileChanged?.Invoke();
        return Profiles;
    }

    /// <summary>
    /// Opens a profile: its settings, its shelves, and remembers it for the
    /// next start.
    /// </summary>
    public async Task OpenAsync(Profile profile)
    {
        var settings = await Journal.SettingsAsync(profile.Id);
        Profile = profile;
        Settings = settings;
        CurrentShelf = settings.StartShelf;
        _builtFrom = null;
        Array.Clear(_shelves);
        var folder = Journal.DataDirectory;
        await Task.Run(() => new AppState { LastProfileId = profile.Id }.Save(folder));
        ProfileChanged?.Invoke();
        await RefreshShelvesAsync();
    }

    /// <summary>Forgets the open profile (it was deleted).</summary>
    public void Close()
    {
        Profile = null;
        Settings = null;
        _builtFrom = null;
        Array.Clear(_shelves);
        SetSearch("");
        ProfileChanged?.Invoke();
        ShelvesChanged?.Invoke();
    }

    /// <summary>Saves changed settings for the open profile.</summary>
    public async Task UpdateSettingsAsync(ProfileSettings settings)
    {
        await Journal.UpdateSettingsAsync(settings);
        var dateFormatChanged = Settings?.DateFormat != settings.DateFormat;
        Settings = settings;
        // Dates on the shelves are written in the chosen format.
        if (dateFormatChanged) await RefreshShelvesAsync(force: true);
    }

    /// <summary>Renames the open profile, or sets its email; the core checks both.</summary>
    public async Task UpdateProfileAsync(Func<JournalService, string, Task<Profile>> change)
    {
        if (Profile is null) return;
        Profile = await change(Journal, Profile.Id);
        await LoadProfilesAsync();
    }

    /// <summary>
    /// Lays the shelves out again if the journal or the date changed since
    /// last time (or always, with <paramref name="force"/>).
    /// </summary>
    public async Task RefreshShelvesAsync(bool force = false)
    {
        if (Profile is not { } profile) return;
        var token = await Journal.ChangeTokenAsync();
        var key = (token, DateOnly.FromDateTime(DateTime.Now), profile.Id);
        if (!force && _builtFrom == key) return;

        var views = new ShelfView[3];
        foreach (var shelf in ShelfText.All)
        {
            views[Index(shelf)] = await Journal.ShelfAsync(profile.Id, shelf);
        }
        if (Profile?.Id != profile.Id) return; // switched profiles meanwhile
        views.CopyTo(_shelves, 0);
        _builtFrom = key;
        ShelvesChanged?.Invoke();
    }

    /// <summary>Searches the shelves for <paramref name="text"/> (empty: stop searching).</summary>
    public void SetSearch(string text)
    {
        var query = text.Trim();
        if (query == Query) return;
        Query = query;
        Terms = query.Length == 0 ? [] : BookshelfFfiMethods.QueryTerms(query);
        SearchChanged?.Invoke();
    }

    /// <summary>When quitting: folds the journal's write-ahead log back in.</summary>
    public Task CloseJournalAsync() => Journal.CloseAsync();

    private static int Index(Shelf shelf) => shelf switch
    {
        Shelf.Reading => 0,
        Shelf.Finished => 1,
        _ => 2,
    };
}
