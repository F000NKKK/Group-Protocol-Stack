//! Error class enum and the registry of canonical error codes.
//!
//! The wire-serialisable `ErrorObject` lives in the `gbp` crate (base layer).
//! Only the classification and the code constants live here so that any layer
//! can refer to them without taking a serde dependency.

/// Coarse classification of an error. Single byte on the wire.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ErrorClass {
    /// 0x01 — transport (QUIC / TLS).
    Transport = 0x01,
    /// 0x02 — cryptography (AEAD / MLS).
    Crypto = 0x02,
    /// 0x03 — state (epoch / transition).
    State = 0x03,
    /// 0x04 — policy (replay / quota).
    Policy = 0x04,
    /// 0x05 — schema (CBOR shape, length validation).
    Schema = 0x05,
    /// 0x06 — authorisation (roles, GSP).
    Authz = 0x06,
}

impl TryFrom<u8> for ErrorClass {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        use ErrorClass::*;
        Ok(match v {
            0x01 => Transport,
            0x02 => Crypto,
            0x03 => State,
            0x04 => Policy,
            0x05 => Schema,
            0x06 => Authz,
            other => return Err(other),
        })
    }
}

/// All canonical error codes.
///
/// Allocation:
/// * `0x0000`–`0x0FFF` — GBP base layer
/// * `0x1000`–`0x1FFF` — GAP
/// * `0x2000`–`0x2FFF` — GTP
/// * `0x3000`–`0x3FFF` — GSP
/// * `0xF000`–`0xFFFF` — private extensions
pub mod codes {
    /// Unsupported protocol version.
    pub const UNSUPPORTED_VERSION: u16 = 0x0001;
    /// Unknown group_id.
    pub const UNKNOWN_GROUP: u16 = 0x0002;
    /// Frame epoch does not match the receiver's current epoch.
    pub const EPOCH_MISMATCH: u16 = 0x0003;
    /// Frame transition_id does not match the current transition.
    pub const TRANSITION_MISMATCH: u16 = 0x0004;
    /// Sequence number was already used — replay detected.
    pub const REPLAY_DETECTED: u16 = 0x0005;
    /// AEAD authentication failed.
    pub const DECRYPT_FAILED: u16 = 0x0006;
    /// MLS commit was rejected.
    pub const COMMIT_INVALID: u16 = 0x0007;
    /// Stream class used outside its allowed policy.
    pub const STREAM_POLICY_VIOLATION: u16 = 0x0008;

    /// Unknown audio source.
    pub const GAP_BAD_SOURCE_ID: u16 = 0x1001;
    /// Opus decoder error.
    pub const GAP_DECODE_FAILED: u16 = 0x1002;
    /// rtp_sequence already seen for this source.
    pub const GAP_REPLAY_DETECTED: u16 = 0x1003;
    /// key_phase points at a stale epoch.
    pub const GAP_EPOCH_STALE: u16 = 0x1004;

    /// content_length does not match the body length.
    pub const GTP_BAD_LENGTH: u16 = 0x2001;
    /// content_type is not supported by the profile.
    pub const GTP_UNSUPPORTED_CONTENT_TYPE: u16 = 0x2002;
    /// Duplicate (sender_id, message_id).
    pub const GTP_DUPLICATE_MESSAGE: u16 = 0x2003;
    /// Message rejected by the policy layer (retention / quota / …).
    pub const GTP_POLICY_REJECTED: u16 = 0x2004;

    /// Invalid args schema for the given signal_type.
    pub const GSP_BAD_SCHEMA: u16 = 0x3001;
    /// Sender is not authorised to issue this signal.
    pub const GSP_UNAUTHORIZED: u16 = 0x3002;
    /// signal_type is not in the registry.
    pub const GSP_UNKNOWN_SIGNAL: u16 = 0x3003;
    /// Duplicate request_id.
    pub const GSP_DUPLICATE_REQUEST: u16 = 0x3004;
    /// Signal contradicts the current state.
    pub const GSP_STATE_CONFLICT: u16 = 0x3005;
}

