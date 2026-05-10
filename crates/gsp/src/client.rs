//! Stateful GSP client.

use crate::GspSignal;
use gbp::CodecError;
use gbp_core::{GbpFlags, MemberId, SignalType, StreamType};
use gbp_node::{GroupNode, NodeError, OutboundFrame, Sealer};
use std::collections::HashSet;

/// Errors returned by [`GspClient`].
#[derive(Debug, thiserror::Error)]
pub enum GspError {
    /// Failed to decode the CBOR payload.
    #[error("decode: {0}")]
    Decode(#[from] CodecError),
    /// `signal_type` is not in the registry.
    #[error("unknown signal_type: {0}")]
    UnknownSignal(u32),
    /// Duplicate `request_id`.
    #[error("duplicate request_id: {0}")]
    DuplicateRequest(u32),
    /// Underlying GBP node error during send.
    #[error("node: {0}")]
    Node(#[from] NodeError),
}

/// Accepted signal: decoded fields plus the local state effects already
/// applied by the client.
#[derive(Debug, Clone)]
pub struct GspAccept {
    /// Decoded signal type.
    pub signal: SignalType,
    /// Sender member id.
    pub sender_id: MemberId,
    /// Claimed role (used by `ROLE_CHANGE`).
    pub role_claim: u32,
    /// Request id.
    pub request_id: u32,
}

/// Stateful GSP client.
///
/// Tracks `request_id` deduplication, the current membership set and the
/// mute-list. Membership is updated atomically when JOIN, LEAVE, MUTE or
/// UNMUTE signals are accepted.
#[derive(Default)]
pub struct GspClient {
    seen_requests: HashSet<u32>,
    /// Members that are currently muted.
    pub muted: HashSet<MemberId>,
    /// Current membership set, driven by JOIN / LEAVE.
    pub members: HashSet<MemberId>,
}

impl GspClient {
    /// Creates an empty client.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends a signal. Uses the `O | R | A` profile required by GSP §3.
    pub fn send<S: Sealer>(
        &mut self,
        node: &mut GroupNode,
        seal: &mut S,
        target: MemberId,
        signal: SignalType,
        role_claim: u32,
        request_id: u32,
    ) -> Result<OutboundFrame, GspError> {
        let mut sig = GspSignal::bare(signal as u32, request_id, node.member_id);
        sig.role_claim = role_claim;
        let stream_id = node.member_stream_id(3);
        Ok(node.send_payload(
            seal,
            target,
            StreamType::Signal,
            stream_id,
            GbpFlags::ordered_reliable_ack(),
            &sig.to_cbor(),
        )?)
    }

    /// Accepts a signal payload, applies the state effects defined in GSP §5
    /// and returns the decoded [`GspAccept`].
    pub fn accept(&mut self, plaintext: &[u8]) -> Result<GspAccept, GspError> {
        let s = GspSignal::from_cbor(plaintext)?;
        let signal = SignalType::try_from(s.signal_type).map_err(GspError::UnknownSignal)?;
        if !self.seen_requests.insert(s.request_id) {
            return Err(GspError::DuplicateRequest(s.request_id));
        }
        match signal {
            SignalType::Join => {
                self.members.insert(s.sender_id);
            }
            SignalType::Leave => {
                self.members.remove(&s.sender_id);
                self.muted.remove(&s.sender_id);
            }
            SignalType::Mute => {
                self.muted.insert(s.sender_id);
            }
            SignalType::Unmute => {
                self.muted.remove(&s.sender_id);
            }
            _ => {}
        }
        Ok(GspAccept {
            signal,
            sender_id: s.sender_id,
            role_claim: s.role_claim,
            request_id: s.request_id,
        })
    }

    /// Clears the request-id deduplication set. Intended for use after an
    /// epoch change.
    pub fn reset(&mut self) {
        self.seen_requests.clear();
    }
}
