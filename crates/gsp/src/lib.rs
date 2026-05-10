//! **Group Signaling Protocol** — signalling sub-protocol of the
//! Group Protocol Stack.
//!
//! Receive-side pipeline (GSP §7): decrypt (handled by GBP) → check sender
//! authorisation → validate `args` schema → apply atomically → ACK / NACK.
//!
//! This crate provides:
//!
//! * [`GspSignal`] — the CBOR-encoded signal envelope.
//! * [`GspClient`] — stateful client that maintains:
//!   * `request_id` deduplication;
//!   * a mute-list;
//!   * the current membership set (driven by `JOIN` / `LEAVE`);
//!   * `signal_type` → [`gbp_core::SignalType`] decoding.
//!
//! See [`gbp-protocol`] for the underlying frame format.
//!
//! [`gbp-protocol`]: https://crates.io/crates/gbp-protocol

#![deny(missing_docs)]

pub mod client;
pub mod signal;

pub use client::{GspAccept, GspClient, GspError};
pub use signal::GspSignal;