/// Compile-time descriptor: code plus its `retryable` / `fatal` semantics.
///
/// Used both as a documentation registry and to populate runtime
/// `ErrorObject` values.
#[derive(Copy, Clone, Debug)]
pub struct ErrorSpec {
    /// Numeric code (see [`codes`]).
    pub code: u16,
    /// Class.
    pub class: ErrorClass,
    /// MAY be retried by the client.
    pub retryable: bool,
    /// Fatal — the node moves to FAILED.
    pub fatal: bool,
    /// Stable symbolic name for logs.
    pub name: &'static str,
}

impl ErrorSpec {
    /// Returns the spec for a known code, or `None` otherwise.
    pub fn lookup(code: u16) -> Option<ErrorSpec> {
        use codes::*;
        Some(match code {
            UNSUPPORTED_VERSION => spec(code, ErrorClass::Schema, false, true, "ERR_UNSUPPORTED_VERSION"),
            UNKNOWN_GROUP => spec(code, ErrorClass::State, false, true, "ERR_UNKNOWN_GROUP"),
            EPOCH_MISMATCH => spec(code, ErrorClass::State, true, false, "ERR_EPOCH_MISMATCH"),
            TRANSITION_MISMATCH => spec(code, ErrorClass::State, true, false, "ERR_TRANSITION_MISMATCH"),
            REPLAY_DETECTED => spec(code, ErrorClass::Crypto, false, false, "ERR_REPLAY_DETECTED"),
            DECRYPT_FAILED => spec(code, ErrorClass::Crypto, false, true, "ERR_DECRYPT_FAILED"),
            COMMIT_INVALID => spec(code, ErrorClass::Crypto, false, true, "ERR_COMMIT_INVALID"),
            STREAM_POLICY_VIOLATION => spec(code, ErrorClass::Policy, false, false, "ERR_STREAM_POLICY_VIOLATION"),
            GAP_BAD_SOURCE_ID => spec(code, ErrorClass::Schema, false, false, "ERR_GAP_BAD_SOURCE_ID"),
            GAP_DECODE_FAILED => spec(code, ErrorClass::Schema, false, false, "ERR_GAP_DECODE_FAILED"),
            GAP_REPLAY_DETECTED => spec(code, ErrorClass::Crypto, false, false, "ERR_GAP_REPLAY_DETECTED"),
            GAP_EPOCH_STALE => spec(code, ErrorClass::Crypto, false, false, "ERR_GAP_EPOCH_STALE"),
            GTP_BAD_LENGTH => spec(code, ErrorClass::Schema, false, false, "ERR_GTP_BAD_LENGTH"),
            GTP_UNSUPPORTED_CONTENT_TYPE => spec(code, ErrorClass::Schema, false, false, "ERR_GTP_UNSUPPORTED_CONTENT_TYPE"),
            GTP_DUPLICATE_MESSAGE => spec(code, ErrorClass::Policy, false, false, "ERR_GTP_DUPLICATE_MESSAGE"),
            GTP_POLICY_REJECTED => spec(code, ErrorClass::Policy, false, false, "ERR_GTP_POLICY_REJECTED"),
            GSP_BAD_SCHEMA => spec(code, ErrorClass::Schema, false, false, "ERR_GSP_BAD_SCHEMA"),
            GSP_UNAUTHORIZED => spec(code, ErrorClass::Authz, false, false, "ERR_GSP_UNAUTHORIZED"),
            GSP_UNKNOWN_SIGNAL => spec(code, ErrorClass::Schema, false, false, "ERR_GSP_UNKNOWN_SIGNAL"),
            GSP_DUPLICATE_REQUEST => spec(code, ErrorClass::Policy, false, false, "ERR_GSP_DUPLICATE_REQUEST"),
            GSP_STATE_CONFLICT => spec(code, ErrorClass::State, true, false, "ERR_GSP_STATE_CONFLICT"),
            _ => return None,
        })
    }
}

const fn spec(code: u16, class: ErrorClass, retryable: bool, fatal: bool, name: &'static str) -> ErrorSpec {
    ErrorSpec { code, class, retryable, fatal, name }
}
