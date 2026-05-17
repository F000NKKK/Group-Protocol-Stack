using System.Text.Json;

namespace GBPStack;

/// <summary>Outcome of <see cref="GapClient.Accept"/>.</summary>
public sealed record GapAcceptResult(string Status, uint? Source, uint? Seq, int? Bytes, string? Reason)
{
    internal static GapAcceptResult Parse(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var r = doc.RootElement;
        return new GapAcceptResult(
            r.GetProperty("status").GetString() ?? "?",
            r.TryGetProperty("source", out var s) && s.ValueKind == JsonValueKind.Number ? s.GetUInt32() : null,
            r.TryGetProperty("seq", out var q) && q.ValueKind == JsonValueKind.Number ? q.GetUInt32() : null,
            r.TryGetProperty("bytes", out var b) && b.ValueKind == JsonValueKind.Number ? b.GetInt32() : null,
            r.TryGetProperty("reason", out var rs) && rs.ValueKind == JsonValueKind.String ? rs.GetString() : null);
    }
}
