//! Stateful GAP client.

use crate::GapPayload;
use gbp::CodecError;
use gbp_core::{GbpFlags, MemberId, StreamType, timeouts};
use gbp_node::{GroupNode, NodeError, OutboundFrame, Sealer};
use std::collections::HashMap;
use std::time::{Duration, Instant};

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

/// A snapshot of one old epoch's replay window, kept for `T_GAP_KEY_OVERLAP_MS`
/// so that late in-flight audio frames from that epoch are still accepted.
struct OldEpochWindow {
    epoch: u64,
    in_hw: HashMap<u32, u32>,
    expires: Instant,
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
///
/// Old-epoch windows are retained for [`timeouts::T_GAP_KEY_OVERLAP_MS`]
/// (default 10 s) so that in-flight audio frames from the previous epoch can
/// still be accepted after an epoch transition (gap_rfc §4).
pub struct GapClient {
    out_rtp_seq: HashMap<u32, u32>,
    in_hw: HashMap<u32, u32>,
    current_epoch: Option<u64>,
    /// Old replay windows from previous epochs, retained until T_overlap expires.
    old_windows: Vec<OldEpochWindow>,
}

impl Default for GapClient {
    fn default() -> Self {
        Self {
            out_rtp_seq: HashMap::new(),
            in_hw: HashMap::new(),
            current_epoch: None,
            old_windows: Vec::new(),
        }
    }
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
    ///
    /// Returns [`GapAccept::New`] for fresh frames, [`GapAccept::Late`] for
    /// replays that the spec allows to drop. Returns [`GapError::EpochStale`]
    /// only when `key_phase` refers to an epoch that has already expired its
    /// T_overlap window; frames from epochs still within T_overlap are
    /// accepted normally (gap_rfc §4).
    pub fn accept(&mut self, plaintext: &[u8], current_epoch: u64) -> Result<GapAccept, GapError> {
        self.sync_epoch(current_epoch);
        let p = GapPayload::from_cbor(plaintext)?;
        if p.key_phase == current_epoch as u32 {
            // Fast path: current epoch.
            let hw = self.in_hw.get(&p.media_source_id).copied().unwrap_or(0);
            if p.rtp_sequence <= hw && hw.wrapping_sub(p.rtp_sequence) <= 0x7FFF {
                return Ok(GapAccept::Late(p));
            }
            self.in_hw.insert(p.media_source_id, p.rtp_sequence);
            return Ok(GapAccept::New(p));
        }
        // Slow path: frame from an older epoch — check the overlap buffer.
        let now = Instant::now();
        if let Some(old) = self.old_windows.iter_mut().find(|w| {
            w.epoch == p.key_phase as u64 && w.expires > now
        }) {
            let hw = old.in_hw.get(&p.media_source_id).copied().unwrap_or(0);
            if p.rtp_sequence <= hw && hw.wrapping_sub(p.rtp_sequence) <= 0x7FFF {
                return Ok(GapAccept::Late(p));
            }
            old.in_hw.insert(p.media_source_id, p.rtp_sequence);
            return Ok(GapAccept::New(p));
        }
        Err(GapError::EpochStale { kp: p.key_phase, expected: current_epoch as u32 })
    }

    /// Synchronises the client's view of the group epoch.
    ///
    /// When the epoch advances, the current replay window is moved to the
    /// overlap buffer (retained for `T_GAP_KEY_OVERLAP_MS`) instead of being
    /// discarded, so late in-flight frames from the previous epoch are still
    /// accepted (gap_rfc §4). Expired entries are pruned on each call.
    /// Called automatically by [`GapClient::send`] and [`GapClient::accept`].
    pub fn sync_epoch(&mut self, epoch: u64) {
        // Prune expired old windows.
        let now = Instant::now();
        self.old_windows.retain(|w| w.expires > now);

        if Some(epoch) != self.current_epoch {
            // Save current window to overlap buffer before resetting.
            if let Some(old_epoch) = self.current_epoch {
                if !self.in_hw.is_empty() {
                    self.old_windows.push(OldEpochWindow {
                        epoch: old_epoch,
                        in_hw: std::mem::take(&mut self.in_hw),
                        expires: now + Duration::from_millis(timeouts::T_GAP_KEY_OVERLAP_MS),
                    });
                }
            }
            self.out_rtp_seq.clear();
            self.in_hw.clear();
            self.current_epoch = Some(epoch);
        }
    }

