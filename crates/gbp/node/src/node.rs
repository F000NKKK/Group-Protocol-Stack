//! GBP-layer group node.
//!
//! Responsibilities of this layer (analogous to IP):
//!
//! * Decode incoming CBOR frames and validate `version`, `group_id`, `epoch`
//!   and `transition_id` per the GBP spec.
//! * Enforce a per-`(stream_type, stream_id)` replay window via
//!   `sequence_no`.
//! * Open the AEAD payload through the [`Sealer`] trait (typically backed by
//!   `gbp-mls`).
//! * Surface decoded payloads to sub-protocols as
//!   [`Event::PayloadReceived`]; the sub-protocol layer is responsible for
//!   message-level semantics.
//! * Drive the control plane: handle `EXECUTE_TRANSITION`, request resync on
//!   `EPOCH_MISMATCH`, etc.
//!
//! Out of scope: parsing GTP / GAP / GSP payloads, GTP idempotency, GAP
//! `key_phase` validation and mute-list tracking. Those concerns belong to
//! the per-sub-protocol clients in the `gtp-protocol`, `gap-protocol` and
//! `gsp-protocol` crates.

use gbp::{CodecError, ControlMessage, ErrorObject, GbpFrame};
use gbp_core::{
    ControlOpcode, ErrorClass, GbpFlags, GroupId, MemberId, NodeState, SequenceNo, StreamId,
    StreamType, TransitionId, TransitionState, codes,
    errors::ErrorSpec,
};
use gbp_mls::{MlsError, label_for};
use std::collections::HashMap;

