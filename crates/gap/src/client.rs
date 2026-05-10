//! Stateful GAP client.

use crate::GapPayload;
use gbp::CodecError;
use gbp_core::{GbpFlags, MemberId, StreamType};
use gbp_node::{GroupNode, NodeError, OutboundFrame, Sealer};
use std::collections::HashMap;

/// Errors returned by [`GapClient`].
#[derive(Debug, thiserror::Error)]
pub enum GapError {
    /// Failed to decode the CBOR payload.
    #[error("decode: {0}")]
    Decode(#[from] CodecError),
    /// `key_phase` does not match the current group epoch (GAP §10).
    #[error("epoch stale: kp={kp}, expected={expected}")]
    EpochStale {
        /// Reported `key_phase`.
        kp: u32,
        /// Expected `key_phase` (current epoch).
        expected: u32,
    },
    /// `rtp_sequence` was already seen for the same `media_source_id`.
    #[error("rtp replay: src={src}, seq={seq}, hw={hw}")]
    RtpReplay {
        /// `media_source_id`.
        src: u32,
        /// Reported `rtp_sequence`.
        seq: u32,
        /// Replay-window high-water mark.
        hw: u32,
    },
    /// Underlying GBP node error during send.
    #[error("node: {0}")]
    Node(#[from] NodeError),
}

/// Outcome of accepting a GAP payload.
#[derive(Debug)]
pub enum GapAccept {
    /// New audio frame.
    New(GapPayload),
    /// Late audio frame (`rtp_sequence` <= last seen). MAY be dropped per
    /// GAP §7.
    Late(GapPayload),
}

/// Stateful GAP client.
///
/// Maintains an outbound `rtp_sequence` counter and an inbound replay window,
/// both keyed by `media_source_id`.
///
/// The client observes the current group epoch on every [`GapClient::send`]
/// or [`GapClient::accept`] call and automatically clears its replay window
/// when the epoch advances. Callers may also drive a reset explicitly via
/// [`GapClient::reset`].
#[derive(Default)]
pub struct GapClient {
    out_rtp_seq: HashMap<u32, u32>,
    in_hw: HashMap<u32, u32>,
    current_epoch: Option<u64>,
}

impl GapClient {
    /// Creates an empty client.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends an Opus frame. `key_phase` is taken from `node.current_epoch`.
    /// Uses the `O` profile (no `R` / `A` — voice is not reliable, GAP §7).
    ///
    /// The wire `rtp_sequence` is clamped to the 16-bit RTP range; on
    /// overflow it wraps from `0xFFFF` back to `0x0000`.
    pub fn send<S: Sealer>(
        &mut self,
        node: &mut GroupNode,
        seal: &mut S,
        target: MemberId,
        media_source_id: u32,
        rtp_timestamp: u64,
        opus: Vec<u8>,
    ) -> Result<OutboundFrame, GapError> {
        self.sync_epoch(node.current_epoch);
        let seq = self.out_rtp_seq.entry(media_source_id).or_insert(0);
        // RTP `sequence_number` is 16 bits — clamp every increment.
        *seq = seq.wrapping_add(1) & 0xFFFF;
        let payload = GapPayload {
            media_source_id,
            rtp_sequence: *seq,
            rtp_timestamp,
            key_phase: node.current_epoch as u32,
            opus_frame: serde_bytes::ByteBuf::from(opus),
        };
        let stream_id = node.member_stream_id(2);
        Ok(node.send_payload(
            seal,
            target,
            StreamType::Audio,
            stream_id,
            GbpFlags::ordered_only(),
            &payload.to_cbor(),
        )?)
    }

    /// Accepts a plaintext payload delivered by the GBP layer.
    /// Returns [`GapAccept::New`] for fresh frames, [`GapAccept::Late`] for
    /// replays that the spec allows to drop, or [`GapError::EpochStale`] when
    /// `key_phase` does not match the current epoch.
    pub fn accept(&mut self, plaintext: &[u8], current_epoch: u64) -> Result<GapAccept, GapError> {
        self.sync_epoch(current_epoch);
        let p = GapPayload::from_cbor(plaintext)?;
        if p.key_phase != current_epoch as u32 {
            return Err(GapError::EpochStale { kp: p.key_phase, expected: current_epoch as u32 });
        }
        let hw = self.in_hw.get(&p.media_source_id).copied().unwrap_or(0);
        if p.rtp_sequence <= hw {
            return Ok(GapAccept::Late(p));
        }
        self.in_hw.insert(p.media_source_id, p.rtp_sequence);
        Ok(GapAccept::New(p))
    }

    /// Synchronises the client's view of the group epoch and resets the
    /// outbound counters and replay window when the epoch has advanced.
    /// Called automatically by [`GapClient::send`] and [`GapClient::accept`].
    pub fn sync_epoch(&mut self, epoch: u64) {
        if Some(epoch) != self.current_epoch {
            self.out_rtp_seq.clear();
            self.in_hw.clear();
            self.current_epoch = Some(epoch);
        }
    }

    /// Clears the outbound counters and the replay window unconditionally.
    pub fn reset(&mut self) {
        self.out_rtp_seq.clear();
        self.in_hw.clear();
        self.current_epoch = None;
    }
}
