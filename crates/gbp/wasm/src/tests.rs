//! Integration tests for the gbp-stack-wasm WASM bindings.
//!
//! Covers:
//!  - MlsContext: create, epoch, keyPackage, groupId, invite, acceptWelcome
//!  - GroupNode: create, bootstrap (creator + joiner), onWire, checkTimeouts
//!  - GtpClient: create, send, accept (roundtrip, duplicate, unicode)
//!  - Two-member group lifecycle (MLS invite → GBP bootstrap → GTP exchange)

use super::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_node_experimental);

// ─── helpers ────────────────────────────────────────────────────────────────

fn gid(b: u8) -> Vec<u8> {
    vec![b; 16]
}

/// Creates a bootstrapped single-member creator group.
fn creator_group(user: &str, group_byte: u8) -> (MlsContext, GroupNode, GtpClient) {
    let mls  = MlsContext::create(user).expect("MlsContext::create");
    let node = GroupNode::create(1, &gid(group_byte));
    node.bootstrap_as_creator(mls.epoch());
    let gtp = GtpClient::create();
    (mls, node, gtp)
}

/// Creates a two-member group via MLS invite.
/// Returns (alice_mls, alice_node, alice_gtp, bob_mls, bob_node, bob_gtp).
fn two_member_group() -> (MlsContext, GroupNode, GtpClient, MlsContext, GroupNode, GtpClient) {
    let alice_mls = MlsContext::create("alice").unwrap();
    let bob_mls   = MlsContext::create("bob").unwrap();

    let welcome = alice_mls.invite(&bob_mls.key_package()).unwrap();
    bob_mls.accept_welcome(&welcome.to_vec()).unwrap();

    let group_id = alice_mls.group_id().to_vec();
    let alice_node = GroupNode::create(1, &group_id);
    let bob_node   = GroupNode::create(2, &group_id);
    alice_node.bootstrap_as_creator(alice_mls.epoch());
    bob_node.bootstrap_as_joiner(bob_mls.epoch(), 0);

    (
        alice_mls, alice_node, GtpClient::create(),
        bob_mls,   bob_node,   GtpClient::create(),
    )
}

fn text_events(arr: &Array) -> Vec<JsValue> {
    (0..arr.length())
        .map(|i| arr.get(i))
        .filter(|ev| {
            let kind = Reflect::get(ev, &"kind".into())
                .map(|v| v.as_string().unwrap_or_default())
                .unwrap_or_default();
            let st = Reflect::get(ev, &"streamType".into())
                .map(|v| v.as_f64().unwrap_or(-1.0) as u8)
                .unwrap_or(255);
            kind == "payload_received" && st == StreamType::Text.as_u8()
        })
        .collect()
}

fn plaintext_of(ev: &JsValue) -> Vec<u8> {
    let pt = Reflect::get(ev, &"plaintext".into()).unwrap();
    Uint8Array::new(&pt).to_vec()
}

// ─── MlsContext ──────────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn mls_create_epoch_zero() {
    let ctx = MlsContext::create("alice").unwrap();
    assert_eq!(ctx.epoch(), 0u64);
}

#[wasm_bindgen_test]
fn mls_key_package_nonempty() {
    let ctx = MlsContext::create("alice").unwrap();
    assert!(ctx.key_package().length() > 0);
}

#[wasm_bindgen_test]
fn mls_group_id_16_bytes() {
    let ctx = MlsContext::create("alice").unwrap();
    assert_eq!(ctx.group_id().length(), 16);
}

#[wasm_bindgen_test]
fn mls_invite_accept_syncs_epoch() {
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();

    assert_eq!(alice.epoch(), 0u64);
    assert_eq!(bob.epoch(), 0u64);

    let welcome = alice.invite(&bob.key_package().to_vec()).unwrap();
    bob.accept_welcome(&welcome.to_vec()).unwrap();

    assert_eq!(alice.epoch(), 1u64);
    assert_eq!(bob.epoch(), 1u64);
    assert_eq!(alice.group_id().to_vec(), bob.group_id().to_vec());
}

