using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Bookshelf.Services;

/// <summary>
/// Making sure Narrator (and other screen readers) hear what changes on
/// screen without the focus moving there.
/// </summary>
internal static class ScreenReader
{
    /// <summary>
    /// Has screen readers read <paramref name="element"/> again, politely or
    /// assertively as its <c>AutomationProperties.LiveSetting</c> says. WinUI
    /// doesn't tell them a live region's text changed by itself.
    /// </summary>
    public static void LiveRegionChanged(UIElement element) =>
        FrameworkElementAutomationPeer.CreatePeerForElement(element)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);

    /// <summary>
    /// Opens <paramref name="bar"/> so that it's read out. An InfoBar
    /// announces itself as it opens, but only through an automation peer
    /// that already exists, which one that was closed or hidden may not have
    /// yet; and one that's already open says nothing about a new message.
    /// </summary>
    public static void Open(InfoBar bar)
    {
        bar.IsOpen = false;
        FrameworkElementAutomationPeer.CreatePeerForElement(bar);
        bar.IsOpen = true;
    }
}
