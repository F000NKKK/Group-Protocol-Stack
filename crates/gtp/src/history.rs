//! Bounded message log + per-sender resynchronisation watermark.
//!
//! GTP §11 (resync): a re-joining client SHOULD be able to ask the group for
//! every message produced after the last `message_id` it observed. Servers
//! and peer caches that hold recent messages can use [`MessageHistory`] as a
//! ready-made ring buffer with `since(...)` queries, and re-joining clients
//! can use [`Watermark`] to track the highest `message_id` they have seen
//! per sender so they know where to resume from.

use crate::GtpMessage;
use gbp_core::MemberId;
use std::collections::HashMap;
use std::collections::VecDeque;

/// Bounded ring-buffer of recent GTP messages.
///
/// The capacity is fixed at construction time; older messages are discarded
/// once the buffer is full. Insertion is O(1); resync queries are O(n) over
/// the buffer (typically small).
pub struct MessageHistory {
    capacity: usize,
    buffer: VecDeque<GtpMessage>,
}

impl MessageHistory {
    /// Builds a history that retains up to `capacity` messages.
    ///
    /// # Panics
    /// Panics if `capacity == 0`.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    /// Records a message. Returns `true` if it was newly added, `false` if
    /// `(sender_id, message_id)` was already present (idempotent insert).
    pub fn push(&mut self, msg: GtpMessage) -> bool {
        if self.contains(msg.sender_id, msg.message_id) {
            return false;
        }
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(msg);
        true
    }

    /// Number of messages currently in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns `true` if `(sender_id, message_id)` is present.
    pub fn contains(&self, sender_id: MemberId, message_id: u64) -> bool {
        self.buffer
            .iter()
            .any(|m| m.sender_id == sender_id && m.message_id == message_id)
    }

    /// Returns every message produced **after** the given watermark, in
    /// insertion order. Use this to satisfy a peer's resync request.
    pub fn since<'a>(
        &'a self,
        watermark: &'a Watermark,
    ) -> impl Iterator<Item = &'a GtpMessage> + 'a {
        self.buffer.iter().filter(move |m| {
            watermark
                .last_seen
                .get(&m.sender_id)
                .copied()
                .map(|hw| m.message_id > hw)
                .unwrap_or(true)
        })
    }

    /// Returns every message from a single sender produced strictly after
    /// `since_message_id`.
    pub fn since_for_sender(
        &self,
        sender_id: MemberId,
        since_message_id: u64,
    ) -> impl Iterator<Item = &GtpMessage> {
        self.buffer
            .iter()
            .filter(move |m| m.sender_id == sender_id && m.message_id > since_message_id)
    }

    /// Drops every message in the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Per-sender high-water mark of accepted GTP `message_id`s.
///
/// Use this to remember the latest `message_id` seen from each sender; a
/// re-joining client can ship its watermark to the group and ask for every
/// message above it (see [`MessageHistory::since`]).
#[derive(Default, Clone, Debug)]
pub struct Watermark {
    last_seen: HashMap<MemberId, u64>,
}

impl Watermark {
    /// Empty watermark (no messages seen).
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `message_id` from `sender_id` has been observed.
    /// Keeps the maximum.
    pub fn observe(&mut self, sender_id: MemberId, message_id: u64) {
        let entry = self.last_seen.entry(sender_id).or_insert(0);
        if message_id > *entry {
            *entry = message_id;
        }
    }

    /// Returns the last observed `message_id` for `sender_id`, or `None` if
    /// nothing has been seen.
    pub fn last_seen(&self, sender_id: MemberId) -> Option<u64> {
        self.last_seen.get(&sender_id).copied()
    }

    /// Iterates `(sender_id, last_message_id)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (MemberId, u64)> + '_ {
        self.last_seen.iter().map(|(&s, &m)| (s, m))
    }

    /// Number of senders tracked.
    pub fn len(&self) -> usize {
        self.last_seen.len()
    }

    /// Empty?
    pub fn is_empty(&self) -> bool {
        self.last_seen.is_empty()
    }

    /// Drops every entry.
    pub fn clear(&mut self) {
        self.last_seen.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(sender: u32, mid: u64) -> GtpMessage {
        GtpMessage::plain(sender, mid, "x")
    }

    #[test]
    fn push_dedups_and_evicts() {
        let mut h = MessageHistory::with_capacity(3);
        assert!(h.push(msg(1, 1)));
        assert!(h.push(msg(1, 2)));
        assert!(!h.push(msg(1, 1)));
        assert!(h.push(msg(1, 3)));
        assert!(h.push(msg(1, 4)));
        assert_eq!(h.len(), 3);
        assert!(!h.contains(1, 1));
        assert!(h.contains(1, 4));
    }

    #[test]
    fn since_returns_only_after_watermark() {
        let mut h = MessageHistory::with_capacity(10);
        for mid in 1..=5 {
            h.push(msg(1, mid));
        }
        let mut wm = Watermark::new();
        wm.observe(1, 3);
        let after: Vec<u64> = h.since(&wm).map(|m| m.message_id).collect();
        assert_eq!(after, vec![4, 5]);
    }

    #[test]
    fn watermark_keeps_max() {
        let mut wm = Watermark::new();
        wm.observe(1, 5);
        wm.observe(1, 3);
        wm.observe(1, 7);
        assert_eq!(wm.last_seen(1), Some(7));
    }
}
