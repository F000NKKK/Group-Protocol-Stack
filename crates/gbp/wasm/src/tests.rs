//! Integration tests for the gbp-stack-wasm WASM bindings.
//!
//! Covers:
//!  - MlsContext: create, epoch, keyPackage, groupId, invite, acceptWelcome,
//!    inviteFull/finalizeCommit, removeMember, processMessage
//!  - GroupNode: create, bootstrap (creator + joiner), onWire, checkTimeouts,
//!    sendControl, drainEvents
//!  - GtpClient: create, send, accept (roundtrip, duplicate, unicode, codec)
//!  - GapClient: send, accept (audio roundtrip)
//!  - GspClient: send, accept (signal roundtrip)
//!  - SFrameSession/SFrameEncryptor: encrypt → decrypt roundtrip
//!  - Two-member group lifecycle (MLS invite → GBP bootstrap → exchange)

use super::*;
use gbp_core::StreamType;
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

    let welcome = alice_mls.invite(&bob_mls.key_package().to_vec()).unwrap();
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

/// Filters `payload_received` events of one stream type out of an events array.
fn events_for(arr: &Array, want: StreamType) -> Vec<JsValue> {
    (0..arr.length())
        .map(|i| arr.get(i))
        .filter(|ev| {
            let kind = Reflect::get(ev, &"kind".into())
                .map(|v| v.as_string().unwrap_or_default())
                .unwrap_or_default();
            let st = Reflect::get(ev, &"streamType".into())
                .map(|v| v.as_f64().unwrap_or(-1.0) as u8)
                .unwrap_or(255);
            kind == "payload_received" && st == want.as_u8()
        })
        .collect()
}

fn text_events(arr: &Array) -> Vec<JsValue> {
    events_for(arr, StreamType::Text)
}
fn audio_events(arr: &Array) -> Vec<JsValue> {
    events_for(arr, StreamType::Audio)
}
fn signal_events(arr: &Array) -> Vec<JsValue> {
    events_for(arr, StreamType::Signal)
}

fn plaintext_of(ev: &JsValue) -> Vec<u8> {
    let pt = Reflect::get(ev, &"plaintext".into()).unwrap();
    Uint8Array::new(&pt).to_vec()
}

fn wire_of(frame: &JsValue) -> Vec<u8> {
    Uint8Array::new(&Reflect::get(frame, &"wire".into()).unwrap()).to_vec()
}