    /// Clears the outbound counters, the replay window, and the overlap buffer
    /// unconditionally.
    pub fn reset(&mut self) {
        self.out_rtp_seq.clear();
        self.in_hw.clear();
        self.old_windows.clear();
        self.current_epoch = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payload(seq: u32, key_phase: u32) -> Vec<u8> {
        crate::GapPayload {
            media_source_id: 1,
            rtp_sequence: seq,
            rtp_timestamp: 960,
            key_phase,
            opus_frame: serde_bytes::ByteBuf::from(b"opus-data".to_vec()),
        }
        .to_cbor()
    }

    #[test]
    fn wraparound_after_ffff_is_accepted() {
        let mut client = GapClient::new();
        // Prime the high-water mark near wraparound point.
        let _ = client.accept(&make_payload(0xFFFE, 1), 1).unwrap();
        let _ = client.accept(&make_payload(0xFFFF, 1), 1).unwrap();
        // After wraparound, seq=0 should be accepted as New, not Late.
        let result = client.accept(&make_payload(0x0000, 1), 1).unwrap();
        assert!(matches!(result, GapAccept::New(_)), "seq=0 after 0xFFFF must be New");
    }

    #[test]
    fn strict_replay_within_window_is_late() {
        let mut client = GapClient::new();
        let _ = client.accept(&make_payload(100, 1), 1).unwrap();
        let result = client.accept(&make_payload(100, 1), 1).unwrap();
        assert!(matches!(result, GapAccept::Late(_)), "exact dup must be Late");
    }

    #[test]
    fn epoch_change_clears_window() {
        let mut client = GapClient::new();
        let _ = client.accept(&make_payload(1, 1), 1).unwrap();
        // Epoch change: seq 1 was seen in epoch 1, but in epoch 2 it's new again.
        let result = client.accept(&make_payload(1, 2), 2).unwrap();
        assert!(matches!(result, GapAccept::New(_)), "new epoch resets window");
    }

    // ---- T_overlap buffer (gap_rfc §4) --------------------------------------

    #[test]
    fn old_epoch_frame_accepted_within_overlap() {
        let mut client = GapClient::new();
        // Establish seq 5 in epoch 1.
        let _ = client.accept(&make_payload(5, 1), 1).unwrap();
        // Advance to epoch 2 — old window is buffered.
        let _ = client.accept(&make_payload(1, 2), 2).unwrap();
        // A late frame from epoch 1 (seq 6, not seen yet) arrives before T_overlap expires.
        let result = client.accept(&make_payload(6, 1), 2).unwrap();
        assert!(matches!(result, GapAccept::New(_)), "late epoch-1 frame accepted within T_overlap");
    }

    #[test]
    fn old_epoch_replay_is_late_within_overlap() {
        let mut client = GapClient::new();
        let _ = client.accept(&make_payload(5, 1), 1).unwrap();
        // Advance epoch.
        let _ = client.accept(&make_payload(1, 2), 2).unwrap();
        // Same seq from epoch 1 arrives again — replay → Late.
        let result = client.accept(&make_payload(5, 1), 2).unwrap();
        assert!(matches!(result, GapAccept::Late(_)), "duplicate from old epoch is Late");
    }

    #[test]
    fn expired_old_epoch_frame_is_stale() {
        let mut client = GapClient::new();
        let _ = client.accept(&make_payload(5, 1), 1).unwrap();
        // Advance epoch.
        let _ = client.accept(&make_payload(1, 2), 2).unwrap();
        // Manually expire the overlap window.
        for w in &mut client.old_windows {
            w.expires = Instant::now() - Duration::from_millis(1);
        }
        // Now a late epoch-1 frame should be rejected.
        let result = client.accept(&make_payload(6, 1), 2);
        assert!(matches!(result, Err(GapError::EpochStale { .. })), "expired epoch is Stale");
    }

    #[test]
    fn reset_clears_overlap_buffer() {
        let mut client = GapClient::new();
        let _ = client.accept(&make_payload(1, 1), 1).unwrap();
        let _ = client.accept(&make_payload(1, 2), 2).unwrap();
        assert!(!client.old_windows.is_empty(), "overlap buffer populated");
        client.reset();
        assert!(client.old_windows.is_empty(), "overlap buffer cleared after reset");
    }
}
