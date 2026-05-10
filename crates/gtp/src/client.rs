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
///
/// The client observes the current group epoch on every [`GtpClient::send`]
/// or [`GtpClient::accept`] call and automatically clears its idempotency
/// set when the epoch advances. Callers may also drive a reset explicitly
/// via [`GtpClient::reset`].
#[derive(Default)]
pub struct GtpClient {
    seen: HashSet<(MemberId, u64)>,
    current_epoch: Option<u64>,
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
        self.sync_epoch(node.current_epoch);
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
    /// (`Event::PayloadReceived`).
    ///
    /// `current_epoch` is the receiver node's current epoch — passing it lets
    /// the client auto-reset its idempotency set when the epoch advances.
    /// Returns either [`GtpAccept::New`] or [`GtpAccept::Duplicate`], or
    /// [`GtpError::Decode`] if the plaintext is not a valid GTP message.
    pub fn accept(&mut self, plaintext: &[u8], current_epoch: u64) -> Result<GtpAccept, GtpError> {
        self.sync_epoch(current_epoch);
        let m = GtpMessage::from_cbor(plaintext)?;
        let key = (m.sender_id, m.message_id);
        if !self.seen.insert(key) {
            return Ok(GtpAccept::Duplicate(m));
        }
        Ok(GtpAccept::New(m))
    }

    /// Synchronises the client's view of the group epoch and resets
    /// idempotency state when the epoch has advanced. Called automatically
    /// by [`GtpClient::send`] and [`GtpClient::accept`].
    pub fn sync_epoch(&mut self, epoch: u64) {
        if Some(epoch) != self.current_epoch {
            self.seen.clear();
            self.current_epoch = Some(epoch);
        }
    }

    /// Clears the idempotency set unconditionally.
    pub fn reset(&mut self) {
        self.seen.clear();
        self.current_epoch = None;
    }
}
