//! **GBP-SFrame** — SFrame ([draft-ietf-sframe-enc]) E2EE for GAP audio
//! streams in the Group Protocol Stack.
//!
//! # Overview
//!
//! SFrame sits *inside* SRTP (or any transport-level encryption) and provides
//! **end-to-end** confidentiality for media payloads: the SFU can forward
//! packets based on RTP headers without seeing the Opus frame content.
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │              Transport encryption                │  ← client ↔ SFU
//! │  ┌────────────────────────────────────────────┐  │
//! │  │               SFrame                       │  │  ← E2E client ↔ client
//! │  │   ┌──────────────────────────────────────┐  │  │
//! │  │   │   Encoded media (Opus / VP8 / VP9)   │  │  │
//! │  │   └──────────────────────────────────────┘  │  │
//! │  └────────────────────────────────────────────┘  │
//! └──────────────────────────────────────────────────┘
//! ```
//!
//! # Key derivation
//!
//! After each MLS epoch change:
//!
//! 1. **Base key** — `MLS.ExportSecret(label, context=epoch_be8, length=32)`.
//! 2. **Per-sender key** — `HKDF-Expand(base_key, "gbp sframe key " ‖ leaf_be4, L)`.
//! 3. **Per-sender salt** — `HKDF-Expand(base_key, "gbp sframe salt " ‖ leaf_be4, 12)`.
//! 4. **Frame nonce** — `salt XOR (CTR_LE64 ‖ 0x00_00_00_00)`.
//!
//! The `label` passed to [`SFrameSession::from_mls`] is application-defined
//! (e.g. `"gbp/sframe v1"`); this lets different deployments use distinct
//! key universes without changing any other parameter.
//!
//! # Quick start
//!
//! ```
//! use gbp_sframe::{SFrameSession, CipherSuite};
//!
//! // Both sides derive a session from the same base key (in production this
//! // comes from SFrameSession::from_mls).
//! let session = SFrameSession::new([0x42u8; 32], 1, CipherSuite::Aes128Gcm);
//!
//! let mut enc = session.encryptor(0);
//! let payload = enc.encrypt(b"hello audio", b"")?;
//!
//! let mut dec = session.decryptor();
//! let (plaintext, sender_leaf) = dec.decrypt(&payload, b"")?;
//! assert_eq!(plaintext, b"hello audio");
//! assert_eq!(sender_leaf, 0);
//! # Ok::<(), gbp_sframe::SFrameError>(())
//! ```
//!
//! [draft-ietf-sframe-enc]: https://datatracker.ietf.org/doc/draft-ietf-sframe-enc/

#![deny(missing_docs)]

/// AEAD encrypt/decrypt and the stateful encryptor/decryptor types.
pub mod cipher;
/// Error type for SFrame operations.
pub mod error;
/// SFrame header wire format.
pub mod header;
/// Key derivation from MLS export secret.
pub mod kdf;
/// Sliding-window replay protection.
pub mod replay;

pub use cipher::{SFrameDecryptor, SFrameEncryptor};
pub use error::SFrameError;
pub use header::SFrameHeader;
pub use kdf::{CipherSuite, derive_base_key};

use gbp_mls::MlsContext;
use kdf::derive_participant;

/// An SFrame session bound to one MLS epoch.
///
/// A new session must be created whenever the MLS group commits (epoch
/// changes) — the old base key becomes unreachable and all per-sender keys
/// are rotated automatically.
pub struct SFrameSession {
    base_key: [u8; 32],
    epoch: u64,
    suite: CipherSuite,
}

impl SFrameSession {
    /// Creates a session from a raw 32-byte base key.
    ///
    /// Prefer [`from_mls`](Self::from_mls) when an [`MlsContext`] is
    /// available; this constructor is mainly for testing.
    pub fn new(base_key: [u8; 32], epoch: u64, suite: CipherSuite) -> Self {
        Self { base_key, epoch, suite }
    }

    /// Derives a session from the current MLS group state.
    ///
    /// Calls `MLS.ExportSecret(label, context=epoch_be8, length=32)` to
    /// obtain the base key, then stores it alongside the current epoch and
    /// ciphersuite.
    ///
    /// `label` is application-defined (e.g. `"gbp/sframe v1"`).
    pub fn from_mls(
        mls: &MlsContext,
        label: &str,
        suite: CipherSuite,
    ) -> Result<Self, SFrameError> {
        let epoch = mls.epoch();
        let base_key = derive_base_key(mls, label, epoch)?;
        Ok(Self::new(base_key, epoch, suite))
    }

