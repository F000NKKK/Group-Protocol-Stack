//! C ABI surface for the Group Protocol Stack.
//!
//! Designed for consumption from `.NET` (or any FFI-capable runtime) via
//! P/Invoke. The C ABI is grouped into the following families:
//!
//! * **GBP** (`gbp_node_*`) — the IP-like base layer: framing, AEAD, replay
//!   window, control plane.
//! * **GTP** (`gtp_client_*`) — text sub-protocol.
//! * **GAP** (`gap_client_*`) — audio sub-protocol.
//! * **GSP** (`gsp_client_*`) — signalling sub-protocol.
//! * **MLS** (`gbp_mls_*`) — RFC 9420 context.
//!
//! Conventions:
//!
//! * **Handle-based** — every long-lived object lives in a Rust-side
//!   registry keyed by an `i32` handle.
//! * **GbpBuffer** — binary blobs are returned as `(ptr, len, cap)` triples
//!   and MUST be released with `gbp_buffer_free`.
//! * **Owned C-string** — text values are returned as owned `*mut c_char`
//!   and MUST be released with `gbp_string_free`.
//! * **Last error** — every fallible call writes to a thread-local error
//!   slot that callers can read via `gbp_last_error`.

#![allow(unsafe_op_in_unsafe_fn)]

use gbp_stack::core::{ControlOpcode, NodeState, SignalType, StreamType};
use gbp_stack::{
    DeliveredPayload, ErrorObject, Event, GapAccept, GapClient, GbpFrame, GroupNode, GspAccept,
    GspClient, GtpAccept, GtpClient, MlsContext, OutboundFrame, ProcessedKind, StreamLabel,
};
use openmls::prelude::tls_codec::Serialize as _;
use openmls::prelude::*;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CString, c_char};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};

// ============================================================================
// Buffer / string types (FFI memory protocol)
// ============================================================================

/// Binary buffer produced by Rust. The caller MUST release it via
/// [`gbp_buffer_free`].
#[repr(C)]
pub struct GbpBuffer {
    /// Pointer to the bytes (may be null when `len == 0`).
    pub ptr: *mut u8,
    /// Current length in bytes.
    pub len: usize,
    /// Capacity used when reconstructing the underlying `Vec` on free.
    pub cap: usize,
}

impl GbpBuffer {
    fn empty() -> Self {
        Self { ptr: std::ptr::null_mut(), len: 0, cap: 0 }
    }
    fn from_vec(mut v: Vec<u8>) -> Self {
        let ptr = v.as_mut_ptr();
        let len = v.len();
        let cap = v.capacity();
        std::mem::forget(v);
        Self { ptr, len, cap }
    }
}

/// Releases a [`GbpBuffer`].
///
/// # Safety
/// `buf` MUST have been returned by one of the `gbp_*` FFI functions and
/// MUST NOT have been freed already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_buffer_free(buf: GbpBuffer) {
    if buf.ptr.is_null() {
        return;
    }
    unsafe {
        let _ = Vec::from_raw_parts(buf.ptr, buf.len, buf.cap);
    }
}

/// Releases a string previously returned by an FFI function.
///
/// # Safety
/// `ptr` MUST have been returned by one of the `gbp_*` FFI functions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

fn alloc_cstring(s: &str) -> *mut c_char {
    CString::new(s.as_bytes())
        .unwrap_or_else(|_| CString::new(s.replace('\0', "?")).unwrap())
        .into_raw()
}

// ============================================================================
// Last-error machinery
// ============================================================================

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_last_error(e: impl ToString) {
    LAST_ERROR.with(|s| *s.borrow_mut() = e.to_string());
}

fn clear_last_error() {
    LAST_ERROR.with(|s| s.borrow_mut().clear());
}

/// Returns the text of the last error, or an empty string if none.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_last_error() -> *mut c_char {
    LAST_ERROR.with(|s| alloc_cstring(&s.borrow()))
}

// ============================================================================
// Handle registries
// ============================================================================

macro_rules! registry {
    ($vis:vis $name:ident<$t:ty>) => {
        $vis struct $name {
            next: AtomicI32,
            map: Mutex<HashMap<i32, Box<$t>>>,
        }
        impl $name {
            fn new() -> Self {
                Self { next: AtomicI32::new(1), map: Mutex::new(HashMap::new()) }
            }
            fn insert(&self, v: $t) -> i32 {
                let id = self.next.fetch_add(1, Ordering::Relaxed);
                self.map.lock().unwrap().insert(id, Box::new(v));
                id
            }
            fn remove(&self, id: i32) {
                self.map.lock().unwrap().remove(&id);
            }
        }
    };
}

