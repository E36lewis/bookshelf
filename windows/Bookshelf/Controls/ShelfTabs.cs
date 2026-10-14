using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Bookshelf.Controls;

/// <summary>
/// The three shelves as a row of pills at the top of the window, as in the
/// GTK app. To Narrator it's a tab list of three tabs, one of them selected.
/// Tab reaches it once (on the open shelf), the arrow keys move along it,
/// and Space or Enter opens a shelf.
/// </summary>
public sealed partial class ShelfTabList : StackPanel
{
    public ShelfTabList()
    {
        GettingFocus += OnGettingFocus;
    }

    /// <summary>The tab that's on, if any.</summary>
    public ShelfTab? Selected => Children.OfType<ShelfTab>().FirstOrDefault(t => t.IsChecked == true);

    /// <summary>Coming in from outside, the focus goes to the open shelf's tab.</summary>
    private void OnGettingFocus(UIElement sender, GettingFocusEventArgs args)
    {
        if (args.NewFocusedElement is ShelfTab tab && tab.IsChecked != true
            && args.OldFocusedElement is not ShelfTab && Selected is { } selected)
        {
            args.TrySetNewFocusedElement(selected);
        }
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new ShelfTabListPeer(this);

    private sealed partial class ShelfTabListPeer(ShelfTabList owner) : FrameworkElementAutomationPeer(owner), ISelectionProvider
    {
        public bool CanSelectMultiple => false;

        public bool IsSelectionRequired => true;

        public IRawElementProviderSimple[] GetSelection()
        {
            if (((ShelfTabList)Owner).Selected is not { } tab) return [];
            var peer = FromElement(tab) ?? CreatePeerForElement(tab);
            return peer is null ? [] : [ProviderFromPeer(peer)];
        }

        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Tab;

        protected override string GetClassNameCore() => nameof(ShelfTabList);

        protected override object GetPatternCore(PatternInterface patternInterface) =>
            patternInterface == PatternInterface.Selection ? this : base.GetPatternCore(patternInterface);
    }
}

/// <summary>One shelf's tab: a radio button that Narrator knows as a tab.</summary>
public sealed partial class ShelfTab : RadioButton
{
    protected override AutomationPeer OnCreateAutomationPeer() => new ShelfTabPeer(this);

    private sealed partial class ShelfTabPeer(ShelfTab owner) : RadioButtonAutomationPeer(owner)
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.TabItem;

        protected override string GetClassNameCore() => nameof(ShelfTab);

        // A tab is selected, not toggled: only the selection pattern.
        protected override object GetPatternCore(PatternInterface patternInterface) =>
            patternInterface == PatternInterface.Toggle ? null! : base.GetPatternCore(patternInterface);
    }
}
