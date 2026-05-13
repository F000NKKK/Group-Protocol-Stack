//! Bounded reorder buffer for GAP audio frames.
//!
//! Real-time voice traffic arrives out of order. GAP §7 RECOMMENDS a small
//! jitter buffer at the receiver: hold incoming frames briefly so the
//! decoder consumes them in `rtp_sequence` order, drop frames that arrive
//! after their playout deadline.
//!
//! [`JitterBuffer`] is a tiny, allocation-free reorder window keyed by
//! `media_source_id`. It does not own a clock — the application calls
//! [`JitterBuffer::pop_in_order`] from its render thread to drain the
//! next-expected frame.

use crate::GapPayload;
use std::collections::HashMap;
use std::collections::VecDeque;

/// Per-source state held by [`JitterBuffer`].
struct SourceState {
    /// Frames waiting in the reorder window, sorted by `rtp_sequence` ascending.
    waiting: VecDeque<GapPayload>,
    /// Next sequence number we expect to pop.
    next: Option<u32>,
}

impl SourceState {
    fn new() -> Self {
        Self { waiting: VecDeque::new(), next: None }
    }

    fn insert(&mut self, p: GapPayload, capacity: usize) -> Option<GapPayload> {
        if self.waiting.iter().any(|q| q.rtp_sequence == p.rtp_sequence) {
            return None;
        }
        // O(n) linear scan — n ≤ 16 in practice (Opus frame window), so this
        // is cheaper than a BTreeMap for tiny N.
        let pos = self
            .waiting
            .iter()
            .position(|q| q.rtp_sequence > p.rtp_sequence)
            .unwrap_or(self.waiting.len());
        self.waiting.insert(pos, p);
        if self.waiting.len() > capacity {
            return self.waiting.pop_front();
        }
        None
    }
}

/// Outcome of [`JitterBuffer::push`].
#[derive(Debug)]
pub enum JitterPush {
    /// Frame was buffered.
    Accepted,
    /// Frame is older than the next expected sequence — dropped without
    /// inserting.
    Late,
    /// Buffer was full; the oldest queued frame was evicted to make room
    /// (returned here so the application can still meter / log it).
    Evicted(GapPayload),
}

/// Bounded reorder window for GAP audio frames.
pub struct JitterBuffer {
    capacity_per_source: usize,
    sources: HashMap<u32, SourceState>,
}

impl JitterBuffer {
    /// Builds a jitter buffer that retains up to `capacity_per_source`
    /// frames for each `media_source_id`.
    ///
    /// # Panics
    /// Panics if `capacity_per_source == 0`.
    pub fn new(capacity_per_source: usize) -> Self {
        assert!(capacity_per_source > 0, "capacity must be > 0");
        Self {
            capacity_per_source,
            sources: HashMap::new(),
        }
    }

    /// Inserts a frame into the buffer.
    pub fn push(&mut self, frame: GapPayload) -> JitterPush {
        let state = self
            .sources
            .entry(frame.media_source_id)
            .or_insert_with(SourceState::new);
        if let Some(next) = state.next
            && frame.rtp_sequence < next
        {
            return JitterPush::Late;
        }
        match state.insert(frame, self.capacity_per_source) {
            Some(evicted) => JitterPush::Evicted(evicted),
            None => JitterPush::Accepted,
        }
    }

    /// Pops the next frame for `media_source_id` if it is the next one
    /// expected (i.e. its `rtp_sequence` is contiguous with the previous
    /// pop). Returns `None` if the head of the queue is from the future
    /// (caller should wait or skip via [`JitterBuffer::pop_force`]).
    pub fn pop_in_order(&mut self, media_source_id: u32) -> Option<GapPayload> {
        let state = self.sources.get_mut(&media_source_id)?;
        let head = state.waiting.front()?;
        if let Some(next) = state.next
            && head.rtp_sequence != next
        {
            return None;
        }
        let popped = state.waiting.pop_front()?;
        state.next = Some(popped.rtp_sequence.wrapping_add(1));
        Some(popped)
    }

    /// Forces the next frame out regardless of contiguity. Use this when the
    /// playout deadline is reached and the next-expected packet is still
    /// missing — the caller will play the next available frame and skip the
    /// gap.
    pub fn pop_force(&mut self, media_source_id: u32) -> Option<GapPayload> {
        let state = self.sources.get_mut(&media_source_id)?;
        let popped = state.waiting.pop_front()?;
        state.next = Some(popped.rtp_sequence.wrapping_add(1));
        Some(popped)
    }

    /// Returns the number of frames buffered for the given source.
    pub fn len_for(&self, media_source_id: u32) -> usize {
        self.sources
            .get(&media_source_id)
            .map(|s| s.waiting.len())
            .unwrap_or(0)
    }

    /// Drops every queued frame and forgets every source.
    pub fn clear(&mut self) {
        self.sources.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(src: u32, seq: u32) -> GapPayload {
        GapPayload::opus_20ms(src, seq as u16, 0, vec![seq as u8])
    }

    #[test]
    fn reorders_out_of_order_pushes() {
        let mut jb = JitterBuffer::new(8);
        for s in [3u32, 1, 2, 4] {
            assert!(matches!(jb.push(frame(1, s)), JitterPush::Accepted));
        }
        let order: Vec<u32> = std::iter::from_fn(|| jb.pop_in_order(1))
            .map(|f| f.rtp_sequence)
            .collect();
        assert_eq!(order, vec![1, 2, 3, 4]);
    }

    #[test]
    fn rejects_late_frames() {
        let mut jb = JitterBuffer::new(4);
        jb.push(frame(1, 5));
        jb.pop_in_order(1).unwrap();
        let r = jb.push(frame(1, 4));
        assert!(matches!(r, JitterPush::Late));
    }

    #[test]
    fn pop_force_skips_missing() {
        let mut jb = JitterBuffer::new(4);
        jb.push(frame(1, 1));
        jb.push(frame(1, 3));
        assert_eq!(jb.pop_in_order(1).unwrap().rtp_sequence, 1);
        assert!(jb.pop_in_order(1).is_none());
        assert_eq!(jb.pop_force(1).unwrap().rtp_sequence, 3);
    }
}
