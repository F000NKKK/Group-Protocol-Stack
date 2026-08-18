/// Errors that can occur in the SFrame layer.
#[derive(Debug, thiserror::Error)]
pub enum SFrameError {
    /// MLS `ExportSecret` failed.
    #[error("MLS export failed: {0}")]
    MlsExport(String),

    /// AEAD encryption failed.
    #[error("AEAD encryption failed")]
    Encrypt,

    /// AEAD decryption failed (wrong key, tampered ciphertext, bad AAD).
    #[error("AEAD decryption failed")]
    Decrypt,

    /// SFrame header could not be parsed.
    #[error("invalid SFrame header: {0}")]
    Header(String),

    /// The counter in the received frame has already been seen — replay.
    #[error("replay detected: KID={kid} CTR={ctr}")]
    Replay {
        /// Key ID that carried the duplicate counter.
        kid: u64,
        /// The duplicated counter value.
        ctr: u64,
    },

    /// This sender used up every counter value for its KID. Encrypting again
    /// would have to reuse a `(key, KID, CTR)` nonce, so the encryptor refuses;
    /// a new epoch (and hence a new base key) is required.
    #[error("counter exhausted for KID {0:#x}: rotate the epoch")]
    CounterExhausted(u64),

    /// No key material is available for the KID carried in the frame.
    #[error("unknown KID {0:#x}: epoch or sender not registered")]
    UnknownKid(u64),
}
