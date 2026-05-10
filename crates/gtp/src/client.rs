//! Stateful GTP client.

use crate::GtpMessage;
use gbp::CodecError;
use gbp_core::{GbpFlags, MemberId, StreamType};
use gbp_node::{GroupNode, NodeError, OutboundFrame, Sealer};
use std::collections::HashSet;

/// Errors returned by [`GtpClient`].
#[derive(Debug, thiserror::Error)]
pub enum GtpError {
    /// Failed to decode the CBOR payload.
    #[error("decode: {0}")]
    Decode(#[from] CodecError),
    /// Duplicate `(sender_id, message_id)` (idempotency).
    #[error("duplicate (sender={sender_id}, mid=0x{message_id:X})")]
    Duplicate {
        /// Sender member id.
        sender_id: MemberId,
        /// Message id.
        message_id: u64,
    },
    /// Underlying GBP node error during send.
    #[error("node: {0}")]
    Node(#[from] NodeError),
}

/// Outcome of accepting a GTP payload.
#[derive(Debug)]
pub enum GtpAccept {
    /// First time `(sender_id, message_id)` is seen.
    New(GtpMessage),
    /// `(sender_id, message_id)` was already seen.
    Duplicate(GtpMessage),
}

/// Stateful GTP client.
///
/// Tracks the set of already-seen `(sender_id, message_id)` pairs to enforce
/// the idempotency contract of GTP §5.
#[derive(Default)]
pub struct GtpClient {
    seen: HashSet<(MemberId, u64)>,
}

impl GtpClient {
    /// Creates an empty client.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends a text message via the given GBP node and AEAD sealer.
    ///
    /// Returns a wire-ready [`OutboundFrame`] that the caller MUST hand to the
    /// transport. Uses the `O | R | A` profile from GTP §5.
    pub fn send<S: Sealer>(
        &mut self,
        node: &mut GroupNode,
        seal: &mut S,
        target: MemberId,
        message_id: u64,
        text: &str,
    ) -> Result<OutboundFrame, GtpError> {
        let msg = GtpMessage::plain(node.member_id, message_id, text);
        let stream_id = node.member_stream_id(1);
        let of = node.send_payload(
            seal,
            target,
            StreamType::Text,
            stream_id,
            GbpFlags::ordered_reliable_ack(),
            &msg.to_cbor(),
        )?;
        Ok(of)
    }

    /// Accepts a plaintext payload delivered by the GBP layer
    /// (`Event::PayloadReceived`). Returns either [`GtpAccept::New`] or
    /// [`GtpAccept::Duplicate`]. Returns [`GtpError::Decode`] if the
    /// plaintext does not decode as a valid GTP message.
    pub fn accept(&mut self, plaintext: &[u8]) -> Result<GtpAccept, GtpError> {
        let m = GtpMessage::from_cbor(plaintext)?;
        let key = (m.sender_id, m.message_id);
        if !self.seen.insert(key) {
            return Ok(GtpAccept::Duplicate(m));
        }
        Ok(GtpAccept::New(m))
    }

    /// Clears the idempotency set. Intended for use after an epoch change.
    pub fn reset(&mut self) {
        self.seen.clear();
    }
}
