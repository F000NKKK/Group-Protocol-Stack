use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes128Gcm, Aes256Gcm};

use crate::error::SFrameError;
use crate::header::SFrameHeader;
use crate::kdf::{CipherSuite, ParticipantKeys, derive_participant, make_nonce};
use crate::replay::ReplayWindow;

// ─── Internal AEAD helper ────────────────────────────────────────────────────

enum AeadCipher {
    Aes128(Aes128Gcm),
    Aes256(Aes256Gcm),
}

impl AeadCipher {
    fn new(key: &[u8], suite: CipherSuite) -> Self {
        match suite {
            CipherSuite::Aes128Gcm => {
                Self::Aes128(Aes128Gcm::new_from_slice(key).expect("key length matches suite"))
            }
            CipherSuite::Aes256Gcm => {
                Self::Aes256(Aes256Gcm::new_from_slice(key).expect("key length matches suite"))
            }
        }
    }

    fn encrypt(
        &self,
        nonce: &[u8; 12],
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, SFrameError> {
        let n = aes_gcm::Nonce::from_slice(nonce);
        let payload = Payload {
            msg: plaintext,
            aad,
        };
        match self {
            Self::Aes128(c) => c.encrypt(n, payload).map_err(|_| SFrameError::Encrypt),
            Self::Aes256(c) => c.encrypt(n, payload).map_err(|_| SFrameError::Encrypt),
        }
    }

    fn decrypt(
        &self,
        nonce: &[u8; 12],
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, SFrameError> {
        let n = aes_gcm::Nonce::from_slice(nonce);
        let payload = Payload {
            msg: ciphertext,
            aad,
        };
        match self {
            Self::Aes128(c) => c.decrypt(n, payload).map_err(|_| SFrameError::Decrypt),
            Self::Aes256(c) => c.decrypt(n, payload).map_err(|_| SFrameError::Decrypt),
        }
    }
}

// ─── SFrameEncryptor ─────────────────────────────────────────────────────────

/// Stateful per-sender SFrame encryptor.
///
/// Holds the derived key+salt for one `(epoch, leaf_index)` pair and an
/// internal counter that increments on every call to [`encrypt`].
///
/// Obtain via [`crate::SFrameSession::encryptor`].
pub struct SFrameEncryptor {
    cipher: AeadCipher,
    salt: [u8; 12],
    kid: u64,
    ctr: u64,
}

impl SFrameEncryptor {
    pub(crate) fn new(keys: ParticipantKeys, kid: u64, suite: CipherSuite) -> Self {
        Self {
            cipher: AeadCipher::new(&keys.key, suite),
            salt: keys.salt,
            kid,
            ctr: 0,
        }
    }

    /// Encrypts `plaintext` and returns the complete SFrame payload:
    /// `header ‖ ciphertext ‖ GCM-tag`.
    ///
    /// `extra_aad` is appended to the SFrame header to form the full AAD
    /// (e.g. pass an RTP header or an empty slice).
    pub fn encrypt(&mut self, plaintext: &[u8], extra_aad: &[u8]) -> Result<Vec<u8>, SFrameError> {
        let header = SFrameHeader {
            kid: self.kid,
            ctr: self.ctr,
        };
        let header_bytes = header.encode();

        let mut aad = Vec::with_capacity(header_bytes.len() + extra_aad.len());
        aad.extend_from_slice(&header_bytes);
        aad.extend_from_slice(extra_aad);

        let nonce = make_nonce(&self.salt, self.ctr);
        let ciphertext = self.cipher.encrypt(&nonce, plaintext, &aad)?;

        self.ctr = self.ctr.wrapping_add(1);

        let mut out = Vec::with_capacity(header_bytes.len() + ciphertext.len());
        out.extend_from_slice(&header_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Current counter value (number of frames encrypted so far).
    pub fn counter(&self) -> u64 {
        self.ctr
    }

    /// KID this encryptor was created for.
    pub fn kid(&self) -> u64 {
        self.kid
    }
}

// ─── SFrameDecryptor ─────────────────────────────────────────────────────────

/// Per-sender decryption state maintained inside [`SFrameDecryptor`].
struct SenderState {
    cipher: AeadCipher,
    salt: [u8; 12],
    window: ReplayWindow,
}

/// Multi-sender SFrame decryptor for one epoch.
///
/// Lazily derives per-sender key material from the epoch's base key as new
/// `KID`s are encountered.  Maintains an independent replay window per sender.
///
/// Obtain via [`crate::SFrameSession::decryptor`].
pub struct SFrameDecryptor {
    base_key: [u8; 32],
    epoch: u64,
    suite: CipherSuite,
    /// Keyed by `leaf_index`.
    senders: HashMap<u32, SenderState>,
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
        let (header, header_len) = SFrameHeader::decode(payload)?;

        let frame_epoch = SFrameHeader::epoch_from_kid(header.kid);
        if frame_epoch != self.epoch {
            return Err(SFrameError::UnknownKid(header.kid));
        }
        let leaf = SFrameHeader::leaf_from_kid(header.kid);

        // Lazily derive key material for this sender.
        let state = self.senders.entry(leaf).or_insert_with(|| {
            let keys = derive_participant(&self.base_key, leaf, self.suite);
            SenderState {
                cipher: AeadCipher::new(&keys.key, self.suite),
                salt: keys.salt,
                window: ReplayWindow::new(),
            }
        });

        // Replay check before decryption (fast path for replays).
        state
            .window
            .check_and_mark(header.ctr)
            .map_err(|_| SFrameError::Replay {
                kid: header.kid,
                ctr: header.ctr,
            })?;

        let header_bytes = &payload[..header_len];
        let ciphertext = &payload[header_len..];

        let mut aad = Vec::with_capacity(header_bytes.len() + extra_aad.len());
        aad.extend_from_slice(header_bytes);
        aad.extend_from_slice(extra_aad);

        let nonce = make_nonce(&state.salt, header.ctr);
        let plaintext = state.cipher.decrypt(&nonce, ciphertext, &aad)?;

        Ok((plaintext, leaf))
    }

    /// Resets all per-sender replay windows (call on epoch change).
    pub fn reset(&mut self) {
        self.senders.values_mut().for_each(|s| s.window.reset());
    }
}
