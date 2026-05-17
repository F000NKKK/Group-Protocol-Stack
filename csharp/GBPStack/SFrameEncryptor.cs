using System;

namespace GBPStack;

/// <summary>
/// Stateful per-sender SFrame encryptor.
///
/// <para>Maintains an internal counter that increments on every
/// <see cref="Encrypt"/> call.  Do <em>not</em> share across threads.</para>
/// </summary>
public sealed class SFrameEncryptor : IDisposable
{
    private int _handle;
    private bool _disposed;

    internal SFrameEncryptor(int handle) => _handle = handle;

    /// <summary>
    /// Encrypts one audio frame and returns the SFrame payload
    /// (header + ciphertext + GCM tag).
    /// </summary>
    /// <param name="plaintext">The raw Opus frame bytes.</param>
    /// <param name="extraAad">Additional authenticated data; empty if none.</param>
    /// <exception cref="InvalidOperationException">Thrown on encryption failure.</exception>
    public byte[] Encrypt(
        ReadOnlySpan<byte> plaintext,
        ReadOnlySpan<byte> extraAad = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        Native.GbpBuffer buf;
        unsafe
        {
            fixed (byte* ptPtr = plaintext)
            fixed (byte* aadPtr = extraAad.IsEmpty ? (ReadOnlySpan<byte>)[] : extraAad)
            {
                buf = Native.gbp_sframe_encrypt(
                    _handle,
                    (IntPtr)ptPtr, (nuint)plaintext.Length,
                    extraAad.IsEmpty ? IntPtr.Zero : (IntPtr)aadPtr, (nuint)extraAad.Length);
            }
        }
        if (buf.IsEmpty)
            throw new InvalidOperationException(
                $"gbp_sframe_encrypt failed: {Native.LastError()}");
        return Native.CopyAndFree(buf);
    }

    /// <inheritdoc/>
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Native.gbp_sframe_encryptor_free(_handle);
        _handle = 0;
    }
}
