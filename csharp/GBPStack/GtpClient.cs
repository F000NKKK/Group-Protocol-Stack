using System.Text;

namespace GBPStack;

/// <summary>
/// Group Text Protocol client. Tracks idempotency by
/// <c>(sender_id, message_id)</c>.
/// </summary>
public sealed class GtpClient : IDisposable
{
    /// <summary>Native handle.</summary>
    public int Handle { get; private set; }

    /// <summary>Creates a new GTP client.</summary>
    public static GtpClient Create()
    {
        var h = Native.gtp_client_create();
        if (h <= 0) throw new InvalidOperationException("gtp_client_create");
        return new GtpClient(h);
    }

    private GtpClient(int h) => Handle = h;

    /// <summary>
    /// Sends a text message to <paramref name="target"/>.
    /// <paramref name="codec"/> selects the payload encoding (default: CBOR).
    /// </summary>
    public OutboundFrame Send(GroupNode node, MlsContext mls, uint target, ulong messageId, string text,
        PayloadCodec codec = PayloadCodec.Cbor)
    {
        var bytes = Encoding.UTF8.GetBytes(text);
        var buf = Native.WithBytes(bytes, (p, l) =>
            Native.gtp_client_send(Handle, node.Handle, mls.Handle, target, messageId, p, l, (byte)codec));
        return GroupNode.Unpack(buf, "gtp_client_send");
    }

    /// <summary>
    /// Accepts a plaintext payload delivered by the GBP layer.
    /// <paramref name="currentEpoch"/> lets the client auto-reset its
    /// idempotency state when the epoch advances.
    /// <paramref name="codec"/> must match <see cref="NodeEvent.Codec"/> from the event.
    /// </summary>
    public GtpAcceptResult Accept(byte[] plaintext, ulong currentEpoch, PayloadCodec codec = PayloadCodec.Cbor)
    {
        IntPtr cstr;
        unsafe
        {
            fixed (byte* p = plaintext)
                cstr = Native.gtp_client_accept(Handle, currentEpoch, (IntPtr)p, (nuint)plaintext.Length, (byte)codec);
        }
        return GtpAcceptResult.Parse(Native.CopyAndFree(cstr));
    }

    /// <summary>Clears the idempotency state. Intended for use after an epoch change.</summary>
    public void Reset() => Native.gtp_client_reset(Handle);

    /// <inheritdoc />
    public void Dispose()
    {
        if (Handle != 0) { Native.gtp_client_destroy(Handle); Handle = 0; }
        GC.SuppressFinalize(this);
    }

    ~GtpClient() { Dispose(); }
}