    /// Returns the MLS epoch this session was created for.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Returns the active ciphersuite.
    pub fn suite(&self) -> CipherSuite {
        self.suite
    }

    /// Creates a sender-side encryptor for `leaf_index`.
    ///
    /// The returned [`SFrameEncryptor`] owns the derived key+salt for this
    /// sender and maintains an internal counter.  Create one per sender; do
    /// **not** share an encryptor across multiple goroutines/threads.
    pub fn encryptor(&self, leaf_index: u32) -> SFrameEncryptor {
        let kid = SFrameHeader::kid_from(self.epoch, leaf_index);
        let keys = derive_participant(&self.base_key, leaf_index, self.suite);
        SFrameEncryptor::new(keys, kid, self.suite)
    }

    /// Creates a receiver-side decryptor for this epoch.
    ///
    /// The [`SFrameDecryptor`] lazily derives per-sender keys as new KIDs
    /// arrive, and maintains an independent 1024-entry replay window per sender.
    pub fn decryptor(&self) -> SFrameDecryptor {
        SFrameDecryptor::new(self.base_key, self.epoch, self.suite)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session(epoch: u64) -> SFrameSession {
        SFrameSession::new([0x42u8; 32], epoch, CipherSuite::Aes128Gcm)
    }

    #[test]
    fn encrypt_decrypt_roundtrip_128() {
        let session = test_session(1);
        let mut enc = session.encryptor(0);
        let mut dec = session.decryptor();

        let frame = b"hello sframe";
        let payload = enc.encrypt(frame, b"").unwrap();
        let (plain, leaf) = dec.decrypt(&payload, b"").unwrap();
        assert_eq!(plain, frame);
        assert_eq!(leaf, 0);
    }

    #[test]
    fn encrypt_decrypt_roundtrip_256() {
        let session = SFrameSession::new([0x11u8; 32], 5, CipherSuite::Aes256Gcm);
        let mut enc = session.encryptor(3);
        let mut dec = session.decryptor();

        let frame = b"audio payload aes256";
        let payload = enc.encrypt(frame, b"rtp-header").unwrap();
        let (plain, leaf) = dec.decrypt(&payload, b"rtp-header").unwrap();
        assert_eq!(plain, frame);
        assert_eq!(leaf, 3);
    }

    #[test]
    fn wrong_aad_fails_decryption() {
        let session = test_session(0);
        let mut enc = session.encryptor(0);
        let mut dec = session.decryptor();

        let payload = enc.encrypt(b"data", b"correct-aad").unwrap();
        assert!(dec.decrypt(&payload, b"wrong-aad").is_err());
    }

    #[test]
    fn replay_rejected() {
        let session = test_session(0);
        let mut enc = session.encryptor(1);
        let mut dec = session.decryptor();

        let payload = enc.encrypt(b"frame", b"").unwrap();
        dec.decrypt(&payload, b"").unwrap();
        assert!(dec.decrypt(&payload, b"").is_err());
    }

    #[test]
    fn multi_sender() {
        let session = test_session(2);
        let mut enc0 = session.encryptor(0);
        let mut enc1 = session.encryptor(1);
        let mut dec = session.decryptor();

        let p0 = enc0.encrypt(b"from-0", b"").unwrap();
        let p1 = enc1.encrypt(b"from-1", b"").unwrap();

        let (msg0, leaf0) = dec.decrypt(&p0, b"").unwrap();
        let (msg1, leaf1) = dec.decrypt(&p1, b"").unwrap();

        assert_eq!(msg0, b"from-0");
        assert_eq!(leaf0, 0);
        assert_eq!(msg1, b"from-1");
        assert_eq!(leaf1, 1);
    }

    #[test]
    fn epoch_mismatch_rejected() {
        let session_a = test_session(1);
        let session_b = test_session(2); // different epoch

        let mut enc = session_a.encryptor(0);
        let mut dec = session_b.decryptor();

        let payload = enc.encrypt(b"stale", b"").unwrap();
        assert!(dec.decrypt(&payload, b"").is_err());
    }
}
