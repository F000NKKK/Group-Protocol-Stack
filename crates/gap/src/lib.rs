//! **Group Audio Protocol** — audio sub-protocol of the Group Protocol Stack.
//!
//! Profile (GAP §7): Opus at 48 kHz REQUIRED, 20 ms packetisation
//! RECOMMENDED, FEC RECOMMENDED, reliable delivery NOT RECOMMENDED.
//!
//! This crate provides:
//!
//! * [`GapPayload`] — the CBOR-encoded audio payload format.
//! * [`GapClient`] — stateful client that maintains a per-source
//!   `rtp_sequence` window (replay protection, GAP §10) and validates
//!   `key_phase` against the current group epoch.
//!
//! See [`gbp-protocol`] for the underlying frame format.
//!
//! [`gbp-protocol`]: https://crates.io/crates/gbp-protocol

#![deny(missing_docs)]

pub mod client;
pub mod payload;

pub use client::{GapAccept, GapClient, GapError};
pub use payload::GapPayload;
