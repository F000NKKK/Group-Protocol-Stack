//! `gbp-node` — reference CLI for the Group Protocol Stack.
//!
//! Commands:
//! * `gbp-node listen [addr]` — listener: accepts one peer, performs the MLS
//!   handshake and reads a single GTP message.
//! * `gbp-node connect <addr>` — connector: completes the MLS handshake and
//!   sends a single GTP message.
//!
//! This is an integration smoke-runner for library developers. Application
//! demos live in separate consumers (e.g. the WPF messenger) and use the
//! C ABI exposed by the `gbp-stack-ffi` crate.

use gbp_stack::{
    GbpFlags, GbpFrame, GtpMessage, MlsContext, StreamLabel, StreamType, label_for, read_blob,
    read_frame, write_blob, write_frame,
};
use openmls::prelude::tls_codec::Serialize as _;
use openmls::prelude::*;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mode = std::env::args().nth(1).unwrap_or_else(|| "listen".into());
    let addr = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:7878".into());

    match mode.as_str() {
        "listen" => run_listener(&addr).await?,
        "connect" => run_connector(&addr).await?,
        other => anyhow::bail!("unknown mode {other}; use listen|connect"),
    }
    Ok(())
}

async fn run_listener(addr: &str) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(?addr, "listener up");
    let (mut sock, peer) = listener.accept().await?;
    tracing::info!(?peer, "peer connected");

    let (mut bob, bob_kp_bundle) =
        MlsContext::new_member(b"bob").map_err(|e| anyhow::anyhow!("mls init: {e}"))?;
    let kp_bytes = bob_kp_bundle.key_package().tls_serialize_detached()?;
    write_blob(&mut sock, &kp_bytes).await?;

    let welcome = read_blob(&mut sock).await?;
    bob.accept_welcome(&welcome)
        .map_err(|e| anyhow::anyhow!("accept welcome: {e}"))?;
    tracing::info!(epoch = bob.epoch(), "joined group");

    let frame = read_frame(&mut sock).await?;
    let st = frame.stream_type_typed()?;
    let pt = bob
        .open(label_for(st), frame.sequence_no, &frame.encrypted_payload)
        .map_err(|e| anyhow::anyhow!("aead open: {e}"))?;
    let msg = GtpMessage::from_cbor(&pt)?;
    tracing::info!(
        sender = msg.sender_id,
        text = %String::from_utf8_lossy(&msg.content),
        "received GTP"
    );
    Ok(())
}

async fn run_connector(addr: &str) -> anyhow::Result<()> {
    let mut sock = TcpStream::connect(addr).await?;
    let (mut alice, _kp) =
        MlsContext::new_member(b"alice").map_err(|e| anyhow::anyhow!("mls init: {e}"))?;

    let kp_bytes = read_blob(&mut sock).await?;
    let bob_kp = KeyPackageIn::tls_deserialize_exact_bytes(&kp_bytes)?
        .validate(alice.provider.crypto(), ProtocolVersion::Mls10)
        .map_err(|e| anyhow::anyhow!("bob kp validate: {e:?}"))?;

    let (commit, welcome) = alice
        .invite_full(&[bob_kp])
        .map_err(|e| anyhow::anyhow!("invite: {e}"))?;
    // In a real app the Commit MUST be broadcast to all existing members
    // BEFORE calling finalize_pending_commit. Here there are no other members,
    // so we finalize immediately (simulating the coordinator path).
    alice
        .finalize_pending_commit()
        .map_err(|e| anyhow::anyhow!("finalize: {e}"))?;
    tracing::info!(commit_len = commit.len(), "commit ready for broadcast");
    write_blob(&mut sock, &welcome).await?;

    let gtp = GtpMessage::plain(1, 0xCAFE_F00D, "hello over real MLS");
    let pt = gtp.to_cbor();
    let seq = 1u32;
    let ct = alice
        .seal(StreamLabel::Text, seq, &pt)
        .map_err(|e| anyhow::anyhow!("aead seal: {e}"))?;
    let frame = GbpFrame::new(
        alice.group_id_16(),
        alice.epoch(),
        0,
        StreamType::Text,
        201,
        GbpFlags::ordered_reliable_ack(),
        seq,
        ct,
        0,
    );
    write_frame(&mut sock, &frame).await?;
    tracing::info!("frame sent");
    Ok(())
}