registry!(MlsRegistry<MlsContext>);
registry!(NodeRegistry<GroupNode>);
registry!(GtpRegistry<GtpClient>);
registry!(GapRegistry<GapClient>);
registry!(GspRegistry<GspClient>);

struct MlsBundles {
    map: Mutex<HashMap<i32, KeyPackageBundle>>,
}
impl MlsBundles {
    fn new() -> Self {
        Self { map: Mutex::new(HashMap::new()) }
    }
}

fn mls() -> &'static MlsRegistry {
    use std::sync::OnceLock;
    static R: OnceLock<MlsRegistry> = OnceLock::new();
    R.get_or_init(MlsRegistry::new)
}
fn mls_bundles() -> &'static MlsBundles {
    use std::sync::OnceLock;
    static R: OnceLock<MlsBundles> = OnceLock::new();
    R.get_or_init(MlsBundles::new)
}
fn nodes() -> &'static NodeRegistry {
    use std::sync::OnceLock;
    static R: OnceLock<NodeRegistry> = OnceLock::new();
    R.get_or_init(NodeRegistry::new)
}
fn gtps() -> &'static GtpRegistry {
    use std::sync::OnceLock;
    static R: OnceLock<GtpRegistry> = OnceLock::new();
    R.get_or_init(GtpRegistry::new)
}
fn gaps() -> &'static GapRegistry {
    use std::sync::OnceLock;
    static R: OnceLock<GapRegistry> = OnceLock::new();
    R.get_or_init(GapRegistry::new)
}
fn gsps() -> &'static GspRegistry {
    use std::sync::OnceLock;
    static R: OnceLock<GspRegistry> = OnceLock::new();
    R.get_or_init(GspRegistry::new)
}

// ============================================================================
// Version
// ============================================================================

/// Returns the FFI library version with a short summary of the layers.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_version() -> *mut c_char {
    alloc_cstring(&format!(
        "group-protocol-stack {} (gbp + gtp + gap + gsp)",
        env!("CARGO_PKG_VERSION")
    ))
}

// ============================================================================
// MLS API
// ============================================================================

/// Creates a new MLS context. Returns the new handle, or `0` on failure.
///
/// # Safety
/// `identity_ptr` MUST be valid for `identity_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_mls_create(identity_ptr: *const u8, identity_len: usize) -> i32 {
    clear_last_error();
    let ident = unsafe { std::slice::from_raw_parts(identity_ptr, identity_len) };
    match MlsContext::new_member(ident) {
        Ok((ctx, kp)) => {
            let id = mls().insert(ctx);
            mls_bundles().map.lock().unwrap().insert(id, kp);
            id
        }
        Err(e) => {
            set_last_error(e);
            0
        }
    }
}

/// Destroys an MLS context.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_mls_destroy(h: i32) {
    mls().remove(h);
    mls_bundles().map.lock().unwrap().remove(&h);
}

/// Returns the current epoch of the MLS context, or `0` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_mls_epoch(h: i32) -> u64 {
    let map = mls().map.lock().unwrap();
    map.get(&h).map(|c| c.epoch()).unwrap_or(0)
}

/// Writes the 16-byte group identifier into `out16`.
///
/// # Safety
/// `out16` MUST be valid for 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_mls_group_id(h: i32, out16: *mut u8) -> bool {
    clear_last_error();
    let map = mls().map.lock().unwrap();
    let Some(ctx) = map.get(&h) else {
        set_last_error("invalid MLS handle");
        return false;
    };
    let gid = ctx.group_id_16();
    unsafe { std::ptr::copy_nonoverlapping(gid.as_ptr(), out16, 16) };
    true
}

/// Exports the TLS-serialised KeyPackage that can be used to invite this
/// member into someone else's group.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_mls_export_key_package(h: i32) -> GbpBuffer {
    clear_last_error();
    let bundles = mls_bundles().map.lock().unwrap();
    let Some(b) = bundles.get(&h) else {
        set_last_error("invalid MLS handle");
        return GbpBuffer::empty();
    };
    match b.key_package().tls_serialize_detached() {
        Ok(b) => GbpBuffer::from_vec(b),
        Err(e) => {
            set_last_error(format!("kp serialize: {e:?}"));
            GbpBuffer::empty()
        }
    }
}

