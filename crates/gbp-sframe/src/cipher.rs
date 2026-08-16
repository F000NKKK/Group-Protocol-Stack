use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sframe::frame::{
    EncryptedFrameView, FrameValidation, MediaFrameView, MonotonicCounter,
    ReplayAttackProtectionStore,
};
use sframe::header::KeyId;
use sframe::key::{DecryptionKey, EncryptionKey};

use crate::error::SFrameError;
use crate::header::SFrameHeader;
use crate::kdf::CipherSuite;

/// Replay-window tolerance in frames (gbp's historical fixed 1024-frame window).
const REPLAY_WINDOW: u64 = 1024;

// ─── SFrameEncryptor ─────────────────────────────────────────────────────────

/// The key + counter for one `(epoch, leaf_index)` KID.
///
/// Shared behind an [`Arc<Mutex<_>>`] by every [`SFrameEncryptor`] handle for
/// that KID, so cloning a handle - or obtaining a new one from
/// [`crate::SFrameSession::encryptor`] - can never produce a second,
/// independent counter that would reuse a `(key, KID, CTR)` nonce.
struct EncryptorState {
    key: EncryptionKey,
    counter: MonotonicCounter,
}

/// Stateful per-sender SFrame encryptor handle.
///
/// Cloning a handle, or requesting another one for the same `(epoch,
/// leaf_index)` via [`crate::SFrameSession::encryptor`], shares the same
/// underlying counter - it does **not** create an independent one. This
/// makes it safe to hold multiple handles (e.g. one per thread) for the same
/// sender without risking AEAD nonce reuse.
#[derive(Clone)]
pub struct SFrameEncryptor {
    state: Arc<Mutex<EncryptorState>>,
    kid: KeyId,
}

impl SFrameEncryptor {
    pub(crate) fn new(base_key: &[u8; 32], kid: u64, suite: CipherSuite) -> Self {
        let key = EncryptionKey::derive_from(suite.to_sframe(), kid, base_key)
            .expect("key derivation from a 32-byte base key never fails");
        Self {
            state: Arc::new(Mutex::new(EncryptorState {
                key,
                counter: MonotonicCounter::default(),
            })),
            kid,
        }
    }

    /// Encrypts `plaintext` and returns the complete SFrame payload:
    /// `header ‖ ciphertext ‖ GCM-tag`.
    ///
    /// `extra_aad` is bound into the AEAD tag (e.g. an RTP header) but is **not**
    /// carried in the returned payload; the receiver supplies the same slice to
    /// [`SFrameDecryptor::decrypt`].
    ///
    /// Safe to call concurrently from multiple handles sharing this sender's
    /// state: the counter is allocated under a lock, so concurrent calls
    /// never allocate the same value twice.
    pub fn encrypt(&mut self, plaintext: &[u8], extra_aad: &[u8]) -> Result<Vec<u8>, SFrameError> {
        let mut state = self
            .state
            .lock()
            .expect("sframe encryptor state mutex poisoned");
        // next() panics once exhausted, which would poison this mutex.
        if state.counter.is_exhausted() {
            return Err(SFrameError::CounterExhausted(self.kid));
        }
        let frame = MediaFrameView::with_meta_data(&mut state.counter, plaintext, extra_aad);
        let encrypted = frame
            .encrypt(&state.key)
            .map_err(|_| SFrameError::Encrypt)?;

        // sframe serialises `meta_data ‖ header ‖ ciphertext`; strip the
        // metadata prefix so `extra_aad` stays off the wire.
        Ok(encrypted.as_ref()[extra_aad.len()..].to_vec())
    }

    /// Starts the counter at `start`, to reach exhaustion without 2^64 frames.
    #[cfg(test)]
    fn with_counter_at(base_key: &[u8; 32], kid: u64, suite: CipherSuite, start: u64) -> Self {
        let encryptor = Self::new(base_key, kid, suite);
        encryptor.state.lock().unwrap().counter =
            MonotonicCounter::with_start_value(start, u64::MAX);
        encryptor
    }

    /// Current counter value (number of frames encrypted so far).
    pub fn counter(&self) -> u64 {
        self.state
            .lock()
            .expect("sframe encryptor state mutex poisoned")
            .counter
            .current()
    }

    /// KID this encryptor was created for.
    pub fn kid(&self) -> u64 {
        self.kid
    }
}

// ─── SFrameDecryptor ─────────────────────────────────────────────────────────

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
    /// Per-sender keys, keyed by KID.
    keys: HashMap<KeyId, DecryptionKey>,
    /// Replay windows, one per KID (sframe's own per-key-id store).
    replay: ReplayAttackProtectionStore,
}

impl SFrameDecryptor {
    pub(crate) fn new(base_key: [u8; 32], epoch: u64, suite: CipherSuite) -> Self {
        Self {
            base_key,
            epoch,
            suite,
            keys: HashMap::new(),
            replay: ReplayAttackProtectionStore::with_tolerance(REPLAY_WINDOW),
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

        // Screen without recording: the header is not authenticated yet.
        self.replay
            .inspect(view.header())
            .map_err(|_| SFrameError::Replay { kid, ctr })?;

        let suite = self.suite;
        let base_key = self.base_key;
        let key = self.keys.entry(kid).or_insert_with(|| {
            DecryptionKey::derive_from(suite.to_sframe(), kid, base_key)
                .expect("key derivation from a 32-byte base key never fails")
        });

        let media = view.decrypt(key).map_err(|_| SFrameError::Decrypt)?;

        // Authenticated - record the counter.
        self.replay
            .validate(view.header())
            .map_err(|_| SFrameError::Replay { kid, ctr })?;

        Ok((media.payload().to_vec(), leaf))
    }

    /// Drops all per-sender key + replay state (call on epoch change).
    pub fn reset(&mut self) {
        self.keys.clear();
        self.replay = ReplayAttackProtectionStore::with_tolerance(REPLAY_WINDOW);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KID: u64 = 0xabc;

    #[test]
    fn last_counter_value_still_encrypts() {
        let mut enc =
            SFrameEncryptor::with_counter_at(&[7u8; 32], KID, CipherSuite::Aes128Gcm, u64::MAX);

        assert!(enc.encrypt(b"last frame", b"").is_ok());
    }

    #[test]
    fn exhausted_counter_errors_instead_of_panicking() {
        // A panic here would poison the state mutex shared by every handle.
        let mut enc =
            SFrameEncryptor::with_counter_at(&[7u8; 32], KID, CipherSuite::Aes128Gcm, u64::MAX);
        enc.encrypt(b"last frame", b"").unwrap();

        assert!(matches!(
            enc.encrypt(b"one too many", b""),
            Err(SFrameError::CounterExhausted(KID))
        ));

        // Still usable, not poisoned.
        assert!(enc.encrypt(b"and another", b"").is_err());
    }
}
