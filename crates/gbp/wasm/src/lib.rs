//! Browser/WASM bindings for the Group Protocol Stack.
//!
//! Exported JS classes: [`MlsContext`], [`GroupNode`], [`GtpClient`],
//! [`GapClient`], [`GspClient`], [`SFrameSession`], [`SFrameEncryptor`], plus
//! the [`PayloadCodec`], [`SignalType`], [`ControlOpcode`] and [`CipherSuite`]
//! enums. This surface mirrors the C-ABI (`gbp-stack-ffi`) and the C#/Python/JS
//! SDKs so a browser client has the same gap (audio) / gsp (signalling) / gtp
//! (text) / sframe (media E2EE) capabilities as every other binding.
//! The async init function is generated automatically by wasm-pack.

#![cfg(target_arch = "wasm32")]

use gap::{GapAccept, GapClient as RustGapClient};
use gbp_core::MemberId;
use gbp_node::{Event, GroupNode as RustGroupNode};
use gbp_sframe::{
    CipherSuite as RustCipherSuite, SFrameDecryptor as RustSFrameDecryptor,
    SFrameEncryptor as RustSFrameEncryptor, SFrameSession as RustSFrameSession,
};
use gsp::{GspAccept, GspClient as RustGspClient, GspError};
use gtp::{GtpAccept, GtpClient as RustGtpClient};
use js_sys::{Array, Object, Reflect, Uint8Array};
use openmls::prelude::tls_codec::Serialize as TlsSerialize;
use openmls::prelude::{KeyPackageIn, OpenMlsProvider, ProtocolVersion};
use tls_codec::Deserialize as TlsDeserialize;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

// ─── helpers ────────────────────────────────────────────────────────────────

fn set(obj: &Object, key: &str, val: &JsValue) {
    Reflect::set(obj, &JsValue::from_str(key), val).unwrap_throw();
}

fn u8s(bytes: &[u8]) -> JsValue {
    Uint8Array::from(bytes).into()
}

fn js_err(msg: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&msg.to_string())
}

fn event_to_js(ev: Event) -> JsValue {
    let obj = Object::new();
    match ev {
        Event::PayloadReceived(p) => {
            set(&obj, "kind", &"payload_received".into());
            set(&obj, "streamType", &JsValue::from_f64(p.stream_type.as_u8() as f64));
            set(&obj, "plaintext", &u8s(&p.plaintext));
            set(&obj, "sequenceNo", &JsValue::from_f64(p.sequence_no as f64));
            set(&obj, "codec", &JsValue::from_f64(p.codec as u8 as f64));
        }
        Event::StateChanged { from, to } => {
            set(&obj, "kind", &"state_changed".into());
            set(&obj, "from", &JsValue::from_str(&from.to_string()));
            set(&obj, "to", &JsValue::from_str(&to.to_string()));
        }
        Event::EpochAdvanced { epoch, transition_id } => {
            set(&obj, "kind", &"epoch_advanced".into());
            set(&obj, "epoch", &js_sys::BigInt::from(epoch).into());
            set(&obj, "transitionId", &JsValue::from_f64(transition_id as f64));
        }
        Event::Error { code, reason, fatal, retryable, .. } => {
            set(&obj, "kind", &"error".into());
            set(&obj, "code", &JsValue::from_f64(code as f64));
            set(&obj, "reason", &JsValue::from_str(&reason));
            set(&obj, "fatal", &JsValue::from_bool(fatal));
            set(&obj, "retryable", &JsValue::from_bool(retryable));
        }
        Event::Control { from, opcode, transition_id, .. } => {
            set(&obj, "kind", &"control".into());
            set(&obj, "from", &JsValue::from_f64(from as f64));
            set(&obj, "opcode", &JsValue::from_f64(opcode as u8 as f64));
            set(&obj, "transitionId", &JsValue::from_f64(transition_id as f64));
        }
        _ => {
            set(&obj, "kind", &"other".into());
        }
    }
    obj.into()
}

/// Converts an optional JS codec selector into the canonical payload codec,
/// defaulting to CBOR — the same fallback the C ABI uses. JS callers pass
/// `undefined` (or omit the argument) for the default, or a [`PayloadCodec`]
/// value / raw `0|1|2` to select an encoding.
fn codec_from(c: Option<u8>) -> gbp_core::PayloadCodec {
    c.and_then(gbp_core::PayloadCodec::from_u8)
        .unwrap_or(gbp_core::PayloadCodec::Cbor)
}

// ─── Exported enums (parity with the C#/Python/JS SDKs) ───────────────────────

/// Payload wire-encoding selector. `Cbor` is the interoperable default;
/// `FlatBuffers` minimises decode latency and is preferred for audio.
#[wasm_bindgen]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PayloadCodec {
    Cbor = 0,
    Protobuf = 1,
    FlatBuffers = 2,
}