/// Invites the given KeyPackage into the local group. Returns the
/// TLS-serialised Welcome bytes that the invitee must consume with
/// [`gbp_mls_accept_welcome`].
///
/// # Safety
/// `kp_ptr` MUST be valid for `kp_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_mls_invite(h: i32, kp_ptr: *const u8, kp_len: usize) -> GbpBuffer {
    clear_last_error();
    let bytes = unsafe { std::slice::from_raw_parts(kp_ptr, kp_len) };
    let mut map = mls().map.lock().unwrap();
    let Some(ctx) = map.get_mut(&h) else {
        set_last_error("invalid MLS handle");
        return GbpBuffer::empty();
    };
    let kp_in = match KeyPackageIn::tls_deserialize_exact_bytes(bytes) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("kp parse: {e:?}"));
            return GbpBuffer::empty();
        }
    };
    let validated = match kp_in.validate(ctx.provider.crypto(), ProtocolVersion::Mls10) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("kp validate: {e:?}"));
            return GbpBuffer::empty();
        }
    };
    match ctx.invite(&[validated]) {
        Ok(welcome) => GbpBuffer::from_vec(welcome),
        Err(e) => {
            set_last_error(e);
            GbpBuffer::empty()
        }
    }
}

/// Invites the given KeyPackage and returns BOTH the MLS Commit (which the
/// caller MUST broadcast to existing members so they can advance their MLS
/// epoch) and the Welcome (which the caller MUST unicast to the new joiner).
///
/// Buffer layout: `[u32-LE commit_len | commit_bytes | welcome_bytes]`. The
/// total length minus 4 minus `commit_len` is the welcome length.
///
/// # Safety
/// `kp_ptr` MUST be valid for `kp_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_mls_invite_full(
    h: i32,
    kp_ptr: *const u8,
    kp_len: usize,
) -> GbpBuffer {
    clear_last_error();
    let bytes = unsafe { std::slice::from_raw_parts(kp_ptr, kp_len) };
    let mut map = mls().map.lock().unwrap();
    let Some(ctx) = map.get_mut(&h) else {
        set_last_error("invalid MLS handle");
        return GbpBuffer::empty();
    };
    let kp_in = match KeyPackageIn::tls_deserialize_exact_bytes(bytes) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("kp parse: {e:?}"));
            return GbpBuffer::empty();
        }
    };
    let validated = match kp_in.validate(ctx.provider.crypto(), ProtocolVersion::Mls10) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("kp validate: {e:?}"));
            return GbpBuffer::empty();
        }
    };
    match ctx.invite_full(&[validated]) {
        Ok((commit, welcome)) => {
            let mut out = Vec::with_capacity(4 + commit.len() + welcome.len());
            out.extend_from_slice(&(commit.len() as u32).to_le_bytes());
            out.extend_from_slice(&commit);
            out.extend_from_slice(&welcome);
            GbpBuffer::from_vec(out)
        }
        Err(e) => {
            set_last_error(e);
            GbpBuffer::empty()
        }
    }
}

/// Removes the member at the given MLS LeafIndex and returns the
/// TLS-serialised Commit. Caller MUST broadcast the Commit to remaining
/// members so they advance their MLS epoch.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_mls_remove(h: i32, leaf_index: u32) -> GbpBuffer {
    clear_last_error();
    let mut map = mls().map.lock().unwrap();
    let Some(ctx) = map.get_mut(&h) else {
        set_last_error("invalid MLS handle");
        return GbpBuffer::empty();
    };
    match ctx.remove_members(&[leaf_index]) {
        Ok(commit) => GbpBuffer::from_vec(commit),
        Err(e) => {
            set_last_error(e);
            GbpBuffer::empty()
        }
    }
}

/// Applies a Commit (or staged Proposal) message to the local MLS group.
/// Returns:
///   1 — Commit applied (epoch advanced)
///   2 — Application message (no-op for GBP)
///   3 — Proposal staged
///   4 — External message (no group state change)
///   0 — failure (see `gbp_last_error`).
///
/// # Safety
/// `msg_ptr` MUST be valid for `msg_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_mls_process_message(
    h: i32,
    msg_ptr: *const u8,
    msg_len: usize,
) -> u32 {
    clear_last_error();
    let bytes = unsafe { std::slice::from_raw_parts(msg_ptr, msg_len) };
    let mut map = mls().map.lock().unwrap();
    let Some(ctx) = map.get_mut(&h) else {
        set_last_error("invalid MLS handle");
        return 0;
    };
    match ctx.process_message(bytes) {
        Ok(ProcessedKind::Commit) => 1,
        Ok(ProcessedKind::Application) => 2,
        Ok(ProcessedKind::Proposal) => 3,
        Ok(ProcessedKind::External) => 4,
        Err(e) => {
            set_last_error(e);
            0
        }
    }
}

