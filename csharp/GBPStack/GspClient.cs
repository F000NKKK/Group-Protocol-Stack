using System;
using System.Text.Json;

namespace GBPStack;

/// <summary>GSP signal type registry.</summary>
public enum SignalType : uint
{
    Join = 100,
    Leave = 101,
    RoleChange = 102,
    Mute = 200,
    Unmute = 201,
    StreamStart = 300,
    StreamStop = 301,
    CodecUpdate = 400
}

/// <summary>
/// Group Signaling Protocol client. Tracks <c>request_id</c> deduplication
/// and exposes accepted signals to the application.
/// </summary>
public sealed class GspClient : IDisposable
{
    /// <summary>Native handle.</summary>
    public int Handle { get; private set; }

    /// <summary>Creates a new GSP client.</summary>
    public static GspClient Create()
    {
        var h = Native.gsp_client_create();
        if (h <= 0) throw new InvalidOperationException("gsp_client_create");
        return new GspClient(h);
    }

    private GspClient(int h) => Handle = h;

    /// <summary>Sends a signal.</summary>
    public OutboundFrame Send(GroupNode node, MlsContext mls, uint target, SignalType signal, uint roleClaim, uint requestId)
    {
        var buf = Native.gsp_client_send(Handle, node.Handle, mls.Handle, target, (uint)signal, roleClaim, requestId);
        return GroupNode.Unpack(buf, "gsp_client_send");
    }

    /// <summary>
    /// Accepts a plaintext payload delivered by the GBP layer.
    /// <paramref name="currentEpoch"/> lets the client auto-reset its dedup
    /// state when the epoch advances.
    /// </summary>
    public GspAcceptResult Accept(byte[] plaintext, ulong currentEpoch)
    {
        IntPtr cstr;
        unsafe
        {
            fixed (byte* p = plaintext)
                cstr = Native.gsp_client_accept(Handle, currentEpoch, (IntPtr)p, (nuint)plaintext.Length);
        }
        return GspAcceptResult.Parse(Native.CopyAndFree(cstr));
    }

    /// <summary>Clears the request-id deduplication set. Intended for use after an epoch change.</summary>
    public void Reset() => Native.gsp_client_reset(Handle);

    /// <inheritdoc />
    public void Dispose()
    {
        if (Handle != 0) { Native.gsp_client_destroy(Handle); Handle = 0; }
        GC.SuppressFinalize(this);
    }

    ~GspClient() { Dispose(); }
}

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
