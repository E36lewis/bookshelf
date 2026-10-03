using Bookshelf.Ffi;

namespace Bookshelf.Core;

/// <summary>
/// What each build channel is called. A release build (Stable) is
/// "Bookshelf" and opens the real journal; every other build, dev and CI
/// included, is "Bookshelf Preview" with a journal of its own, so trying
/// one out never touches real books. The channel is chosen when the app
/// is built (see <c>BookshelfChannel</c> in Bookshelf.csproj).
/// </summary>
public static class ChannelNames
{
    /// <summary>
    /// The window title, the name in Settings › About and the installer's
    /// name. It's also the journal's folder in %LOCALAPPDATA%, as the core
    /// names it (<c>bookshelf-core/src/paths.rs</c>); the app needs that
    /// before the core is loaded, for the startup log.
    /// </summary>
    public static string AppName(Channel channel) => channel switch
    {
        Channel.Stable => "Bookshelf",
        _ => "Bookshelf Preview",
    };

    /// <summary>
    /// The app's id: the key that keeps it to one window, and the name of
    /// the mutex that tells the installer it's running (AppMutex in
    /// windows/installer/Bookshelf.iss). A preview runs beside a release.
    /// </summary>
    public static string AppId(Channel channel) => channel switch
    {
        Channel.Stable => "io.github.e36lewis.Bookshelf",
        _ => "io.github.e36lewis.Bookshelf.Preview",
    };
}
