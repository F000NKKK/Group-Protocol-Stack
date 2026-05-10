using System;
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
    public byte[] Invite(byte[] keyPackage)
    {
        var buf = Native.WithBytes(keyPackage, (p, l) => Native.gbp_mls_invite(Handle, p, l));
        if (buf.IsEmpty) throw new InvalidOperationException($"invite: {Native.LastError()}");
        return Native.CopyAndFree(buf);
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
