//! Two-party GTP (text) chat — minimal in-memory end-to-end example.
//!
//! Demonstrates:
//! - MLS two-party handshake
//! - GBP node bootstrap + epoch transition
//! - Sending text messages with CBOR and FlatBuffers codecs
//! - Idempotency: duplicate (sender, message_id) returns GtpAccept::Duplicate
//!
//! Run with: cargo run --example gtp_chat -p gbp-stack

use gbp_stack::{
    ControlOpcode, Event, GtpAccept, GtpClient, GroupNode, MlsContext, PayloadCodec, StreamType,
};
use openmls::prelude::{
    DeserializeBytes as _, KeyPackageIn, OpenMlsProvider as _, ProtocolVersion,
    tls_codec::Serialize as _,
};

fn main() -> anyhow::Result<()> {
    // --- MLS handshake -----------------------------------------------
    let (mut alice_mls, _) = MlsContext::new_member(b"alice")?;
    let (mut bob_mls, bob_kp_bundle) = MlsContext::new_member(b"bob")?;

    // Validate Bob's key package through Alice's crypto provider (openmls requirement).
    let bob_kp_bytes = bob_kp_bundle.key_package().tls_serialize_detached()?;
    let bob_kp = KeyPackageIn::tls_deserialize_exact_bytes(&bob_kp_bytes)?
        .validate(alice_mls.provider.crypto(), ProtocolVersion::Mls10)?;

    let (_commit, welcome) = alice_mls.invite_full(&[bob_kp])?;
    bob_mls.accept_welcome(&welcome)?;
    alice_mls.finalize_pending_commit()?;
    assert_eq!(alice_mls.epoch(), bob_mls.epoch());

    // --- GBP nodes ---------------------------------------------------
    let gid = alice_mls.group_id_16();
    let mut alice = GroupNode::new(1, gid);
    let mut bob   = GroupNode::new(2, gid);
    alice.bootstrap_as_creator(0);
    bob.bootstrap_as_joiner(0, 1);

    // Apply epoch-1 transition so both nodes are live.
    let exec = alice.send_control(
        &mut alice_mls, 0, ControlOpcode::ExecuteTransition, 1, 7, vec![],
    )?;
    alice.apply_transition(1);
    bob.on_wire(&mut bob_mls, &exec.wire)?;

    // --- GTP clients -------------------------------------------------
    let mut gtp_alice = GtpClient::new();
    let mut gtp_bob   = GtpClient::new();

    // Send "hello" with default CBOR codec.
    let frame = gtp_alice.send(&mut alice, &mut alice_mls, 2, 1, "hello", PayloadCodec::Cbor)?;
    for ev in bob.on_wire(&mut bob_mls, &frame.wire)? {
        if let Event::PayloadReceived(p) = ev {
            if p.stream_type == StreamType::Text {
                match gtp_bob.accept(&p.plaintext, bob_mls.epoch(), p.codec)? {
                    GtpAccept::New(msg)       => println!("new:       {:?}", msg.text()),
                    GtpAccept::Duplicate(msg) => println!("duplicate: {:?}", msg.text()),
                }
            }
        }
    }

    // Send again with FlatBuffers codec (lower decode overhead).
    let frame2 = gtp_alice.send(
        &mut alice, &mut alice_mls, 2, 2, "hello flatbuffers", PayloadCodec::FlatBuffers,
    )?;
    for ev in bob.on_wire(&mut bob_mls, &frame2.wire)? {
        if let Event::PayloadReceived(p) = ev {
            if p.stream_type == StreamType::Text {
                match gtp_bob.accept(&p.plaintext, bob_mls.epoch(), p.codec)? {
                    GtpAccept::New(msg) => println!("new (fbs): {:?}  codec={:?}", msg.text(), p.codec),
                    GtpAccept::Duplicate(msg) => println!("dup (fbs): {:?}", msg.text()),
                }
            }
        }
    }

    // Replay: resend message_id=1 — must come back as Duplicate.
    let dup = gtp_alice.send(&mut alice, &mut alice_mls, 2, 1, "hello", PayloadCodec::Cbor)?;
    for ev in bob.on_wire(&mut bob_mls, &dup.wire)? {
        if let Event::PayloadReceived(p) = ev {
            if p.stream_type == StreamType::Text {
                match gtp_bob.accept(&p.plaintext, bob_mls.epoch(), p.codec)? {
                    GtpAccept::New(_)       => println!("ERROR: expected duplicate"),
                    GtpAccept::Duplicate(_) => println!("replay correctly rejected as duplicate"),
                }
            }
        }
    }

    Ok(())
}