fn str_field(obj: &JsValue, key: &str) -> String {
    Reflect::get(obj, &key.into()).unwrap().as_string().unwrap()
}
fn num_field(obj: &JsValue, key: &str) -> f64 {
    Reflect::get(obj, &key.into()).unwrap().as_f64().unwrap()
}
fn bytes_field(obj: &JsValue, key: &str) -> Vec<u8> {
    Uint8Array::new(&Reflect::get(obj, &key.into()).unwrap()).to_vec()
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
fn mls_invite_many_one_welcome_serves_all_joiners() {
    // inviteMany adds several members in one commit and returns ONE Welcome
    // that each joiner accepts with their own KeyPackage.
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();
    let carol = MlsContext::create("carol").unwrap();

    let kps = Array::new();
    kps.push(&JsValue::from(bob.key_package()));
    kps.push(&JsValue::from(carol.key_package()));
    let welcome = alice.invite_many(kps).unwrap();
    assert_eq!(alice.epoch(), 1u64, "one Add commit advances the epoch once");

    bob.accept_welcome(&welcome.to_vec()).unwrap();
    carol.accept_welcome(&welcome.to_vec()).unwrap();
    assert_eq!(bob.epoch(), 1u64);
    assert_eq!(carol.epoch(), 1u64);
    assert_eq!(alice.group_id().to_vec(), bob.group_id().to_vec());
    assert_eq!(alice.group_id().to_vec(), carol.group_id().to_vec());
}

#[wasm_bindgen_test]
fn mls_invite_many_empty_errors() {
    let alice = MlsContext::create("alice").unwrap();
    assert!(alice.invite_many(Array::new()).is_err());
}

#[wasm_bindgen_test]
fn mls_two_distinct_users_have_different_key_packages() {
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();
    assert_ne!(alice.key_package().to_vec(), bob.key_package().to_vec());
}

#[wasm_bindgen_test]
fn mls_invite_full_stages_then_finalize_advances_epoch() {
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();

    let res = alice.invite_full(&bob.key_package().to_vec()).unwrap();
    let commit  = Reflect::get(&res, &"commit".into()).unwrap();
    let welcome = Reflect::get(&res, &"welcome".into()).unwrap();
    assert!(Uint8Array::new(&commit).length() > 0);
    assert!(Uint8Array::new(&welcome).length() > 0);

    // inviteFull stages the commit — epoch must NOT advance until finalize.
    assert_eq!(alice.epoch(), 0u64);
    alice.finalize_commit().unwrap();
    assert_eq!(alice.epoch(), 1u64);

    bob.accept_welcome(&Uint8Array::new(&welcome).to_vec()).unwrap();
    assert_eq!(bob.epoch(), 1u64);
    assert_eq!(alice.group_id().to_vec(), bob.group_id().to_vec());
}

#[wasm_bindgen_test]
fn mls_clear_pending_commit_rolls_back() {
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();

    let _res = alice.invite_full(&bob.key_package().to_vec()).unwrap();
    assert_eq!(alice.epoch(), 0u64);
    alice.clear_pending_commit().unwrap();
    // Rolled back — still epoch 0, and a fresh invite still works.
    assert_eq!(alice.epoch(), 0u64);
    let welcome = alice.invite(&bob.key_package().to_vec()).unwrap();
    bob.accept_welcome(&welcome.to_vec()).unwrap();
    assert_eq!(alice.epoch(), 1u64);
}

#[wasm_bindgen_test]
fn mls_remove_member_advances_epoch() {
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();
    let welcome = alice.invite(&bob.key_package().to_vec()).unwrap();
    bob.accept_welcome(&welcome.to_vec()).unwrap();
    assert_eq!(alice.epoch(), 1u64);

    // Creator is leaf 0; the first joiner (bob) is leaf 1.
    let commit = alice.remove_member(1).unwrap();
    assert!(commit.length() > 0);
    alice.finalize_commit().unwrap();
    assert_eq!(alice.epoch(), 2u64);
}

#[wasm_bindgen_test]
fn mls_export_restore_preserves_epoch_and_group_id() {
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();
    bob.accept_welcome(&alice.invite(&bob.key_package().to_vec()).unwrap().to_vec()).unwrap();
    assert_eq!(alice.epoch(), 1u64);

    let blob = alice.export_state().unwrap().to_vec();
    assert!(!blob.is_empty());
    let restored = MlsContext::restore_state(&blob).unwrap();
    assert_eq!(restored.epoch(), alice.epoch());
    assert_eq!(restored.group_id().to_vec(), alice.group_id().to_vec());
}

#[wasm_bindgen_test]
fn mls_process_message_applies_commit() {
    // Three members so a remove produces a Commit that a *third* member must
    // process to advance. alice=leaf0 creator, bob=leaf1, carol=leaf2.
    let alice = MlsContext::create("alice").unwrap();
    let bob   = MlsContext::create("bob").unwrap();
    let carol = MlsContext::create("carol").unwrap();

    let w_bob = alice.invite(&bob.key_package().to_vec()).unwrap();
    bob.accept_welcome(&w_bob.to_vec()).unwrap();

    // Add carol via the two-phase flow and broadcast the commit to bob.
    let res = alice.invite_full(&carol.key_package().to_vec()).unwrap();
    let commit  = Uint8Array::new(&Reflect::get(&res, &"commit".into()).unwrap()).to_vec();
    let welcome = Uint8Array::new(&Reflect::get(&res, &"welcome".into()).unwrap()).to_vec();
    alice.finalize_commit().unwrap();
    carol.accept_welcome(&welcome).unwrap();

    // Bob processes the add-commit → "commit" (staged), then finalizes to
    // advance his epoch to match alice + carol.
    let kind = bob.process_message(&commit).unwrap();
    assert_eq!(kind, "commit");
    bob.finalize_commit().unwrap();
    assert_eq!(bob.epoch(), alice.epoch());
    assert_eq!(bob.epoch(), carol.epoch());
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

#[wasm_bindgen_test]
fn group_node_send_control_returns_wire() {
    let (mls, node, _gtp) = creator_group("alice", 0x20);
    // CapabilitiesAdvertise = 0x0008, no args, broadcast.
    let r = node.send_control(&mls, 0, 0x0008, 0, 1, &[]).unwrap();
    assert!(Uint8Array::new(&Reflect::get(&r, &"wire".into()).unwrap()).length() > 0);
}

#[wasm_bindgen_test]
fn group_node_send_control_rejects_bad_opcode() {
    let (mls, node, _gtp) = creator_group("alice", 0x23);
    assert!(node.send_control(&mls, 0, 0xFFFF, 0, 1, &[]).is_err());
}

#[wasm_bindgen_test]
fn group_node_drain_events_returns_array() {
    let (_mls, node, _gtp) = creator_group("alice", 0x21);
    assert!(node.drain_events().is_array());
}

// ─── GtpClient ───────────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn gtp_send_returns_wire_bytes() {
    let (mls, node, gtp) = creator_group("alice", 0x03);
    let frame = gtp.send(&node, &mls, 0, 1, "hello", None);
    assert!(!frame.is_null());
    assert!(Uint8Array::new(&Reflect::get(&frame, &"wire".into()).unwrap()).length() > 0);
}

#[wasm_bindgen_test]
fn gtp_single_member_roundtrip() {
    let (mls, node, gtp) = creator_group("alice", 0x04);
    let frame = gtp.send(&node, &mls, 0, 1, "hello wasm", None);
    let wire  = wire_of(&frame);

    let texts = text_events(&node.on_wire(&mls, &wire));
    assert_eq!(texts.len(), 1);

    let result = gtp.accept(&plaintext_of(&texts[0]), mls.epoch(), None);
    assert!(!result.is_null());
    assert_eq!(str_field(&result, "text"), "hello wasm");
}

// Re-login regression: a rebuilt node restarts outbound seqs at 1, so its
// frames are dropped by a peer whose replay high-water-mark is already higher.
#[wasm_bindgen_test]
fn relogin_without_out_seq_restore_is_replay_dropped() {
    let (alice_mls, alice_node, alice_gtp, bob_mls, bob_node, _bob_gtp) = two_member_group();
    let gid = alice_mls.group_id().to_vec();
    for (mid, t) in [(1u64, "m1"), (2u64, "m2")] {
        let f = alice_gtp.send(&alice_node, &alice_mls, 0, mid, t, None);
        let _ = bob_node.on_wire(&bob_mls, &wire_of(&f)); // advances bob's in_hw to 2
    }
    // Fresh node + client (re-login), NO out_seq restore.
    let alice_node2 = GroupNode::create(1, &gid);
    alice_node2.bootstrap_as_creator(alice_mls.epoch());
    let alice_gtp2 = GtpClient::create();
    let f = alice_gtp2.send(&alice_node2, &alice_mls, 0, 3, "m3", None);
    let texts = text_events(&bob_node.on_wire(&bob_mls, &wire_of(&f)));
    assert_eq!(texts.len(), 0, "fresh node frame (seq 1) is replay-dropped at the peer");
}

// …restoring the outbound counters lets the rebuilt node resume above the
// peer's high-water-mark, so new messages are delivered.
#[wasm_bindgen_test]
fn relogin_with_out_seq_restore_delivers_new_frames() {
    let (alice_mls, alice_node, alice_gtp, bob_mls, bob_node, bob_gtp) = two_member_group();
    let gid = alice_mls.group_id().to_vec();
    for (mid, t) in [(1u64, "m1"), (2u64, "m2")] {
        let f = alice_gtp.send(&alice_node, &alice_mls, 0, mid, t, None);
        let _ = bob_node.on_wire(&bob_mls, &wire_of(&f));
    }
    let saved = alice_node.export_out_seq().to_vec();
    let alice_node2 = GroupNode::create(1, &gid);
    alice_node2.bootstrap_as_creator(alice_mls.epoch());
    alice_node2.restore_out_seq(&saved); // resume outbound counters
    let alice_gtp2 = GtpClient::create();
    let f = alice_gtp2.send(&alice_node2, &alice_mls, 0, 3, "m3", None);
    let texts = text_events(&bob_node.on_wire(&bob_mls, &wire_of(&f)));
    assert_eq!(texts.len(), 1, "restored out_seq continues above the peer high-water-mark");
    let r = bob_gtp.accept(&plaintext_of(&texts[0]), bob_mls.epoch(), None);
    assert_eq!(str_field(&r, "text"), "m3");
}

#[wasm_bindgen_test]
fn gtp_unicode_roundtrip() {
    let (mls, node, gtp) = creator_group("alice", 0x05);
    let msg = "Привет 🌍 こんにちは";
    let frame = gtp.send(&node, &mls, 0, 1, msg, None);
    let wire  = wire_of(&frame);

    for ev in text_events(&node.on_wire(&mls, &wire)) {
        let result = gtp.accept(&plaintext_of(&ev), mls.epoch(), None);
        assert_eq!(str_field(&result, "text"), msg);
    }
}

#[wasm_bindgen_test]
fn gtp_flatbuffers_codec_roundtrip() {
    let (mls, node, gtp) = creator_group("alice", 0x22);
    // PayloadCodec.FlatBuffers = 2.
    let frame = gtp.send(&node, &mls, 0, 1, "fb!", Some(2));
    let wire  = wire_of(&frame);
    let evs   = text_events(&node.on_wire(&mls, &wire));
    assert_eq!(evs.len(), 1);
    let codec = num_field(&evs[0], "codec") as u8;
    assert_eq!(codec, 2, "delivered payload should report the FlatBuffers codec");
    let r = gtp.accept(&plaintext_of(&evs[0]), mls.epoch(), Some(codec));
    assert_eq!(str_field(&r, "text"), "fb!");
}

#[wasm_bindgen_test]
fn gtp_duplicate_returns_status_duplicate() {
    let (mls, node, gtp) = creator_group("alice", 0x06);

    let wire1 = wire_of(&gtp.send(&node, &mls, 0, 42, "msg", None));
    for ev in text_events(&node.on_wire(&mls, &wire1)) {
        assert_eq!(str_field(&gtp.accept(&plaintext_of(&ev), mls.epoch(), None), "status"), "new");
    }

    let wire2 = wire_of(&gtp.send(&node, &mls, 0, 42, "msg", None));
    for ev in text_events(&node.on_wire(&mls, &wire2)) {
        assert_eq!(str_field(&gtp.accept(&plaintext_of(&ev), mls.epoch(), None), "status"), "duplicate");
    }
}

#[wasm_bindgen_test]
fn gtp_sequential_message_ids() {
    let (mls, node, gtp) = creator_group("alice", 0x07);
    for i in 1u64..=5 {
        let wire = wire_of(&gtp.send(&node, &mls, 0, i, &format!("msg {i}"), None));
        let evs = text_events(&node.on_wire(&mls, &wire));
        assert_eq!(evs.len(), 1, "msg {i} should produce exactly one text event");
        assert_eq!(str_field(&gtp.accept(&plaintext_of(&evs[0]), mls.epoch(), None), "text"), format!("msg {i}"));
    }
}

#[wasm_bindgen_test]
fn gtp_reset_clears_dedup_set() {
    let (mls, node, gtp) = creator_group("alice", 0x08);

    let wire = wire_of(&gtp.send(&node, &mls, 0, 1, "a", None));
    for ev in text_events(&node.on_wire(&mls, &wire)) {
        assert_eq!(str_field(&gtp.accept(&plaintext_of(&ev), mls.epoch(), None), "status"), "new");
    }

    gtp.reset();

    let wire2 = wire_of(&gtp.send(&node, &mls, 0, 1, "a", None));
    for ev in text_events(&node.on_wire(&mls, &wire2)) {
        assert_eq!(str_field(&gtp.accept(&plaintext_of(&ev), mls.epoch(), None), "status"), "new");
    }
}

// ─── GapClient (audio) ─────────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn gap_single_member_roundtrip() {
    let (mls, node, _gtp) = creator_group("alice", 0x10);
    let gap = GapClient::create();
    let opus = vec![0xAAu8, 0xBB, 0xCC, 0xDD, 0xEE];

    let frame = gap.send(&node, &mls, 0, 7, 1_000, &opus, None);
    assert!(!frame.is_null());
    let wire = wire_of(&frame);

    let evs = audio_events(&node.on_wire(&mls, &wire));
    assert_eq!(evs.len(), 1);
    let r = gap.accept(&plaintext_of(&evs[0]), mls.epoch(), None);
    assert!(!r.is_null());
    assert_eq!(str_field(&r, "status"), "new");
    assert_eq!(num_field(&r, "source") as u32, 7);
    assert_eq!(bytes_field(&r, "opus"), opus);
}

