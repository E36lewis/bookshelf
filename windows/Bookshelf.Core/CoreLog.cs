using System.Text;
using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// Writes the core's log messages to <c>core.log</c> in a folder of the
/// app's, starting over in a fresh file once it passes a size limit (the
/// old one is kept as <c>core.log.1</c>). The core never logs what anyone
/// wrote, so nothing personal lands in the file.
/// </summary>
public sealed class CoreLog : Logger
{
    private readonly string _path;
    private readonly long _maxBytes;
    private readonly object _lock = new();

    /// <summary>Logs into <paramref name="folder"/>, rolling over at <paramref name="maxBytes"/>.</summary>
    public CoreLog(string folder, long maxBytes = 512 * 1024)
    {
        Directory.CreateDirectory(folder);
        _path = Path.Combine(folder, "core.log");
        _maxBytes = maxBytes;
    }

    /// <summary>The current log file.</summary>
    public string FilePath => _path;

    /// <summary>One message from the core. Never throws: a log that can't be written is skipped.</summary>
    public void Log(LogLevel level, string target, string message)
    {
        var line = $"{DateTimeOffset.Now:yyyy-MM-dd HH:mm:ss.fff zzz} {level.ToString().ToUpperInvariant()} {target}: {message}{Environment.NewLine}";
        lock (_lock)
        {
            try
            {
                var info = new FileInfo(_path);
                if (info.Exists && info.Length > _maxBytes)
                {
                    File.Move(_path, _path + ".1", overwrite: true);
                }
                File.AppendAllText(_path, line, Encoding.UTF8);
            }
            catch (IOException)
            {
            }
            catch (UnauthorizedAccessException)
            {
            }
        }
    }

    /// <summary>Routes the core's messages to a log file in <paramref name="folder"/> from now on.</summary>
    public static CoreLog Start(string folder)
    {
        var log = new CoreLog(folder);
        BookshelfFfiMethods.SetLogger(log);
        return log;
    }
}
