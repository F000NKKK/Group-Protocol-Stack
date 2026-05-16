//! **Group Text Protocol** — text sub-protocol of the Group Protocol Stack.
//!
//! GTP is to GBP what TCP is to IP: it adds message-level semantics on top of
//! the GBP base layer's framing and AEAD. This crate exposes:
//!
//! 1. [`GtpMessage`] — the CBOR-encoded text message format.
//! 2. [`GtpClient`] — a stateful client that:
//!    * sends text messages through a [`gbp_node::GroupNode`];
//!    * accepts incoming plaintext payloads delivered by GBP and rejects
//!      duplicates by `(sender_id, message_id)`.
//!
//! See [`gbp-protocol`] for the underlying frame format.
//!
//! [`gbp-protocol`]: https://crates.io/crates/gbp-protocol

#![deny(missing_docs)]

pub mod attachment;
pub mod client;
pub mod history;
pub mod message;

pub use attachment::{
    AttachmentAssembler, AttachmentChunk, AttachmentError, AttachmentManifest, AttachmentSender,
    DEFAULT_CHUNK_SIZE,
};
pub use client::{GtpAccept, GtpClient, GtpError};
pub use history::{MessageHistory, Watermark};
pub use message::{GtpContentType, GtpMessage};