#[wasm_bindgen_test]
fn gap_two_member_flatbuffers_roundtrip() {
    let (alice_mls, alice_node, _ga, bob_mls, bob_node, _gb) = two_member_group();
    let gap_alice = GapClient::create();
    let gap_bob   = GapClient::create();
    let opus = vec![1u8, 2, 3, 4, 5, 6, 7, 8];

    // FlatBuffers codec for audio.
    let frame = gap_alice.send(&alice_node, &alice_mls, 2, 11, 2_000, &opus, Some(2));
    let wire  = wire_of(&frame);

    let evs = audio_events(&bob_node.on_wire(&bob_mls, &wire));
    assert_eq!(evs.len(), 1);
    let codec = num_field(&evs[0], "codec") as u8;
    let r = gap_bob.accept(&plaintext_of(&evs[0]), bob_mls.epoch(), Some(codec));
    assert_eq!(str_field(&r, "status"), "new");
    assert_eq!(num_field(&r, "source") as u32, 11);
    assert_eq!(bytes_field(&r, "opus"), opus);
}

// ─── GspClient (signalling) ──────────────────────────────────────────────────

#[wasm_bindgen_test]
fn gsp_join_signal_roundtrip() {
    let (alice_mls, alice_node, _ga, bob_mls, bob_node, _gb) = two_member_group();
    let gsp_alice = GspClient::create();
    let gsp_bob   = GspClient::create();

    // SignalType.Join = 100, request_id 1, broadcast role_claim 0.
    let frame = gsp_alice.send(&alice_node, &alice_mls, 2, 100, 0, 1, None).unwrap();
    let wire  = wire_of(&frame);

    let evs = signal_events(&bob_node.on_wire(&bob_mls, &wire));
    assert_eq!(evs.len(), 1);
    let r = gsp_bob.accept(&plaintext_of(&evs[0]), bob_mls.epoch(), None).unwrap();
    assert_eq!(str_field(&r, "status"), "new");
    assert_eq!(str_field(&r, "signal"), "JOIN");
    assert_eq!(num_field(&r, "signalCode") as u32, 100);
    assert_eq!(num_field(&r, "requestId") as u32, 1);
}

