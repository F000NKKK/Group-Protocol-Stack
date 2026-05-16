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

    /// No key material is available for the KID carried in the frame.
    #[error("unknown KID {0:#x}: epoch or sender not registered")]
    UnknownKid(u64),
}
