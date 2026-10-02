using Bookshelf.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Ffi = Bookshelf.Ffi;

namespace Bookshelf.Controls;

/// <summary>
/// Lays out the core's Markdown blocks (<c>render_markdown</c>) in a
/// RichTextBlock: the book page's preview, the reading page and the
/// manual. HTML is never interpreted; it shows as the text that was typed.
/// </summary>
internal static class MarkdownRenderer
{
    /// <summary>Fills <paramref name="target"/> with <paramref name="blocks"/>.</summary>
    /// <param name="target">Where the text goes; anything in it is replaced.</param>
    /// <param name="blocks">From <c>BookshelfFfiMethods.RenderMarkdown</c>.</param>
    /// <param name="size">Running text size, in pixels.</param>
    /// <param name="text">Running text font.</param>
    /// <param name="headings">Headings font.</param>
    public static void Render(RichTextBlock target, IReadOnlyList<Ffi.Block> blocks, double size, FontFamily text, FontFamily headings)
    {
        target.Blocks.Clear();
        target.FontFamily = text;
        target.FontSize = size;
        target.LineHeight = Math.Round(size * 1.55);
        target.LineStackingStrategy = LineStackingStrategy.BlockLineHeight;

        var gap = Math.Round(size * 0.8);
        var indent = Math.Round(size * 1.5);
        Paragraph? last = null;
        uint listDepth = 0;

        void Add(Paragraph p)
        {
            target.Blocks.Add(p);
            last = p;
        }

        foreach (var block in blocks)
        {
            switch (block)
            {
                case Ffi.Block.Heading h:
                {
                    var scale = h.Level switch { 1 => 1.6, 2 => 1.35, 3 => 1.15, _ => 1.0 };
                    var p = new Paragraph
                    {
                        FontFamily = headings,
                        FontSize = Math.Round(size * scale),
                        FontWeight = FontWeights.SemiBold,
                        LineHeight = Math.Round(size * scale * 1.3),
                        Margin = new Thickness(0, last is null ? 0 : gap, 0, Math.Round(gap / 2)),
                    };
                    AddRuns(p, h.Runs, size);
                    Add(p);
                    break;
                }
                case Ffi.Block.Paragraph para:
                {
                    var p = new Paragraph { Margin = new Thickness(0, 0, 0, gap) };
                    AddRuns(p, para.Runs, size);
                    Add(p);
                    break;
                }
                case Ffi.Block.ListItem item:
                {
                    listDepth = item.Depth;
                    var left = indent * (item.Depth + 1);
                    var p = new Paragraph
                    {
                        Margin = new Thickness(left, 0, 0, item.Loose ? gap : Math.Round(gap / 4)),
                        TextIndent = -indent * 0.9,
                    };
                    var marker = item.Marker switch
                    {
                        Ffi.ListMarker.Number n => $"{n.Value}.",
                        _ => "•",
                    };
                    p.Inlines.Add(new Run { Text = marker + " " });
                    AddRuns(p, item.Runs, size);
                    Add(p);
                    break;
                }
                case Ffi.Block.ItemText more:
                {
                    var p = new Paragraph { Margin = new Thickness(indent * (listDepth + 1), 0, 0, Math.Round(gap / 4)) };
                    AddRuns(p, more.Runs, size);
                    Add(p);
                    break;
                }
                case Ffi.Block.ListEnd end:
                    // Space after the whole list, as after a paragraph.
                    if (end.Depth == 0 && last is not null)
                    {
                        var m = last.Margin;
                        last.Margin = new Thickness(m.Left, m.Top, m.Right, Math.Max(m.Bottom, gap));
                    }
                    break;
                case Ffi.Block.CodeBlock code:
                    Add(Literal(code.Text, size, indent, gap));
                    break;
                case Ffi.Block.Html html:
                    Add(Literal(html.Text, size, indent, gap));
                    break;
                case Ffi.Block.Rule:
                {
                    var p = new Paragraph
                    {
                        TextAlignment = TextAlignment.Center,
                        Margin = new Thickness(0, Math.Round(gap / 2), 0, gap),
                    };
                    p.Inlines.Add(new Run { Text = "*   *   *" });
                    Add(p);
                    break;
                }
            }
        }
        if (last is not null)
        {
            var m = last.Margin;
            last.Margin = new Thickness(m.Left, m.Top, m.Right, 0);
        }
    }

    /// <summary>Code (or HTML) as written, in monospace.</summary>
    private static Paragraph Literal(string text, double size, double indent, double gap)
    {
        var p = new Paragraph
        {
            FontFamily = Look.Mono,
            FontSize = Math.Round(size * 0.9),
            Margin = new Thickness(indent / 2, 0, 0, gap),
        };
        AddText(p.Inlines, text.TrimEnd('\n'), null);
        return p;
    }

    private static void AddRuns(Paragraph p, IEnumerable<Ffi.Run> runs, double size)
    {
        foreach (var run in runs)
        {
            AddText(p.Inlines, run.Text, r =>
            {
                foreach (var style in run.Styles)
                {
                    switch (style)
                    {
                        case Ffi.RunStyle.Bold:
                            r.FontWeight = FontWeights.SemiBold;
                            break;
                        case Ffi.RunStyle.Italic:
                            r.FontStyle = Windows.UI.Text.FontStyle.Italic;
                            break;
                        case Ffi.RunStyle.Strike:
                            r.TextDecorations = Windows.UI.Text.TextDecorations.Strikethrough;
                            break;
                        case Ffi.RunStyle.Code:
                            r.FontFamily = Look.Mono;
                            r.FontSize = Math.Round(size * 0.9);
                            break;
                    }
                }
            });
        }
    }

    /// <summary>Text with its typed line breaks, each piece styled by <paramref name="style"/>.</summary>
    private static void AddText(InlineCollection inlines, string text, Action<Run>? style)
    {
        var lines = text.Split('\n');
        for (var i = 0; i < lines.Length; i++)
        {
            if (i > 0) inlines.Add(new LineBreak());
            if (lines[i].Length == 0) continue;
            var run = new Run { Text = lines[i] };
            style?.Invoke(run);
            inlines.Add(run);
        }
    }
}