#[wasm_bindgen_test]
fn gsp_mute_with_args_roundtrip() {
    let (alice_mls, alice_node, _ga, bob_mls, bob_node, _gb) = two_member_group();
    let gsp_alice = GspClient::create();
    let gsp_bob   = GspClient::create();

    // SignalType.Mute = 200 requires a CBOR map {0: target_member_id (uint)}.
    let mut args = Vec::new();
    let m = ciborium::Value::Map(vec![(
        ciborium::Value::Integer(0u64.into()),
        ciborium::Value::Integer(1u64.into()),
    )]);
    ciborium::ser::into_writer(&m, &mut args).unwrap();
    let frame = gsp_alice.send_with_args(&alice_node, &alice_mls, 2, 200, 0, 5, &args, None).unwrap();
    let wire  = wire_of(&frame);

    let evs = signal_events(&bob_node.on_wire(&bob_mls, &wire));
    assert_eq!(evs.len(), 1);
    let r = gsp_bob.accept(&plaintext_of(&evs[0]), bob_mls.epoch(), None).unwrap();
    assert_eq!(str_field(&r, "signal"), "MUTE");
    assert_eq!(num_field(&r, "requestId") as u32, 5);
}

#[wasm_bindgen_test]
fn gsp_bad_signal_type_errors() {
    let (mls, node, _gtp) = creator_group("alice", 0x24);
    let gsp = GspClient::create();
    // 999 is not a valid SignalType.
    assert!(gsp.send(&node, &mls, 0, 999, 0, 1, None).is_err());
}

