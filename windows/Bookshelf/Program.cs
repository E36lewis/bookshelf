using System.Runtime.InteropServices;
using Bookshelf.Ffi;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

namespace Bookshelf;

/// <summary>
/// The app's entry point, used instead of the one XAML generates
/// (DISABLE_XAML_GENERATED_MAIN) so that every startup step is logged
/// first, and a second launch hands over to the open window.
/// </summary>
public static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        AppDomain.CurrentDomain.UnhandledException += (_, e) => App.ReportCrash(e.ExceptionObject);

        var exe = Environment.ProcessPath ?? "unknown";
        StartupLog.Step($"Started {exe}{(IsUnderOneDrive(exe) ? " (in OneDrive)" : "")}");
        StartupLog.Step($"Files in {AppContext.BaseDirectory}");
        StartupLog.Step($"{AppFolders.AppName} {AppFolders.Version}, Windows {Environment.OSVersion.Version} {RuntimeInformation.OSArchitecture}, " +
            $"app {RuntimeInformation.ProcessArchitecture}, .NET {Environment.Version}");
        var options = LaunchOptions.Parse(args);
        App.Options = options;
        if (options.DemoJournal) StartupLog.Step("Demo journal requested");

        StartupLog.Step("Loading the Rust core (bookshelf_ffi.dll)");
        try
        {
            StartupLog.Step($"Rust core {BookshelfFfiMethods.CoreVersion()} loaded");
        }
        catch (Exception e)
        {
            // The window still opens and says what went wrong.
            StartupLog.Step($"Rust core failed to load: {e.GetType().Name}: {e.Message}");
        }

        // Tells the installer that Bookshelf is open (AppMutex in
        // windows/installer/Bookshelf.iss), so it asks for it to be closed
        // before replacing or removing its files. Held until Main returns.
        using var running = OpenRunningMutex();

        StartupLog.Step("Starting the Windows App SDK (COM wrappers)");
        WinRT.ComWrappersSupport.InitializeComWrappers();

        // One window per journal: a second launch brings the first forward.
        // A demo journal is a new one each time, so it gets its own window.
        if (!options.DemoJournal && SingleInstance.HandOverToRunningApp())
        {
            StartupLog.Step("Closed (handed over to the open window)");
            return 0;
        }

        StartupLog.Step("Starting XAML");
        Application.Start(p =>
        {
            StartupLog.Step("XAML started");
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            new App();
        });
        StartupLog.Step("Closed");
        return 0;
    }

    private static Mutex? OpenRunningMutex()
    {
        try
        {
            return new Mutex(false, AppFolders.AppId);
        }
        catch (Exception e) when (e is UnauthorizedAccessException or IOException or WaitHandleCannotBeOpenedException)
        {
            // Only the installer's check misses it; never a reason not to start.
            StartupLog.Step($"No running-app mutex: {e.Message}");
            return null;
        }
    }

    private static bool IsUnderOneDrive(string path) =>
        new[] { "OneDrive", "OneDriveConsumer", "OneDriveCommercial" }
            .Select(Environment.GetEnvironmentVariable)
            .Any(root => !string.IsNullOrEmpty(root) &&
                path.StartsWith(root.TrimEnd('\\') + "\\", StringComparison.OrdinalIgnoreCase));
}
