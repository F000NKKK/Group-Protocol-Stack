using System;

namespace GBPStack;

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