// ─── SFrame (media E2EE) ──────────────────────────────────────────────────────

#[wasm_bindgen_test]
fn sframe_encrypt_decrypt_roundtrip() {
    // alice + bob share one MLS group at epoch 1 → identical exporter secret.
    let (alice_mls, _an, _ga, bob_mls, _bn, _gb) = two_member_group();
    let label = "gbp/sframe v1";

    // Receiver session on bob; sender encryptor on alice (leaf 0 = creator).
    let bob_session = SFrameSession::create(&bob_mls, label, 0).unwrap();
    let alice_session = SFrameSession::create(&alice_mls, label, 0).unwrap();
    let enc = alice_session.create_encryptor(&alice_mls, 0, label, 0).unwrap();

    let plaintext = vec![9u8, 8, 7, 6, 5, 4, 3, 2, 1, 0];
    let aad: Vec<u8> = Vec::new();
    let ct = enc.encrypt(&plaintext, &aad).unwrap().to_vec();
    assert!(ct.len() > plaintext.len(), "ciphertext carries header + tag");

    let r = bob_session.decrypt(&ct, &aad).unwrap();
    assert_eq!(bytes_field(&r, "plaintext"), plaintext);
    assert_eq!(num_field(&r, "senderLeaf") as u32, 0);
}

#[wasm_bindgen_test]
fn sframe_wrong_aad_fails() {
    let (alice_mls, _an, _ga, bob_mls, _bn, _gb) = two_member_group();
    let label = "gbp/sframe v1";
    let bob_session = SFrameSession::create(&bob_mls, label, 0).unwrap();
    let enc = SFrameSession::create(&alice_mls, label, 0)
        .unwrap()
        .create_encryptor(&alice_mls, 0, label, 0)
        .unwrap();

    let ct = enc.encrypt(&[1, 2, 3], b"aad-A").unwrap().to_vec();
    // Decrypt with a different AAD → authentication failure.
    assert!(bob_session.decrypt(&ct, b"aad-B").is_err());
}

