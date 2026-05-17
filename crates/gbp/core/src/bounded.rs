//! LRU-bounded set for per-epoch deduplication.
//!
//! Each sub-protocol client (GTP, GSP, GAP) maintains a deduplication set
//! that MUST NOT grow unboundedly within a single epoch. This type provides
//! a `HashSet` with insertion-order eviction: when capacity is exceeded the
//! oldest entry is dropped.

use std::collections::{HashSet, VecDeque};
use std::hash::Hash;

/// A set with a bounded capacity that evicts the oldest entry on overflow.
///
/// Insertion (`insert`) returns `true` when the item is new and was added,
/// `false` when it was already present. When `len() == cap`, the least
/// recently inserted item is removed before the new item is added.
#[derive(Clone, Debug)]
pub struct BoundedSeen<T> {
    set: HashSet<T>,
    order: VecDeque<T>,
    cap: usize,
}

impl<T: Eq + Hash + Clone> BoundedSeen<T> {
    /// Creates a new set with the given capacity limit.
    ///
    /// N=10000 is the recommended default per epoch (GTP §5, GSP §5).
    pub fn new(cap: usize) -> Self {
        Self {
            set: HashSet::with_capacity(cap.min(1024)),
            order: VecDeque::with_capacity(cap.min(1024)),
            cap,
        }
    }

    /// Inserts an item. Returns `true` if the item is new and was added,
    /// `false` if it was already present.
    ///
    /// When the set is at capacity, the oldest entry is evicted.
    pub fn insert(&mut self, item: T) -> bool {
        if !self.set.insert(item.clone()) {
            return false;
        }
        self.order.push_back(item);
        if self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.set.remove(&old);
            }
        }
        true
    }

    /// Returns the number of entries currently in the set.
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Returns `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Clears the set.
    pub fn clear(&mut self) {
        self.set.clear();
        self.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_new_returns_true() {
        let mut s = BoundedSeen::new(10);
        assert!(s.insert(1));
    }

    #[test]
    fn insert_duplicate_returns_false() {
        let mut s = BoundedSeen::new(10);
        s.insert(1);
        assert!(!s.insert(1));
    }

    #[test]
    fn evicts_oldest_on_overflow() {
        let mut s = BoundedSeen::new(3);
        s.insert(1);
        s.insert(2);
        s.insert(3);
        // At capacity; next insert evicts 1 (oldest).
        s.insert(4);
        // 1 was evicted, fresh again — but this evicts 2.
        assert!(s.insert(1));
        // 4 was second-newest, still present.
        assert!(!s.insert(4));
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn clear_resets() {
        let mut s = BoundedSeen::new(100);
        s.insert("a");
        s.clear();
        assert!(s.is_empty());
        assert!(s.insert("a")); // fresh after clear
    }
}
