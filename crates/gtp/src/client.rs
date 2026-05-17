//! Stateful GTP client.

use crate::GtpMessage;
use gbp::CodecError;
use gbp_core::{BoundedSeen, GbpFlags, MemberId, PayloadCodec, StreamType};
use gbp_node::{GroupNode, NodeError, OutboundFrame, Sealer};

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

/// Per-epoch message dedup capacity (GTP §5).
const GTP_SEEN_CAP: usize = 10_000;

/// Stateful GTP client.
///
/// Tracks the set of already-seen `(sender_id, message_id)` pairs to enforce
/// the idempotency contract of GTP §5. The seen-set is LRU-bounded at
/// [`GTP_SEEN_CAP`] entries per epoch to prevent unbounded memory growth
/// in long-lived groups.
///
/// The client observes the current group epoch on every [`GtpClient::send`]
/// or [`GtpClient::accept`] call and automatically clears its idempotency
/// set when the epoch advances. Callers may also drive a reset explicitly
/// via [`GtpClient::reset`].
pub struct GtpClient {
    seen: BoundedSeen<(MemberId, u64)>,
    current_epoch: Option<u64>,
}

impl GtpClient {
    /// Creates an empty client.
    pub fn new() -> Self {
        Self {
            seen: BoundedSeen::new(GTP_SEEN_CAP),
            current_epoch: None,
        }
    }

    /// Sends a text message via the given GBP node and AEAD sealer.
    ///
    /// Returns a wire-ready [`OutboundFrame`] that the caller MUST hand to the
    /// transport. Uses the `O | R | A` profile from GTP §5.
    /// `codec` controls how the [`GtpMessage`] payload is encoded inside the
    /// GBP frame; use [`PayloadCodec::Cbor`] for maximum compatibility.
    pub fn send<S: Sealer>(
        &mut self,
        node: &mut GroupNode,
        seal: &mut S,
        target: MemberId,
        message_id: u64,
        text: &str,
        codec: PayloadCodec,
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
            &msg.to_bytes(codec),
            codec,
        )?;
        Ok(of)
    }

    /// Accepts a plaintext payload delivered by the GBP layer
    /// (`Event::PayloadReceived`).
    ///
    /// `current_epoch` is the receiver node's current epoch — passing it lets
    /// the client auto-reset its idempotency set when the epoch advances.
    /// `codec` must match the value from [`DeliveredPayload::codec`].
    /// Returns either [`GtpAccept::New`] or [`GtpAccept::Duplicate`], or
    /// [`GtpError::Decode`] if the plaintext is not a valid GTP message.
    pub fn accept(
        &mut self,
        plaintext: &[u8],
        current_epoch: u64,
        codec: PayloadCodec,
    ) -> Result<GtpAccept, GtpError> {
        self.sync_epoch(current_epoch);
        let m = GtpMessage::from_bytes(plaintext, codec)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GtpMessage;

    fn encode_msg(sender_id: u32, message_id: u64) -> Vec<u8> {
        GtpMessage::plain(sender_id, message_id, "hello").to_cbor()
    }

    #[test]
    fn accept_new_message_returns_new() {
        let mut client = GtpClient::new();
        let payload = encode_msg(1, 100);
        assert!(matches!(
            client.accept(&payload, 0, PayloadCodec::Cbor).unwrap(),
            GtpAccept::New(_)
        ));
    }

    #[test]
    fn accept_duplicate_returns_duplicate() {
        let mut client = GtpClient::new();
        let payload = encode_msg(1, 100);
        client.accept(&payload, 0, PayloadCodec::Cbor).unwrap();
        let result = client.accept(&payload, 0, PayloadCodec::Cbor).unwrap();
        assert!(matches!(result, GtpAccept::Duplicate(_)));
    }

    #[test]
    fn different_message_ids_both_new() {
        let mut client = GtpClient::new();
        let p1 = encode_msg(1, 1);
        let p2 = encode_msg(1, 2);
        assert!(matches!(client.accept(&p1, 0, PayloadCodec::Cbor).unwrap(), GtpAccept::New(_)));
        assert!(matches!(client.accept(&p2, 0, PayloadCodec::Cbor).unwrap(), GtpAccept::New(_)));
    }

    #[test]
    fn different_senders_same_message_id_both_new() {
        let mut client = GtpClient::new();
        let p1 = encode_msg(1, 42);
        let p2 = encode_msg(2, 42);
        assert!(matches!(client.accept(&p1, 0, PayloadCodec::Cbor).unwrap(), GtpAccept::New(_)));
        assert!(matches!(client.accept(&p2, 0, PayloadCodec::Cbor).unwrap(), GtpAccept::New(_)));
    }

    #[test]
    fn epoch_advance_clears_seen_set() {
        let mut client = GtpClient::new();
        let payload = encode_msg(1, 100);
        client.accept(&payload, 0, PayloadCodec::Cbor).unwrap();
        // same message, new epoch → New again
        let result = client.accept(&payload, 1, PayloadCodec::Cbor).unwrap();
        assert!(matches!(result, GtpAccept::New(_)));
    }

    #[test]
    fn reset_clears_idempotency_state() {
        let mut client = GtpClient::new();
        let payload = encode_msg(7, 999);
        client.accept(&payload, 5, PayloadCodec::Cbor).unwrap();
        client.reset();
        let result = client.accept(&payload, 5, PayloadCodec::Cbor).unwrap();
        assert!(matches!(result, GtpAccept::New(_)));
    }

    #[test]
    fn sync_epoch_same_value_keeps_state() {
        let mut client = GtpClient::new();
        let payload = encode_msg(1, 1);
        client.accept(&payload, 3, PayloadCodec::Cbor).unwrap();
        client.sync_epoch(3); // same epoch — does not clear
        let result = client.accept(&payload, 3, PayloadCodec::Cbor).unwrap();
        assert!(matches!(result, GtpAccept::Duplicate(_)));
    }

    #[test]
    fn invalid_cbor_returns_decode_error() {
        let mut client = GtpClient::new();
        let result = client.accept(b"\xFF\xFF", 0, PayloadCodec::Cbor);
        assert!(matches!(result, Err(GtpError::Decode(_))));
    }
}
