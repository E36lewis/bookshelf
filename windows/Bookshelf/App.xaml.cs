using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;

namespace Bookshelf;

public partial class App : Application
{
    private Window? _window;

    public App()
    {
        // Registered first, so even a failure while loading the app's own
        // XAML is reported instead of the app silently closing. (Main
        // registers the one for crashes outside XAML.)
        UnhandledException += (_, e) => ReportCrash(e.Exception);
        StartupLog.Step("Loading App.xaml");
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        StartupLog.Step("Creating the main window");
        _window = new MainWindow();
        StartupLog.Step("Showing the main window");
        _window.Activate();
        StartupLog.Step("Main window shown");
    }

    /// <summary>
    /// Writes crash.log next to the preview journal and shows a plain Windows
    /// message box (which works even when XAML itself is what failed).
    /// </summary>
    internal static void ReportCrash(object? error)
    {
        var details = error?.ToString() ?? "unknown error";
        var where = "";
        try
        {
            var dir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "Bookshelf.Preview");
            Directory.CreateDirectory(dir);
            var log = Path.Combine(dir, "crash.log");
            File.AppendAllText(log, $"--- {DateTime.Now:yyyy-MM-dd HH:mm:ss}\n{details}\n\n");
            where = $"\n\nDetails were saved to:\n{log}";
        }
        catch
        {
            // Nowhere to write; the message box below still says what happened.
        }
        var first = details.Split('\n')[0].Trim();
        StartupLog.Step($"Crashed: {first}");
        MessageBox(IntPtr.Zero, $"Bookshelf hit a problem and has to close.\n\n{first}{where}", "Bookshelf", 0x10);
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int MessageBox(IntPtr owner, string text, string caption, uint type);
}
