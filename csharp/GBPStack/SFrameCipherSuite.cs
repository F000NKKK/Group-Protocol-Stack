namespace GBPStack;

/// <summary>SFrame ciphersuite.</summary>
public enum SFrameCipherSuite : byte
{
    /// <summary>AES-128-GCM (16-byte key).</summary>
    Aes128Gcm = 0,
    /// <summary>AES-256-GCM (32-byte key).</summary>
    Aes256Gcm = 1,
}
