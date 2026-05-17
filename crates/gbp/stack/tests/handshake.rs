//! End-to-end control-plane handshake against real MLS state.
//!
//! Covers the contract documented in `docs/en/gbp-mls-binding.md`:
//!
//! * `invite_full` returns `(commit, welcome)` and stages a pending commit
//!   without advancing the local epoch.
//! * `process_message` on existing members applies the commit and advances
//!   their MLS epoch.
//! * `accept_welcome` on the joiner places them on the post-commit epoch.
//! * After `finalize_pending_commit` on the inviter and `apply_transition`
//!   on every GBP node, the group converges on a single `(epoch, tid)`.
//!
//! These tests exercise the wrappers without going through the FFI/JSON
//! event surface — they verify the underlying MLS+GBP contract.

use gbp_stack::{
    ControlOpcode, GbpFlags, GroupNode, MlsContext, NodeState, PayloadCodec, ProcessedKind,
    StreamType, label_for,
};

use openmls::prelude::DeserializeBytes as _;
use openmls::prelude::{KeyPackage, KeyPackageIn, ProtocolVersion};
use openmls_traits::OpenMlsProvider as _;

fn validated_kp(ctx: &MlsContext, raw: &[u8]) -> KeyPackage {
    let kp_in = KeyPackageIn::tls_deserialize_exact_bytes(raw).expect("kp parse");
    kp_in
        .validate(ctx.provider.crypto(), ProtocolVersion::Mls10)
        .expect("kp validate")
}

#[test]
fn two_party_add_completes_full_handshake() {
    // Alice creates the group; Bob will join.
    let (mut alice, _alice_kp) = MlsContext::new_member(b"alice").unwrap();
    let (mut bob, bob_kp_bundle) = MlsContext::new_member(b"bob").unwrap();
    let bob_kp_bytes =
        openmls::prelude::tls_codec::Serialize::tls_serialize_detached(bob_kp_bundle.key_package())
            .unwrap();

    // 1. invite_full produces both messages, stages but does NOT merge.
    let validated = validated_kp(&alice, &bob_kp_bytes);
    let (commit_bytes, welcome_bytes) = alice.invite_full(&[validated]).unwrap();
    assert_eq!(alice.epoch(), 0, "invite_full must NOT advance epoch");
    assert!(!commit_bytes.is_empty());
    assert!(!welcome_bytes.is_empty());

    // 2. Bob accepts welcome — his MLS epoch advances to 1.
    bob.accept_welcome(&welcome_bytes).unwrap();
    assert_eq!(bob.epoch(), 1);
    assert_eq!(bob.group_id_16(), alice.group_id_16());

    // 3. Alice finalizes after distribution → her MLS epoch advances to 1.
    alice.finalize_pending_commit().unwrap();
    assert_eq!(alice.epoch(), 1);

    // 4. GBP nodes: Alice creator, Bob joiner pre-armed for tid=1.
    let mut a_node = GroupNode::new(1, alice.group_id_16());
    let mut b_node = GroupNode::new(2, bob.group_id_16());
    a_node.bootstrap_as_creator(0);
    b_node.bootstrap_as_joiner(0, 1);
    assert_eq!(b_node.pending_transition_id, 1);

    // 5. Alice broadcasts EXECUTE, both apply.
    let exec = a_node
        .send_control(
            &mut alice,
            0,
            ControlOpcode::ExecuteTransition,
            1,
            7,
            vec![],
        )
        .unwrap();
    a_node.apply_transition(1);
    let evs = b_node.on_wire(&mut bob, &exec.wire).unwrap();
    let errs: Vec<u16> = evs
        .iter()
        .filter_map(|e| match e {
            gbp_stack::Event::Error { code, .. } => Some(*code),
            _ => None,
        })
        .collect();
    assert!(
        errs.is_empty(),
        "got errors during EXECUTE delivery: {errs:?}"
    );
    assert_eq!(a_node.last_transition_id, 1);
    assert_eq!(b_node.last_transition_id, 1);
    assert_eq!(a_node.current_epoch, 1);
    assert_eq!(b_node.current_epoch, 1);
    assert_eq!(a_node.state, NodeState::Active);
    assert_eq!(b_node.state, NodeState::Active);

    // 6. After convergence, an application-stream frame round-trips.
    let sid = a_node.member_stream_id(2);
    let msg = a_node
        .send_payload(
            &mut alice,
            2,
            StreamType::Text,
            sid,
            GbpFlags::ordered_reliable_ack(),
            b"hi bob",
            PayloadCodec::Cbor,
        )
        .unwrap();
    let recv = b_node.on_wire(&mut bob, &msg.wire).unwrap();
    let pr = recv
        .into_iter()
        .find_map(|e| match e {
            gbp_stack::Event::PayloadReceived(p) => Some(p),
            _ => None,
        })
        .expect("payload");
    assert_eq!(pr.plaintext, b"hi bob");
}

