using System.Text;

namespace GBPStack;

/// <summary>
/// SFrame E2EE session for one MLS epoch.
///
/// <para>Derives <c>sframe_base_key</c> from the MLS <c>ExportSecret</c> and
/// provides per-sender encryptors and a multi-sender decryptor.  Create a new
/// session after every MLS commit (epoch change).</para>
///
/// <para>Thread-safe for concurrent decrypt calls.  The encryptor handle is
/// <em>not</em> thread-safe — use one encryptor per thread or protect it
/// externally.</para>
/// </summary>
public sealed class SFrameSession : IDisposable
{
    private int _sessionHandle;
    private bool _disposed;

    private SFrameSession(int handle) => _sessionHandle = handle;

    /// <summary>
    /// Creates an SFrame session from an existing <see cref="MlsContext"/>.
    /// </summary>
    /// <param name="mls">The MLS context for the current epoch.</param>
    /// <param name="label">Export label (e.g. <c>"gbp/sframe v1"</c>).</param>
    /// <param name="suite">Ciphersuite — AES-128-GCM or AES-256-GCM.</param>
    /// <exception cref="InvalidOperationException">
    /// Thrown when the native call fails (check <c>Native.LastError()</c>).
    /// </exception>
    public static SFrameSession Create(
        MlsContext mls,
        string label = "gbp/sframe v1",
        SFrameCipherSuite suite = SFrameCipherSuite.Aes128Gcm)
    {
        ArgumentNullException.ThrowIfNull(mls);
        var labelBytes = Encoding.UTF8.GetBytes(label);
        int handle = Native.WithBytes<int>(labelBytes,
            (ptr, len) => Native.gbp_sframe_session_create(
                mls.Handle, (byte)suite, ptr, len));
        if (handle == 0)
            throw new InvalidOperationException(
                $"gbp_sframe_session_create failed: {Native.LastError()}");
        return new SFrameSession(handle);
    }

    /// <summary>
    /// Creates a per-sender <see cref="SFrameEncryptor"/> for <paramref name="leafIndex"/>.
    /// </summary>
    /// <param name="mls">The MLS context for the current epoch.</param>
    /// <param name="leafIndex">The local sender's MLS leaf index.</param>
    /// <param name="label">Must match the label used in <see cref="Create"/>.</param>
    /// <param name="suite">Must match the suite used in <see cref="Create"/>.</param>
    public SFrameEncryptor CreateEncryptor(
        MlsContext mls,
        uint leafIndex,
        string label = "gbp/sframe v1",
        SFrameCipherSuite suite = SFrameCipherSuite.Aes128Gcm)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        ArgumentNullException.ThrowIfNull(mls);
        var labelBytes = Encoding.UTF8.GetBytes(label);
        int encHandle = Native.WithBytes<int>(labelBytes,
            (ptr, len) => Native.gbp_sframe_encryptor_create(
                mls.Handle, _sessionHandle, leafIndex, (byte)suite, ptr, len));
        if (encHandle == 0)
            throw new InvalidOperationException(
                $"gbp_sframe_encryptor_create failed: {Native.LastError()}");
        return new SFrameEncryptor(encHandle);
    }

    /// <summary>
    /// Decrypts one SFrame payload and returns the plaintext Opus frame
    /// together with the sender's leaf index.
    /// </summary>
    /// <param name="payload">The full SFrame payload (header + ciphertext + tag).</param>
    /// <param name="extraAad">Additional authenticated data (e.g. RTP header); empty if none.</param>
    /// <returns>Plaintext bytes and the sender's leaf index.</returns>
    /// <exception cref="InvalidOperationException">Thrown on decryption failure.</exception>
    public (byte[] Plaintext, uint SenderLeaf) Decrypt(
        ReadOnlySpan<byte> payload,
        ReadOnlySpan<byte> extraAad = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        Native.GbpBuffer buf;
        uint senderLeaf = 0;
        unsafe
        {
            fixed (byte* payPtr = payload)
            fixed (byte* aadPtr = extraAad.IsEmpty ? (ReadOnlySpan<byte>)[] : extraAad)
            {
                buf = Native.gbp_sframe_decrypt(
                    _sessionHandle,
                    (IntPtr)payPtr, (nuint)payload.Length,
                    extraAad.IsEmpty ? IntPtr.Zero : (IntPtr)aadPtr, (nuint)extraAad.Length,
                    &senderLeaf);
            }
        }
        if (buf.IsEmpty)
            throw new InvalidOperationException(
                $"gbp_sframe_decrypt failed: {Native.LastError()}");
        return (Native.CopyAndFree(buf), senderLeaf);
    }

    /// <inheritdoc/>
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Native.gbp_sframe_session_free(_sessionHandle);
        _sessionHandle = 0;
    }

    /// <summary>Internal session handle for FFI calls.</summary>
    internal int Handle => _sessionHandle;
}
