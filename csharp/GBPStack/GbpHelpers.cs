using System;

namespace GBPStack;

/// <summary>Low-level GBP frame and error-code utilities.</summary>
public static class GbpHelpers
{
    /// <summary>
    /// Encodes a raw GBP frame to CBOR bytes.
    /// Most callers should use <see cref="GroupNode.SendControl"/> or the
    /// sub-protocol <c>Send</c> methods instead.
    /// </summary>
    public static byte[] EncodeFrame(
        byte version,
        byte[] groupId16,
        ulong epoch,
        uint transitionId,
        uint streamType,
        uint streamId,
        ushort flags,
        uint sequenceNo,
        byte[] payload)
    {
        if (groupId16.Length != 16) throw new ArgumentException("groupId must be 16 bytes");
        Native.GbpBuffer buf;
        unsafe
        {
            fixed (byte* gid = groupId16)
            fixed (byte* pay = payload)
            {
                buf = Native.gbp_frame_encode_v(
                    version, (IntPtr)gid, epoch, transitionId, streamType,
                    streamId, flags, sequenceNo, (IntPtr)pay, (nuint)payload.Length);
            }
        }
        var data = Native.CopyAndFree(buf);
        if (data.Length == 0) throw new InvalidOperationException(Native.LastError());
        return data;
    }

    /// <summary>
    /// Returns the CBOR-encoded <c>ErrorObject</c> for <paramref name="code"/>,
    /// or <c>null</c> if the code is unknown.
    /// </summary>
    public static byte[]? LookupError(ushort code)
    {
        var buf = Native.gbp_error_lookup(code);
        var data = Native.CopyAndFree(buf);
        return data.Length > 0 ? data : null;
    }
}
