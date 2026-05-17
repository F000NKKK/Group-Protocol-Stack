using System.Text;

namespace GBPStack;

/// <summary>
/// Managed wrapper around an MLS (RFC 9420) context owned by the native library.
/// Owns a single-member group plus a published <c>KeyPackage</c> that can be
/// used to invite this member into another group.
/// </summary>
public sealed class MlsContext : IDisposable
{
    /// <summary>Native handle.</summary>
    public int Handle { get; private set; }

    /// <summary>Application-level identity.</summary>
    public string Identity { get; }

    private MlsContext(int handle, string identity)
    {
        Handle = handle;
        Identity = identity;
    }

    /// <summary>Creates a fresh MLS context.</summary>
    public static MlsContext Create(string identity)
    {
        var bytes = Encoding.UTF8.GetBytes(identity);
        var h = Native.WithBytes(bytes, (p, l) => Native.gbp_mls_create(p, l));
        if (h <= 0) throw new InvalidOperationException($"gbp_mls_create: {Native.LastError()}");
        return new MlsContext(h, identity);
    }

    /// <summary>Returns the current group epoch.</summary>
    public ulong Epoch => Native.gbp_mls_epoch(Handle);

    /// <summary>Returns the 16-byte group identifier.</summary>
    public byte[] GroupId
    {
        get
        {
            var arr = new byte[16];
            unsafe
            {
                fixed (byte* p = arr)
                {
                    if (!Native.gbp_mls_group_id(Handle, (IntPtr)p))
                        throw new InvalidOperationException($"group_id: {Native.LastError()}");
                }
            }
            return arr;
        }
    }

    /// <summary>Exports the TLS-serialised KeyPackage of this member.</summary>
    public byte[] ExportKeyPackage()
    {
        var buf = Native.gbp_mls_export_key_package(Handle);
        if (buf.IsEmpty) throw new InvalidOperationException($"export_kp: {Native.LastError()}");
        return Native.CopyAndFree(buf);
    }

    /// <summary>Invites the given KeyPackage into the local group; returns the Welcome bytes.</summary>
    /// <remarks>
    /// This overload preserves the v1.0 API surface and only returns the Welcome.
    /// Use <see cref="InviteFull"/> if you need the Commit message to broadcast to existing members.
    /// </remarks>
    public byte[] Invite(byte[] keyPackage)
    {
        var buf = Native.WithBytes(keyPackage, (p, l) => Native.gbp_mls_invite(Handle, p, l));
        if (buf.IsEmpty) throw new InvalidOperationException($"invite: {Native.LastError()}");
        return Native.CopyAndFree(buf);
    }

    /// <summary>
    /// Invites the given KeyPackage and returns BOTH the MLS Commit (broadcast to
    /// existing members) and Welcome (unicast to the new joiner). RFC 9420 §11/§12.4
    /// requires existing members to apply the Commit to advance their epoch.
    /// </summary>
    /// <remarks>
    /// Does NOT merge the pending commit. Call <see cref="FinalizeCommit"/> after
    /// the Commit/Welcome have been distributed (e.g. coordinator observed quorum
    /// READY) or <see cref="ClearPendingCommit"/> on abort.
    /// </remarks>
    public InviteResult InviteFull(byte[] keyPackage)
    {
        var buf = Native.WithBytes(keyPackage, (p, l) => Native.gbp_mls_invite_full(Handle, p, l));
        if (buf.IsEmpty) throw new InvalidOperationException($"invite_full: {Native.LastError()}");
        var bytes = Native.CopyAndFree(buf);
        if (bytes.Length < 4) throw new InvalidOperationException("invite_full: truncated buffer");
        uint commitLenU = BitConverter.ToUInt32(bytes, 0);
        if (commitLenU > int.MaxValue || commitLenU > (uint)(bytes.Length - 4))
            throw new InvalidOperationException(
                $"invite_full: bad commit_len {commitLenU} (buf.len={bytes.Length})");
        int commitLen = (int)commitLenU;
        var commit = new byte[commitLen];
        Buffer.BlockCopy(bytes, 4, commit, 0, commitLen);
        var welcomeLen = bytes.Length - 4 - commitLen;
        var welcome = new byte[welcomeLen];
        Buffer.BlockCopy(bytes, 4 + commitLen, welcome, 0, welcomeLen);
        return new InviteResult(commit, welcome);
    }

    /// <summary>
    /// Merges any pending commit produced by <see cref="InviteFull"/> or
    /// <see cref="RemoveMember"/>. Idempotent.
    /// </summary>
    public void FinalizeCommit()
    {
        if (!Native.gbp_mls_finalize_commit(Handle))
            throw new InvalidOperationException($"finalize_commit: {Native.LastError()}");
    }

    /// <summary>
    /// Discards any pending commit without applying it. Used on
    /// <c>ABORT_TRANSITION</c>.
    /// </summary>
    public void ClearPendingCommit()
    {
        if (!Native.gbp_mls_clear_pending_commit(Handle))
            throw new InvalidOperationException($"clear_pending_commit: {Native.LastError()}");
    }

    /// <summary>
    /// Removes the member at the given MLS LeafIndex and returns the Commit
    /// that remaining members must apply via <see cref="ProcessMessage"/>.
    /// </summary>
    /// <remarks>
    /// Does NOT merge the pending commit. Call <see cref="FinalizeCommit"/> after
    /// distribution succeeds or <see cref="ClearPendingCommit"/> on abort.
    /// </remarks>
    public byte[] RemoveMember(uint leafIndex)
    {
        var buf = Native.gbp_mls_remove(Handle, leafIndex);
        if (buf.IsEmpty) throw new InvalidOperationException($"remove: {Native.LastError()}");
        return Native.CopyAndFree(buf);
    }

    /// <summary>
    /// Applies a Commit (or staged Proposal) message to the local MLS group.
    /// Existing members invoke this after receiving the Commit broadcast
    /// embedded in <c>PREPARE_TRANSITION</c> args.
    /// </summary>
    public ProcessedKind ProcessMessage(byte[] message)
    {
        uint code = Native.WithBytes(message, (p, l) => Native.gbp_mls_process_message(Handle, p, l));
        return code switch
        {
            1 => ProcessedKind.Commit,
            2 => ProcessedKind.Application,
            3 => ProcessedKind.Proposal,
            4 => ProcessedKind.External,
            _ => throw new InvalidOperationException($"process_message: {Native.LastError()}"),
        };
    }

    /// <summary>Replaces the local group with the one described by the Welcome.</summary>
    public void AcceptWelcome(byte[] welcome)
    {
        var ok = Native.WithBytes(welcome, (p, l) => Native.gbp_mls_accept_welcome(Handle, p, l));
        if (!ok) throw new InvalidOperationException($"accept_welcome: {Native.LastError()}");
    }

    /// <inheritdoc />
    public void Dispose()
    {
        if (Handle != 0)
        {
            Native.gbp_mls_destroy(Handle);
            Handle = 0;
        }
        GC.SuppressFinalize(this);
    }

    ~MlsContext() { Dispose(); }
}
