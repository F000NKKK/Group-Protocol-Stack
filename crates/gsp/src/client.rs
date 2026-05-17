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
        self.send_with_args(node, seal, target, signal, role_claim, request_id, &[])
    }

    /// Sends a signal with opcode-specific `args` bytes (CBOR-encoded).
    /// Use this for signals that require structured arguments (MUTE, UNMUTE,
    /// ROLE_CHANGE, STREAM_START, STREAM_STOP, CODEC_UPDATE).
    pub fn send_with_args<S: Sealer>(
        &mut self,
        node: &mut GroupNode,
        seal: &mut S,
        target: MemberId,
        signal: SignalType,
        role_claim: u32,
        request_id: u32,
        args: &[u8],
    ) -> Result<OutboundFrame, GspError> {
        self.sync_epoch(node.current_epoch);
        let mut sig = GspSignal::bare(signal as u32, request_id, node.member_id);
        sig.role_claim = role_claim;
        sig.args = serde_bytes::ByteBuf::from(args.to_vec());
        sig.args_length = args.len() as u32;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GspSignal;

    fn encode_bare(signal: SignalType, request_id: u32, sender_id: u32) -> Vec<u8> {
        GspSignal::bare(signal as u32, request_id, sender_id).to_cbor()
    }

    #[test]
    fn join_adds_sender_to_members() {
        let mut c = GspClient::new();
        let payload = encode_bare(SignalType::Join, 1, 42);
        let accept = c.accept(&payload, 0).unwrap();
        assert_eq!(accept.signal, SignalType::Join);
        assert!(c.members.contains(&42));
    }

    #[test]
    fn leave_removes_sender_from_members() {
        let mut c = GspClient::new();
        c.accept(&encode_bare(SignalType::Join, 1, 7), 0).unwrap();
        c.accept(&encode_bare(SignalType::Leave, 2, 7), 0).unwrap();
        assert!(!c.members.contains(&7));
    }

    #[test]
    fn leave_also_removes_from_muted() {
        let mut c = GspClient::new();
        c.accept(&encode_bare(SignalType::Join, 1, 5), 0).unwrap();
        c.muted.insert(5); // manually mute
        c.accept(&encode_bare(SignalType::Leave, 2, 5), 0).unwrap();
        assert!(!c.muted.contains(&5));
    }

    #[test]
    fn duplicate_request_id_is_rejected() {
        let mut c = GspClient::new();
        c.accept(&encode_bare(SignalType::Join, 99, 1), 0).unwrap();
        let result = c.accept(&encode_bare(SignalType::Leave, 99, 1), 0);
        assert!(matches!(result, Err(GspError::DuplicateRequest(99))));
    }

    #[test]
    fn epoch_advance_clears_request_seen_set() {
        let mut c = GspClient::new();
        let payload = encode_bare(SignalType::Join, 1, 10);
        c.accept(&payload, 0).unwrap();
        // same request_id is allowed in new epoch
        let result = c.accept(&encode_bare(SignalType::Leave, 1, 10), 1);
        assert!(result.is_ok());
    }

    #[test]
    fn reset_clears_state() {
        let mut c = GspClient::new();
        c.accept(&encode_bare(SignalType::Join, 1, 3), 0).unwrap();
        c.reset();
        // after reset, same request_id allowed again
        c.accept(&encode_bare(SignalType::Join, 1, 4), 0).unwrap();
        // and member state is NOT cleared by reset (only dedup)
        // members accumulated before reset remain
    }

    #[test]
    fn unknown_signal_type_rejected() {
        let mut c = GspClient::new();
        let bad = GspSignal::bare(999, 1, 1).to_cbor();
        assert!(matches!(
            c.accept(&bad, 0),
            Err(GspError::UnknownSignal(999))
        ));
    }

    #[test]
    fn invalid_cbor_returns_decode_error() {
        let mut c = GspClient::new();
        assert!(matches!(c.accept(b"\xFF\xFF", 0), Err(GspError::Decode(_))));
    }

    #[test]
    fn multiple_members_join_independently() {
        let mut c = GspClient::new();
        c.accept(&encode_bare(SignalType::Join, 1, 10), 0).unwrap();
        c.accept(&encode_bare(SignalType::Join, 2, 20), 0).unwrap();
        c.accept(&encode_bare(SignalType::Join, 3, 30), 0).unwrap();
        assert_eq!(c.members.len(), 3);
        assert!(c.members.contains(&10));
        assert!(c.members.contains(&20));
        assert!(c.members.contains(&30));
    }
}