/// GSP signal kinds (membership / role / stream / codec control).
#[wasm_bindgen]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignalType {
    Join = 100,
    Leave = 101,
    RoleChange = 102,
    Mute = 200,
    Unmute = 201,
    StreamStart = 300,
    StreamStop = 301,
    CodecUpdate = 400,
}

/// GBP control-plane opcodes (epoch-transition coordination + diagnostics).
#[wasm_bindgen]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlOpcode {
    PrepareTransition = 0x0001,
    ReadyForTransition = 0x0002,
    ExecuteTransition = 0x0003,
    AbortTransition = 0x0004,
    GroupStateDigestRequest = 0x0005,
    GroupStateDigestResponse = 0x0006,
    ReportInvalidCommit = 0x0007,
    CapabilitiesAdvertise = 0x0008,
    Ack = 0x0009,
    Nack = 0x000A,
}

/// SFrame AEAD ciphersuite. `Aes128Gcm` is the standard choice.
#[wasm_bindgen]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CipherSuite {
    Aes128Gcm = 0,
    Aes256Gcm = 1,
}

// ─── MlsContext ──────────────────────────────────────────────────────────────

/// MLS group state for one member.
///
/// JS usage:
/// ```js
/// const alice = MlsContext.create("alice");
/// const bob   = MlsContext.create("bob");
/// const welcome = alice.invite(bob.keyPackage);
/// bob.acceptWelcome(welcome);
/// // alice.epoch === bob.epoch === 1n
/// ```
#[wasm_bindgen]
pub struct MlsContext {
    inner: RefCell<gbp_mls::MlsContext>,
    kp_bytes: Vec<u8>,
}

#[wasm_bindgen]
impl MlsContext {
    /// Creates a new member identity.
    ///
    /// The returned object holds a pre-generated key package that another
    /// member can pass to [`invite`] to add this member to a group.
    #[wasm_bindgen(js_name = "create")]
    pub fn create(user_id: &str) -> Result<MlsContext, JsValue> {
        let (ctx, kpb) = gbp_mls::MlsContext::new_member(user_id.as_bytes())
            .map_err(|e| js_err(e))?;
        let kp_bytes = kpb.key_package()
            .tls_serialize_detached()
            .map_err(|e| js_err(format!("kp serialize: {e:?}")))?;
        Ok(MlsContext { inner: RefCell::new(ctx), kp_bytes })
    }

    /// TLS-serialised key package for this member (pass to the inviter's
    /// [`invite`]).
    #[wasm_bindgen(getter, js_name = "keyPackage")]
    pub fn key_package(&self) -> Uint8Array {
        Uint8Array::from(self.kp_bytes.as_slice())
    }

    /// Current MLS group epoch.
    #[wasm_bindgen(getter)]
    pub fn epoch(&self) -> u64 {
        self.inner.borrow().epoch()
    }

    /// 16-byte group identifier (all zeros before the first invite).
    #[wasm_bindgen(getter, js_name = "groupId")]
    pub fn group_id(&self) -> Uint8Array {
        Uint8Array::from(self.inner.borrow().group_id_16().as_slice())
    }

    /// Invites another member into this group.
    ///
    /// `keyPackageBytes` is the raw TLS bytes from the joiner's
    /// [`keyPackage`] getter. Returns the Welcome bytes the joiner must pass
    /// to [`acceptWelcome`]. This call merges the commit immediately and
    /// advances this member's epoch.
    #[wasm_bindgen(js_name = "invite")]
    pub fn invite(&self, key_package_bytes: &[u8]) -> Result<Uint8Array, JsValue> {
        let mut ctx = self.inner.borrow_mut();
        let kp_in = KeyPackageIn::tls_deserialize(&mut key_package_bytes.as_ref())
            .map_err(|e| js_err(format!("kp parse: {e:?}")))?;
        let kp = kp_in
            .validate(ctx.provider.crypto(), ProtocolVersion::Mls10)
            .map_err(|e| js_err(format!("kp validate: {e:?}")))?;
        let welcome = ctx.invite(&[kp]).map_err(|e| js_err(e))?;
        Ok(Uint8Array::from(welcome.as_slice()))
    }

    /// Invites several members in a SINGLE Add commit. `keyPackages` is an
    /// array of raw TLS KeyPackage byte arrays (each from a joiner's
    /// [`keyPackage`] getter). Returns ONE Welcome that every newly-added
    /// member accepts with their own KeyPackage via [`acceptWelcome`]. Merges
    /// the commit immediately and advances this member's epoch by one
    /// (regardless of how many members were added). Errors if the array is
    /// empty.
    #[wasm_bindgen(js_name = "inviteMany")]
    pub fn invite_many(&self, key_packages: Array) -> Result<Uint8Array, JsValue> {
        let mut ctx = self.inner.borrow_mut();
        let mut kps = Vec::with_capacity(key_packages.length() as usize);
        for v in key_packages.iter() {
            let bytes = Uint8Array::new(&v).to_vec();
            let kp_in = KeyPackageIn::tls_deserialize(&mut bytes.as_slice())
                .map_err(|e| js_err(format!("kp parse: {e:?}")))?;
            let kp = kp_in
                .validate(ctx.provider.crypto(), ProtocolVersion::Mls10)
                .map_err(|e| js_err(format!("kp validate: {e:?}")))?;
            kps.push(kp);
        }
        if kps.is_empty() {
            return Err(js_err("inviteMany: no key packages".to_string()));
        }
        let welcome = ctx.invite(&kps).map_err(|e| js_err(e))?;
        Ok(Uint8Array::from(welcome.as_slice()))
    }

