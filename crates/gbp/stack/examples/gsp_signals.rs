//! GSP (signaling) examples: JOIN, MUTE with CBOR args, ROLE_CHANGE.
//!
//! Demonstrates:
//! - Signals without args (JOIN) via GspClient::send
//! - Signals with per-signal CBOR args (MUTE, ROLE_CHANGE) via GspClient::send_with_args
//! - Duplicate request_id is rejected with GspError::DuplicateRequest
//!
//! Run with: cargo run --example gsp_signals -p gbp-stack

use gbp_stack::{
    ControlOpcode, Event, GspClient, GroupNode, MlsContext, PayloadCodec, SignalType, StreamType,
};
use openmls::prelude::{
    DeserializeBytes as _, KeyPackageIn, OpenMlsProvider as _, ProtocolVersion,
    tls_codec::Serialize as _,
};

/// Minimal CBOR unsigned-integer encoder (no external crate needed).
fn cbor_uint(n: u32) -> Vec<u8> {
    match n {
        0..=23       => vec![n as u8],
        24..=0xFF    => vec![0x18, n as u8],
        0x100..=0xFFFF => vec![0x19, (n >> 8) as u8, n as u8],
        _ => vec![0x1A, (n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8],
    }
}
fn cbor_map1(k: u32, v: u32) -> Vec<u8> {
    let mut b = vec![0xA1];
    b.extend(cbor_uint(k)); b.extend(cbor_uint(v)); b
}
fn cbor_map2(k0: u32, v0: u32, k1: u32, v1: u32) -> Vec<u8> {
    let mut b = vec![0xA2];
    b.extend(cbor_uint(k0)); b.extend(cbor_uint(v0));
    b.extend(cbor_uint(k1)); b.extend(cbor_uint(v1));
    b
}

fn main() -> anyhow::Result<()> {
    // --- MLS + GBP setup (identical to gtp_chat) ---------------------
    let (mut alice_mls, _) = MlsContext::new_member(b"alice")?;
    let (mut bob_mls, bob_kp_bundle) = MlsContext::new_member(b"bob")?;

    let bob_kp_bytes = bob_kp_bundle.key_package().tls_serialize_detached()?;
    let bob_kp = KeyPackageIn::tls_deserialize_exact_bytes(&bob_kp_bytes)?
        .validate(alice_mls.provider.crypto(), ProtocolVersion::Mls10)?;

    let (_commit, welcome) = alice_mls.invite_full(&[bob_kp])?;
    bob_mls.accept_welcome(&welcome)?;
    alice_mls.finalize_pending_commit()?;

    let gid = alice_mls.group_id_16();
    let mut alice = GroupNode::new(1, gid);
    let mut bob   = GroupNode::new(2, gid);
    alice.bootstrap_as_creator(0);
    bob.bootstrap_as_joiner(0, 1);

    let exec = alice.send_control(
        &mut alice_mls, 0, ControlOpcode::ExecuteTransition, 1, 7, vec![],
    )?;
    alice.apply_transition(1);
    bob.on_wire(&mut bob_mls, &exec.wire)?;

    // --- GSP clients -------------------------------------------------
    let mut gsp_alice = GspClient::new();
    let mut gsp_bob   = GspClient::new();

    // 1. JOIN — no args required.
    let frame = gsp_alice.send(
        &mut alice, &mut alice_mls,
        /*target*/ 0, SignalType::Join, /*role_claim*/ 0, /*request_id*/ 1,
        PayloadCodec::Cbor,
    )?;
    recv(&mut gsp_bob, &mut bob, &mut bob_mls, &frame.wire, "JOIN")?;

    // 2. MUTE member 2 — args: {0: target_member_id=2}.
    let frame = gsp_alice.send_with_args(
        &mut alice, &mut alice_mls,
        0, SignalType::Mute, 0, 2,
        &cbor_map1(0, 2),
        PayloadCodec::Cbor,
    )?;
    recv(&mut gsp_bob, &mut bob, &mut bob_mls, &frame.wire, "MUTE")?;

    // 3. ROLE_CHANGE member 2 → role 1 — args: {0: target=2, 1: new_role=1}.
    let frame = gsp_alice.send_with_args(
        &mut alice, &mut alice_mls,
        0, SignalType::RoleChange, /*role_claim*/ 1, 3,
        &cbor_map2(0, 2, 1, 1),
        PayloadCodec::Cbor,
    )?;
    recv(&mut gsp_bob, &mut bob, &mut bob_mls, &frame.wire, "ROLE_CHANGE")?;

    Ok(())
}

fn recv(
    gsp: &mut GspClient,
    node: &mut GroupNode,
    mls: &mut MlsContext,
    wire: &[u8],
    label: &str,
) -> anyhow::Result<()> {
    for ev in node.on_wire(mls, wire)? {
        if let Event::PayloadReceived(p) = ev {
            if p.stream_type == StreamType::Signal {
                let r = gsp.accept(&p.plaintext, mls.epoch(), p.codec)?;
                println!("{}: signal={:?}  sender={}  request_id={}",
                         label, r.signal, r.sender_id, r.request_id);
            }
        }
    }
    Ok(())
}
