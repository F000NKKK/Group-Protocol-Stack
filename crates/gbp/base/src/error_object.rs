//! Wire-serialisable error object.

use crate::CodecError;
use gbp_core::{ErrorClass, errors::ErrorSpec};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

/// Wire-serialisable error object.
///
/// `details_cbor` carries opaque, structured details. Implementations MUST NOT
/// place secret material or sensitive payload bytes into `reason` or
/// `details_cbor` — error objects are forwarded across trust boundaries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorObject {
    /// Numeric error code (see `gbp_core::errors::codes`).
    pub code: u16,
    /// Error class as a `u8` (see `gbp_core::ErrorClass`).
    pub class: u8,
    /// Whether the operation MAY be retried.
    pub retryable: bool,
    /// Whether the error is fatal (the node moves to FAILED).
    pub fatal: bool,
    /// Human-readable reason. MUST NOT carry secrets.
    pub reason: String,
    /// Structured detail bytes (typically CBOR).
    #[serde(rename = "det")]
    pub details_cbor: ByteBuf,
}

impl ErrorObject {
    /// Builds an error object from a registry [`ErrorSpec`].
    pub fn from_spec(spec: ErrorSpec, reason: impl Into<String>) -> Self {
        Self {
            code: spec.code,
            class: spec.class as u8,
            retryable: spec.retryable,
            fatal: spec.fatal,
            reason: reason.into(),
            details_cbor: ByteBuf::new(),
        }
    }

    /// Builds an error object with arbitrary fields (for codes outside the
    /// registry).
    pub fn new(
        code: u16,
        class: ErrorClass,
        retryable: bool,
        fatal: bool,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            code,
            class: class as u8,
            retryable,
            fatal,
            reason: reason.into(),
            details_cbor: ByteBuf::new(),
        }
    }

    /// CBOR-encodes the error object.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode");
        buf
    }

    /// Decodes a CBOR-encoded error object.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_round_trip() {
        let err = ErrorObject::new(404, ErrorClass::Schema, true, false, "not found");
        let bytes = err.to_cbor();
        let decoded = ErrorObject::from_cbor(&bytes).unwrap();
        assert_eq!(decoded.code, 404);
        assert_eq!(decoded.class, ErrorClass::Schema as u8);
        assert!(decoded.retryable);
        assert!(!decoded.fatal);
        assert_eq!(decoded.reason, "not found");
        assert!(decoded.details_cbor.is_empty());
    }

    #[test]
    fn fatal_error_round_trip() {
        let err = ErrorObject::new(500, ErrorClass::Crypto, false, true, "aead failure");
        let decoded = ErrorObject::from_cbor(&err.to_cbor()).unwrap();
        assert_eq!(decoded.code, 500);
        assert!(decoded.fatal);
        assert!(!decoded.retryable);
    }

    #[test]
    fn invalid_cbor_returns_decode_error() {
        assert!(matches!(ErrorObject::from_cbor(b"\xFF\xFF"), Err(CodecError::Decode(_))));
    }
}