    /// Joins a group from a Welcome message produced by [`invite`].
    ///
    /// After this call [`epoch`] will match the inviter's epoch and
    /// [`groupId`] will match the inviter's group id.
    #[wasm_bindgen(js_name = "acceptWelcome")]
    pub fn accept_welcome(&self, welcome_bytes: &[u8]) -> Result<(), JsValue> {
        self.inner.borrow_mut()
            .accept_welcome(welcome_bytes)
            .map_err(|e| js_err(e))
    }

    /// Invites a member and returns BOTH the Commit and the Welcome as
    /// `{ commit: Uint8Array, welcome: Uint8Array }`. Unlike [`invite`], this
    /// stages a pending commit instead of merging immediately — broadcast the
    /// Commit to existing members, unicast the Welcome to the joiner, then call
    /// [`finalizeCommit`] (or [`clearPendingCommit`] to roll back). This is the
    /// two-phase flow used for coordinated epoch transitions.
    #[wasm_bindgen(js_name = "inviteFull")]
    pub fn invite_full(&self, key_package_bytes: &[u8]) -> Result<JsValue, JsValue> {
        let mut ctx = self.inner.borrow_mut();
        let kp_in = KeyPackageIn::tls_deserialize(&mut key_package_bytes.as_ref())
            .map_err(|e| js_err(format!("kp parse: {e:?}")))?;
        let kp = kp_in
            .validate(ctx.provider.crypto(), ProtocolVersion::Mls10)
            .map_err(|e| js_err(format!("kp validate: {e:?}")))?;
        let (commit, welcome) = ctx.invite_full(&[kp]).map_err(|e| js_err(e))?;
        let obj = Object::new();
        set(&obj, "commit", &u8s(&commit));
        set(&obj, "welcome", &u8s(&welcome));
        Ok(obj.into())
    }

    /// Removes the member at `leafIndex` and returns the Commit to broadcast to
    /// the remaining members. Stages a pending commit; pair with
    /// [`finalizeCommit`] / [`clearPendingCommit`]. This is the membership
    /// change that SFrame keys rotate on — create a new [`SFrameSession`] after
    /// the epoch advances.
    #[wasm_bindgen(js_name = "removeMember")]
    pub fn remove_member(&self, leaf_index: u32) -> Result<Uint8Array, JsValue> {
        let mut ctx = self.inner.borrow_mut();
        let commit = ctx.remove_members(&[leaf_index]).map_err(|e| js_err(e))?;
        Ok(Uint8Array::from(commit.as_slice()))
    }

    /// Applies an inbound MLS message and returns the processed kind as a
    /// string: `"commit"` (epoch advanced), `"application"`, `"proposal"`
    /// (staged) or `"external"`.
    #[wasm_bindgen(js_name = "processMessage")]
    pub fn process_message(&self, msg_bytes: &[u8]) -> Result<String, JsValue> {
        let mut ctx = self.inner.borrow_mut();
        let kind = ctx.process_message(msg_bytes).map_err(|e| js_err(e))?;
        Ok(match kind {
            gbp_mls::ProcessedKind::Commit => "commit",
            gbp_mls::ProcessedKind::Application => "application",
            gbp_mls::ProcessedKind::Proposal => "proposal",
            gbp_mls::ProcessedKind::External => "external",
        }
        .to_string())
    }

    /// Merges a pending Commit produced by [`inviteFull`] / [`removeMember`],
    /// advancing this member's epoch.
    #[wasm_bindgen(js_name = "finalizeCommit")]
    pub fn finalize_commit(&self) -> Result<(), JsValue> {
        self.inner.borrow_mut()
            .finalize_pending_commit()
            .map_err(|e| js_err(e))
    }

    /// Discards a pending Commit without applying it — used on
    /// `ABORT_TRANSITION` to roll back to the pre-commit MLS state.
    #[wasm_bindgen(js_name = "clearPendingCommit")]
    pub fn clear_pending_commit(&self) -> Result<(), JsValue> {
        self.inner.borrow_mut()
            .clear_pending_commit()
            .map_err(|e| js_err(e))
    }

