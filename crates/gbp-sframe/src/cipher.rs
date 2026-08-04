use std::collections::HashMap;

use sframe::frame::{EncryptedFrameView, MediaFrameView, MonotonicCounter, ReplayAttackProtection};
use sframe::header::KeyId;
use sframe::key::{DecryptionKey, EncryptionKey};

use crate::error::SFrameError;
use crate::header::SFrameHeader;
use crate::kdf::CipherSuite;

/// Replay-window tolerance in frames (gbp's historical fixed 1024-frame window).
const REPLAY_WINDOW: u64 = 1024;

// ─── SFrameEncryptor ─────────────────────────────────────────────────────────

/// Stateful per-sender SFrame encryptor.
///
/// Holds the derived key for one `(epoch, leaf_index)` KID and an internal
/// counter that increments on every call to [`encrypt`](Self::encrypt).
///
/// Obtain via [`crate::SFrameSession::encryptor`].
pub struct SFrameEncryptor {
    key: EncryptionKey,
    counter: MonotonicCounter,
    kid: KeyId,
}

impl SFrameEncryptor {
    pub(crate) fn new(base_key: &[u8; 32], kid: u64, suite: CipherSuite) -> Self {
        let key = EncryptionKey::derive_from(suite.to_sframe(), kid, base_key)
            .expect("key derivation from a 32-byte base key never fails");
        Self {
            key,
            counter: MonotonicCounter::default(),
            kid,
        }
    }

    /// Encrypts `plaintext` and returns the complete SFrame payload:
    /// `header ‖ ciphertext ‖ GCM-tag`.
    ///
    /// `extra_aad` is bound into the AEAD tag (e.g. an RTP header) but is **not**
    /// carried in the returned payload; the receiver supplies the same slice to
    /// [`SFrameDecryptor::decrypt`].
    pub fn encrypt(&mut self, plaintext: &[u8], extra_aad: &[u8]) -> Result<Vec<u8>, SFrameError> {
        let frame = MediaFrameView::with_meta_data(&mut self.counter, plaintext, extra_aad);
        let encrypted = frame.encrypt(&self.key).map_err(|_| SFrameError::Encrypt)?;

        // sframe serialises `meta_data ‖ header ‖ ciphertext`; strip the
        // metadata prefix so `extra_aad` stays off the wire.
        Ok(encrypted.as_ref()[extra_aad.len()..].to_vec())
    }

    /// Current counter value (number of frames encrypted so far).
    pub fn counter(&self) -> u64 {
        self.counter.current()
    }

    /// KID this encryptor was created for.
    pub fn kid(&self) -> u64 {
        self.kid
    }
}

// ─── SFrameDecryptor ─────────────────────────────────────────────────────────

/// Per-sender decryption state maintained inside [`SFrameDecryptor`].
///
/// One [`ReplayAttackProtection`] per KID: sframe's validator keys off the
/// counter only (it ignores the KID), so a shared validator would mix
/// interleaved senders' counters and raise false rejections.
struct SenderState {
    key: DecryptionKey,
    replay: ReplayAttackProtection,
}

/// Multi-sender SFrame decryptor for one epoch.
///
/// Lazily derives per-sender key material and replay state from the epoch's
/// base key as new `KID`s are encountered.
///
/// Obtain via [`crate::SFrameSession::decryptor`].
pub struct SFrameDecryptor {
    base_key: [u8; 32],
    epoch: u64,
    suite: CipherSuite,
    /// Keyed by KID.
    senders: HashMap<KeyId, SenderState>,
}

impl SFrameDecryptor {
    pub(crate) fn new(base_key: [u8; 32], epoch: u64, suite: CipherSuite) -> Self {
        Self {
            base_key,
            epoch,
            suite,
            senders: HashMap::new(),
        }
    }

    /// Decrypts an SFrame `payload` and returns `(plaintext, sender_leaf)`.
    ///
    /// `extra_aad` must be the same slice passed on the encrypting side.
    pub fn decrypt(
        &mut self,
        payload: &[u8],
        extra_aad: &[u8],
    ) -> Result<(Vec<u8>, u32), SFrameError> {
        let view = EncryptedFrameView::try_with_meta_data(&payload, &extra_aad)
            .map_err(|e| SFrameError::Header(e.to_string()))?;

        let kid = view.header().key_id();
        let ctr = view.header().counter();

        if SFrameHeader::epoch_from_kid(kid) != SFrameHeader::epoch_lsb(self.epoch) {
            return Err(SFrameError::UnknownKid(kid));
        }
        let leaf = SFrameHeader::leaf_from_kid(kid);

        let suite = self.suite;
        let base_key = self.base_key;
        let state = self.senders.entry(kid).or_insert_with(|| SenderState {
            key: DecryptionKey::derive_from(suite.to_sframe(), kid, base_key)
                .expect("key derivation from a 32-byte base key never fails"),
            replay: ReplayAttackProtection::with_tolerance(REPLAY_WINDOW),
        });

        // Replay check before decryption (matches sframe's example flow).
        let view = view
            .validate(&state.replay)
            .map_err(|_| SFrameError::Replay { kid, ctr })?;

        let media = view.decrypt(&state.key).map_err(|_| SFrameError::Decrypt)?;
        Ok((media.payload().to_vec(), leaf))
    }

    /// Drops all per-sender key + replay state (call on epoch change).
    pub fn reset(&mut self) {
        self.senders.clear();
    }
}
