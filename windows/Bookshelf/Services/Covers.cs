using System.Runtime.InteropServices.WindowsRuntime;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace Bookshelf.Services;

/// <summary>
/// Book covers from the journal's covers folder: read and checked off the
/// UI thread, decoded by XAML at the size they're shown (not their full
/// size), and kept so scrolling back up doesn't decode them again.
/// </summary>
internal static class Covers
{
    /// <summary>Covers are small; anything bigger than this isn't one.</summary>
    private const long MaxBytes = 15 * 1024 * 1024;

    /// <summary>A small file can still decode to gigabytes: refuse absurd pixel counts.</summary>
    private const long MaxPixels = 40_000_000;

    private const int MaxCached = 400;
    private static readonly Dictionary<(string Path, int Width), ImageSource> Cache = new();

    /// <summary>
    /// The cover at <paramref name="path"/>, decoded <paramref name="width"/>
    /// logical pixels wide; null if there's none or it can't be read. Call on
    /// the UI thread.
    /// </summary>
    public static async Task<ImageSource?> LoadAsync(string? path, int width)
    {
        if (string.IsNullOrEmpty(path)) return null;
        if (Cache.TryGetValue((path, width), out var cached)) return cached;

        var bytes = await Task.Run(() => Read(path));
        if (bytes is null) return null;
        try
        {
            using var stream = new InMemoryRandomAccessStream();
            await stream.WriteAsync(bytes.AsBuffer());
            stream.Seek(0);
            var image = new BitmapImage { DecodePixelWidth = width, DecodePixelType = DecodePixelType.Logical };
            await image.SetSourceAsync(stream);
            if (Cache.Count >= MaxCached) Cache.Clear();
            Cache[(path, width)] = image;
            return image;
        }
        catch (Exception e)
        {
            StartupLog.Step($"Couldn't show a cover: {e.Message}");
            return null;
        }
    }

    /// <summary>The file's bytes if it's a picture of a sensible size. Runs off the UI thread.</summary>
    private static byte[]? Read(string path)
    {
        try
        {
            var file = new FileInfo(path);
            if (!file.Exists || file.Length == 0 || file.Length > MaxBytes) return null;
            var bytes = File.ReadAllBytes(path);
            using var stream = new InMemoryRandomAccessStream();
            stream.WriteAsync(bytes.AsBuffer()).AsTask().GetAwaiter().GetResult();
            stream.Seek(0);
            var decoder = BitmapDecoder.CreateAsync(stream).AsTask().GetAwaiter().GetResult();
            return (long)decoder.PixelWidth * decoder.PixelHeight <= MaxPixels ? bytes : null;
        }
        catch (Exception)
        {
            return null; // deleted, unreadable or not a picture: no cover
        }
    }
}
