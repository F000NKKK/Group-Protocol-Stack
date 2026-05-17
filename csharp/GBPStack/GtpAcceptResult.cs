using System.Text.Json;

namespace GBPStack;

/// <summary>Outcome of <see cref="GtpClient.Accept"/>.</summary>
public sealed record GtpAcceptResult(string Status, uint? Sender, ulong? MessageId, string? Text, string? Reason)
{
    internal static GtpAcceptResult Parse(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var r = doc.RootElement;
        return new GtpAcceptResult(
            r.GetProperty("status").GetString() ?? "?",
            r.TryGetProperty("sender", out var s) && s.ValueKind == JsonValueKind.Number ? s.GetUInt32() : null,
            r.TryGetProperty("message_id", out var m) && m.ValueKind == JsonValueKind.Number ? m.GetUInt64() : null,
            r.TryGetProperty("text", out var t) && t.ValueKind == JsonValueKind.String ? t.GetString() : null,
            r.TryGetProperty("reason", out var rs) && rs.ValueKind == JsonValueKind.String ? rs.GetString() : null);
    }
}