#[wasm_bindgen_test]
fn mls_two_distinct_users_have_different_key_packages() {
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();
    assert_ne!(alice.key_package().to_vec(), bob.key_package().to_vec());
}

// ─── GroupNode ───────────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn group_node_create_bootstrap_as_creator() {
    let mls  = MlsContext::create("alice").unwrap();
    let node = GroupNode::create(1, &gid(0x01));
    node.bootstrap_as_creator(mls.epoch());
    assert_eq!(node.current_epoch(), 0u64);
    assert_eq!(node.member_id(), 1u32);
    assert_eq!(node.last_transition_id(), 0u32);
}

#[wasm_bindgen_test]
fn group_node_bootstrap_as_joiner() {
    let alice_mls = MlsContext::create("alice").unwrap();
    let bob_mls   = MlsContext::create("bob").unwrap();
    let welcome   = alice_mls.invite(&bob_mls.key_package().to_vec()).unwrap();
    bob_mls.accept_welcome(&welcome.to_vec()).unwrap();

    let gid = alice_mls.group_id().to_vec();
    let bob_node = GroupNode::create(2, &gid);
    bob_node.bootstrap_as_joiner(bob_mls.epoch(), 0);
    assert_eq!(bob_node.current_epoch(), 1u64);
    assert_eq!(bob_node.member_id(), 2u32);
}

#[wasm_bindgen_test]
fn group_node_check_timeouts_returns_array() {
    let mls  = MlsContext::create("alice").unwrap();
    let node = GroupNode::create(1, &gid(0x02));
    node.bootstrap_as_creator(mls.epoch());
    let evs = node.check_timeouts();
    assert!(evs.is_array());
}

// ─── GtpClient ───────────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn gtp_send_returns_wire_bytes() {
    let (mls, node, gtp) = creator_group("alice", 0x03);
    let frame = gtp.send(&node, &mls, 0, 1, "hello");
    assert!(!frame.is_null());
    let wire = Reflect::get(&frame, &"wire".into()).unwrap();
    assert!(Uint8Array::new(&wire).length() > 0);
}

#[wasm_bindgen_test]
fn gtp_single_member_roundtrip() {
    let (mls, node, gtp) = creator_group("alice", 0x04);
    let frame = gtp.send(&node, &mls, 0, 1, "hello wasm");
    let wire = Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec();

    let evs   = node.on_wire(&mls, &wire);
    let texts = text_events(&evs);
    assert_eq!(texts.len(), 1);

    let result = gtp.accept(&plaintext_of(&texts[0]), mls.epoch());
    assert!(!result.is_null());
    let text = Reflect::get(&result, &"text".into()).unwrap().as_string().unwrap();
    assert_eq!(text, "hello wasm");
}

#[wasm_bindgen_test]
fn gtp_unicode_roundtrip() {
    let (mls, node, gtp) = creator_group("alice", 0x05);
    let msg = "Привет 🌍 こんにちは";
    let frame = gtp.send(&node, &mls, 0, 1, msg);
    let wire = Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec();

    for ev in text_events(&node.on_wire(&mls, &wire)) {
        let result = gtp.accept(&plaintext_of(&ev), mls.epoch());
        let text = Reflect::get(&result, &"text".into()).unwrap().as_string().unwrap();
        assert_eq!(text, msg);
    }
}

#[wasm_bindgen_test]
fn gtp_duplicate_returns_status_duplicate() {
    let (mls, node, gtp) = creator_group("alice", 0x06);

    // First delivery → "new"
    let wire1 = {
        let frame = gtp.send(&node, &mls, 0, 42, "msg");
        Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec()
    };
    for ev in text_events(&node.on_wire(&mls, &wire1)) {
        let r = gtp.accept(&plaintext_of(&ev), mls.epoch());
        let status = Reflect::get(&r, &"status".into()).unwrap().as_string().unwrap();
        assert_eq!(status, "new");
    }

    // Same message_id again → "duplicate"
    let wire2 = {
        let frame = gtp.send(&node, &mls, 0, 42, "msg");
        Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec()
    };
    for ev in text_events(&node.on_wire(&mls, &wire2)) {
        let r = gtp.accept(&plaintext_of(&ev), mls.epoch());
        let status = Reflect::get(&r, &"status".into()).unwrap().as_string().unwrap();
        assert_eq!(status, "duplicate");
    }
}

