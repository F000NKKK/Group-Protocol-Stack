using System.Text.Json;

namespace GBPStack;

/// <summary>Outcome of <see cref="GspClient.Accept"/>.</summary>
public sealed record GspAcceptResult(
    string Status, string? Signal, SignalType? SignalCode, uint? Sender,
    uint? RoleClaim, uint? RequestId, string? Reason)
{
    internal static GspAcceptResult Parse(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var r = doc.RootElement;
        SignalType? code = r.TryGetProperty("signal_code", out var s) && s.ValueKind == JsonValueKind.Number
            ? (SignalType)s.GetUInt32() : null;
        return new GspAcceptResult(
            r.GetProperty("status").GetString() ?? "?",
            r.TryGetProperty("signal", out var sg) && sg.ValueKind == JsonValueKind.String ? sg.GetString() : null,
            code,
            r.TryGetProperty("sender", out var sn) && sn.ValueKind == JsonValueKind.Number ? sn.GetUInt32() : null,
            r.TryGetProperty("role_claim", out var rc) && rc.ValueKind == JsonValueKind.Number ? rc.GetUInt32() : null,
            r.TryGetProperty("request_id", out var rid) && rid.ValueKind == JsonValueKind.Number ? rid.GetUInt32() : null,
            r.TryGetProperty("reason", out var rs) && rs.ValueKind == JsonValueKind.String ? rs.GetString() : null);
    }
}
