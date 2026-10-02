using Microsoft.UI.Xaml.Navigation;

namespace Bookshelf.Pages;

/// <summary>Says why Bookshelf couldn't open the journal (the message is the navigation parameter).</summary>
public sealed partial class ProblemPage : BookshelfPage
{
    public ProblemPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        Message.Text = $"{e.Parameter as string}\n\nIf your journal file is damaged, daily backups are kept in the " +
            "“backups” folder beside it.";
    }
}
