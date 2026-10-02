using System.Text.Json;
using System.Text.Json.Serialization;

namespace Bookshelf.Core;

/// <summary>
/// What the Windows app remembers between runs that isn't part of the
/// journal: which profile was open last. Kept as a small JSON file in the
/// journal's folder (an unpackaged app has no <c>ApplicationData</c>), so a
/// demo or test journal remembers its own.
/// </summary>
public sealed record AppState
{
    /// <summary>The file's name inside the journal's folder.</summary>
    public const string FileName = "windows-app.json";

    /// <summary>Anything bigger isn't ours: don't read it.</summary>
    private const long MaxBytes = 64 * 1024;

    private static readonly JsonSerializerOptions Json = new() { WriteIndented = true };

    /// <summary>The profile that was open when the app last closed or switched.</summary>
    [JsonPropertyName("lastProfileId")]
    public string? LastProfileId { get; init; }

    /// <summary>
    /// Reads the state kept in <paramref name="folder"/>. A missing, damaged
    /// or unreadable file reads as a fresh start, never as an error: this is
    /// a convenience, and the journal itself is unaffected.
    /// </summary>
    public static AppState Load(string folder)
    {
        try
        {
            var file = new FileInfo(Path.Combine(folder, FileName));
            if (!file.Exists || file.Length > MaxBytes) return new AppState();
            return JsonSerializer.Deserialize<AppState>(File.ReadAllText(file.FullName), Json) ?? new AppState();
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException)
        {
            return new AppState();
        }
    }

    /// <summary>
    /// Writes the state into <paramref name="folder"/>: to a temporary file
    /// first, then moved over the old one, so a crash mid-write never leaves
    /// half a file. Returns false (and changes nothing) if it can't be written.
    /// </summary>
    public bool Save(string folder)
    {
        var path = Path.Combine(folder, FileName);
        var temp = path + ".tmp";
        try
        {
            File.WriteAllText(temp, JsonSerializer.Serialize(this, Json));
            File.Move(temp, path, overwrite: true);
            return true;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            try
            {
                File.Delete(temp);
            }
            catch (Exception cleanup) when (cleanup is IOException or UnauthorizedAccessException)
            {
            }
            return false;
        }
    }
}