    /// Serialises the full MLS state into an opaque blob that [`restoreState`]
    /// can reconstruct, so a browser client can persist the context (e.g. to
    /// IndexedDB) and survive a reload. The blob contains **private key
    /// material** — store it encrypted at rest.
    #[wasm_bindgen(js_name = "exportState")]
    pub fn export_state(&self) -> Result<Uint8Array, JsValue> {
        let bytes = self.inner.borrow().export_state().map_err(|e| js_err(e))?;
        Ok(Uint8Array::from(bytes.as_slice()))
    }

    /// Reconstructs a context from a blob produced by [`exportState`]. The
    /// restored context is at the same epoch / group state and can immediately
    /// send and receive again.
    #[wasm_bindgen(js_name = "restoreState")]
    pub fn restore_state(blob: &[u8]) -> Result<MlsContext, JsValue> {
        let ctx = gbp_mls::MlsContext::restore_state(blob).map_err(|e| js_err(e))?;
        Ok(MlsContext { inner: RefCell::new(ctx), kp_bytes: Vec::new() })
    }
}

// ─── GroupNode ───────────────────────────────────────────────────────────────

/// GBP group node — framing, AEAD, replay window, control plane.
///
/// JS usage:
/// ```js
/// const node = GroupNode.create(1, groupId);
/// node.bootstrapAsCreator(mls.epoch);
/// const events = node.onWire(mls, wireBytes);
/// ```
#[wasm_bindgen]
pub struct GroupNode {
    inner: RefCell<RustGroupNode>,
}

#[wasm_bindgen]
impl GroupNode {
    /// Creates a node for `leafIndex` (member id) and the given 16-byte group id.
    #[wasm_bindgen(js_name = "create")]
    pub fn create(leaf_index: u32, group_id_bytes: &[u8]) -> GroupNode {
        let gid: [u8; 16] = group_id_bytes.try_into().unwrap_or([0u8; 16]);
        GroupNode { inner: RefCell::new(RustGroupNode::new(leaf_index as MemberId, gid)) }
    }

    /// Drives the node to `ACTIVE` as the group creator at the given epoch.
    #[wasm_bindgen(js_name = "bootstrapAsCreator")]
    pub fn bootstrap_as_creator(&self, epoch: u64) {
        self.inner.borrow_mut().bootstrap_as_creator(epoch);
    }

    /// Drives the node to `ACTIVE` as a joiner.
    ///
    /// Pass `expectedFirstTid = 0` unless you know the in-flight
    /// `transition_id` the coordinator will send in `EXECUTE_TRANSITION`.
    #[wasm_bindgen(js_name = "bootstrapAsJoiner")]
    pub fn bootstrap_as_joiner(&self, epoch: u64, expected_first_tid: u32) {
        self.inner.borrow_mut().bootstrap_as_joiner(epoch, expected_first_tid);
    }

    /// Serialises the outbound sequence counters so a rebuilt node (after a
    /// client restart / re-login that restores the MLS state) resumes sending
    /// above the high-water-marks peers already recorded — otherwise its frames
    /// are dropped as replays. The inbound window is NOT included (the rebuilt
    /// node must re-accept re-fetched history).
    #[wasm_bindgen(js_name = "exportOutSeq")]
    pub fn export_out_seq(&self) -> Uint8Array {
        Uint8Array::from(self.inner.borrow().export_out_seq().as_slice())
    }

    /// Restores outbound counters produced by [`GroupNode::exportOutSeq`].
    #[wasm_bindgen(js_name = "restoreOutSeq")]
    pub fn restore_out_seq(&self, bytes: &[u8]) {
        self.inner.borrow_mut().restore_out_seq(bytes);
    }

    /// Delivers a wire frame and returns the resulting events array.
    ///
    /// Each element is a plain JS object with at minimum `{ kind: string }`.
    ///
    /// | `kind` | Extra fields |
    /// |--------|-------------|
    /// | `"payload_received"` | `streamType`, `plaintext`, `sequenceNo`, `codec` |
    /// | `"state_changed"` | `from`, `to` |
    /// | `"epoch_advanced"` | `epoch` (bigint), `transitionId` |
    /// | `"error"` | `code`, `reason`, `fatal`, `retryable` |
    /// | `"control"` | `from`, `opcode`, `transitionId` |
    #[wasm_bindgen(js_name = "onWire")]
    pub fn on_wire(&self, mls: &MlsContext, wire_bytes: &[u8]) -> Array {
        let mut node = self.inner.borrow_mut();
        let mut mls_inner = mls.inner.borrow_mut();
        let events = node.on_wire(&mut *mls_inner, wire_bytes).unwrap_or_default();
        let arr = Array::new();
        for ev in events {
            arr.push(&event_to_js(ev));
        }
        arr
    }

    /// Polls pending timeout events — call ~every 500 ms from the app loop.
    #[wasm_bindgen(js_name = "checkTimeouts")]
    pub fn check_timeouts(&self) -> Array {
        let arr = Array::new();
        for ev in self.inner.borrow_mut().check_timeouts() {
            arr.push(&event_to_js(ev));
        }
        arr
    }