#[wasm_bindgen_test]
fn sframe_full_audio_pipeline() {
    // End-to-end: encrypt opus → GAP send → onWire → GAP accept → SFrame decrypt.
    let (alice_mls, alice_node, _ga, bob_mls, bob_node, _gb) = two_member_group();
    let label = "gbp/sframe v1";
    let gap_alice = GapClient::create();
    let gap_bob   = GapClient::create();
    let bob_session = SFrameSession::create(&bob_mls, label, 0).unwrap();
    let enc = SFrameSession::create(&alice_mls, label, 0)
        .unwrap()
        .create_encryptor(&alice_mls, 0, label, 0)
        .unwrap();

    let opus = vec![0x10u8, 0x20, 0x30, 0x40];
    let sframe = enc.encrypt(&opus, &[]).unwrap().to_vec();

    // Ship the SFrame ciphertext as the GAP payload.
    let frame = gap_alice.send(&alice_node, &alice_mls, 2, 3, 4_000, &sframe, Some(2));
    let wire  = wire_of(&frame);
    let evs   = audio_events(&bob_node.on_wire(&bob_mls, &wire));
    assert_eq!(evs.len(), 1);
    let r = gap_bob.accept(&plaintext_of(&evs[0]), bob_mls.epoch(), Some(num_field(&evs[0], "codec") as u8));
    let recovered_sframe = bytes_field(&r, "opus");

    let dec = bob_session.decrypt(&recovered_sframe, &[]).unwrap();
    assert_eq!(bytes_field(&dec, "plaintext"), opus);
}

// ─── Two-member group (text) ───────────────────────────────────────────────────

#[wasm_bindgen_test]
fn two_member_gtp_alice_to_bob() {
    let (alice_mls, alice_node, gtp_alice, bob_mls, bob_node, gtp_bob) = two_member_group();

    let frame = gtp_alice.send(&alice_node, &alice_mls, 2, 1, "hello bob", None);
    let wire  = wire_of(&frame);

    let evs = text_events(&bob_node.on_wire(&bob_mls, &wire));
    assert_eq!(evs.len(), 1);
    let r = gtp_bob.accept(&plaintext_of(&evs[0]), bob_mls.epoch(), None);
    assert_eq!(str_field(&r, "text"), "hello bob");
    assert_eq!(str_field(&r, "status"), "new");
}

#[wasm_bindgen_test]
fn two_member_gtp_bidirectional() {
    let (alice_mls, alice_node, gtp_alice, bob_mls, bob_node, gtp_bob) = two_member_group();

    let wire_ab = wire_of(&gtp_alice.send(&alice_node, &alice_mls, 2, 1, "ping", None));
    for ev in text_events(&bob_node.on_wire(&bob_mls, &wire_ab)) {
        assert_eq!(str_field(&gtp_bob.accept(&plaintext_of(&ev), bob_mls.epoch(), None), "text"), "ping");
    }

    let wire_ba = wire_of(&gtp_bob.send(&bob_node, &bob_mls, 1, 1, "pong", None));
    for ev in text_events(&alice_node.on_wire(&alice_mls, &wire_ba)) {
        assert_eq!(str_field(&gtp_alice.accept(&plaintext_of(&ev), alice_mls.epoch(), None), "text"), "pong");
    }
}

#[wasm_bindgen_test]
fn two_member_epochs_match() {
    let (alice_mls, _an, _ga, bob_mls, _bn, _gb) = two_member_group();
    assert_eq!(alice_mls.epoch(), bob_mls.epoch());
    assert_eq!(alice_mls.group_id().to_vec(), bob_mls.group_id().to_vec());
}
