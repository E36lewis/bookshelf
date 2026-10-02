using Bookshelf.Core;
using Bookshelf.Ffi;
using Microsoft.UI.Dispatching;

namespace Bookshelf.Services;

/// <summary>
/// The daily backup: one at startup and a check every hour (still one copy
/// a day), so it keeps happening if Bookshelf stays open for days. Runs
/// beside the journal's queue, so a slow or unplugged drive never holds up
/// anything else.
/// </summary>
internal static class Backups
{
    private static DispatcherQueueTimer? _timer;

    /// <summary>Makes today's backup now, then checks hourly.</summary>
    public static void Start(JournalService journal, DispatcherQueue queue)
    {
        _ = BackUpAsync(journal);
        _timer?.Stop();
        _timer = queue.CreateTimer();
        _timer.Interval = TimeSpan.FromHours(1);
        _timer.IsRepeating = true;
        _timer.Tick += (_, _) => _ = BackUpAsync(journal);
        _timer.Start();
    }

    private static async Task BackUpAsync(JournalService journal)
    {
        try
        {
            if (await journal.BackUpNowAsync() is { } made) StartupLog.Step($"Backup made: {Path.GetFileName(made)}");
        }
        catch (CoreException e)
        {
            // Settings shows where backups go and how recent the last one is.
            StartupLog.Step($"Daily backup failed: {e.UserMessage()}");
        }
    }
}