    /// The `transition_id` of the last applied epoch transition.
    #[wasm_bindgen(getter, js_name = "lastTransitionId")]
    pub fn last_transition_id(&self) -> u32 {
        self.inner.borrow().last_transition_id
    }

    /// Current epoch as seen by the GBP layer.
    #[wasm_bindgen(getter, js_name = "currentEpoch")]
    pub fn current_epoch(&self) -> u64 {
        self.inner.borrow().current_epoch
    }

    /// This node's member id (leaf index).
    #[wasm_bindgen(getter, js_name = "memberId")]
    pub fn member_id(&self) -> u32 {
        self.inner.borrow().member_id
    }

    /// Sends a control-plane message on Stream 0 — epoch-transition coordination
    /// (PREPARE/READY/EXECUTE/ABORT), capabilities advertise, ACK/NACK.
    /// `opcode` is a [`ControlOpcode`] value. Returns `{ wire: Uint8Array, to: number }`
    /// or throws. Pass `target = 0` to broadcast; pass an empty `args` array
    /// when the opcode carries no arguments.
    #[wasm_bindgen(js_name = "sendControl")]
    pub fn send_control(
        &self,
        mls: &MlsContext,
        target: u32,
        opcode: u16,
        transition_id: u32,
        request_id: u32,
        args: &[u8],
    ) -> Result<JsValue, JsValue> {
        let op = gbp_core::ControlOpcode::try_from(opcode)
            .map_err(|_| js_err(format!("bad opcode 0x{opcode:04X}")))?;
        let mut node = self.inner.borrow_mut();
        let mut m = mls.inner.borrow_mut();
        let of = node
            .send_control(&mut *m, target as MemberId, op, transition_id, request_id, args.to_vec())
            .map_err(|e| js_err(e))?;
        let obj = Object::new();
        set(&obj, "wire", &u8s(&of.wire));
        set(&obj, "to", &JsValue::from_f64(of.to as f64));
        Ok(obj.into())
    }

    /// Applies an epoch transition locally (advances `currentEpoch` and
    /// `lastTransitionId`).
    #[wasm_bindgen(js_name = "applyTransition")]
    pub fn apply_transition(&self, tid: u32) {
        self.inner.borrow_mut().apply_transition(tid);
    }

    /// Drains queued events without consuming wire bytes. Each element has the
    /// same shape as the array returned by [`onWire`].
    #[wasm_bindgen(js_name = "drainEvents")]
    pub fn drain_events(&self) -> Array {
        let arr = Array::new();
        for ev in self.inner.borrow_mut().drain_events() {
            arr.push(&event_to_js(ev));
        }
        arr
    }
}

// ─── GtpClient ───────────────────────────────────────────────────────────────

/// Group Text Protocol client — idempotent text delivery over GBP.
///
/// JS usage:
/// ```js
/// const gtp = GtpClient.create();
/// const frame = gtp.send(node, mls, 0, 1n, "hello");
/// // frame.wire: Uint8Array — hand to transport
///
/// // on receive:
/// const result = gtp.accept(ev.plaintext, mls.epoch);
/// // result.text / result.messageId / result.senderId
/// ```
#[wasm_bindgen]
pub struct GtpClient {
    inner: RefCell<RustGtpClient>,
}

#[wasm_bindgen]
impl GtpClient {
    /// Creates an empty GTP client.
    #[wasm_bindgen(js_name = "create")]
    pub fn create() -> GtpClient {
        GtpClient { inner: RefCell::new(RustGtpClient::new()) }
    }

    /// Sends a text message.
    ///
    /// Returns `{ wire: Uint8Array, to: number }` or `null` on error.
    /// Pass `target = 0` to broadcast to all members. `codec` is optional
    /// (a [`PayloadCodec`] value); omit it for the CBOR default.
    #[wasm_bindgen(js_name = "send")]
    pub fn send(
        &self,
        node: &GroupNode,
        mls: &MlsContext,
        target: u32,
        message_id: u64,
        text: &str,
        codec: Option<u8>,
    ) -> JsValue {
        let mut gtp = self.inner.borrow_mut();
        let mut n = node.inner.borrow_mut();
        let mut m = mls.inner.borrow_mut();
        match gtp.send(&mut *n, &mut *m, target as MemberId, message_id, text, codec_from(codec)) {
            Ok(frame) => {
                let obj = Object::new();
                set(&obj, "wire", &u8s(&frame.wire));
                set(&obj, "to", &JsValue::from_f64(frame.to as f64));
                obj.into()
            }
            Err(_) => JsValue::NULL,
        }
    }

