//! **Group Broadcast Protocol — base layer**.
//!
//! This crate implements the protocol substrate that every sub-protocol
//! (`gtp-protocol`, `gap-protocol`, `gsp-protocol`) sits on top of:
//!
//! * [`GbpFrame`] — the CBOR-encoded transport frame.
//! * [`ControlMessage`] — control plane message envelope.
//! * [`ErrorObject`] — wire-serialisable error object.
//! * [`CodecError`] — unified codec error type used across the stack.
//!
//! The role of GBP in the protocol family is roughly the same as IP's role in
//! the TCP/IP stack: it provides addressed, integrity-protected delivery of
//! opaque payloads, leaving message semantics to the layers above.
//!
//! See the [`gbp-core`] crate for the shared type vocabulary (StreamType,
//! flags, FSM states, error codes).
//!
//! [`gbp-core`]: https://crates.io/crates/gbp-core

#![deny(missing_docs)]

pub mod control;
pub mod error_object;
pub mod frame;

pub use control::ControlMessage;
pub use error_object::ErrorObject;
pub use frame::GbpFrame;

/// Unified codec error type used by every codec in the stack.
///
/// GTP, GAP and GSP all return this error so that callers can handle decoding
/// failures uniformly without having to know which sub-protocol was being
/// parsed.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// CBOR decoding failure.
    #[error("CBOR decode: {0}")]
    Decode(String),
    /// CBOR encoding failure (practically unreachable when writing to a `Vec`).
    #[error("CBOR encode: {0}")]
    Encode(String),
    /// A length field inside the frame does not match the actual payload
    /// length (`payload_size`, `content_length`, `args_length`, …).
    #[error("payload_size/content_length/args_length mismatch")]
    PayloadSizeMismatch,
    /// An unknown enum value was observed (for example an out-of-range
    /// `stream_type` or `signal_type`).
    #[error("unknown enum value: {0}")]
    UnknownEnumValue(u32),
}
