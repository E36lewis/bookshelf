using System.Diagnostics;

namespace Bookshelf;

/// <summary>
/// Preview diagnostics: writes a line to startup.log before each startup
/// step. Each line is on disk before the step runs, so if the app dies
/// without a crash.log or a message box, the last line says where.
/// Kept in %LOCALAPPDATA%\Bookshelf.Preview (not synced by OneDrive), or
/// in %TEMP% if that can't be written.
/// </summary>
internal static class StartupLog
{
    private static readonly Stopwatch Clock = Stopwatch.StartNew();
    private static string? _file;
    private static bool _opened;

    public static void Step(string what)
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

    private static string? Open()
    {
        var folders = new[]
        {
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Bookshelf.Preview"),
            Path.GetTempPath(),
        };
        foreach (var folder in folders)
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