    /// Accepts a plaintext GTP payload delivered from a `payload_received` event.
    ///
    /// Returns `{ text: string, messageId: bigint, senderId: number }` or
    /// `null` if the payload is malformed.
    /// The `status` field is `"new"` or `"duplicate"` based on idempotency.
    /// `codec` is optional and must match the encoding used by the sender
    /// (defaults to CBOR).
    #[wasm_bindgen(js_name = "accept")]
    pub fn accept(&self, plaintext: &[u8], epoch: u64, codec: Option<u8>) -> JsValue {
        let mut gtp = self.inner.borrow_mut();
        match gtp.accept(plaintext, epoch, codec_from(codec)) {
            Ok(result) => {
                let (msg, status) = match result {
                    GtpAccept::New(m) => (m, "new"),
                    GtpAccept::Duplicate(m) => (m, "duplicate"),
                };
                let text = String::from_utf8_lossy(&msg.content).into_owned();
                let obj = Object::new();
                set(&obj, "text", &JsValue::from_str(&text));
                set(&obj, "messageId", &js_sys::BigInt::from(msg.message_id).into());
                set(&obj, "senderId", &JsValue::from_f64(msg.sender_id as f64));
                set(&obj, "status", &JsValue::from_str(status));
                obj.into()
            }
            Err(_) => JsValue::NULL,
        }
    }

    /// Resets the idempotency set unconditionally.
    #[wasm_bindgen(js_name = "reset")]
    pub fn reset(&self) {
        self.inner.borrow_mut().reset();
    }
}

// ─── Shared frame helpers ─────────────────────────────────────────────────────

/// `OutboundFrame` → `{ wire: Uint8Array, to: number }`.
fn outbound_to_js(of: gbp_node::OutboundFrame) -> JsValue {
    let obj = Object::new();
    set(&obj, "wire", &u8s(&of.wire));
    set(&obj, "to", &JsValue::from_f64(of.to as f64));
    obj.into()
}

fn gap_payload_to_js(status: &str, p: gap::GapPayload) -> JsValue {
    let obj = Object::new();
    set(&obj, "status", &JsValue::from_str(status));
    set(&obj, "source", &JsValue::from_f64(p.media_source_id as f64));
    set(&obj, "seq", &JsValue::from_f64(p.rtp_sequence as f64));
    set(&obj, "rtpTimestamp", &js_sys::BigInt::from(p.rtp_timestamp).into());
    set(&obj, "opus", &u8s(&p.opus_frame.into_vec()));
    obj.into()
}

fn cipher_suite_from(v: u8) -> Result<RustCipherSuite, JsValue> {
    RustCipherSuite::from_u8(v).ok_or_else(|| js_err(format!("unknown ciphersuite {v}")))
}

// ─── GapClient ─────────────────────────────────────────────────────────────────

/// Group Audio Protocol client — Opus frame delivery with per-source replay
/// protection over GBP. The Opus payload is opaque bytes (encode/decode audio
/// in the app, e.g. WebCodecs). Combine with [`SFrameSession`] for media E2EE.
///
/// JS usage:
/// ```js
/// const gap = GapClient.create();
/// const frame = gap.send(node, mls, 0, mediaSourceId, rtpTimestamp, opusBytes, PayloadCodec.FlatBuffers);
/// // on receive (payload_received event whose streamType is audio):
/// const r = gap.accept(ev.plaintext, mls.epoch);
/// // r.status ("new"|"late"), r.source, r.seq, r.opus (Uint8Array)
/// ```
#[wasm_bindgen]
pub struct GapClient {
    inner: RefCell<RustGapClient>,
}

#[wasm_bindgen]
impl GapClient {
    /// Creates an empty GAP client.
    #[wasm_bindgen(js_name = "create")]
    pub fn create() -> GapClient {
        GapClient { inner: RefCell::new(RustGapClient::new()) }
    }

    /// Sends one Opus audio frame. Returns `{ wire: Uint8Array, to: number }`
    /// or `null` on error. Pass `target = 0` to broadcast. `codec` is optional;
    /// for audio prefer `PayloadCodec.FlatBuffers` for lowest decode latency.
    #[wasm_bindgen(js_name = "send")]
    pub fn send(
        &self,
        node: &GroupNode,
        mls: &MlsContext,
        target: u32,
        media_source_id: u32,
        rtp_timestamp: u64,
        opus: &[u8],
        codec: Option<u8>,
    ) -> JsValue {
        let mut gap = self.inner.borrow_mut();
        let mut n = node.inner.borrow_mut();
        let mut m = mls.inner.borrow_mut();
        match gap.send(
            &mut *n,
            &mut *m,
            target as MemberId,
            media_source_id,
            rtp_timestamp,
            opus.to_vec(),
            codec_from(codec),
        ) {
            Ok(of) => outbound_to_js(of),
            Err(_) => JsValue::NULL,
        }
    }

