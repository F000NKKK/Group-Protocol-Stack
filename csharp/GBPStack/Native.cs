using System;
using System.Runtime.InteropServices;

namespace GBPStack;

/// <summary>
/// P/Invoke surface over the native <c>gbp_stack</c> shared library
/// (cdylib produced by the <c>gbp-stack-ffi</c> Rust crate).
/// </summary>
internal static class Native
{
    private const string Lib = "gbp_stack";

    /// <summary>FFI buffer triple <c>(ptr, len, cap)</c>. Released via <see cref="gbp_buffer_free"/>.</summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct GbpBuffer
    {
        public IntPtr Ptr;
        public nuint Len;
        public nuint Cap;

        public bool IsEmpty => Ptr == IntPtr.Zero || Len == 0;
    }

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gbp_buffer_free(GbpBuffer buf);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gbp_string_free(IntPtr ptr);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr gbp_last_error();

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr gbp_version();

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gbp_mls_create(IntPtr identityPtr, nuint identityLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gbp_mls_destroy(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern ulong gbp_mls_epoch(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_mls_group_id(int handle, IntPtr out16);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gbp_mls_export_key_package(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gbp_mls_invite(int handle, IntPtr kpPtr, nuint kpLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gbp_mls_invite_full(int handle, IntPtr kpPtr, nuint kpLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gbp_mls_remove(int handle, uint leafIndex);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern uint gbp_mls_process_message(int handle, IntPtr msgPtr, nuint msgLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_mls_finalize_commit(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_mls_clear_pending_commit(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_mls_accept_welcome(int handle, IntPtr welcomePtr, nuint welcomeLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gbp_node_create(uint memberId, IntPtr groupId16);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gbp_node_destroy(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_node_bootstrap_creator(int handle, ulong epoch);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_node_bootstrap_joiner(int handle, ulong epoch, uint expectedFirstTid);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern uint gbp_node_state(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern ulong gbp_node_epoch(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern uint gbp_node_last_transition_id(int handle);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_node_set_epoch(int handle, ulong epoch);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static extern bool gbp_node_apply_transition(int handle, uint tid);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gbp_node_send_control(
        int nh, int mh, uint target, ushort opcode, uint transitionId, uint requestId,
        IntPtr argsPtr, nuint argsLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr gbp_node_on_wire(int nh, int mh, IntPtr wirePtr, nuint wireLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr gbp_node_drain_events(int nh);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gtp_client_create();

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gtp_client_destroy(int h);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gtp_client_reset(int h);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gtp_client_send(
        int ch, int nh, int mh, uint target, ulong messageId,
        IntPtr textPtr, nuint textLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr gtp_client_accept(int ch, ulong currentEpoch, IntPtr ptPtr, nuint ptLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gap_client_create();

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gap_client_destroy(int h);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gap_client_reset(int h);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gap_client_send(
        int ch, int nh, int mh, uint target, uint mediaSourceId, ulong rtpTimestamp,
        IntPtr opusPtr, nuint opusLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr gap_client_accept(int ch, ulong currentEpoch, IntPtr ptPtr, nuint ptLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gsp_client_create();

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gsp_client_destroy(int h);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gsp_client_reset(int h);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gsp_client_send(
        int ch, int nh, int mh, uint target, uint signalType, uint roleClaim, uint requestId);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr gsp_client_accept(int ch, ulong currentEpoch, IntPtr ptPtr, nuint ptLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gbp_frame_encode_v(
        byte version, IntPtr groupId16, ulong epoch, uint transitionId, uint streamType,
        uint streamId, ushort flags, uint sequenceNo, IntPtr payloadPtr, nuint payloadLen);

    // ── SFrame ────────────────────────────────────────────────────────────────

    /// <summary>Creates an SFrame session from an MLS context handle.</summary>
    /// <param name="mlsHandle">Handle from <c>gbp_mls_create</c>.</param>
    /// <param name="suite">0 = AES-128-GCM, 1 = AES-256-GCM.</param>
    /// <param name="labelPtr">UTF-8 export label (e.g. "gbp/sframe v1").</param>
    /// <param name="labelLen">Byte length of the label.</param>
    /// <returns>Positive session handle, or 0 on failure.</returns>
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gbp_sframe_session_create(
        int mlsHandle, byte suite, IntPtr labelPtr, nuint labelLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gbp_sframe_session_free(int handle);

    /// <summary>Creates a per-sender encryptor for the given leaf index.</summary>
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern int gbp_sframe_encryptor_create(
        int mlsHandle, int sessionHandle, uint leafIndex, byte suite,
        IntPtr labelPtr, nuint labelLen);

    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern void gbp_sframe_encryptor_free(int handle);

    /// <summary>Encrypts one audio frame. Caller must free result with <c>gbp_buffer_free</c>.</summary>
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern GbpBuffer gbp_sframe_encrypt(
        int encHandle,
        IntPtr plaintextPtr, nuint plaintextLen,
        IntPtr aadPtr, nuint aadLen);

    /// <summary>Decrypts one SFrame payload. Fills <paramref name="senderLeaf"/> with the sender's leaf index.</summary>
    [DllImport(Lib, CallingConvention = CallingConvention.Cdecl)]
    public static extern unsafe GbpBuffer gbp_sframe_decrypt(
        int sessionHandle,
        IntPtr payloadPtr, nuint payloadLen,
        IntPtr aadPtr, nuint aadLen,
        uint* senderLeaf);

    public static byte[] CopyAndFree(GbpBuffer buf)
    {
        if (buf.IsEmpty) { gbp_buffer_free(buf); return Array.Empty<byte>(); }
        var len = (int)buf.Len;
        var arr = new byte[len];
        Marshal.Copy(buf.Ptr, arr, 0, len);
        gbp_buffer_free(buf);
        return arr;
    }

    public static string CopyAndFree(IntPtr cstr)
    {
        if (cstr == IntPtr.Zero) return string.Empty;
        try { return Marshal.PtrToStringUTF8(cstr) ?? string.Empty; }
        finally { gbp_string_free(cstr); }
    }

    public static string LastError() => CopyAndFree(gbp_last_error());

    public static T WithBytes<T>(ReadOnlySpan<byte> data, Func<IntPtr, nuint, T> fn)
    {
        unsafe
        {
            fixed (byte* p = data) return fn((IntPtr)p, (nuint)data.Length);
        }
    }
}
