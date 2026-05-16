//! Stateful GSP client.

use crate::{GspSignal, args::validate_args};
use gbp::CodecError;
use gbp_core::{BoundedSeen, GbpFlags, MemberId, SignalType, StreamType};
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
    /// `args` do not conform to the per-signal schema (gsp_rfc §6).
    #[error("bad args schema: {0}")]
    BadSchema(&'static str),
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

/// Per-epoch request dedup capacity (GSP §5).
const GSP_SEEN_CAP: usize = 10_000;

/// Stateful GSP client.
///
/// Tracks `request_id` deduplication, the current membership set and the
/// mute-list. Membership is updated atomically when JOIN, LEAVE, MUTE or
/// UNMUTE signals are accepted. The `request_id` set is LRU-bounded at
/// [`GSP_SEEN_CAP`] entries per epoch.
///
/// The client observes the current group epoch on every [`GspClient::send`]
/// or [`GspClient::accept`] call and automatically clears its
/// `request_id` deduplication set when the epoch advances. Callers may also
/// drive a reset explicitly via [`GspClient::reset`].
pub struct GspClient {
    seen_requests: BoundedSeen<u32>,
    /// Members that are currently muted.
    pub muted: HashSet<MemberId>,
    /// Current membership set, driven by JOIN / LEAVE.
    pub members: HashSet<MemberId>,
    current_epoch: Option<u64>,
}

impl GspClient {
    /// Creates an empty client.
    pub fn new() -> Self {
        Self {
            seen_requests: BoundedSeen::new(GSP_SEEN_CAP),
            muted: HashSet::new(),
            members: HashSet::new(),
            current_epoch: None,
        }
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
        self.sync_epoch(node.current_epoch);
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
    ///
    /// `current_epoch` is the receiver node's current epoch — passing it lets
    /// the client auto-reset its `request_id` deduplication set when the
    /// epoch advances.
    pub fn accept(&mut self, plaintext: &[u8], current_epoch: u64) -> Result<GspAccept, GspError> {
        self.sync_epoch(current_epoch);
        let s = GspSignal::from_cbor(plaintext)?;
        let signal = SignalType::try_from(s.signal_type).map_err(GspError::UnknownSignal)?;
        // Per-signal args schema validation (gsp_rfc §6, step 3).
        validate_args(signal, &s.args).map_err(GspError::BadSchema)?;
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

    /// Synchronises the client's view of the group epoch and resets the
    /// `request_id` deduplication set when the epoch has advanced. Called
    /// automatically by [`GspClient::send`] and [`GspClient::accept`].
    pub fn sync_epoch(&mut self, epoch: u64) {
        if Some(epoch) != self.current_epoch {
            self.seen_requests.clear();
            self.current_epoch = Some(epoch);
        }
    }

    /// Clears the request-id deduplication set unconditionally.
    pub fn reset(&mut self) {
        self.seen_requests.clear();
        self.current_epoch = None;
    }
}