/// Replaces the local group with the one described by the given Welcome.
///
/// # Safety
/// `welcome_ptr` MUST be valid for `welcome_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_mls_accept_welcome(
    h: i32,
    welcome_ptr: *const u8,
    welcome_len: usize,
) -> bool {
    clear_last_error();
    let bytes = unsafe { std::slice::from_raw_parts(welcome_ptr, welcome_len) };
    let mut map = mls().map.lock().unwrap();
    let Some(ctx) = map.get_mut(&h) else {
        set_last_error("invalid MLS handle");
        return false;
    };
    match ctx.accept_welcome(bytes) {
        Ok(()) => true,
        Err(e) => {
            set_last_error(e);
            false
        }
    }
}

// ============================================================================
// GBP node API (the IP-like base layer)
// ============================================================================

/// Creates a new GBP node and returns its handle.
///
/// # Safety
/// `group_id_16` MUST be valid for 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_node_create(member_id: u32, group_id_16: *const u8) -> i32 {
    clear_last_error();
    let mut gid = [0u8; 16];
    unsafe { std::ptr::copy_nonoverlapping(group_id_16, gid.as_mut_ptr(), 16) };
    nodes().insert(GroupNode::new(member_id, gid))
}

/// Destroys a GBP node.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_destroy(h: i32) {
    nodes().remove(h);
}

/// Drives the node to `ACTIVE` as a creator.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_bootstrap_creator(h: i32, epoch: u64) -> bool {
    let mut map = nodes().map.lock().unwrap();
    let Some(n) = map.get_mut(&h) else { return false };
    n.bootstrap_as_creator(epoch);
    true
}

/// Drives the node to `ACTIVE` as a joiner.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_bootstrap_joiner(h: i32, epoch: u64) -> bool {
    let mut map = nodes().map.lock().unwrap();
    let Some(n) = map.get_mut(&h) else { return false };
    n.bootstrap_as_joiner(epoch);
    true
}

/// Returns the current `NodeState` encoded as `u32`.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_state(h: i32) -> u32 {
    nodes()
        .map
        .lock()
        .unwrap()
        .get(&h)
        .map(|n| n.state as u32)
        .unwrap_or(u32::MAX)
}

/// Returns the node's current epoch.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_epoch(h: i32) -> u64 {
    nodes().map.lock().unwrap().get(&h).map(|n| n.current_epoch).unwrap_or(0)
}

/// Returns the node's last applied `transition_id`.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_last_transition_id(h: i32) -> u32 {
    nodes()
        .map
        .lock()
        .unwrap()
        .get(&h)
        .map(|n| n.last_transition_id)
        .unwrap_or(0)
}

/// Forcibly sets the node's `current_epoch` (intended for tests of late
/// peers and `EPOCH_MISMATCH` recovery).
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_set_epoch(h: i32, epoch: u64) -> bool {
    let mut map = nodes().map.lock().unwrap();
    let Some(n) = map.get_mut(&h) else { return false };
    n.current_epoch = epoch;
    true
}

/// Applies an epoch transition locally.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_apply_transition(h: i32, tid: u32) -> bool {
    let mut map = nodes().map.lock().unwrap();
    let Some(n) = map.get_mut(&h) else { return false };
    n.apply_transition(tid);
    true
}

/// Sends a control plane message on Stream 0.
///
/// The returned buffer layout is `[u32-LE target | wire]`.
///
/// # Safety
/// `args_ptr` MUST be valid for `args_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_node_send_control(
    nh: i32,
    mh: i32,
    target: u32,
    opcode: u16,
    transition_id: u32,
    request_id: u32,
    args_ptr: *const u8,
    args_len: usize,
) -> GbpBuffer {
    clear_last_error();
    let op = match ControlOpcode::try_from(opcode) {
        Ok(o) => o,
        Err(_) => {
            set_last_error(format!("bad opcode 0x{opcode:04X}"));
            return GbpBuffer::empty();
        }
    };
    let args = if args_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(args_ptr, args_len) }.to_vec()
    };
    let mut nmap = nodes().map.lock().unwrap();
    let mut mmap = mls().map.lock().unwrap();
    let Some(n) = nmap.get_mut(&nh) else {
        set_last_error("bad node");
        return GbpBuffer::empty();
    };
    let Some(m) = mmap.get_mut(&mh) else {
        set_last_error("bad mls");
        return GbpBuffer::empty();
    };
    match n.send_control(&mut **m, target, op, transition_id, request_id, args) {
        Ok(of) => outbound_to_buffer(of),
        Err(e) => {
            set_last_error(e.to_string());
            GbpBuffer::empty()
        }
    }
}