#[wasm_bindgen_test]
fn gtp_sequential_message_ids() {
    let (mls, node, gtp) = creator_group("alice", 0x07);
    for i in 1u64..=5 {
        let frame = gtp.send(&node, &mls, 0, i, &format!("msg {i}"));
        let wire = Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec();
        let evs = text_events(&node.on_wire(&mls, &wire));
        assert_eq!(evs.len(), 1, "msg {i} should produce exactly one text event");
        let r = gtp.accept(&plaintext_of(&evs[0]), mls.epoch());
        let text = Reflect::get(&r, &"text".into()).unwrap().as_string().unwrap();
        assert_eq!(text, format!("msg {i}"));
    }
}

#[wasm_bindgen_test]
fn gtp_reset_clears_dedup_set() {
    let (mls, node, gtp) = creator_group("alice", 0x08);

    let wire = {
        let frame = gtp.send(&node, &mls, 0, 1, "a");
        Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec()
    };
    for ev in text_events(&node.on_wire(&mls, &wire)) {
        let r = gtp.accept(&plaintext_of(&ev), mls.epoch());
        assert_eq!(Reflect::get(&r, &"status".into()).unwrap().as_string().unwrap(), "new");
    }

    gtp.reset();

    // After reset, same message_id is "new" again.
    let wire2 = {
        let frame = gtp.send(&node, &mls, 0, 1, "a");
        Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec()
    };
    for ev in text_events(&node.on_wire(&mls, &wire2)) {
        let r = gtp.accept(&plaintext_of(&ev), mls.epoch());
        assert_eq!(Reflect::get(&r, &"status".into()).unwrap().as_string().unwrap(), "new");
    }
}

// ─── Two-member group ────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn two_member_gtp_alice_to_bob() {
    let (alice_mls, alice_node, gtp_alice, bob_mls, bob_node, gtp_bob) = two_member_group();

    let frame = gtp_alice.send(&alice_node, &alice_mls, 2, 1, "hello bob");
    let wire  = Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec();

    let evs = text_events(&bob_node.on_wire(&bob_mls, &wire));
    assert_eq!(evs.len(), 1);
    let r = gtp_bob.accept(&plaintext_of(&evs[0]), bob_mls.epoch());
    assert_eq!(
        Reflect::get(&r, &"text".into()).unwrap().as_string().unwrap(),
        "hello bob"
    );
    assert_eq!(
        Reflect::get(&r, &"status".into()).unwrap().as_string().unwrap(),
        "new"
    );
}

#[wasm_bindgen_test]
fn two_member_gtp_bidirectional() {
    let (alice_mls, alice_node, gtp_alice, bob_mls, bob_node, gtp_bob) = two_member_group();

    // Alice → Bob
    let wire_ab = {
        let frame = gtp_alice.send(&alice_node, &alice_mls, 2, 1, "ping");
        Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec()
    };
    for ev in text_events(&bob_node.on_wire(&bob_mls, &wire_ab)) {
        let r = gtp_bob.accept(&plaintext_of(&ev), bob_mls.epoch());
        assert_eq!(Reflect::get(&r, &"text".into()).unwrap().as_string().unwrap(), "ping");
    }

    // Bob → Alice
    let wire_ba = {
        let frame = gtp_bob.send(&bob_node, &bob_mls, 1, 1, "pong");
        Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).to_vec()
    };
    for ev in text_events(&alice_node.on_wire(&alice_mls, &wire_ba)) {
        let r = gtp_alice.accept(&plaintext_of(&ev), alice_mls.epoch());
        assert_eq!(Reflect::get(&r, &"text".into()).unwrap().as_string().unwrap(), "pong");
    }
}

#[wasm_bindgen_test]
fn two_member_epochs_match() {
    let (alice_mls, _an, _ga, bob_mls, _bn, _gb) = two_member_group();
    assert_eq!(alice_mls.epoch(), bob_mls.epoch());
    assert_eq!(alice_mls.group_id().to_vec(), bob_mls.group_id().to_vec());
}
