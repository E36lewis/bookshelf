using System.Diagnostics;
using System.Text;
using Bookshelf.Core;
using Bookshelf.Ffi;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Bookshelf;

public sealed partial class MainWindow : Window
{
    private JournalService? _journal;
    private string _lastText = "";
    private bool _restyling;
    private Windows.UI.Color _ink;

    public MainWindow()
    {
        StartupLog.Step("Loading MainWindow.xaml");
        InitializeComponent();
        Title = "Bookshelf (preview)";
        StartupLog.Step("Setting the Mica backdrop");
        SystemBackdrop = new MicaBackdrop(); // Windows 11; Windows 10 keeps a plain background
        AppWindow.Resize(new Windows.Graphics.SizeInt32(1100, 760));

        StartupLog.Step("Filling the writing box");
        _ink = Editor.Document.GetDefaultCharacterFormat().ForegroundColor;
        Editor.Document.SetText(TextSetOptions.None, ShortSample);
        StartupLog.Step("First Markdown highlighting");
        Restyle(force: true);
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

    private void OnEditorTextChanged(object sender, RoutedEventArgs e) => Restyle(force: false);

    private void OnHighlightToggled(object sender, RoutedEventArgs e) => Restyle(force: true);

    private void OnLoadLongSample(object sender, RoutedEventArgs e)
    {
        Editor.Document.SetText(TextSetOptions.None, LongSample());
        Restyle(force: true);
    }

    /// <summary>
    /// Re-applies the shared core's highlighting to the whole text and shows
    /// how long it took. The real writer will only restyle edited lines.
    /// </summary>
    private void Restyle(bool force)
    {
        if (_restyling) return;
        Editor.Document.GetText(TextGetOptions.None, out var raw);
        // RichEditBox separates paragraphs with \r; the core splits lines on \n.
        // One character for one, so offsets stay the same.
        var text = raw.Replace('\r', '\n');
        if (!force && text == _lastText) return; // only formatting changed
        _lastText = text;

        var clock = Stopwatch.StartNew();
        _restyling = true;
        var doc = Editor.Document;
        doc.BatchDisplayUpdates();
        try
        {
            var all = doc.GetRange(0, text.Length).CharacterFormat;
            all.Bold = FormatEffect.Off;
            all.Italic = FormatEffect.Off;
            all.Strikethrough = FormatEffect.Off;
            all.Size = 15;
            all.ForegroundColor = _ink;
            all.BackgroundColor = Microsoft.UI.Colors.Transparent;

            if (HighlightToggle.IsChecked == true)
            {
                var dim = Windows.UI.Color.FromArgb(110, _ink.R, _ink.G, _ink.B);
                var codeBack = Windows.UI.Color.FromArgb(28, _ink.R, _ink.G, _ink.B);
                foreach (var span in BookshelfFfiMethods.MarkdownSpans(text))
                {
                    var f = doc.GetRange((int)span.Start, (int)span.End).CharacterFormat;
                    switch (span.Kind)
                    {
                        case StyleKind.Heading: f.Bold = FormatEffect.On; f.Size = 19; break;
                        case StyleKind.Bold: f.Bold = FormatEffect.On; break;
                        case StyleKind.Italic:
                        case StyleKind.Quote: f.Italic = FormatEffect.On; break;
                        case StyleKind.Code: f.BackgroundColor = codeBack; break;
                        case StyleKind.Strike: f.Strikethrough = FormatEffect.On; break;
                        case StyleKind.Syntax: f.ForegroundColor = dim; break;
                    }
                }
            }
        }
        finally
        {
            doc.ApplyDisplayUpdates();
            _restyling = false;
        }
        Timing.Text = $"{text.Length:N0} characters · restyled in {clock.Elapsed.TotalMilliseconds:F1} ms";
    }

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