/// Feeds wire bytes to the node. Returns a JSON-encoded array of events.
///
/// # Safety
/// `wire_ptr` MUST be valid for `wire_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_node_on_wire(
    nh: i32,
    mh: i32,
    wire_ptr: *const u8,
    wire_len: usize,
) -> *mut c_char {
    clear_last_error();
    let wire = unsafe { std::slice::from_raw_parts(wire_ptr, wire_len) };
    let mut nmap = nodes().map.lock().unwrap();
    let mut mmap = mls().map.lock().unwrap();
    let (Some(n), Some(m)) = (nmap.get_mut(&nh), mmap.get_mut(&mh)) else {
        set_last_error("bad node/mls handle");
        return alloc_cstring("[]");
    };
    let events = match n.on_wire(&mut **m, wire) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(e.to_string());
            return alloc_cstring("[]");
        }
    };
    alloc_cstring(&events_to_json(&events))
}

/// Drains the queued events (without consuming any wire bytes).
#[unsafe(no_mangle)]
pub extern "C" fn gbp_node_drain_events(nh: i32) -> *mut c_char {
    let mut map = nodes().map.lock().unwrap();
    let Some(n) = map.get_mut(&nh) else {
        return alloc_cstring("[]");
    };
    alloc_cstring(&events_to_json(&n.drain_events()))
}

fn outbound_to_buffer(of: OutboundFrame) -> GbpBuffer {
    let mut out = Vec::with_capacity(4 + of.wire.len());
    out.extend_from_slice(&of.to.to_le_bytes());
    out.extend_from_slice(&of.wire);
    GbpBuffer::from_vec(out)
}

// ============================================================================
// GTP client API
// ============================================================================

/// Creates a stateful GTP client (idempotency tracking).
#[unsafe(no_mangle)]
pub extern "C" fn gtp_client_create() -> i32 {
    gtps().insert(GtpClient::new())
}

/// Destroys a GTP client.
#[unsafe(no_mangle)]
pub extern "C" fn gtp_client_destroy(h: i32) {
    gtps().remove(h);
}

/// Clears the client state. Intended for use after an epoch change.
#[unsafe(no_mangle)]
pub extern "C" fn gtp_client_reset(h: i32) {
    if let Some(c) = gtps().map.lock().unwrap().get_mut(&h) {
        c.reset();
    }
}

/// Sends a text message via GTP.
///
/// # Safety
/// `text_ptr` MUST be valid UTF-8 for `text_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtp_client_send(
    ch: i32,
    nh: i32,
    mh: i32,
    target: u32,
    message_id: u64,
    text_ptr: *const u8,
    text_len: usize,
) -> GbpBuffer {
    clear_last_error();
    let text = unsafe { std::slice::from_raw_parts(text_ptr, text_len) };
    let text = match std::str::from_utf8(text) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("utf8: {e}"));
            return GbpBuffer::empty();
        }
    };
    let mut cmap = gtps().map.lock().unwrap();
    let mut nmap = nodes().map.lock().unwrap();
    let mut mmap = mls().map.lock().unwrap();
    let (Some(c), Some(n), Some(m)) =
        (cmap.get_mut(&ch), nmap.get_mut(&nh), mmap.get_mut(&mh))
    else {
        set_last_error("bad handle");
        return GbpBuffer::empty();
    };
    match c.send(&mut **n, &mut **m, target, message_id, text) {
        Ok(of) => outbound_to_buffer(of),
        Err(e) => {
            set_last_error(e.to_string());
            GbpBuffer::empty()
        }
    }
}

/// Accepts a plaintext payload that the GBP layer surfaced via a
/// `payload_received` event. Returns a JSON object of the form
/// `{"status":"new|duplicate|error", ...}`.
///
/// `current_epoch` is the receiver node's current epoch — the client uses
/// it to auto-reset its idempotency state when the epoch advances.
///
/// # Safety
/// `pt_ptr` MUST be valid for `pt_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gtp_client_accept(
    ch: i32,
    current_epoch: u64,
    pt_ptr: *const u8,
    pt_len: usize,
) -> *mut c_char {
    clear_last_error();
    let pt = unsafe { std::slice::from_raw_parts(pt_ptr, pt_len) };
    let mut cmap = gtps().map.lock().unwrap();
    let Some(c) = cmap.get_mut(&ch) else {
        return alloc_cstring(r#"{"status":"error","reason":"bad client"}"#);
    };
    #[derive(Serialize)]
    struct Out<'a> {
        status: &'a str,
        sender: Option<u32>,
        message_id: Option<u64>,
        text: Option<String>,
        reason: Option<String>,
    }
    let out = match c.accept(pt, current_epoch) {
        Ok(GtpAccept::New(m)) => Out {
            status: "new",
            sender: Some(m.sender_id),
            message_id: Some(m.message_id),
            text: Some(m.text().unwrap_or("<binary>").to_string()),
            reason: None,
        },
        Ok(GtpAccept::Duplicate(m)) => Out {
            status: "duplicate",
            sender: Some(m.sender_id),
            message_id: Some(m.message_id),
            text: Some(m.text().unwrap_or("<binary>").to_string()),
            reason: None,
        },
        Err(e) => Out {
            status: "error",
            sender: None,
            message_id: None,
            text: None,
            reason: Some(e.to_string()),
        },
    };
    alloc_cstring(&serde_json::to_string(&out).unwrap_or_default())
}