#[test]
fn abort_rolls_back_pending_commit() {
    let (mut alice, _) = MlsContext::new_member(b"alice").unwrap();
    let (_bob, bob_kp_bundle) = MlsContext::new_member(b"bob").unwrap();
    let bob_kp_bytes =
        openmls::prelude::tls_codec::Serialize::tls_serialize_detached(bob_kp_bundle.key_package())
            .unwrap();
    let validated = validated_kp(&alice, &bob_kp_bytes);
    let _ = alice.invite_full(&[validated]).unwrap();
    assert_eq!(alice.epoch(), 0);
    alice.clear_pending_commit().unwrap();
    assert_eq!(alice.epoch(), 0, "epoch must stay at 0 after abort");
}

#[test]
fn process_message_on_existing_member_advances_epoch() {
    // Three-way: alice (creator), bob (existing member), carol (new joiner).
    let (mut alice, _) = MlsContext::new_member(b"alice").unwrap();
    let (mut bob, bob_kp_bundle) = MlsContext::new_member(b"bob").unwrap();
    let bob_kp_bytes =
        openmls::prelude::tls_codec::Serialize::tls_serialize_detached(bob_kp_bundle.key_package())
            .unwrap();

    // First invite: alice adds bob.
    let v_bob = validated_kp(&alice, &bob_kp_bytes);
    let (_commit1, welcome_b) = alice.invite_full(&[v_bob]).unwrap();
    bob.accept_welcome(&welcome_b).unwrap();
    alice.finalize_pending_commit().unwrap();
    assert_eq!(alice.epoch(), 1);
    assert_eq!(bob.epoch(), 1);

    // Second invite: alice adds carol; bob must apply commit2 to keep up.
    let (_carol, carol_kp_bundle) = MlsContext::new_member(b"carol").unwrap();
    let carol_kp_bytes = openmls::prelude::tls_codec::Serialize::tls_serialize_detached(
        carol_kp_bundle.key_package(),
    )
    .unwrap();
    let v_carol = validated_kp(&alice, &carol_kp_bytes);
    let (commit2, _welcome_c) = alice.invite_full(&[v_carol]).unwrap();

    // Bob, an existing member, stages the commit. Per the deferred-merge
    // contract his MLS epoch does NOT advance until finalize_pending_commit
    // (which the demo client invokes on EXECUTE_TRANSITION).
    assert_eq!(bob.epoch(), 1);
    let kind = bob.process_message(&commit2).unwrap();
    assert_eq!(kind, ProcessedKind::Commit);
    assert_eq!(bob.epoch(), 1, "staged but not merged");

    bob.finalize_pending_commit().unwrap();
    assert_eq!(bob.epoch(), 2, "finalize merges the staged commit");

    alice.finalize_pending_commit().unwrap();
    assert_eq!(alice.epoch(), 2);
}

#[test]
fn aead_round_trips_under_label() {
    let (alice, _) = MlsContext::new_member(b"alice").unwrap();
    let label = label_for(StreamType::Text);
    let pt = b"the quick brown fox";
    let ct = alice.seal(label, 1, pt).unwrap();
    let dec = alice.open(label, 1, &ct).unwrap();
    assert_eq!(dec, pt);
    // Wrong sequence number must fail.
    assert!(alice.open(label, 2, &ct).is_err());
}
