using System;
using System.Text.Json;

namespace GBPStack;

/// <summary>
/// Group Audio Protocol client. Maintains a per-source <c>rtp_sequence</c>
/// window and validates <c>key_phase</c> against the current group epoch.
/// </summary>
public sealed class GapClient : IDisposable
{
    /// <summary>Native handle.</summary>
    public int Handle { get; private set; }

    /// <summary>Creates a new GAP client.</summary>
    public static GapClient Create()
    {
        var h = Native.gap_client_create();
        if (h <= 0) throw new InvalidOperationException("gap_client_create");
        return new GapClient(h);
    }

    private GapClient(int h) => Handle = h;

    /// <summary>Sends an Opus frame.</summary>
    public OutboundFrame Send(GroupNode node, MlsContext mls, uint target,
        uint mediaSourceId, ulong rtpTimestamp, byte[] opus)
    {
        var buf = Native.WithBytes(opus, (p, l) =>
            Native.gap_client_send(Handle, node.Handle, mls.Handle, target, mediaSourceId, rtpTimestamp, p, l));
        return GroupNode.Unpack(buf, "gap_client_send");
    }

    /// <summary>Accepts a plaintext payload delivered by the GBP layer.</summary>
    public GapAcceptResult Accept(byte[] plaintext, ulong currentEpoch)
    {
        IntPtr cstr;
        unsafe
        {
            fixed (byte* p = plaintext)
                cstr = Native.gap_client_accept(Handle, currentEpoch, (IntPtr)p, (nuint)plaintext.Length);
        }
        return GapAcceptResult.Parse(Native.CopyAndFree(cstr));
    }

    /// <summary>Clears the replay window. Intended for use after an epoch change.</summary>
    public void Reset() => Native.gap_client_reset(Handle);

    /// <inheritdoc />
    public void Dispose()
    {
        if (Handle != 0) { Native.gap_client_destroy(Handle); Handle = 0; }
        GC.SuppressFinalize(this);
    }

    ~GapClient() { Dispose(); }
}

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