// ============================================================================
// GAP client API
// ============================================================================

/// Creates a stateful GAP client.
#[unsafe(no_mangle)]
pub extern "C" fn gap_client_create() -> i32 {
    gaps().insert(GapClient::new())
}

/// Destroys a GAP client.
#[unsafe(no_mangle)]
pub extern "C" fn gap_client_destroy(h: i32) {
    gaps().remove(h);
}

/// Clears the client state. Intended for use after an epoch change.
#[unsafe(no_mangle)]
pub extern "C" fn gap_client_reset(h: i32) {
    if let Some(c) = gaps().map.lock().unwrap().get_mut(&h) {
        c.reset();
    }
}

/// Sends an Opus audio frame via GAP.
///
/// # Safety
/// `opus_ptr` MUST be valid for `opus_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gap_client_send(
    ch: i32,
    nh: i32,
    mh: i32,
    target: u32,
    media_source_id: u32,
    rtp_timestamp: u64,
    opus_ptr: *const u8,
    opus_len: usize,
) -> GbpBuffer {
    clear_last_error();
    let opus = unsafe { std::slice::from_raw_parts(opus_ptr, opus_len) }.to_vec();
    let mut cmap = gaps().map.lock().unwrap();
    let mut nmap = nodes().map.lock().unwrap();
    let mut mmap = mls().map.lock().unwrap();
    let (Some(c), Some(n), Some(m)) =
        (cmap.get_mut(&ch), nmap.get_mut(&nh), mmap.get_mut(&mh))
    else {
        set_last_error("bad handle");
        return GbpBuffer::empty();
    };
    match c.send(&mut **n, &mut **m, target, media_source_id, rtp_timestamp, opus) {
        Ok(of) => outbound_to_buffer(of),
        Err(e) => {
            set_last_error(e.to_string());
            GbpBuffer::empty()
        }
    }
}

/// Accepts a GAP audio payload.
///
/// # Safety
/// `pt_ptr` MUST be valid for `pt_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gap_client_accept(
    ch: i32,
    current_epoch: u64,
    pt_ptr: *const u8,
    pt_len: usize,
) -> *mut c_char {
    clear_last_error();
    let pt = unsafe { std::slice::from_raw_parts(pt_ptr, pt_len) };
    let mut cmap = gaps().map.lock().unwrap();
    let Some(c) = cmap.get_mut(&ch) else {
        return alloc_cstring(r#"{"status":"error","reason":"bad client"}"#);
    };
    #[derive(Serialize)]
    struct Out<'a> {
        status: &'a str,
        source: Option<u32>,
        seq: Option<u32>,
        bytes: Option<usize>,
        reason: Option<String>,
    }
    let out = match c.accept(pt, current_epoch) {
        Ok(GapAccept::New(p)) => Out {
            status: "new",
            source: Some(p.media_source_id),
            seq: Some(p.rtp_sequence),
            bytes: Some(p.opus_frame.len()),
            reason: None,
        },
        Ok(GapAccept::Late(p)) => Out {
            status: "late",
            source: Some(p.media_source_id),
            seq: Some(p.rtp_sequence),
            bytes: Some(p.opus_frame.len()),
            reason: None,
        },
        Err(e) => Out {
            status: "error",
            source: None,
            seq: None,
            bytes: None,
            reason: Some(e.to_string()),
        },
    };
    alloc_cstring(&serde_json::to_string(&out).unwrap_or_default())
}

// ============================================================================
// GSP client API
// ============================================================================

/// Creates a stateful GSP client.
#[unsafe(no_mangle)]
pub extern "C" fn gsp_client_create() -> i32 {
    gsps().insert(GspClient::new())
}

