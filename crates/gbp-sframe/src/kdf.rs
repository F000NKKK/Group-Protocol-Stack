use gbp_mls::MlsContext;

use crate::error::SFrameError;

/// SFrame ciphersuite selection.
///
/// `Aes128Gcm` is the standard choice; `Aes256Gcm` is available for
/// high-assurance deployments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CipherSuite {
    /// AES-128-GCM with SHA-256 key expansion (RFC 9605 `AES_128_GCM_SHA256_128`).
    Aes128Gcm,
    /// AES-256-GCM with SHA-512 key expansion (RFC 9605 `AES_256_GCM_SHA512_128`).
    Aes256Gcm,
}

impl CipherSuite {
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

    /// Maps to the corresponding RFC 9605 suite in the `sframe` crate.
    pub(crate) fn to_sframe(self) -> sframe::CipherSuite {
        match self {
            Self::Aes128Gcm => sframe::CipherSuite::AesGcm128Sha256,
            Self::Aes256Gcm => sframe::CipherSuite::AesGcm256Sha512,
        }
    }
}

/// Derives the 32-byte SFrame base key from the MLS `ExportSecret`.
///
/// `label` is the application-defined export label (e.g. `"gbp/sframe v1"`).
/// `epoch` is passed as an 8-byte big-endian context to bind the key to the
/// current MLS epoch. Per-sender keys are then expanded from this base key by
/// the `sframe` crate (RFC 9605 §5.2), keyed by the frame's KID.
pub fn derive_base_key(mls: &MlsContext, label: &str, epoch: u64) -> Result<[u8; 32], SFrameError> {
    let context = epoch.to_be_bytes();
    let raw = mls
        .export_raw(label, &context, 32)
        .map_err(|e| SFrameError::MlsExport(e.to_string()))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Ok(out)
}
