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
    ControlOpcode, ErrorClass, GbpFlags, GroupId, MemberId, NodeState, PayloadCodec, SequenceNo,
    StreamId, StreamType, TransitionId, TransitionState, codes, errors::ErrorSpec, timeouts,
};
use gbp_mls::{MlsError, label_for};
use std::collections::HashMap;
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

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
    /// Codec used to encode the plaintext (from the frame's `pf` field).
    pub codec: PayloadCodec,
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
    /// Coordinator silence exceeded `T_coordinator_grace`. The application
    /// should call [`GroupNode::claim_coordinator`] if this node has the
    /// lowest `MemberId` among currently active members.
    CoordinatorElectionNeeded,
    /// This node successfully claimed the coordinator role (sent
    /// `CAPABILITIES_ADVERTISE` with `coordinator_claim=true`).
    BecameCoordinator,
    /// A remote member broadcast a coordinator claim.
    CoordinatorClaim {
        /// The claiming member's id.
        claimant: MemberId,
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
    /// Whether this node currently holds the coordinator role.
    pub is_coordinator: bool,
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

    /// MemberId of the member whose PREPARE_TRANSITION is currently pending.
    /// Used for tie-break: if two PREPAREs arrive for the same transition_id,
    /// the one from the lower MemberId wins (gbp_rfc §8).
    pending_commit_sender: Option<MemberId>,
    /// Deadline for receiving quorum READY after issuing PREPARE_TRANSITION.
    /// Armed when coordinator sends PREPARE; fires ERR_PREPARE_TIMEOUT.
    prepare_deadline: Option<Instant>,
    /// Deadline for receiving EXECUTE_TRANSITION after sending READY_FOR_TRANSITION.
    /// Armed when a member sends READY; fires ERR_EXECUTE_TIMEOUT.
    execute_deadline: Option<Instant>,
    /// Timestamp of last coordinator activity. When silence exceeds
    /// T_COORDINATOR_GRACE_MS the node emits ERR_COORDINATOR_GONE.
    coordinator_last_seen: Option<Instant>,
}