    /// Accepts a GAP audio payload from a `payload_received` event. Returns
    /// `{ status, source, seq, rtpTimestamp, opus }` where status is `"new"`
    /// or `"late"`, or `null` on a malformed/stale payload. `codec` must match
    /// the sender's encoding (defaults to CBOR).
    #[wasm_bindgen(js_name = "accept")]
    pub fn accept(&self, plaintext: &[u8], epoch: u64, codec: Option<u8>) -> JsValue {
        let mut gap = self.inner.borrow_mut();
        match gap.accept(plaintext, epoch, codec_from(codec)) {
            Ok(GapAccept::New(p)) => gap_payload_to_js("new", p),
            Ok(GapAccept::Late(p)) => gap_payload_to_js("late", p),
            Err(_) => JsValue::NULL,
        }
    }

    /// Clears outbound counters + replay window (use after an epoch change).
    #[wasm_bindgen(js_name = "reset")]
    pub fn reset(&self) {
        self.inner.borrow_mut().reset();
    }
}

// ─── GspClient ─────────────────────────────────────────────────────────────────

/// Group Signaling Protocol client — membership / role / stream / codec control
/// signals over GBP. Drives call membership and mute/stream state.
///
/// JS usage:
/// ```js
/// const gsp = GspClient.create();
/// const f  = gsp.send(node, mls, 0, SignalType.Join, 0, requestId);
/// const f2 = gsp.sendWithArgs(node, mls, 0, SignalType.Mute, 0, requestId, argsBytes);
/// const r  = gsp.accept(ev.plaintext, mls.epoch);
/// // r.status, r.signal, r.signalCode, r.sender, r.roleClaim, r.requestId
/// ```
#[wasm_bindgen]
pub struct GspClient {
    inner: RefCell<RustGspClient>,
}

#[wasm_bindgen]
impl GspClient {
    /// Creates an empty GSP client.
    #[wasm_bindgen(js_name = "create")]
    pub fn create() -> GspClient {
        GspClient { inner: RefCell::new(RustGspClient::new()) }
    }

    /// Sends a bare signal with no arguments (e.g. `SignalType.Join` /
    /// `SignalType.Leave`). Returns `{ wire, to }` or throws. `target = 0`
    /// broadcasts.
    #[wasm_bindgen(js_name = "send")]
    pub fn send(
        &self,
        node: &GroupNode,
        mls: &MlsContext,
        target: u32,
        signal_type: u32,
        role_claim: u32,
        request_id: u32,
        codec: Option<u8>,
    ) -> Result<JsValue, JsValue> {
        let sig = gbp_core::SignalType::try_from(signal_type)
            .map_err(|_| js_err(format!("bad signal {signal_type}")))?;
        let mut gsp = self.inner.borrow_mut();
        let mut n = node.inner.borrow_mut();
        let mut m = mls.inner.borrow_mut();
        gsp.send(&mut *n, &mut *m, target as MemberId, sig, role_claim, request_id, codec_from(codec))
            .map(outbound_to_js)
            .map_err(|e| js_err(e))
    }

    /// Sends a signal carrying opcode-specific CBOR `args` — MUTE, UNMUTE,
    /// ROLE_CHANGE, STREAM_START, STREAM_STOP, CODEC_UPDATE.
    #[wasm_bindgen(js_name = "sendWithArgs")]
    pub fn send_with_args(
        &self,
        node: &GroupNode,
        mls: &MlsContext,
        target: u32,
        signal_type: u32,
        role_claim: u32,
        request_id: u32,
        args: &[u8],
        codec: Option<u8>,
    ) -> Result<JsValue, JsValue> {
        let sig = gbp_core::SignalType::try_from(signal_type)
            .map_err(|_| js_err(format!("bad signal {signal_type}")))?;
        let mut gsp = self.inner.borrow_mut();
        let mut n = node.inner.borrow_mut();
        let mut m = mls.inner.borrow_mut();
        gsp.send_with_args(&mut *n, &mut *m, target as MemberId, sig, role_claim, request_id, args, codec_from(codec))
            .map(outbound_to_js)
            .map_err(|e| js_err(e))
    }

    /// Accepts a GSP signal payload. Returns
    /// `{ status, signal, signalCode, sender, roleClaim, requestId }` for a new
    /// signal, `{ status: "duplicate", requestId }` for a replayed request, or
    /// throws on a hard error. `codec` must match the sender (defaults to CBOR).
    #[wasm_bindgen(js_name = "accept")]
    pub fn accept(&self, plaintext: &[u8], epoch: u64, codec: Option<u8>) -> Result<JsValue, JsValue> {
        let mut gsp = self.inner.borrow_mut();
        match gsp.accept(plaintext, epoch, codec_from(codec)) {
            Ok(GspAccept { signal, sender_id, role_claim, request_id }) => {
                let obj = Object::new();
                set(&obj, "status", &JsValue::from_str("new"));
                set(&obj, "signal", &JsValue::from_str(signal.name()));
                set(&obj, "signalCode", &JsValue::from_f64(signal as u32 as f64));
                set(&obj, "sender", &JsValue::from_f64(sender_id as f64));
                set(&obj, "roleClaim", &JsValue::from_f64(role_claim as f64));
                set(&obj, "requestId", &JsValue::from_f64(request_id as f64));
                Ok(obj.into())
            }
            Err(GspError::DuplicateRequest(rid)) => {
                let obj = Object::new();
                set(&obj, "status", &JsValue::from_str("duplicate"));
                set(&obj, "requestId", &JsValue::from_f64(rid as f64));
                Ok(obj.into())
            }
            Err(e) => Err(js_err(e)),
        }
    }

