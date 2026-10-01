using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// Runs calls into the Rust core one at a time, off the UI thread. The core
/// is synchronous; this is what keeps the window responsive, and the queue
/// keeps writes (like autosaves) in the order they were made.
/// </summary>
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
            return new JournalService(Journal.OpenAt(directory));
        });

    public Task<Profile[]> ProfilesAsync() => Run(() => _journal.Profiles());

    public Task<Profile> CreateProfileAsync(string name) => Run(() => _journal.CreateProfile(name));

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

    public void Dispose()
    {
        _journal.Dispose();
        _queue.Dispose();
    }
}
