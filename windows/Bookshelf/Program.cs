using System.Runtime.InteropServices;
using Bookshelf.Ffi;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

namespace Bookshelf;

/// <summary>
/// The app's entry point, used instead of the one XAML generates
/// (DISABLE_XAML_GENERATED_MAIN) so that every startup step is logged
/// first. It does what the generated one does, plus the logging.
/// </summary>
public static class Program
{
    [STAThread]
    private static void Main()
    {
        AppDomain.CurrentDomain.UnhandledException += (_, e) => App.ReportCrash(e.ExceptionObject);

        var exe = Environment.ProcessPath ?? "unknown";
        StartupLog.Step($"Started {exe}{(IsUnderOneDrive(exe) ? " (in OneDrive)" : "")}");
        StartupLog.Step($"Files in {AppContext.BaseDirectory}");
        StartupLog.Step($"Windows {Environment.OSVersion.Version} {RuntimeInformation.OSArchitecture}, " +
            $"app {RuntimeInformation.ProcessArchitecture}, .NET {Environment.Version}");

        StartupLog.Step("Loading the Rust core (bookshelf_ffi.dll)");
        try
        {
            StartupLog.Step($"Rust core {BookshelfFfiMethods.CoreVersion()} loaded");
        }
        catch (Exception e)
        {
            // The window still opens and shows the problem in its status line.
            StartupLog.Step($"Rust core failed to load: {e.GetType().Name}: {e.Message}");
        }

        StartupLog.Step("Starting the Windows App SDK (COM wrappers)");
        WinRT.ComWrappersSupport.InitializeComWrappers();
        StartupLog.Step("Starting XAML");
        Application.Start(p =>
        {
            StartupLog.Step("XAML started");
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            new App();
        });
        StartupLog.Step("Closed");
    }

    private static bool IsUnderOneDrive(string path) =>
        new[] { "OneDrive", "OneDriveConsumer", "OneDriveCommercial" }
            .Select(Environment.GetEnvironmentVariable)
            .Any(root => !string.IsNullOrEmpty(root) &&
                path.StartsWith(root.TrimEnd('\\') + "\\", StringComparison.OrdinalIgnoreCase));
}
