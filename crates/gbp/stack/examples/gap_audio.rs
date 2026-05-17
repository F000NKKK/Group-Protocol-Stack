//! Two-party GAP (audio) frame exchange — minimal in-memory end-to-end example.
//!
//! Demonstrates:
//! - Sending synthetic Opus frames with FlatBuffers codec (recommended for audio)
//! - Per-source replay-window protection: late frames return GapAccept::Late
//!
//! Run with: cargo run --example gap_audio -p gbp-stack

use gbp_stack::{
    ControlOpcode, Event, GapAccept, GapClient, GroupNode, MlsContext, PayloadCodec, StreamType,
};
use openmls::prelude::{
    DeserializeBytes as _, KeyPackageIn, OpenMlsProvider as _, ProtocolVersion,
    tls_codec::Serialize as _,
};

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

    // --- GAP clients -------------------------------------------------
    let mut gap_alice = GapClient::new();
    let mut gap_bob   = GapClient::new();

    // Synthetic 20 ms Opus frame (zeroed; real usage: encode from PCM).
    let opus = vec![0u8; 40];

    // FlatBuffers codec minimises decode latency on real-time audio paths.
    let frame = gap_alice.send(
        &mut alice, &mut alice_mls,
        /*target*/          2,
        /*media_source_id*/ 1,
        /*rtp_timestamp*/   0,
        opus.clone(),
        PayloadCodec::FlatBuffers,
    )?;

    for ev in bob.on_wire(&mut bob_mls, &frame.wire)? {
        if let Event::PayloadReceived(p) = ev {
            if p.stream_type == StreamType::Audio {
                match gap_bob.accept(&p.plaintext, bob_mls.epoch(), p.codec)? {
                    GapAccept::New(pl) => println!(
                        "new audio frame: {} bytes  seq={}  codec={:?}",
                        pl.opus_frame.len(), pl.rtp_sequence, p.codec,
                    ),
                    GapAccept::Late(pl) => println!("late frame: seq={}", pl.rtp_sequence),
                }
            }
        }
    }

    // Second frame — rtp_sequence advances automatically inside GapClient.
    let frame2 = gap_alice.send(
        &mut alice, &mut alice_mls, 2, 1, 960, opus, PayloadCodec::FlatBuffers,
    )?;
    for ev in bob.on_wire(&mut bob_mls, &frame2.wire)? {
        if let Event::PayloadReceived(p) = ev {
            if p.stream_type == StreamType::Audio {
                match gap_bob.accept(&p.plaintext, bob_mls.epoch(), p.codec)? {
                    GapAccept::New(pl)  => println!("frame 2: seq={}", pl.rtp_sequence),
                    GapAccept::Late(pl) => println!("late:    seq={}", pl.rtp_sequence),
                }
            }
        }
    }

    Ok(())
}