    /// Clears dedup state (use after an epoch change).
    #[wasm_bindgen(js_name = "reset")]
    pub fn reset(&self) {
        self.inner.borrow_mut().reset();
    }
}

// ─── SFrame (media E2EE) ──────────────────────────────────────────────────────

/// SFrame E2EE session for one MLS epoch — wraps the receiver-side decryptor.
/// Derive a fresh session after every epoch change (invite/remove/commit), as
/// the base key rotates with the MLS exporter secret. Create per-sender
/// encryptors via [`createEncryptor`].
///
/// JS usage:
/// ```js
/// const session = SFrameSession.create(mls, "gbp/sframe v1", CipherSuite.Aes128Gcm);
/// const enc = session.createEncryptor(mls, myLeafIndex, "gbp/sframe v1", CipherSuite.Aes128Gcm);
/// const ct  = enc.encrypt(opusBytes, new Uint8Array());     // wrap before gap.send
/// const { plaintext, senderLeaf } = session.decrypt(ct, new Uint8Array());
/// ```
#[wasm_bindgen]
pub struct SFrameSession {
    inner: RefCell<RustSFrameDecryptor>,
}

#[wasm_bindgen]
impl SFrameSession {
    /// Derives an SFrame session from the current MLS group state via
    /// `MLS.ExportSecret(label, epoch, 32)`. `suite` is a [`CipherSuite`] value
    /// (0 = AES-128-GCM, 1 = AES-256-GCM). `label` must match across the group.
    #[wasm_bindgen(js_name = "create")]
    pub fn create(mls: &MlsContext, label: &str, suite: u8) -> Result<SFrameSession, JsValue> {
        let suite = cipher_suite_from(suite)?;
        let m = mls.inner.borrow();
        let session = RustSFrameSession::from_mls(&m, label, suite).map_err(|e| js_err(e))?;
        Ok(SFrameSession { inner: RefCell::new(session.decryptor()) })
    }

    /// Creates a sender-side encryptor for `leafIndex` in this epoch. Re-derives
    /// the session from MLS (same `label` + `suite`) so encryptor and decryptor
    /// share the epoch base key.
    #[wasm_bindgen(js_name = "createEncryptor")]
    pub fn create_encryptor(
        &self,
        mls: &MlsContext,
        leaf_index: u32,
        label: &str,
        suite: u8,
    ) -> Result<SFrameEncryptor, JsValue> {
        let suite = cipher_suite_from(suite)?;
        let m = mls.inner.borrow();
        let session = RustSFrameSession::from_mls(&m, label, suite).map_err(|e| js_err(e))?;
        Ok(SFrameEncryptor { inner: RefCell::new(session.encryptor(leaf_index)) })
    }

    /// Decrypts an SFrame payload, returning
    /// `{ plaintext: Uint8Array, senderLeaf: number }` or throwing on failure.
    /// `aad` must equal the sender's `extra_aad` (pass an empty array if none).
    #[wasm_bindgen(js_name = "decrypt")]
    pub fn decrypt(&self, payload: &[u8], aad: &[u8]) -> Result<JsValue, JsValue> {
        let mut dec = self.inner.borrow_mut();
        match dec.decrypt(payload, aad) {
            Ok((plaintext, leaf)) => {
                let obj = Object::new();
                set(&obj, "plaintext", &u8s(&plaintext));
                set(&obj, "senderLeaf", &JsValue::from_f64(leaf as f64));
                Ok(obj.into())
            }
            Err(e) => Err(js_err(e)),
        }
    }
}

/// Sender-side SFrame encryptor (one per sender per epoch). Created via
/// [`SFrameSession::createEncryptor`]; maintains an internal frame counter.
#[wasm_bindgen]
pub struct SFrameEncryptor {
    inner: RefCell<RustSFrameEncryptor>,
}

#[wasm_bindgen]
impl SFrameEncryptor {
    /// Encrypts one frame, returning `sframe_header ‖ ciphertext ‖ tag`.
    /// `aad` is additional authenticated data (e.g. an RTP header); pass an
    /// empty array if none.
    #[wasm_bindgen(js_name = "encrypt")]
    pub fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<Uint8Array, JsValue> {
        let mut enc = self.inner.borrow_mut();
        let ct = enc.encrypt(plaintext, aad).map_err(|e| js_err(e))?;
        Ok(Uint8Array::from(ct.as_slice()))
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
