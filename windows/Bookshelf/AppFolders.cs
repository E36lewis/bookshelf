using Bookshelf.Core;
using Bookshelf.Ffi;

namespace Bookshelf;

/// <summary>Which build this is, and where the app keeps its own files, beside the journal.</summary>
internal static class AppFolders
{
    /// <summary>
    /// This build's channel. Only a release build is Stable and opens the
    /// real journal: <c>dotnet publish -p:BookshelfChannel=Stable</c> (see
    /// Bookshelf.csproj). Everything else is a preview, with its own.
    /// </summary>
#if BOOKSHELF_STABLE
    public static readonly Channel BuildChannel = Channel.Stable;
#else
    public static readonly Channel BuildChannel = Channel.Preview;
#endif

    /// <summary>
    /// "Bookshelf" or "Bookshelf Preview": the window title, and the
    /// journal's folder name in %LOCALAPPDATA%.
    /// </summary>
    public static readonly string AppName = ChannelNames.AppName(BuildChannel);

    /// <summary>The key that keeps the app to one window, and the installer's mutex name.</summary>
    public static readonly string AppId = ChannelNames.AppId(BuildChannel);

    /// <summary>The app's version (Version in Bookshelf.csproj), as "0.9.0".</summary>
    public static string Version => typeof(AppFolders).Assembly.GetName().Version?.ToString(3) ?? "unknown";

    /// <summary>
    /// startup.log, crash.log and core.log: in a "logs" folder inside the
    /// journal's, so there's one place to look.
    /// </summary>
    public static string Logs => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), AppName, "logs");
}

/// <summary>What the app was started with.</summary>
internal sealed record LaunchOptions
{
    /// <summary>Open a freshly made demo journal in a temporary folder instead of the real one.</summary>
    public bool DemoJournal { get; init; }

    /// <summary>With <see cref="DemoJournal"/>: leave it empty, to see the first-run page.</summary>
    public bool DemoEmpty { get; init; }

    /// <summary>With <see cref="DemoJournal"/>: the demo profile to open (Avery by default).</summary>
    public string? DemoProfile { get; init; }

    /// <summary>
    /// With <see cref="DemoJournal"/>: keep the demo journal in this folder,
    /// so a second run opens it again (to check what the first one saved).
    /// Only an empty folder or one holding a demo journal is used.
    /// </summary>
    public string? DemoFolder { get; init; }

    /// <summary>
    /// With <see cref="DemoJournal"/>: the writing page's saves fail, to test
    /// how text is rescued. Never possible on a real journal.
    /// </summary>
    public bool DemoSaveFails { get; init; }

    /// <summary>
    /// Reads the command line: <c>--demo-journal</c>, <c>--demo-empty</c>,
    /// <c>--demo-profile NAME</c>, <c>--demo-journal-in FOLDER</c> and
    /// <c>--demo-save-fails</c>. Each of them means a demo journal, never the
    /// real one. Anything else is ignored.
    /// </summary>
    public static LaunchOptions Parse(IReadOnlyList<string> args)
    {
        var options = new LaunchOptions();
        for (var i = 0; i < args.Count; i++)
        {
            switch (args[i])
            {
                case "--demo-journal":
                    options = options with { DemoJournal = true };
                    break;
                case "--demo-empty":
                    options = options with { DemoJournal = true, DemoEmpty = true };
                    break;
                case "--demo-profile" when i + 1 < args.Count:
                    options = options with { DemoJournal = true, DemoProfile = args[++i] };
                    break;
                case "--demo-journal-in" when i + 1 < args.Count:
                    options = options with { DemoJournal = true, DemoFolder = args[++i] };
                    break;
                case "--demo-save-fails":
                    options = options with { DemoJournal = true, DemoSaveFails = true };
                    break;
            }
        }
        return options;
    }
}