/// Errors raised by [`GroupNode`].
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    /// Codec error.
    #[error("codec: {0}")]
    Codec(#[from] CodecError),
    /// MLS / AEAD error.
    #[error("mls: {0}")]
    Mls(#[from] MlsError),
    /// The node is not in a state that allows the requested operation.
    #[error("invalid state: {0}")]
    InvalidState(String),
}

/// A wire-ready outbound frame: the recipient and its serialised CBOR bytes.
pub struct OutboundFrame {
    /// Target member id.
    pub to: MemberId,
    /// CBOR-encoded [`GbpFrame`] bytes.
    pub wire: Vec<u8>,
}

/// Information about a payload delivered by GBP to a sub-protocol.
#[derive(Debug, Clone)]
pub struct DeliveredPayload {
    /// Stream class on which the frame arrived.
    pub stream_type: StreamType,
    /// Stream id from the frame (preserved so receivers can demultiplex
    /// multiple sub-streams).
    pub stream_id: StreamId,
    /// Sequence number after passing the replay window.
    pub sequence_no: SequenceNo,
    /// Frame flag bits, copied as-is.
    pub flags: u16,
    /// Decrypted plaintext bytes.
    pub plaintext: Vec<u8>,
}

/// Events surfaced by the GBP layer.
#[derive(Debug, Clone)]
pub enum Event {
    /// Node FSM changed state.
    StateChanged {
        /// Previous state.
        from: NodeState,
        /// New state.
        to: NodeState,
    },
    /// Payload delivered to a sub-protocol (Text / Audio / Signal). Control
    /// frames are handled internally and do not surface as
    /// [`Event::PayloadReceived`].
    PayloadReceived(DeliveredPayload),
    /// A control plane message was received and parsed.
    Control {
        /// Sender member id.
        from: MemberId,
        /// Decoded opcode.
        opcode: ControlOpcode,
        /// `transition_id` carried by the message.
        transition_id: TransitionId,
        /// `request_id` echoed by ACK / NACK responders.
        request_id: u32,
        /// Opcode-specific args (CBOR or opaque bytes; e.g. the MLS Commit
        /// embedded in `PREPARE_TRANSITION`).
        args: Vec<u8>,
    },
    /// An error was raised.
    Error {
        /// Numeric error code.
        code: u16,
        /// Error class.
        class: ErrorClass,
        /// MAY be retried.
        retryable: bool,
        /// Fatal — the node is now in `FAILED`.
        fatal: bool,
        /// Human-readable reason.
        reason: String,
    },
    /// Epoch transition has been applied locally.
    EpochAdvanced {
        /// New epoch.
        epoch: u64,
        /// `transition_id` that produced the new epoch.
        transition_id: TransitionId,
    },
}

/// GBP-layer node.
///
/// Owns the framing, AEAD, replay window, FSM and control plane.
/// Sub-protocol semantics live in their own crates and use this type plus a
/// [`Sealer`] for outbound traffic and `on_wire` + the resulting events for
/// inbound traffic.
pub struct GroupNode {
    /// Application-level member id.
    pub member_id: MemberId,
    /// 16-byte group identifier.
    pub group_id: GroupId,
    /// Current epoch as observed by the GBP layer (the authoritative epoch
    /// lives in the underlying MLS group).
    pub current_epoch: u64,
    /// Last applied `transition_id`.
    pub last_transition_id: TransitionId,
    /// Pending `transition_id` (set during PREPARE / READY).
    pub pending_transition_id: TransitionId,
    /// Node FSM.
    pub state: NodeState,
    /// Transition FSM.
    pub transition_state: TransitionState,

    out_seq: HashMap<(StreamType, StreamId), SequenceNo>,
    in_hw: HashMap<(StreamType, StreamId), SequenceNo>,
    events: Vec<Event>,
}

impl GroupNode {
    /// Builds a fresh node in the `IDLE` state.
    pub fn new(member_id: MemberId, group_id: GroupId) -> Self {
        Self {
            member_id,
            group_id,
            current_epoch: 0,
            last_transition_id: 0,
            pending_transition_id: 0,
            state: NodeState::Idle,
            transition_state: TransitionState::TIdle,
            out_seq: HashMap::new(),
            in_hw: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Drives the node from `IDLE` to `ACTIVE` as a creator.
    pub fn bootstrap_as_creator(&mut self, epoch: u64) {
        self.transition(NodeState::Connecting);
        self.transition(NodeState::EstablishingGroup);
        self.current_epoch = epoch;
        self.transition(NodeState::Active);
    }

    /// Drives the node from `IDLE` to `ACTIVE` as a joiner.
    pub fn bootstrap_as_joiner(&mut self, epoch: u64) {
        self.transition(NodeState::Connecting);
        self.transition(NodeState::EstablishingGroup);
        self.current_epoch = epoch;
        self.transition(NodeState::Active);
    }

    /// Drains and returns all queued events.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Returns a sender-unique `stream_id` within the given base class.
    ///
    /// This is used so that the receiver's replay window does not conflate
    /// streams that originate from different members.
    pub fn member_stream_id(&self, base: u32) -> StreamId {
        base + self.member_id * 100
    }

    /// Sends an opaque plaintext payload on the given stream.
    ///
    /// Used by the sub-protocol clients: each one CBOR-encodes its message
    /// and forwards the resulting bytes here.
    pub fn send_payload<S: Sealer>(
        &mut self,
        seal: &mut S,
        target: MemberId,
        stream_type: StreamType,
        stream_id: StreamId,
        flags: u16,
        plaintext: &[u8],
    ) -> Result<OutboundFrame, NodeError> {
        self.assert_can_send()?;
        let seq = self.next_seq(stream_type, stream_id);
        let ciphertext = seal.seal(stream_type, seq, plaintext)?;
        let frame = GbpFrame::new(
            self.group_id,
            self.current_epoch,
            self.last_transition_id,
            stream_type,
            stream_id,
            flags,
            seq,
            ciphertext,
        );
        Ok(OutboundFrame { to: target, wire: frame.to_cbor() })
    }

    /// Sends a control plane message on Stream 0. Wrapper around
    /// [`GroupNode::send_payload`].
    ///
    /// Side effect: when the coordinator originates a `PREPARE_TRANSITION`,
    /// it must locally adopt the same `pending_transition_id` so that the
    /// inbound READY / EXECUTE validation matrix in `handle_control` lines
    /// up. Without this, the coordinator never matches its own pending tid
    /// against the remote READY frames it expects, and the handshake never
    /// completes.
    pub fn send_control<S: Sealer>(
        &mut self,
        seal: &mut S,
        target: MemberId,
        opcode: ControlOpcode,
        transition_id: TransitionId,
        request_id: u32,
        args: Vec<u8>,
    ) -> Result<OutboundFrame, NodeError> {
        let ctl = ControlMessage::with_args(
            opcode as u16,
            request_id,
            self.member_id,
            transition_id,
            args,
        );
        let mut flags = GbpFlags::ordered_reliable_system();
        if matches!(
            opcode,
            ControlOpcode::PrepareTransition
                | ControlOpcode::ReadyForTransition
                | ControlOpcode::ExecuteTransition
        ) {
            flags |= GbpFlags::CRITICAL;
        }
        // Sender-side state mirroring (matches what `handle_control` does on
        // the receiver side). We only update on PREPARE/EXECUTE/ABORT — READY
        // is purely an ack carrying the existing pending tid.
        match opcode {
            ControlOpcode::PrepareTransition => {
                self.pending_transition_id = transition_id;
                self.transition_state = TransitionState::TPrepared;
            }
            ControlOpcode::AbortTransition => {
                self.pending_transition_id = 0;
                self.transition_state = TransitionState::TAborted;
            }
            _ => {}
        }
        let stream_id = self.member_stream_id(0);
        self.send_payload(seal, target, StreamType::Control, stream_id, flags, &ctl.to_cbor())
    }

    /// Feeds wire bytes to the node.
    ///
    /// Performs the §6.2 validation pipeline (version → group_id → epoch →
    /// payload_size → transition_id → replay), opens the AEAD payload and
    /// either:
    /// * dispatches the parsed control message internally (for
    ///   `StreamType::Control`), or
    /// * surfaces an [`Event::PayloadReceived`] (for application streams).
    ///
    /// Returns every event that was produced as a result.
    pub fn on_wire<S: Sealer>(
        &mut self,
        seal: &mut S,
        wire: &[u8],
    ) -> Result<Vec<Event>, NodeError> {
        // Decode without payload-size validation — we want a malformed v!=1
        // frame to surface as `ERR_UNSUPPORTED_VERSION`, not as
        // `ERR_PAYLOAD_SIZE_MISMATCH`. Validation runs in deliver_frame, in
        // the order required by §6.2.
        let frame = match GbpFrame::decode(wire) {
            Ok(f) => f,
            Err(e) => {
                self.emit_err_named(
                    codes::DECRYPT_FAILED,
                    ErrorClass::Schema,
                    false,
                    false,
                    format!("frame decode: {e}"),
                );
                return Ok(self.drain_events());
            }
        };
        self.deliver_frame(seal, frame)?;
        Ok(self.drain_events())
    }

    fn deliver_frame<S: Sealer>(&mut self, seal: &mut S, frame: GbpFrame) -> Result<(), NodeError> {
        // §6.2 order: version → group_id → epoch → payload_size →
        // transition_id (when CRITICAL) → replay.
        if frame.version != 1 {
            self.emit_err_spec(codes::UNSUPPORTED_VERSION, "version != 1");
            return Ok(());
        }
        if frame.group_id_array() != self.group_id {
            self.emit_err_spec(codes::UNKNOWN_GROUP, "group_id");
            return Ok(());
        }
        if frame.epoch != self.current_epoch {
            self.emit_err_spec(
                codes::EPOCH_MISMATCH,
                format!("got {}, expected {}", frame.epoch, self.current_epoch),
            );
            self.trigger_resync();
            return Ok(());
        }
        if let Err(e) = frame.validate_payload_size() {
            self.emit_err_named(
                codes::DECRYPT_FAILED,
                ErrorClass::Schema,
                false,
                false,
                format!("payload size: {e}"),
            );
            return Ok(());
        }
        let flags = GbpFlags::from_bits(frame.flags);
        let st = match frame.stream_type_typed() {
            Ok(st) => st,
            Err(_) => {
                self.emit_err_spec(codes::STREAM_POLICY_VIOLATION, "unknown stream_type");
                return Ok(());
            }
        };

        // §6.2 transition_id ordering: CRITICAL frames on application streams
        // MUST equal `last_transition_id`. Control-stream frames are exempt
        // from this check and validated per-opcode inside `handle_control`,
        // because PREPARE_TRANSITION legitimately carries `last + 1` and
        // EXECUTE / ACK carry `pending_transition_id`.
        if st != StreamType::Control
            && flags.has(GbpFlags::CRITICAL)
            && frame.transition_id != self.last_transition_id
        {
            self.emit_err_spec(
                codes::TRANSITION_MISMATCH,
                format!("got tid={}, expected {}", frame.transition_id, self.last_transition_id),
            );
            return Ok(());
        }

        let key = (st, frame.stream_id);
        let hw = self.in_hw.get(&key).copied().unwrap_or(0);
        if frame.sequence_no <= hw {
            self.emit_err_spec(
                codes::REPLAY_DETECTED,
                format!(
                    "st={} sid={} seq={} hw={}",
                    st, frame.stream_id, frame.sequence_no, hw
                ),
            );
            return Ok(());
        }
        self.in_hw.insert(key, frame.sequence_no);

        let plain = match seal.open(st, frame.sequence_no, &frame.encrypted_payload) {
            Ok(p) => p,
            Err(e) => {
                self.emit_err_named(
                    codes::DECRYPT_FAILED,
                    ErrorClass::Crypto,
                    false,
                    true,
                    format!("aead open: {e}"),
                );
                return Ok(());
            }
        };

        match st {
            StreamType::Control => self.handle_control(plain),
            other => self.events.push(Event::PayloadReceived(DeliveredPayload {
                stream_type: other,
                stream_id: frame.stream_id,
                sequence_no: frame.sequence_no,
                flags: frame.flags,
                plaintext: plain,
            })),
        }
        Ok(())
    }

    fn handle_control(&mut self, plain: Vec<u8>) {
        let c = match ControlMessage::from_cbor(&plain) {
            Ok(c) => c,
            Err(_) => {
                self.emit_err_spec(codes::STREAM_POLICY_VIOLATION, "control decode");
                return;
            }
        };
        let opcode = match ControlOpcode::try_from(c.opcode) {
            Ok(op) => op,
            Err(_) => {
                self.emit_err_spec(codes::STREAM_POLICY_VIOLATION, "unknown opcode");
                return;
            }
        };
        // Per-opcode TransitionID validation (§5 of gbp-control-plane).
        let tid_ok = match opcode {
            // PREPARE introduces last+1; receiver simply records it as pending.
            // Re-issuing a PREPARE for an already-pending tid is allowed; a
            // smaller-or-equal tid that is not strictly newer is rejected.
            ControlOpcode::PrepareTransition => {
                c.transition_id > self.last_transition_id
                    && (self.pending_transition_id == 0
                        || self.pending_transition_id == c.transition_id)
            }
            // READY / EXECUTE / ABORT must reference the pending tid.
            ControlOpcode::ReadyForTransition
            | ControlOpcode::ExecuteTransition
            | ControlOpcode::AbortTransition => {
                self.pending_transition_id != 0
                    && c.transition_id == self.pending_transition_id
            }
            // Digest / capability / ack / nack: tid is informational, no
            // ordering constraint at the GBP layer.
            _ => true,
        };
        if !tid_ok {
            self.emit_err_spec(
                codes::TRANSITION_MISMATCH,
                format!(
                    "control tid={} not valid for {:?} (last={}, pending={})",
                    c.transition_id, opcode, self.last_transition_id, self.pending_transition_id
                ),
            );
            return;
        }
        match opcode {
            ControlOpcode::PrepareTransition => {
                self.pending_transition_id = c.transition_id;
                self.transition_state = TransitionState::TPrepared;
            }
            ControlOpcode::ReadyForTransition => {
                self.transition_state = TransitionState::TReady;
            }
            ControlOpcode::ExecuteTransition => {
                self.apply_transition(c.transition_id);
            }
            ControlOpcode::AbortTransition => {
                self.transition_state = TransitionState::TAborted;
                self.pending_transition_id = 0;
            }
            ControlOpcode::GroupStateDigestResponse => {
                if self.state == NodeState::Resyncing {
                    self.transition(NodeState::Active);
                }
            }
            _ => {}
        }
        self.events.push(Event::Control {
            from: c.sender_id,
            opcode,
            transition_id: c.transition_id,
            request_id: c.request_id,
            args: c.args.to_vec(),
        });
    }

    /// Applies a new epoch (called by the coordinator after
    /// `EXECUTE_TRANSITION`).
    pub fn apply_transition(&mut self, tid: TransitionId) {
        self.current_epoch += 1;
        self.last_transition_id = tid;
        self.pending_transition_id = 0;
        self.transition_state = TransitionState::TExecuted;
        self.out_seq.clear();
        self.in_hw.clear();
        self.events.push(Event::EpochAdvanced {
            epoch: self.current_epoch,
            transition_id: tid,
        });
    }

    /// Forces the node into the `RESYNCING` state.
    pub fn trigger_resync(&mut self) {
        if self.state != NodeState::Resyncing {
            self.transition(NodeState::Resyncing);
        }
    }

    fn transition(&mut self, next: NodeState) {
        if self.state == next {
            return;
        }
        if !self.state.can_transition_to(next) {
            let from = self.state;
            self.state = NodeState::Failed;
            self.events.push(Event::StateChanged { from, to: NodeState::Failed });
            return;
        }
        let from = self.state;
        self.state = next;
        self.events.push(Event::StateChanged { from, to: next });
    }

    fn assert_can_send(&self) -> Result<(), NodeError> {
        if matches!(
            self.state,
            NodeState::Active | NodeState::Resyncing | NodeState::EstablishingGroup
        ) {
            Ok(())
        } else {
            Err(NodeError::InvalidState(format!("cannot send in state {}", self.state)))
        }
    }

    fn next_seq(&mut self, st: StreamType, sid: StreamId) -> SequenceNo {
        let entry = self.out_seq.entry((st, sid)).or_insert(0);
        *entry += 1;
        *entry
    }

    fn emit_err_spec(&mut self, code: u16, reason: impl Into<String>) {
        if let Some(spec) = ErrorSpec::lookup(code) {
            self.emit_err_named(spec.code, spec.class, spec.retryable, spec.fatal, reason);
        } else {
            self.emit_err_named(code, ErrorClass::Policy, false, false, reason);
        }
    }

    fn emit_err_named(
        &mut self,
        code: u16,
        class: ErrorClass,
        retryable: bool,
        fatal: bool,
        reason: impl Into<String>,
    ) {
        let reason = reason.into();
        let _ = ErrorObject::new(code, class, retryable, fatal, reason.clone()).to_cbor();
        self.events.push(Event::Error { code, class, retryable, fatal, reason });
        if fatal {
            let from = self.state;
            self.state = NodeState::Failed;
            self.events.push(Event::StateChanged { from, to: NodeState::Failed });
        }
    }
}

/// Trait abstracting the AEAD seal / open operations.
///
/// Implemented for [`gbp_mls::MlsContext`] in this crate; tests typically
/// implement a no-op identity sealer to exercise the FSM without standing
/// up an MLS group.
pub trait Sealer {
    /// Encrypts `pt` for the given stream and per-stream sequence number.
    fn seal(&mut self, st: StreamType, seq: SequenceNo, pt: &[u8]) -> Result<Vec<u8>, MlsError>;
    /// Decrypts `ct` for the given stream and per-stream sequence number.
    fn open(&mut self, st: StreamType, seq: SequenceNo, ct: &[u8]) -> Result<Vec<u8>, MlsError>;
}

impl Sealer for gbp_mls::MlsContext {
    fn seal(&mut self, st: StreamType, seq: SequenceNo, pt: &[u8]) -> Result<Vec<u8>, MlsError> {
        gbp_mls::MlsContext::seal(self, label_for(st), seq, pt)
    }
    fn open(&mut self, st: StreamType, seq: SequenceNo, ct: &[u8]) -> Result<Vec<u8>, MlsError> {
        gbp_mls::MlsContext::open(self, label_for(st), seq, ct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PlainSealer;
    impl Sealer for PlainSealer {
        fn seal(&mut self, _st: StreamType, _seq: SequenceNo, pt: &[u8]) -> Result<Vec<u8>, MlsError> {
            Ok(pt.to_vec())
        }
        fn open(&mut self, _st: StreamType, _seq: SequenceNo, ct: &[u8]) -> Result<Vec<u8>, MlsError> {
            Ok(ct.to_vec())
        }
    }

    fn group_id() -> GroupId {
        let mut g = [0u8; 16];
        g[..3].copy_from_slice(b"GBP");
        g
    }

    #[test]
    fn replay_window_rejects_repeat() {
        let mut alice = GroupNode::new(1, group_id());
        let mut bob = GroupNode::new(2, group_id());
        alice.bootstrap_as_creator(1);
        bob.bootstrap_as_joiner(1);
        let mut s = PlainSealer;
        let sid = alice.member_stream_id(2);
        let f = alice
            .send_payload(&mut s, 2, StreamType::Text, sid, GbpFlags::ordered_reliable_ack(), b"hi")
            .unwrap();
        let _ = bob.on_wire(&mut s, &f.wire).unwrap();
        let evs = bob.on_wire(&mut s, &f.wire).unwrap();
        assert!(evs.iter().any(|e| matches!(
            e, Event::Error { code: codes::REPLAY_DETECTED, .. }
        )));
    }

    #[test]
    fn epoch_mismatch_triggers_resync() {
        let mut alice = GroupNode::new(1, group_id());
        let mut bob = GroupNode::new(2, group_id());
        alice.bootstrap_as_creator(1);
        bob.bootstrap_as_joiner(1);
        alice.current_epoch = 2;
        let mut s = PlainSealer;
        let sid = alice.member_stream_id(2);
        let f = alice
            .send_payload(&mut s, 2, StreamType::Text, sid, GbpFlags::ordered_reliable_ack(), b"x")
            .unwrap();
        let _ = bob.on_wire(&mut s, &f.wire).unwrap();
        assert_eq!(bob.state, NodeState::Resyncing);
    }

    #[test]
    fn payload_emits_received_event() {
        let mut alice = GroupNode::new(1, group_id());
        let mut bob = GroupNode::new(2, group_id());
        alice.bootstrap_as_creator(1);
        bob.bootstrap_as_joiner(1);
        let mut s = PlainSealer;
        let sid = alice.member_stream_id(2);
        let f = alice
            .send_payload(&mut s, 2, StreamType::Text, sid, GbpFlags::ordered_reliable_ack(), b"payload")
            .unwrap();
        let evs = bob.on_wire(&mut s, &f.wire).unwrap();
        let pr = evs.into_iter().find_map(|e| match e {
            Event::PayloadReceived(p) => Some(p),
            _ => None,
        }).expect("payload");
        assert_eq!(pr.stream_type, StreamType::Text);
        assert_eq!(pr.plaintext, b"payload");
    }
}