/// Destroys a GSP client.
#[unsafe(no_mangle)]
pub extern "C" fn gsp_client_destroy(h: i32) {
    gsps().remove(h);
}

/// Clears the client state. Intended for use after an epoch change.
#[unsafe(no_mangle)]
pub extern "C" fn gsp_client_reset(h: i32) {
    if let Some(c) = gsps().map.lock().unwrap().get_mut(&h) {
        c.reset();
    }
}

/// Sends a GSP signal.
#[unsafe(no_mangle)]
pub extern "C" fn gsp_client_send(
    ch: i32,
    nh: i32,
    mh: i32,
    target: u32,
    signal_type: u32,
    role_claim: u32,
    request_id: u32,
) -> GbpBuffer {
    clear_last_error();
    let sig = match SignalType::try_from(signal_type) {
        Ok(s) => s,
        Err(_) => {
            set_last_error(format!("bad signal {signal_type}"));
            return GbpBuffer::empty();
        }
    };
    let mut cmap = gsps().map.lock().unwrap();
    let mut nmap = nodes().map.lock().unwrap();
    let mut mmap = mls().map.lock().unwrap();
    let (Some(c), Some(n), Some(m)) =
        (cmap.get_mut(&ch), nmap.get_mut(&nh), mmap.get_mut(&mh))
    else {
        set_last_error("bad handle");
        return GbpBuffer::empty();
    };
    match c.send(&mut **n, &mut **m, target, sig, role_claim, request_id) {
        Ok(of) => outbound_to_buffer(of),
        Err(e) => {
            set_last_error(e.to_string());
            GbpBuffer::empty()
        }
    }
}

/// Accepts a GSP signal payload.
///
/// `current_epoch` is the receiver node's current epoch — the client uses
/// it to auto-reset its dedup state when the epoch advances.
///
/// # Safety
/// `pt_ptr` MUST be valid for `pt_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gsp_client_accept(
    ch: i32,
    current_epoch: u64,
    pt_ptr: *const u8,
    pt_len: usize,
) -> *mut c_char {
    clear_last_error();
    let pt = unsafe { std::slice::from_raw_parts(pt_ptr, pt_len) };
    let mut cmap = gsps().map.lock().unwrap();
    let Some(c) = cmap.get_mut(&ch) else {
        return alloc_cstring(r#"{"status":"error","reason":"bad client"}"#);
    };
    #[derive(Serialize)]
    struct Out<'a> {
        status: &'a str,
        signal: Option<&'a str>,
        signal_code: Option<u32>,
        sender: Option<u32>,
        role_claim: Option<u32>,
        request_id: Option<u32>,
        reason: Option<String>,
    }
    let out = match c.accept(pt, current_epoch) {
        Ok(GspAccept { signal, sender_id, role_claim, request_id }) => Out {
            status: "new",
            signal: Some(signal.name()),
            signal_code: Some(signal as u32),
            sender: Some(sender_id),
            role_claim: Some(role_claim),
            request_id: Some(request_id),
            reason: None,
        },
        Err(e) => Out {
            status: "error",
            signal: None,
            signal_code: None,
            sender: None,
            role_claim: None,
            request_id: None,
            reason: Some(e.to_string()),
        },
    };
    alloc_cstring(&serde_json::to_string(&out).unwrap_or_default())
}

// ============================================================================
// Codec helpers (used for tests that need malformed frames)
// ============================================================================

/// CBOR-encodes a [`gbp::GbpFrame`] with an arbitrary version byte.
///
/// # Safety
/// Every pointer MUST be valid for the corresponding declared length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gbp_frame_encode_v(
    version: u8,
    group_id_16: *const u8,
    epoch: u64,
    transition_id: u32,
    stream_type: u32,
    stream_id: u32,
    flags: u16,
    sequence_no: u32,
    payload_ptr: *const u8,
    payload_len: usize,
) -> GbpBuffer {
    clear_last_error();
    let mut gid = [0u8; 16];
    unsafe { std::ptr::copy_nonoverlapping(group_id_16, gid.as_mut_ptr(), 16) };
    let st_u8 = StreamType::try_from(stream_type).map(|s| s as u8).unwrap_or(stream_type as u8);
    let payload = unsafe { std::slice::from_raw_parts(payload_ptr, payload_len) }.to_vec();
    let frame = gbp_stack::gbp::GbpFrame {
        version,
        group_id: serde_bytes::ByteBuf::from(gid.to_vec()),
        epoch,
        transition_id,
        stream_type: st_u8,
        stream_id,
        flags,
        sequence_no,
        payload_size: payload.len() as u32,
        encrypted_payload: serde_bytes::ByteBuf::from(payload),
    };
    GbpBuffer::from_vec(frame.to_cbor())
}

