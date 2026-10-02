using System.Text;
using Bookshelf.Core;
using Bookshelf.Ffi;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Bookshelf;

public sealed partial class MainWindow : Window
{
    private JournalService? _journal;
    private readonly WritingBox _writing;

    public MainWindow()
    {
        StartupLog.Step("Loading MainWindow.xaml");
        InitializeComponent();
        Title = "Bookshelf (preview)";
        StartupLog.Step("Setting the Mica backdrop");
        SystemBackdrop = new MicaBackdrop(); // Windows 11; Windows 10 keeps a plain background
        AppWindow.Resize(new Windows.Graphics.SizeInt32(1100, 760));

        StartupLog.Step("Filling the writing box");
        _writing = new WritingBox(Editor);
        _writing.Restyled += report => Timing.Text = report;
        StartupLog.Step("First Markdown highlighting");
        _writing.SetText(ShortSample);
        _ = LoadAsync();
    }

    private async Task LoadAsync()
    {
        // Previews use their own journal, never a real one.
        var dir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Bookshelf.Preview");
        try
        {
            StartupLog.Step("Opening the journal");
            _journal = await JournalService.OpenAsync(dir);
            await ShowProfilesAsync();
            Status.Text = $"Rust core {BookshelfFfiMethods.CoreVersion()} · {dir}";
        }
        catch (Exception e)
        {
            Status.Text = $"Couldn't open the journal: {e.Message}";
        }
        StartupLog.Step($"Ready: {Status.Text}");
    }

    private async Task ShowProfilesAsync()
    {
        if (_journal is null) return;
        var profiles = await _journal.ProfilesAsync();
        ProfileList.ItemsSource = profiles.Select(p => p.Name).ToList();
    }

    private async void OnAddProfile(object sender, RoutedEventArgs e) => await AddProfileAsync();

    private async void OnNewProfileKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == Windows.System.VirtualKey.Enter) await AddProfileAsync();
    }

    private async Task AddProfileAsync()
    {
        var name = NewProfileName.Text.Trim();
        if (_journal is null || name.Length == 0) return;
        try
        {
            await _journal.CreateProfileAsync(name);
            NewProfileName.Text = "";
            ProfileError.Text = "";
            await ShowProfilesAsync();
        }
        catch (CoreException ex)
        {
            ProfileError.Text = ex.Message;
        }
    }

    // ---- writing box test ---------------------------------------------------

    private void OnHighlightToggled(object sender, RoutedEventArgs e)
    {
        _writing.Highlight = HighlightToggle.IsChecked == true;
        _writing.RestyleAll();
    }

    private void OnLoadLongSample(object sender, RoutedEventArgs e) => _writing.SetText(LongSample());

    private const string ShortSample =
        "# What stayed with me\r" +
        "A **quiet** book about *patience*, with ~~no~~ one `code` word.\r" +
        "> The best thoughts arrive between chapters.\r" +
        "- 📚 Emoji first, then **bold** still lines up\r";

    private static string LongSample()
    {
        var b = new StringBuilder();
        for (var i = 1; i <= 120; i++)
        {
            b.Append($"## Chapter {i}\r");
            b.Append("The **lighthouse** keeper wrote *every evening*, even when nothing happened; ");
            b.Append("the sea was `grey`, the ~~boats~~ gulls were loud, and the lamp turned all night.\r");
            b.Append("> A quote worth keeping, one line long.\r");
            b.Append("- one thing **worth** noting\r- another, *briefly*\r\r");
        }
        return b.ToString(); // ≈ 5,000 words
    }
}
