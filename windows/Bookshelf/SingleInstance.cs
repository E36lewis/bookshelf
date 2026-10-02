using System.Runtime.InteropServices;
using Microsoft.Windows.AppLifecycle;

namespace Bookshelf;

/// <summary>
/// Keeps Bookshelf to one window: two windows on one journal could each
/// hold an unsaved summary, and the last save would win. A second launch
/// hands its activation to the first, which comes forward, and exits.
/// </summary>
internal static class SingleInstance
{
    /// <summary>Raised in the first instance (on a background thread) when another launch hands over.</summary>
    public static event Action? Activated;

    /// <summary>
    /// Registers this process as the app, or, if one is already running,
    /// hands over to it. True means this process should exit now.
    /// </summary>
    public static bool HandOverToRunningApp()
    {
        AppInstance main;
        try
        {
            main = AppInstance.FindOrRegisterForKey(AppFolders.AppName);
        }
        catch (Exception e)
        {
            // Better two windows than none.
            StartupLog.Step($"Couldn't check for another Bookshelf: {e.Message}");
            return false;
        }
        if (main.IsCurrent)
        {
            main.Activated += (_, _) => Activated?.Invoke();
            return false;
        }

        StartupLog.Step($"Bookshelf is already open (process {main.ProcessId}): bringing it forward");
        // Windows only lets the app the person is using bring a window to
        // the front; this lets the open one do it for us.
        AllowSetForegroundWindow(main.ProcessId);
        var activation = AppInstance.GetCurrent().GetActivatedEventArgs();
        var done = new ManualResetEventSlim(); // not disposed: the task may set it after a timeout
        // Off this (UI) thread, as Microsoft's sample does, so the wait
        // can't block the call it's waiting for.
        _ = Task.Run(async () =>
        {
            try
            {
                await main.RedirectActivationToAsync(activation);
            }
            catch (Exception e)
            {
                StartupLog.Step($"Couldn't hand over: {e.Message}");
            }
            finally
            {
                done.Set();
            }
        });
        done.Wait(TimeSpan.FromSeconds(10));
        return true;
    }

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool AllowSetForegroundWindow(uint processId);
}