/// Returns a CBOR-encoded `ErrorObject` for the given code.
#[unsafe(no_mangle)]
pub extern "C" fn gbp_error_lookup(code: u16) -> GbpBuffer {
    use gbp_stack::core::errors::ErrorSpec;
    match ErrorSpec::lookup(code) {
        Some(spec) => GbpBuffer::from_vec(ErrorObject::from_spec(spec, spec.name).to_cbor()),
        None => {
            set_last_error(format!("unknown error code 0x{code:04X}"));
            GbpBuffer::empty()
        }
    }
}

#[allow(dead_code)]
fn _link(_f: &GbpFrame, _l: StreamLabel) {}

// ============================================================================
// Event JSON
// ============================================================================

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum EventDto<'a> {
    StateChanged {
        from: String,
        to: String,
    },
    PayloadReceived {
        stream_type: &'a str,
        stream_type_code: u32,
        stream_id: u32,
        sequence_no: u32,
        flags: u16,
        plaintext_b64: String,
    },
    Control {
        from: u32,
        opcode: &'a str,
        opcode_code: u16,
        transition_id: u32,
        request_id: u32,
        args_b64: String,
    },
    Error {
        code: u16,
        code_hex: String,
        class: u8,
        retryable: bool,
        fatal: bool,
        reason: String,
    },
    EpochAdvanced {
        epoch: u64,
        transition_id: u32,
    },
}

fn b64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= b.len() {
        let n = ((b[i] as u32) << 16) | ((b[i + 1] as u32) << 8) | (b[i + 2] as u32);
        out.push(A[(n >> 18) as usize & 0x3F] as char);
        out.push(A[(n >> 12) as usize & 0x3F] as char);
        out.push(A[(n >> 6) as usize & 0x3F] as char);
        out.push(A[n as usize & 0x3F] as char);
        i += 3;
    }
    let rem = b.len() - i;
    if rem == 1 {
        let n = (b[i] as u32) << 16;
        out.push(A[(n >> 18) as usize & 0x3F] as char);
        out.push(A[(n >> 12) as usize & 0x3F] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((b[i] as u32) << 16) | ((b[i + 1] as u32) << 8);
        out.push(A[(n >> 18) as usize & 0x3F] as char);
        out.push(A[(n >> 12) as usize & 0x3F] as char);
        out.push(A[(n >> 6) as usize & 0x3F] as char);
        out.push('=');
    }
    out
}

fn dto<'a>(e: &'a Event) -> EventDto<'a> {
    match e {
        Event::StateChanged { from, to } => EventDto::StateChanged {
            from: from.to_string(),
            to: to.to_string(),
        },
        Event::PayloadReceived(DeliveredPayload {
            stream_type,
            stream_id,
            sequence_no,
            flags,
            plaintext,
        }) => EventDto::PayloadReceived {
            stream_type: match stream_type {
                StreamType::Control => "control",
                StreamType::Audio => "audio",
                StreamType::Text => "text",
                StreamType::Signal => "signal",
            },
            stream_type_code: *stream_type as u32,
            stream_id: *stream_id,
            sequence_no: *sequence_no,
            flags: *flags,
            plaintext_b64: b64(plaintext),
        },
        Event::Control { from, opcode, transition_id, request_id, args } => EventDto::Control {
            from: *from,
            opcode: opcode.name(),
            opcode_code: *opcode as u16,
            transition_id: *transition_id,
            request_id: *request_id,
            args_b64: b64(args),
        },
        Event::Error { code, class, retryable, fatal, reason } => EventDto::Error {
            code: *code,
            code_hex: format!("0x{code:04X}"),
            class: *class as u8,
            retryable: *retryable,
            fatal: *fatal,
            reason: reason.clone(),
        },
        Event::EpochAdvanced { epoch, transition_id } => EventDto::EpochAdvanced {
            epoch: *epoch,
            transition_id: *transition_id,
        },
    }
}

fn events_to_json(events: &[Event]) -> String {
    let dtos: Vec<EventDto> = events.iter().map(dto).collect();
    serde_json::to_string(&dtos).unwrap_or_else(|_| "[]".to_string())
}

#[allow(dead_code)]
const _STATES: [NodeState; 7] = [
    NodeState::Idle,
    NodeState::Connecting,
    NodeState::EstablishingGroup,
    NodeState::Active,
    NodeState::Resyncing,
    NodeState::Failed,
    NodeState::Closed,
];
