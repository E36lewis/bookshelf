using System.Diagnostics;

namespace Bookshelf;

/// <summary>
/// Preview diagnostics: writes a line to startup.log before each startup
/// step. Each line is on disk before the step runs, so if the app dies
/// without a crash.log or a message box, the last line says where.
/// Kept in the logs folder (<see cref="AppFolders.Logs"/>, not synced by
/// OneDrive), or in %TEMP% if that can't be written. Later problems that
/// don't stop the app (a backup that failed, say) are noted here too.
/// Nothing anyone wrote is ever logged.
/// </summary>
internal static class StartupLog
{
    private static readonly Stopwatch Clock = Stopwatch.StartNew();
    private static readonly object Lock = new();
    private static string? _file;
    private static bool _opened;

    public static void Step(string what)
    {
        lock (Lock)
        {
            try
            {
                if (!_opened)
                {
                    _opened = true;
                    _file = Open();
                }
                if (_file is not null)
                {
                    File.AppendAllText(_file, $"{Clock.ElapsedMilliseconds,6} ms  {what}\r\n");
                }
            }
            catch
            {
                // Logging must never be what stops the app.
            }
        }
    }

    private static string? Open()
    {
        foreach (var folder in new[] { AppFolders.Logs, Path.GetTempPath() })
        {
            try
            {
                Directory.CreateDirectory(folder);
                var file = Path.Combine(folder, "startup.log");
                // Keeps the last few starts; begins again once it's large.
                if (File.Exists(file) && new FileInfo(file).Length > 256 * 1024) File.Delete(file);
                File.AppendAllText(file, $"\r\n=== {DateTime.Now:yyyy-MM-dd HH:mm:ss}\r\n");
                return file;
            }
            catch
            {
                // Try the next folder.
            }
        }
        return null;
    }
}
