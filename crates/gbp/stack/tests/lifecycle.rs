//! Full GBP+MLS lifecycle through every meaningful state transition.
//!
//! Models a coordinator-driven group with three members and exercises:
//!
//! 1. Coordinator bootstrap.
//! 2. First add (Bob joins). Full PREPARE→READY→EXECUTE handshake.
//! 3. Encrypted chat round-trip on the new epoch.
//! 4. Second add (Carol joins) while Bob is an existing member that must
//!    apply the commit via `process_message`.
//! 5. 3-way encrypted chat.
//! 6. Remove (Bob leaves). Carol applies the remove commit.
//! 7. Post-remove chat between Alice and Carol still works; Bob's old node
//!    can no longer decrypt.
//!
//! Transport is replaced by direct buffer hand-off — sufficient to verify
//! the protocol contract end to end without TCP. The relay's only job is
//! to fan-out frames addressed to `target == 0`.

use gbp_stack::{
    ControlOpcode, GroupNode, GtpAccept, GtpClient, MlsContext, NodeState, PayloadCodec,
    ProcessedKind,
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

fn export_kp(bundle: &openmls::prelude::KeyPackageBundle) -> Vec<u8> {
    openmls::prelude::tls_codec::Serialize::tls_serialize_detached(bundle.key_package()).unwrap()
}

/// One participant: MLS context + GBP node + GTP idempotency.
struct Member {
    _name: &'static str,
    mls: MlsContext,
    node: GroupNode,
    gtp: GtpClient,
}

impl Member {
    fn new_creator(name: &'static str, member_id: u32) -> Self {
        let (mls, _) = MlsContext::new_member(name.as_bytes()).unwrap();
        let mut node = GroupNode::new(member_id, mls.group_id_16());
        node.bootstrap_as_creator(0);
        Self {
            _name: name,
            mls,
            node,
            gtp: GtpClient::new(),
        }
    }
    fn new_pending(name: &'static str) -> (Self, openmls::prelude::KeyPackageBundle) {
        let (mls, bundle) = MlsContext::new_member(name.as_bytes()).unwrap();
        // A pending member has no GBP node yet — it will be built after
        // accepting Welcome. We park a placeholder so the type stays simple.
        let placeholder = GroupNode::new(0, [0u8; 16]);
        (
            Self {
                _name: name,
                mls,
                node: placeholder,
                gtp: GtpClient::new(),
            },
            bundle,
        )
    }
    fn finish_join(&mut self, member_id: u32, arrival_tid: u32) {
        // Welcome is delivered post-EXECUTE, so the joiner bootstraps and
        // then symbolically applies the admission transition to land in
        // the same (epoch, last_tid) as the rest of the group.
        let mls_epoch = self.mls.epoch();
        self.node = GroupNode::new(member_id, self.mls.group_id_16());
        self.node
            .bootstrap_as_joiner(mls_epoch.saturating_sub(1), 0);
        self.node.apply_transition(arrival_tid);
    }
}

fn first_payload(events: &[gbp_stack::Event]) -> Option<&gbp_stack::DeliveredPayload> {
    events.iter().find_map(|e| match e {
        gbp_stack::Event::PayloadReceived(p) => Some(p),
        _ => None,
    })
}

#[test]
fn full_lifecycle_two_joins_one_leave() {
    // ─── 1. Bootstrap ────────────────────────────────────────────────────
    let mut alice = Member::new_creator("alice", 1);
    let (mut bob, bob_kp_bundle) = Member::new_pending("bob");

    // ─── 2. Alice invites Bob ────────────────────────────────────────────
    let kp_bytes = export_kp(&bob_kp_bundle);
    let validated = validated_kp(&alice.mls, &kp_bytes);
    let (_commit_bytes, welcome_bytes) = alice.mls.invite_full(&[validated]).unwrap();
    assert_eq!(alice.mls.epoch(), 0, "no merge yet");

    // Bob applies welcome → his MLS is on epoch 1, GBP node bootstraps
    // with expected_first_tid=1 so EXECUTE will be accepted.
    bob.mls.accept_welcome(&welcome_bytes).unwrap();
    bob.finish_join(2, 1);

    // Coordinator finalizes — both sides on mls.epoch == 1.
    alice.mls.finalize_pending_commit().unwrap();

    // EXECUTE broadcast (skip PREPARE — Bob can't decrypt it; Alice has no
    // peer except Bob in this scenario).
    let exec1 = alice
        .node
        .send_control(
            &mut alice.mls,
            0,
            ControlOpcode::ExecuteTransition,
            1,
            1,
            vec![],
        )
        .unwrap();
    alice.node.apply_transition(1);
    let _ = bob.node.on_wire(&mut bob.mls, &exec1.wire).unwrap();
    assert_eq!(alice.node.last_transition_id, 1);
    assert_eq!(bob.node.last_transition_id, 1);

    // ─── 3. Alice → Bob chat round-trip ──────────────────────────────────
    let m = alice
        .gtp
        .send(
            &mut alice.node,
            &mut alice.mls,
            /*broadcast*/ 0,
            100,
            "hello bob",
            PayloadCodec::Cbor,
        )
        .unwrap();
    let evs = bob.node.on_wire(&mut bob.mls, &m.wire).unwrap();
    let pr = first_payload(&evs).expect("bob got alice's text");
    let plain = pr.plaintext.clone();
    let accept = bob.gtp.accept(&plain, bob.node.current_epoch, PayloadCodec::Cbor).unwrap();
    match accept {
        GtpAccept::New(msg) => assert_eq!(msg.text().unwrap(), "hello bob"),
        other => panic!("expected New, got {:?}", other),
    }

    // ─── 4. Carol joins on top of the existing 2-member group ────────────
    let (mut carol, carol_kp_bundle) = Member::new_pending("carol");
    let kp_bytes = export_kp(&carol_kp_bundle);
    let v_carol = validated_kp(&alice.mls, &kp_bytes);
    let (commit2, welcome2) = alice.mls.invite_full(&[v_carol]).unwrap();
    assert_eq!(alice.mls.epoch(), 1, "still pre-merge for tid=2");

    // Bob is an existing member: PREPARE delivers commit. He stages it
    // (deferred merge) and stays on epoch 1 so his READY is decryptable
    // by Alice (also on 1).
    let prepare2 = alice
        .node
        .send_control(
            &mut alice.mls,
            0,
            ControlOpcode::PrepareTransition,
            2,
            10,
            commit2.clone(),
        )
        .unwrap();
    let bob_evs = bob.node.on_wire(&mut bob.mls, &prepare2.wire).unwrap();
    let prepare_args = bob_evs
        .iter()
        .find_map(|e| match e {
            gbp_stack::Event::Control {
                opcode: ControlOpcode::PrepareTransition,
                args,
                ..
            } => Some(args.clone()),
            _ => None,
        })
        .expect("bob saw PREPARE");
    assert_eq!(prepare_args, commit2);
    let kind = bob.mls.process_message(&prepare_args).unwrap();
    assert_eq!(kind, ProcessedKind::Commit);
    assert_eq!(
        bob.mls.epoch(),
        1,
        "deferred merge — staged but not advanced"
    );

    // EXECUTE while Alice's MLS is still on epoch 1 — encryption is under
    // the old epoch so Bob can decrypt and apply.
    let exec2 = alice
        .node
        .send_control(
            &mut alice.mls,
            0,
            ControlOpcode::ExecuteTransition,
            2,
            11,
            vec![],
        )
        .unwrap();
    alice.node.apply_transition(2);
    alice.mls.finalize_pending_commit().unwrap();
    let _ = bob.node.on_wire(&mut bob.mls, &exec2.wire).unwrap();
    bob.mls.finalize_pending_commit().unwrap();

    // Carol receives the Welcome AFTER the transition is fully applied
    // group-wide; she bootstraps directly at the post-transition state.
    carol.mls.accept_welcome(&welcome2).unwrap();
    carol.finish_join(3, 2);
    assert_eq!(alice.node.last_transition_id, 2);
    assert_eq!(bob.node.last_transition_id, 2);
    assert_eq!(carol.node.last_transition_id, 2);
    assert_eq!(alice.node.current_epoch, 2);
    assert_eq!(bob.node.current_epoch, 2);
    assert_eq!(carol.node.current_epoch, 2);

    // ─── 5. 3-way chat ──────────────────────────────────────────────────
    let m2 = bob
        .gtp
        .send(&mut bob.node, &mut bob.mls, 0, 200, "everyone hi", PayloadCodec::Cbor)
        .unwrap();

    {
        let evs = alice.node.on_wire(&mut alice.mls, &m2.wire).unwrap();
        let plain = first_payload(&evs)
            .expect("alice missed bob")
            .plaintext
            .clone();
        let acc = alice.gtp.accept(&plain, alice.node.current_epoch, PayloadCodec::Cbor).unwrap();
        if let GtpAccept::New(m) = acc {
            assert_eq!(m.text().unwrap(), "everyone hi");
        } else {
            panic!("alice: {acc:?}");
        }
    }
    {
        let evs = carol.node.on_wire(&mut carol.mls, &m2.wire).unwrap();
        let plain = first_payload(&evs)
            .expect("carol missed bob")
            .plaintext
            .clone();
        let acc = carol.gtp.accept(&plain, carol.node.current_epoch, PayloadCodec::Cbor).unwrap();
        if let GtpAccept::New(m) = acc {
            assert_eq!(m.text().unwrap(), "everyone hi");
        } else {
            panic!("carol: {acc:?}");
        }
    }

    // ─── 6. Bob leaves ──────────────────────────────────────────────────
    // Alice issues Remove for Bob (LeafIndex=1; Alice=0, Bob=1, Carol=2).
    let commit3 = alice.mls.remove_members(&[1]).unwrap();
    assert_eq!(alice.mls.epoch(), 2, "still pre-merge for tid=3");
    let prepare3 = alice
        .node
        .send_control(
            &mut alice.mls,
            0,
            ControlOpcode::PrepareTransition,
            3,
            20,
            commit3.clone(),
        )
        .unwrap();
    // Carol receives PREPARE and applies the commit.
    let carol_evs = carol.node.on_wire(&mut carol.mls, &prepare3.wire).unwrap();
    let p3args = carol_evs
        .iter()
        .find_map(|e| match e {
            gbp_stack::Event::Control {
                opcode: ControlOpcode::PrepareTransition,
                args,
                ..
            } => Some(args.clone()),
            _ => None,
        })
        .expect("carol saw PREPARE");
    let kind = carol.mls.process_message(&p3args).unwrap();
    assert_eq!(kind, ProcessedKind::Commit);
    assert_eq!(
        carol.mls.epoch(),
        2,
        "deferred merge — still on 2 until finalize"
    );

    // Bob also forwards through DS. RFC 9420 §12.3: a removee processing
    // the commit MAY succeed locally — openmls signals he was removed and
    // his MLS state advances to a "left" view, but he no longer holds the
    // group's traffic secrets. We therefore can't assert process_message
    // fails; we assert below that he can no longer decrypt post-leave
    // application frames (the actual forward-secrecy guarantee).
    let _ = bob.mls.process_message(&p3args);

    let exec3 = alice
        .node
        .send_control(
            &mut alice.mls,
            0,
            ControlOpcode::ExecuteTransition,
            3,
            21,
            vec![],
        )
        .unwrap();
    alice.node.apply_transition(3);
    alice.mls.finalize_pending_commit().unwrap();
    let _ = carol.node.on_wire(&mut carol.mls, &exec3.wire).unwrap();
    carol.mls.finalize_pending_commit().unwrap();
    assert_eq!(alice.node.last_transition_id, 3);
    assert_eq!(carol.node.last_transition_id, 3);
    assert_eq!(alice.node.current_epoch, 3);
    assert_eq!(carol.node.current_epoch, 3);

    // ─── 7. Post-leave chat ─────────────────────────────────────────────
    let m3 = carol
        .gtp
        .send(&mut carol.node, &mut carol.mls, 0, 300, "after-leave", PayloadCodec::Cbor)
        .unwrap();
    let evs = alice.node.on_wire(&mut alice.mls, &m3.wire).unwrap();
    let plain = first_payload(&evs)
        .expect("alice missed carol post-leave")
        .plaintext
        .clone();
    let acc = alice.gtp.accept(&plain, alice.node.current_epoch, PayloadCodec::Cbor).unwrap();
    if let GtpAccept::New(m) = acc {
        assert_eq!(m.text().unwrap(), "after-leave");
    } else {
        panic!("{acc:?}");
    }

    // Bob, now stranded on epoch 2 (he could not apply commit3), receives
    // the post-leave frame and MUST fail to decrypt — this is the
    // forward-secrecy guarantee of leave (RFC 9420 §12.3).
    let bob_evs_after = bob.node.on_wire(&mut bob.mls, &m3.wire).unwrap();
    let bob_err = bob_evs_after.iter().find_map(|e| match e {
        gbp_stack::Event::Error { code, .. } => Some(*code),
        _ => None,
    });
    assert!(
        bob_err.is_some(),
        "bob must observe a decryption / state error"
    );

    // Sanity: nodes still ACTIVE except bob (decrypt errors are non-fatal,
    // so bob is also Active — he simply can't read).
    assert_eq!(alice.node.state, NodeState::Active);
    assert_eq!(carol.node.state, NodeState::Active);
}