impl GroupNode {
    /// Builds a fresh node in the `IDLE` state.
    pub fn new(member_id: MemberId, group_id: GroupId) -> Self {
        Self {
            member_id,
            group_id,
            is_coordinator: false,
            current_epoch: 0,
            last_transition_id: 0,
            pending_transition_id: 0,
            state: NodeState::Idle,
            transition_state: TransitionState::TIdle,
            out_seq: HashMap::new(),
            in_hw: HashMap::new(),
            events: Vec::new(),
            pending_commit_sender: None,
            prepare_deadline: None,
            execute_deadline: None,
            coordinator_last_seen: None,
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
    ///
    /// `expected_first_tid` lets the joiner pre-arm its pending transition
    /// state so that the very next `EXECUTE_TRANSITION` (which will arrive
    /// without a preceding PREPARE the joiner could decrypt — that PREPARE
    /// was sealed under the pre-Welcome epoch) is accepted by
    /// `handle_control`'s tid-validation matrix. Pass `0` if the joiner
    /// recovered out-of-band and is already current.
    pub fn bootstrap_as_joiner(&mut self, epoch: u64, expected_first_tid: u32) {
        self.transition(NodeState::Connecting);
        self.transition(NodeState::EstablishingGroup);
        self.current_epoch = epoch;
        if expected_first_tid > 0 {
            self.pending_transition_id = expected_first_tid;
            self.transition_state = TransitionState::TPrepared;
        }
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
        debug_assert!(
            self.member_id < 1_000_000,
            "member_id overflow: {0}",
            self.member_id
        );
        base + self.member_id * 100
    }

    /// Sends an opaque plaintext payload on the given stream.
    ///
    /// Used by the sub-protocol clients: each one encodes its message and
    /// forwards the resulting bytes here together with the codec that was used.
    /// Pass [`PayloadCodec::Cbor`] for the default encoding; it is
    /// backward-compatible with pre-1.5 peers.
    pub fn send_payload<S: Sealer>(
        &mut self,
        seal: &mut S,
        target: MemberId,
        stream_type: StreamType,
        stream_id: StreamId,
        flags: u16,
        plaintext: &[u8],
        codec: PayloadCodec,
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
            codec.as_u8(),
        );
        Ok(OutboundFrame {
            to: target,
            wire: frame.to_cbor(),
        })
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
                self.prepare_deadline =
                    Some(Instant::now() + Duration::from_millis(timeouts::T_PREPARE_MAX_MS));
                self.execute_deadline = None;
            }
            ControlOpcode::ReadyForTransition => {
                self.execute_deadline =
                    Some(Instant::now() + Duration::from_millis(timeouts::T_EXECUTE_MAX_MS));
            }
            ControlOpcode::ExecuteTransition | ControlOpcode::AbortTransition => {
                self.prepare_deadline = None;
                self.execute_deadline = None;
                if opcode == ControlOpcode::AbortTransition {
                    self.pending_transition_id = 0;
                    self.transition_state = TransitionState::TAborted;
                }
            }
            _ => {}
        }
        let stream_id = self.member_stream_id(0);
        self.send_payload(
            seal,
            target,
            StreamType::Control,
            stream_id,
            flags,
            &ctl.to_cbor(),
            PayloadCodec::Cbor,
        )
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
                self.emit_err_spec(codes::STREAM_POLICY_VIOLATION, format!("frame decode: {e}"));
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
            self.emit_err_spec(codes::STREAM_POLICY_VIOLATION, format!("payload size: {e}"));
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
                format!(
                    "got tid={}, expected {}",
                    frame.transition_id, self.last_transition_id
                ),
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
                // Non-fatal: a frame addressed under a different MLS epoch
                // (e.g. PREPARE_TRANSITION reaching a fresh joiner that has
                // already accepted the post-commit Welcome) cannot be
                // decrypted, but that's expected and the node MUST keep
                // running to receive the subsequent EXECUTE frame on the
                // shared post-merge epoch.
                self.emit_err_named(
                    codes::DECRYPT_FAILED,
                    ErrorClass::Crypto,
                    true,  // retryable: caller may resync via digest
                    false, // non-fatal
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
                codec: frame.payload_codec(),
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
                self.pending_transition_id != 0 && c.transition_id == self.pending_transition_id
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
                // Tie-break (gbp_rfc §8): if a PREPARE for the same tid already
                // exists from another sender, keep whichever has the lower
                // MemberId — that member's commit is the canonical winner.
                if self.pending_transition_id == c.transition_id {
                    let current_winner = self.pending_commit_sender.unwrap_or(MemberId::MAX);
                    if c.sender_id >= current_winner {
                        // Existing winner holds; discard this competing commit
                        // but still surface the Control event so upper layers
                        // can observe it.
                        self.events.push(Event::Control {
                            from: c.sender_id,
                            opcode,
                            transition_id: c.transition_id,
                            request_id: c.request_id,
                            args: c.args.to_vec(),
                        });
                        return;
                    }
                    // New sender wins — replace.
                }
                self.pending_transition_id = c.transition_id;
                self.pending_commit_sender = Some(c.sender_id);
                self.transition_state = TransitionState::TPrepared;
                // PREPARE originates from the coordinator — record activity.
                self.note_coordinator_activity();
                // Arm execute deadline: member must see EXECUTE within T_execute_max.
                self.execute_deadline =
                    Some(Instant::now() + Duration::from_millis(timeouts::T_EXECUTE_MAX_MS));
            }
            ControlOpcode::ReadyForTransition => {
                self.transition_state = TransitionState::TReady;
                // Coordinator received a READY — clear the per-member wait.
                self.prepare_deadline = None;
            }
            ControlOpcode::ExecuteTransition => {
                self.execute_deadline = None;
                self.pending_commit_sender = None;
                self.apply_transition(c.transition_id);
                self.note_coordinator_activity();
            }
            ControlOpcode::AbortTransition => {
                self.prepare_deadline = None;
                self.execute_deadline = None;
                self.pending_commit_sender = None;
                self.transition_state = TransitionState::TAborted;
                self.pending_transition_id = 0;
            }
            ControlOpcode::GroupStateDigestResponse => {
                if self.state == NodeState::Resyncing {
                    self.transition(NodeState::Active);
                }
            }
            ControlOpcode::CapabilitiesAdvertise => {
                if Self::is_coordinator_claim(&c.args) {
                    // Coordinator is alive — reset silence timer.
                    self.note_coordinator_activity();
                    // Collision resolution (gbp-control-plane §5.1): if we
                    // also claimed and the remote claimant has a lower
                    // MemberId, yield the coordinator role to them.
                    if self.is_coordinator && c.sender_id < self.member_id {
                        self.is_coordinator = false;
                    }
                    self.events.push(Event::CoordinatorClaim {
                        claimant: c.sender_id,
                    });
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
        self.pending_commit_sender = None;
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

    /// Checks FSM deadlines and emits timeout events if any have expired.
    ///
    /// Call this regularly from the application event loop (e.g. every 500 ms).
    /// Returns the same events that would come from [`GroupNode::drain_events`];
    /// the caller may also drain events separately — this method does not
    /// duplicate them.
    pub fn check_timeouts(&mut self) -> Vec<Event> {
        let now = Instant::now();

        if self.prepare_deadline.is_some_and(|d| now >= d) {
            self.prepare_deadline = None;
            self.execute_deadline = None;
            self.pending_transition_id = 0;
            self.transition_state = TransitionState::TAborted;
            self.emit_err_spec(codes::PREPARE_TIMEOUT, "T_prepare_max exceeded");
        }

        if self.execute_deadline.is_some_and(|d| now >= d) {
            self.execute_deadline = None;
            self.emit_err_spec(codes::EXECUTE_TIMEOUT, "T_execute_max exceeded");
        }

        if self.coordinator_last_seen.is_some_and(|t| {
            now.duration_since(t).as_millis() as u64 >= timeouts::T_COORDINATOR_GRACE_MS
        }) {
            self.coordinator_last_seen = None;
            self.is_coordinator = false;
            self.emit_err_spec(
                codes::COORDINATOR_GONE,
                "coordinator silence exceeded T_coordinator_grace",
            );
            self.events.push(Event::CoordinatorElectionNeeded);
        }

        self.drain_events()
    }

    /// Records that the coordinator was active right now.
    ///
    /// Call this whenever the node receives a frame from the current
    /// coordinator (e.g. `PREPARE_TRANSITION`, `EXECUTE_TRANSITION`,
    /// `CAPABILITIES_ADVERTISE` with `coordinator_claim`). Resets the
    /// coordinator-silence timer used to detect `ERR_COORDINATOR_GONE`.
    pub fn note_coordinator_activity(&mut self) {
        self.coordinator_last_seen = Some(Instant::now());
    }

    /// Claims the coordinator role by broadcasting `CAPABILITIES_ADVERTISE`
    /// with `coordinator_claim=true` (gbp-control-plane §5.1).
    ///
    /// Call this when [`Event::CoordinatorElectionNeeded`] fires **and** this
    /// node has the lowest `MemberId` among currently active members. The
    /// caller is responsible for delivering the returned frame to every group
    /// member.
    ///
    /// The args payload is the minimal CBOR map `{0: true}` encoding a
    /// coordinator claim flag.
    pub fn claim_coordinator<S: Sealer>(
        &mut self,
        seal: &mut S,
        target: MemberId,
    ) -> Result<OutboundFrame, NodeError> {
        // CBOR: {0: true}  →  A1 00 F5
        let args = vec![0xA1u8, 0x00, 0xF5];
        self.is_coordinator = true;
        self.coordinator_last_seen = Some(Instant::now());
        self.events.push(Event::BecameCoordinator);
        self.send_control(
            seal,
            target,
            ControlOpcode::CapabilitiesAdvertise,
            self.last_transition_id,
            0,
            args,
        )
    }

    /// Returns `true` if the raw args bytes of a `CAPABILITIES_ADVERTISE`
    /// frame encode a coordinator claim (`{0: true}` in CBOR).
    fn is_coordinator_claim(args: &[u8]) -> bool {
        // Minimal CBOR map {0: true}: A1 00 F5
        // We also accept larger maps where key 0 maps to true.
        // Fast path: exact match on the minimal encoding.
        if args == [0xA1, 0x00, 0xF5] {
            return true;
        }
        // General path: scan for the sequence 00 F5 (uint(0) → true) inside a
        // CBOR map. This is intentionally simple; a full CBOR parser lives in
        // the gbp-protocol crate and is not a dependency here.
        args.windows(2).any(|w| w == [0x00, 0xF5])
    }

    fn transition(&mut self, next: NodeState) {
        if self.state == next {
            return;
        }
        if !self.state.can_transition_to(next) {
            let from = self.state;
            self.state = NodeState::Failed;
            self.events.push(Event::StateChanged {
                from,
                to: NodeState::Failed,
            });
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
            Err(NodeError::InvalidState(format!(
                "cannot send in state {}",
                self.state
            )))
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
        // ErrorSpec is authoritative for known codes — use its class/retryable/fatal
        // so that the wire error always matches the registry.
        let (class, retryable, fatal) = if let Some(spec) = ErrorSpec::lookup(code) {
            (spec.class, spec.retryable, spec.fatal)
        } else {
            (class, retryable, fatal)
        };
        let _ = ErrorObject::new(code, class, retryable, fatal, reason.clone()).to_cbor();
        self.events.push(Event::Error {
            code,
            class,
            retryable,
            fatal,
            reason,
        });
        if fatal {
            let from = self.state;
            self.state = NodeState::Failed;
            self.events.push(Event::StateChanged {
                from,
                to: NodeState::Failed,
            });
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
        fn seal(
            &mut self,
            _st: StreamType,
            _seq: SequenceNo,
            pt: &[u8],
        ) -> Result<Vec<u8>, MlsError> {
            Ok(pt.to_vec())
        }
        fn open(
            &mut self,
            _st: StreamType,
            _seq: SequenceNo,
            ct: &[u8],
        ) -> Result<Vec<u8>, MlsError> {
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
        bob.bootstrap_as_joiner(1, 0);
        let mut s = PlainSealer;
        let sid = alice.member_stream_id(2);
        let f = alice
            .send_payload(
                &mut s,
                2,
                StreamType::Text,
                sid,
                GbpFlags::ordered_reliable_ack(),
                b"hi",
                PayloadCodec::Cbor,
            )
            .unwrap();
        let _ = bob.on_wire(&mut s, &f.wire).unwrap();
        let evs = bob.on_wire(&mut s, &f.wire).unwrap();
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::Error {
                code: codes::REPLAY_DETECTED,
                ..
            }
        )));
    }

    #[test]
    fn epoch_mismatch_triggers_resync() {
        let mut alice = GroupNode::new(1, group_id());
        let mut bob = GroupNode::new(2, group_id());
        alice.bootstrap_as_creator(1);
        bob.bootstrap_as_joiner(1, 0);
        alice.current_epoch = 2;
        let mut s = PlainSealer;
        let sid = alice.member_stream_id(2);
        let f = alice
            .send_payload(
                &mut s,
                2,
                StreamType::Text,
                sid,
                GbpFlags::ordered_reliable_ack(),
                b"x",
                PayloadCodec::Cbor,
            )
            .unwrap();
        let _ = bob.on_wire(&mut s, &f.wire).unwrap();
        assert_eq!(bob.state, NodeState::Resyncing);
    }

    #[test]
    fn payload_emits_received_event() {
        let mut alice = GroupNode::new(1, group_id());
        let mut bob = GroupNode::new(2, group_id());
        alice.bootstrap_as_creator(1);
        bob.bootstrap_as_joiner(1, 0);
        let mut s = PlainSealer;
        let sid = alice.member_stream_id(2);
        let f = alice
            .send_payload(
                &mut s,
                2,
                StreamType::Text,
                sid,
                GbpFlags::ordered_reliable_ack(),
                b"payload",
                PayloadCodec::Cbor,
            )
            .unwrap();
        let evs = bob.on_wire(&mut s, &f.wire).unwrap();
        let pr = evs
            .into_iter()
            .find_map(|e| match e {
                Event::PayloadReceived(p) => Some(p),
                _ => None,
            })
            .expect("payload");
        assert_eq!(pr.stream_type, StreamType::Text);
        assert_eq!(pr.plaintext, b"payload");
    }

    // ---- Control-plane handshake -----------------------------------------

    fn drain_errs(events: &[Event]) -> Vec<u16> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Error { code, .. } => Some(*code),
                _ => None,
            })
            .collect()
    }

    fn drain_controls(events: &[Event]) -> Vec<(ControlOpcode, TransitionId)> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Control {
                    opcode,
                    transition_id,
                    ..
                } => Some((*opcode, *transition_id)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn prepare_transition_sets_pending_on_sender_and_receiver() {
        let mut coord = GroupNode::new(1, group_id());
        let mut peer = GroupNode::new(2, group_id());
        coord.bootstrap_as_creator(0);
        peer.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        // Coordinator sends PREPARE for tid=1
        let f = coord
            .send_control(
                &mut s,
                0,
                ControlOpcode::PrepareTransition,
                1,
                100,
                b"commit-blob".to_vec(),
            )
            .unwrap();
        assert_eq!(coord.pending_transition_id, 1, "sender mirrors pending");
        assert_eq!(coord.transition_state, TransitionState::TPrepared);
        let evs = peer.on_wire(&mut s, &f.wire).unwrap();
        assert_eq!(peer.pending_transition_id, 1, "receiver records pending");
        assert!(
            drain_errs(&evs).is_empty(),
            "no error: {:?}",
            drain_errs(&evs)
        );
        let ctls = drain_controls(&evs);
        assert_eq!(ctls, vec![(ControlOpcode::PrepareTransition, 1)]);
    }

    #[test]
    fn ready_with_wrong_tid_is_rejected() {
        let mut coord = GroupNode::new(1, group_id());
        let mut peer = GroupNode::new(2, group_id());
        coord.bootstrap_as_creator(0);
        peer.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        let f = coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        peer.on_wire(&mut s, &f.wire).unwrap();
        // Peer fakes a READY for the wrong tid
        let bogus = peer
            .send_control(&mut s, 1, ControlOpcode::ReadyForTransition, 7, 1, vec![])
            .unwrap();
        let evs = coord.on_wire(&mut s, &bogus.wire).unwrap();
        let errs = drain_errs(&evs);
        assert!(errs.contains(&codes::TRANSITION_MISMATCH), "got {:?}", errs);
    }

    #[test]
    fn execute_advances_epoch_and_clears_pending() {
        let mut coord = GroupNode::new(1, group_id());
        let mut peer = GroupNode::new(2, group_id());
        coord.bootstrap_as_creator(0);
        peer.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        let prep = coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        peer.on_wire(&mut s, &prep.wire).unwrap();
        // Coordinator broadcasts EXECUTE; both sides apply (coord locally, peer via on_wire)
        let exec = coord
            .send_control(&mut s, 0, ControlOpcode::ExecuteTransition, 1, 2, vec![])
            .unwrap();
        coord.apply_transition(1);
        let evs = peer.on_wire(&mut s, &exec.wire).unwrap();
        assert_eq!(coord.last_transition_id, 1);
        assert_eq!(coord.current_epoch, 1);
        assert_eq!(peer.last_transition_id, 1);
        assert_eq!(peer.current_epoch, 1);
        assert_eq!(peer.pending_transition_id, 0);
        assert!(evs.iter().any(|e| matches!(
            e,
            Event::EpochAdvanced {
                transition_id: 1,
                ..
            }
        )));
    }

    #[test]
    fn abort_clears_pending_no_advance() {
        let mut coord = GroupNode::new(1, group_id());
        let mut peer = GroupNode::new(2, group_id());
        coord.bootstrap_as_creator(0);
        peer.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        let prep = coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        peer.on_wire(&mut s, &prep.wire).unwrap();
        let abort = coord
            .send_control(&mut s, 0, ControlOpcode::AbortTransition, 1, 2, vec![])
            .unwrap();
        peer.on_wire(&mut s, &abort.wire).unwrap();
        assert_eq!(peer.pending_transition_id, 0);
        assert_eq!(peer.current_epoch, 0);
        assert_eq!(peer.transition_state, TransitionState::TAborted);
        assert_eq!(coord.transition_state, TransitionState::TAborted);
    }

    #[test]
    fn bootstrap_as_joiner_with_expected_tid_accepts_first_execute() {
        let mut coord = GroupNode::new(1, group_id());
        // Joiner pre-arms expected_first_tid=1 — typical post-Welcome state.
        let mut joiner = GroupNode::new(2, group_id());
        coord.bootstrap_as_creator(0);
        joiner.bootstrap_as_joiner(0, 1);
        assert_eq!(joiner.pending_transition_id, 1);
        let mut s = PlainSealer;
        // Coordinator must mirror its pending too — simulate by sending PREPARE
        let _ = coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        // EXECUTE should be accepted by the joiner without ever seeing PREPARE
        let exec = coord
            .send_control(&mut s, 0, ControlOpcode::ExecuteTransition, 1, 2, vec![])
            .unwrap();
        let evs = joiner.on_wire(&mut s, &exec.wire).unwrap();
        let errs = drain_errs(&evs);
        assert!(
            errs.is_empty(),
            "expected clean apply, got errors {:?}",
            errs
        );
        assert_eq!(joiner.last_transition_id, 1);
        assert_eq!(joiner.current_epoch, 1);
    }

    // ---- Coordinator handover (gbp-control-plane §5.1) ---------------------

    #[test]
    fn claim_coordinator_sets_flag_and_emits_event() {
        let mut node = GroupNode::new(1, group_id());
        node.bootstrap_as_creator(0);
        node.drain_events();
        let mut s = PlainSealer;
        let _ = node.claim_coordinator(&mut s, 0).unwrap();
        assert!(node.is_coordinator);
        let evs = node.drain_events();
        assert!(evs.iter().any(|e| matches!(e, Event::BecameCoordinator)));
    }

    #[test]
    fn coordinator_gone_emits_election_needed() {
        let mut member = GroupNode::new(2, group_id());
        member.bootstrap_as_joiner(0, 0);
        member.coordinator_last_seen = Some(Instant::now() - Duration::from_millis(11_000));
        let evs = member.check_timeouts();
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::CoordinatorElectionNeeded))
        );
        assert!(!member.is_coordinator, "flag cleared on silence");
    }

    #[test]
    fn capabilities_advertise_with_claim_resets_silence_timer() {
        let mut member = GroupNode::new(2, group_id());
        let mut coord = GroupNode::new(1, group_id());
        member.bootstrap_as_joiner(0, 0);
        coord.bootstrap_as_creator(0);
        let mut s = PlainSealer;
        // coord sends claim
        let f = coord.claim_coordinator(&mut s, 2).unwrap();
        // on_wire already drains events — use the returned vec.
        let evs = member.on_wire(&mut s, &f.wire).unwrap();
        assert!(
            member.coordinator_last_seen.is_some(),
            "silence timer reset"
        );
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::CoordinatorClaim { claimant: 1 }))
        );
    }

    #[test]
    fn higher_id_yields_to_lower_claimant() {
        // Node 5 claims first, then receives a claim from node 2 (lower) → yields.
        let mut node5 = GroupNode::new(5, group_id());
        let mut node2 = GroupNode::new(2, group_id());
        node5.bootstrap_as_joiner(0, 0);
        node2.bootstrap_as_creator(0);
        let mut s = PlainSealer;
        // node5 claims
        node5.is_coordinator = true;
        // node2 broadcasts claim
        let f = node2.claim_coordinator(&mut s, 5).unwrap();
        node5.on_wire(&mut s, &f.wire).unwrap();
        assert!(!node5.is_coordinator, "node5 yielded to node2");
    }

    #[test]
    fn lower_id_keeps_coordinator_against_higher_claimant() {
        let mut node1 = GroupNode::new(1, group_id());
        let mut node5 = GroupNode::new(5, group_id());
        node1.bootstrap_as_creator(0);
        node5.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        node1.is_coordinator = true;
        let f = node5.claim_coordinator(&mut s, 1).unwrap();
        node1.on_wire(&mut s, &f.wire).unwrap();
        assert!(node1.is_coordinator, "node1 keeps role — it has lower id");
    }

    // ---- Tie-break (gbp_rfc §8) ---------------------------------------------

    #[test]
    fn competing_prepare_lower_member_id_wins() {
        // Two coordinators issue PREPARE for the same tid.
        // Member 1 (lower) sends first — member 3 (higher) is the loser.
        let mut node = GroupNode::new(10, group_id());
        node.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;

        // Build a PREPARE from member 1 (lower id).
        let mut sender1 = GroupNode::new(1, group_id());
        sender1.bootstrap_as_creator(0);
        let f1 = sender1
            .send_control(
                &mut s,
                10,
                ControlOpcode::PrepareTransition,
                1,
                1,
                b"commit-A".to_vec(),
            )
            .unwrap();
        node.on_wire(&mut s, &f1.wire).unwrap();
        assert_eq!(
            node.pending_commit_sender,
            Some(1),
            "member 1 is initial winner"
        );

        // Build a PREPARE from member 3 (higher id, same tid).
        let mut sender3 = GroupNode::new(3, group_id());
        sender3.bootstrap_as_creator(0);
        let f3 = sender3
            .send_control(
                &mut s,
                10,
                ControlOpcode::PrepareTransition,
                1,
                2,
                b"commit-B".to_vec(),
            )
            .unwrap();
        node.on_wire(&mut s, &f3.wire).unwrap();
        // Lower sender (1) keeps the win.
        assert_eq!(node.pending_commit_sender, Some(1), "member 1 still wins");
        assert_eq!(node.pending_transition_id, 1);
    }

    #[test]
    fn competing_prepare_later_lower_id_displaces_winner() {
        // Member 5 arrives first, then member 2 (lower) — member 2 wins.
        let mut node = GroupNode::new(10, group_id());
        node.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;

        let mut sender5 = GroupNode::new(5, group_id());
        sender5.bootstrap_as_creator(0);
        let f5 = sender5
            .send_control(
                &mut s,
                10,
                ControlOpcode::PrepareTransition,
                1,
                1,
                b"commit-X".to_vec(),
            )
            .unwrap();
        node.on_wire(&mut s, &f5.wire).unwrap();
        assert_eq!(node.pending_commit_sender, Some(5));

        let mut sender2 = GroupNode::new(2, group_id());
        sender2.bootstrap_as_creator(0);
        let f2 = sender2
            .send_control(
                &mut s,
                10,
                ControlOpcode::PrepareTransition,
                1,
                2,
                b"commit-Y".to_vec(),
            )
            .unwrap();
        node.on_wire(&mut s, &f2.wire).unwrap();
        assert_eq!(
            node.pending_commit_sender,
            Some(2),
            "member 2 displaces member 5"
        );
    }

    #[test]
    fn apply_transition_clears_commit_sender() {
        let mut coord = GroupNode::new(1, group_id());
        coord.bootstrap_as_creator(0);
        let mut s = PlainSealer;
        coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        coord.apply_transition(1);
        assert_eq!(coord.pending_commit_sender, None);
    }

    // ---- Timer engine -------------------------------------------------------

    #[test]
    fn prepare_timeout_fires_when_deadline_exceeded() {
        let mut coord = GroupNode::new(1, group_id());
        coord.bootstrap_as_creator(0);
        let mut s = PlainSealer;
        coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        // Manually backdate the deadline so it appears expired.
        coord.prepare_deadline = Some(Instant::now() - Duration::from_millis(1));
        let evs = coord.check_timeouts();
        assert!(
            evs.iter().any(|e| matches!(
                e,
                Event::Error {
                    code: codes::PREPARE_TIMEOUT,
                    ..
                }
            )),
            "expected PREPARE_TIMEOUT, got {:?}",
            evs
        );
        assert_eq!(
            coord.transition_state,
            TransitionState::TAborted,
            "transition aborted"
        );
        assert_eq!(coord.prepare_deadline, None, "deadline cleared");
    }

    #[test]
    fn execute_timeout_fires_when_deadline_exceeded() {
        let mut member = GroupNode::new(2, group_id());
        member.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        // Simulate READY sent → execute_deadline armed.
        member.pending_transition_id = 1;
        member.transition_state = TransitionState::TPrepared;
        member
            .send_control(&mut s, 1, ControlOpcode::ReadyForTransition, 1, 1, vec![])
            .unwrap();
        // Backdate.
        member.execute_deadline = Some(Instant::now() - Duration::from_millis(1));
        let evs = member.check_timeouts();
        assert!(
            evs.iter().any(|e| matches!(
                e,
                Event::Error {
                    code: codes::EXECUTE_TIMEOUT,
                    ..
                }
            )),
            "expected EXECUTE_TIMEOUT, got {:?}",
            evs
        );
        assert_eq!(member.execute_deadline, None, "deadline cleared");
    }

    #[test]
    fn coordinator_gone_fires_after_silence() {
        let mut member = GroupNode::new(2, group_id());
        member.bootstrap_as_joiner(0, 0);
        // Simulate coordinator was seen 11 seconds ago (> T_COORDINATOR_GRACE_MS = 10_000).
        member.coordinator_last_seen = Some(Instant::now() - Duration::from_millis(11_000));
        let evs = member.check_timeouts();
        assert!(
            evs.iter().any(|e| matches!(
                e,
                Event::Error {
                    code: codes::COORDINATOR_GONE,
                    ..
                }
            )),
            "expected COORDINATOR_GONE, got {:?}",
            evs
        );
        assert_eq!(member.coordinator_last_seen, None, "timer cleared");
    }

    #[test]
    fn note_coordinator_activity_resets_silence_timer() {
        let mut member = GroupNode::new(2, group_id());
        member.bootstrap_as_joiner(0, 0);
        // Old timestamp — would fire.
        member.coordinator_last_seen = Some(Instant::now() - Duration::from_millis(11_000));
        // Reset.
        member.note_coordinator_activity();
        let evs = member.check_timeouts();
        assert!(
            !evs.iter().any(|e| matches!(
                e,
                Event::Error {
                    code: codes::COORDINATOR_GONE,
                    ..
                }
            )),
            "should NOT fire after reset"
        );
    }

    #[test]
    fn execute_clears_prepare_deadline() {
        let mut coord = GroupNode::new(1, group_id());
        coord.bootstrap_as_creator(0);
        let mut s = PlainSealer;
        coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        assert!(coord.prepare_deadline.is_some(), "deadline armed");
        coord
            .send_control(&mut s, 0, ControlOpcode::ExecuteTransition, 1, 2, vec![])
            .unwrap();
        assert_eq!(coord.prepare_deadline, None, "deadline cleared on EXECUTE");
        assert_eq!(
            coord.execute_deadline, None,
            "execute_deadline also cleared"
        );
    }

    #[test]
    fn receive_prepare_arms_execute_deadline() {
        let mut coord = GroupNode::new(1, group_id());
        let mut member = GroupNode::new(2, group_id());
        coord.bootstrap_as_creator(0);
        member.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        let f = coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        member.on_wire(&mut s, &f.wire).unwrap();
        assert!(
            member.execute_deadline.is_some(),
            "execute_deadline armed on receiving PREPARE"
        );
    }

    #[test]
    fn receive_execute_clears_execute_deadline() {
        let mut coord = GroupNode::new(1, group_id());
        let mut member = GroupNode::new(2, group_id());
        coord.bootstrap_as_creator(0);
        member.bootstrap_as_joiner(0, 0);
        let mut s = PlainSealer;
        let prep = coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        member.on_wire(&mut s, &prep.wire).unwrap();
        let exec = coord
            .send_control(&mut s, 0, ControlOpcode::ExecuteTransition, 1, 2, vec![])
            .unwrap();
        member.on_wire(&mut s, &exec.wire).unwrap();
        assert_eq!(member.execute_deadline, None, "cleared on EXECUTE");
    }

    #[test]
    fn no_timeout_when_deadlines_not_set() {
        let mut node = GroupNode::new(1, group_id());
        node.bootstrap_as_creator(0);
        node.drain_events(); // clear bootstrap StateChanged events
        let evs = node.check_timeouts();
        assert!(evs.is_empty(), "no events without armed deadlines");
    }

    #[test]
    fn prepare_with_already_applied_tid_is_rejected() {
        // After the coordinator has fully applied tid=1, a replay or
        // late-coordinator PREPARE with the same tid must fail validation.
        let mut coord = GroupNode::new(1, group_id());
        coord.bootstrap_as_creator(0);
        let mut s = PlainSealer;
        let _ = coord
            .send_control(&mut s, 0, ControlOpcode::PrepareTransition, 1, 1, vec![])
            .unwrap();
        coord.apply_transition(1);
        assert_eq!(coord.last_transition_id, 1);
        assert_eq!(coord.pending_transition_id, 0);
        // Forge a PREPARE with the same already-applied tid (epoch matches
        // because we synthesise it locally with a peer node on the same
        // post-apply epoch).
        let mut peer = GroupNode::new(2, group_id());
        peer.bootstrap_as_joiner(coord.current_epoch, 0);
        let stale = peer
            .send_control(&mut s, 1, ControlOpcode::PrepareTransition, 1, 9, vec![])
            .unwrap();
        let evs = coord.on_wire(&mut s, &stale.wire).unwrap();
        let errs = drain_errs(&evs);
        assert!(
            errs.contains(&codes::TRANSITION_MISMATCH),
            "expected TRANSITION_MISMATCH, got {:?}",
            errs
        );
    }

    #[test]
    fn decrypt_failed_is_non_fatal() {
        // Simulate a frame our open() can't unlock: a sealer that fails on `open`.
        struct OpenFailSealer;
        impl Sealer for OpenFailSealer {
            fn seal(
                &mut self,
                _: StreamType,
                _: SequenceNo,
                p: &[u8],
            ) -> Result<Vec<u8>, MlsError> {
                Ok(p.to_vec())
            }
            fn open(
                &mut self,
                _: StreamType,
                _: SequenceNo,
                _: &[u8],
            ) -> Result<Vec<u8>, MlsError> {
                Err(MlsError::Aead("simulated".into()))
            }
        }
        let mut alice = GroupNode::new(1, group_id());
        let mut bob = GroupNode::new(2, group_id());
        alice.bootstrap_as_creator(1);
        bob.bootstrap_as_joiner(1, 0);
        let mut s = PlainSealer;
        let sid = alice.member_stream_id(2);
        let f = alice
            .send_payload(
                &mut s,
                2,
                StreamType::Text,
                sid,
                GbpFlags::ordered_reliable_ack(),
                b"x",
                PayloadCodec::Cbor,
            )
            .unwrap();
        let mut fail = OpenFailSealer;
        let evs = bob.on_wire(&mut fail, &f.wire).unwrap();
        let err = evs
            .iter()
            .find_map(|e| match e {
                Event::Error {
                    code,
                    fatal,
                    retryable,
                    ..
                } => Some((*code, *fatal, *retryable)),
                _ => None,
            })
            .expect("error event");
        assert_eq!(err.0, codes::DECRYPT_FAILED);
        assert!(!err.1, "must be non-fatal");
        assert!(err.2, "must be retryable");
        assert_eq!(bob.state, NodeState::Active, "bob stays Active");
    }
}
