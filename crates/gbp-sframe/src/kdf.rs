use gbp_mls::MlsContext;
use hkdf::Hkdf;
use sha2::Sha256;

use crate::error::SFrameError;

/// SFrame ciphersuite selection.
///
/// `Aes128Gcm` is the standard choice; `Aes256Gcm` is available for
/// high-assurance deployments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CipherSuite {
    /// AES-128-GCM: 16-byte key, 12-byte nonce.
    Aes128Gcm,
    /// AES-256-GCM: 32-byte key, 12-byte nonce.
    Aes256Gcm,
}

impl CipherSuite {
    /// Key length in bytes.
    pub(crate) fn key_len(self) -> usize {
        match self {
            Self::Aes128Gcm => 16,
            Self::Aes256Gcm => 32,
        }
    }

    /// Numeric discriminant used in the FFI (`0` = AES-128, `1` = AES-256).
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Aes128Gcm),
            1 => Some(Self::Aes256Gcm),
            _ => None,
        }
    }

    /// Numeric discriminant.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Aes128Gcm => 0,
            Self::Aes256Gcm => 1,
        }
    }
}

/// Derived key material for one participant in one epoch.
pub(crate) struct ParticipantKeys {
    /// AES key: 16 bytes for AES-128-GCM, 32 bytes for AES-256-GCM.
    pub key: Vec<u8>,
    /// 12-byte base nonce (XOR'd with the counter to produce each frame nonce).
    pub salt: [u8; 12],
}

/// Derives the 32-byte SFrame base key from the MLS `ExportSecret`.
///
/// `label` is the application-defined export label
/// (e.g. `"gbp/sframe v1"`).
/// `epoch` is passed as an 8-byte big-endian context to bind the key to the
/// current MLS epoch.
pub fn derive_base_key(mls: &MlsContext, label: &str, epoch: u64) -> Result<[u8; 32], SFrameError> {
    let context = epoch.to_be_bytes();
    let raw = mls
        .export_raw(label, &context, 32)
        .map_err(|e| SFrameError::MlsExport(e.to_string()))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Ok(out)
}

// Protocol-defined HKDF domain-separation labels (public constants, not secret values).
// HKDF-Expand takes an `info` parameter that is intentionally a well-known, fixed label;
// the *output* is the derived key material, which is cryptographically bound to `base_key`.
const HKDF_LABEL_KEY: &[u8] = b"gbp sframe key ";
const HKDF_LABEL_NONCE: &[u8] = b"gbp sframe salt ";

/// Derives the encryption key and base nonce for participant `leaf_index`.
///
/// Uses HKDF-Expand (SHA-256) over the epoch's `base_key` with
/// deterministic domain labels so every member can reproduce any sender's
/// key material.
pub(crate) fn derive_participant(
    base_key: &[u8; 32],
    leaf_index: u32,
    suite: CipherSuite,
) -> ParticipantKeys {
    // SAFETY: base_key is 32 bytes = SHA-256 HashLen, so from_prk never panics.
    let hk =
        Hkdf::<Sha256>::from_prk(base_key).expect("base_key is exactly SHA-256 HashLen (32 bytes)");

    let leaf_be = leaf_index.to_be_bytes();

    let mut label = HKDF_LABEL_KEY.to_vec();
    label.extend_from_slice(&leaf_be);
    let mut key = vec![0u8; suite.key_len()];
    hk.expand(&label, &mut key)
        .expect("key length is well within 255 * HashLen");

    let mut label = HKDF_LABEL_NONCE.to_vec();
    label.extend_from_slice(&leaf_be);
    let mut salt = [0u8; 12];
    hk.expand(&label, &mut salt)
        .expect("nonce length (12) is well within 255 * HashLen");

    ParticipantKeys { key, salt }
}

/// Constructs the 12-byte per-frame nonce:
/// `participant_salt XOR (CTR_LE64 || 0x00_00_00_00)`.
pub(crate) fn make_nonce(salt: &[u8; 12], ctr: u64) -> [u8; 12] {
    let mut nonce = *salt;
    let ctr_le = ctr.to_le_bytes(); // 8 bytes little-endian
    for i in 0..8 {
        nonce[i] ^= ctr_le[i];
    }
    nonce
}
