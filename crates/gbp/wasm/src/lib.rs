//! Browser/WASM bindings for the Group Protocol Stack.
//!
//! Exported classes: [`MlsContext`], [`GroupNode`], [`GtpClient`].
//! The default wasm-pack init function is automatically generated.

#![cfg(target_arch = "wasm32")]

use gbp_core::{MemberId, PayloadCodec, StreamType};
use gbp_node::{Event, GroupNode as RustGroupNode, Sealer};
use gtp::{GtpAccept, GtpClient as RustGtpClient};
use js_sys::{Array, Object, Reflect, Uint8Array};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

// ─── helpers ────────────────────────────────────────────────────────────────

fn set(obj: &Object, key: &str, val: &JsValue) {
    Reflect::set(obj, &JsValue::from_str(key), val).unwrap_throw();
}

fn u8s(bytes: &[u8]) -> JsValue {
    Uint8Array::from(bytes).into()
}

fn event_to_js(ev: Event) -> JsValue {
    let obj = Object::new();
    match ev {
        Event::PayloadReceived(p) => {
            set(&obj, "kind", &"payload_received".into());
            set(&obj, "streamType", &JsValue::from_f64(p.stream_type.as_u8() as f64));
            set(&obj, "plaintext", &u8s(&p.plaintext));
            set(&obj, "sequenceNo", &JsValue::from_f64(p.sequence_no as f64));
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

// ─── MlsContext ──────────────────────────────────────────────────────────────

/// MLS group state for one member.
///
/// JS usage:
/// ```js
/// const mls = MlsContext.create("alice");
/// console.log(mls.epoch); // bigint
/// ```
#[wasm_bindgen]
pub struct MlsContext {
    inner: RefCell<gbp_mls::MlsContext>,
}

#[wasm_bindgen]
impl MlsContext {
    /// Creates a new member identity and an empty group.
    #[wasm_bindgen(js_name = "create", static_method_of = MlsContext)]
    pub fn create(user_id: &str) -> MlsContext {
        let (ctx, _kpb) = gbp_mls::MlsContext::new_member(user_id.as_bytes())
            .expect_throw("MlsContext::new_member failed");
        MlsContext { inner: RefCell::new(ctx) }
    }

    /// Current MLS group epoch.
    #[wasm_bindgen(getter)]
    pub fn epoch(&self) -> u64 {
        self.inner.borrow().epoch()
    }
}

// ─── GroupNode ───────────────────────────────────────────────────────────────

/// GBP group node — framing, AEAD, replay window, control plane.
///
/// JS usage:
/// ```js
/// const gid = new Uint8Array(16); // 16-byte group id
/// const node = GroupNode.create(1, gid);
/// node.bootstrapAsCreator(mls.epoch);
/// const events = node.onWire(mls, wireBytes);
/// ```
#[wasm_bindgen]
pub struct GroupNode {
    inner: RefCell<RustGroupNode>,
}

#[wasm_bindgen]
impl GroupNode {
    /// Creates a node for `leaf_index` (member id) and the given 16-byte group id.
    #[wasm_bindgen(js_name = "create", static_method_of = GroupNode)]
    pub fn create(leaf_index: u32, group_id_bytes: &[u8]) -> GroupNode {
        let gid: [u8; 16] = group_id_bytes.try_into().unwrap_or([0u8; 16]);
        let node = RustGroupNode::new(leaf_index as MemberId, gid);
        GroupNode { inner: RefCell::new(node) }
    }

    /// Drives the node to `ACTIVE` as the group creator at the given epoch.
    #[wasm_bindgen(js_name = "bootstrapAsCreator")]
    pub fn bootstrap_as_creator(&self, epoch: u64) {
        self.inner.borrow_mut().bootstrap_as_creator(epoch);
    }

    /// Drives the node to `ACTIVE` as a joiner.
    #[wasm_bindgen(js_name = "bootstrapAsJoiner")]
    pub fn bootstrap_as_joiner(&self, epoch: u64, expected_first_tid: u32) {
        self.inner.borrow_mut().bootstrap_as_joiner(epoch, expected_first_tid);
    }

    /// Delivers a wire frame and returns the resulting events array.
    ///
    /// Each element is a plain JS object with at least `{ kind: string }`.
    /// For `kind === "payload_received"`: `{ streamType, plaintext, sequenceNo }`.
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

    /// Polls pending timeout events (call ~every 500 ms from the app loop).
    #[wasm_bindgen(js_name = "checkTimeouts")]
    pub fn check_timeouts(&self) -> Array {
        let events = self.inner.borrow_mut().check_timeouts();
        let arr = Array::new();
        for ev in events {
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
}

// ─── GtpClient ───────────────────────────────────────────────────────────────

/// Group Text Protocol client — idempotent text delivery over GBP.
///
/// JS usage:
/// ```js
/// const gtp = GtpClient.create();
/// const frame = gtp.send(node, mls, 0, 1n, "hello");
/// // frame.wire: Uint8Array  — hand to transport
///
/// // on receive:
/// const result = gtp.accept(plaintext, mls.epoch);
/// // result.text: string | null
/// ```
#[wasm_bindgen]
pub struct GtpClient {
    inner: RefCell<RustGtpClient>,
}

#[wasm_bindgen]
impl GtpClient {
    /// Creates an empty GTP client.
    #[wasm_bindgen(js_name = "create", static_method_of = GtpClient)]
    pub fn create() -> GtpClient {
        GtpClient { inner: RefCell::new(RustGtpClient::new()) }
    }

    /// Sends a text message.
    ///
    /// Returns `{ wire: Uint8Array, to: number }` or `null` on error.
    /// Pass `target = 0` to address all members (broadcast).
    #[wasm_bindgen(js_name = "send")]
    pub fn send(
        &self,
        node: &GroupNode,
        mls: &MlsContext,
        target: u32,
        message_id: u64,
        text: &str,
    ) -> JsValue {
        let mut gtp = self.inner.borrow_mut();
        let mut n = node.inner.borrow_mut();
        let mut m = mls.inner.borrow_mut();
        match gtp.send(&mut *n, &mut *m, target as MemberId, message_id, text, PayloadCodec::Cbor) {
            Ok(frame) => {
                let obj = Object::new();
                set(&obj, "wire", &u8s(&frame.wire));
                set(&obj, "to", &JsValue::from_f64(frame.to as f64));
                obj.into()
            }
            Err(_) => JsValue::NULL,
        }
    }

    /// Accepts a plaintext GTP payload delivered by the GBP layer.
    ///
    /// Returns `{ text: string, messageId: bigint, senderId: number }` or `null` on error.
    #[wasm_bindgen(js_name = "accept")]
    pub fn accept(&self, plaintext: &[u8], epoch: u64) -> JsValue {
        let mut gtp = self.inner.borrow_mut();
        match gtp.accept(plaintext, epoch, PayloadCodec::Cbor) {
            Ok(result) => {
                let msg = match result {
                    GtpAccept::New(m) | GtpAccept::Duplicate(m) => m,
                };
                let text = String::from_utf8_lossy(&msg.content).into_owned();
                let obj = Object::new();
                set(&obj, "text", &JsValue::from_str(&text));
                set(&obj, "messageId", &js_sys::BigInt::from(msg.message_id).into());
                set(&obj, "senderId", &JsValue::from_f64(msg.sender_id as f64));
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
