//! Capability negotiation (`CAPABILITIES_ADVERTISE`, gbp-control-plane §3).
//!
//! Capability negotiation lets every member tell the rest of the group what
//! optional features it supports (codecs, extensions, version flags). The
//! group's effective set is the **intersection** of every member's
//! capabilities, so any feature outside the intersection is unsafe to use.

use gbp_core::MemberId;
use std::collections::{BTreeSet, HashMap};

/// Per-member set of advertised capability tokens.
#[derive(Default)]
pub struct CapabilitiesNegotiator {
    advertised: HashMap<MemberId, BTreeSet<String>>,
}

impl CapabilitiesNegotiator {
    /// Empty negotiator (no member has advertised anything yet).
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an advertisement. Replaces any prior advertisement from the
    /// same member.
    pub fn advertise<I, S>(&mut self, member: MemberId, capabilities: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let set: BTreeSet<String> = capabilities.into_iter().map(Into::into).collect();
        self.advertised.insert(member, set);
    }

    /// Removes a member's advertisement (e.g. after `LEAVE`).
    pub fn forget(&mut self, member: MemberId) {
        self.advertised.remove(&member);
    }

    /// Returns the current advertisement for `member`.
    pub fn capabilities_of(&self, member: MemberId) -> Option<&BTreeSet<String>> {
        self.advertised.get(&member)
    }

    /// `true` if every advertised member supports `cap`.
    pub fn group_supports(&self, cap: &str) -> bool {
        if self.advertised.is_empty() {
            return false;
        }
        self.advertised.values().all(|set| set.contains(cap))
    }

    /// Returns the **intersection** — capabilities that every member
    /// advertises, i.e. the safe-to-use set.
    pub fn intersection(&self) -> BTreeSet<String> {
        let mut iter = self.advertised.values();
        let Some(first) = iter.next() else {
            return BTreeSet::new();
        };
        let mut acc = first.clone();
        for set in iter {
            acc.retain(|c| set.contains(c));
        }
        acc
    }

    /// Returns the **union** — every capability advertised by any member.
    pub fn union(&self) -> BTreeSet<String> {
        let mut acc = BTreeSet::new();
        for set in self.advertised.values() {
            for c in set {
                acc.insert(c.clone());
            }
        }
        acc
    }

    /// Returns the members that did **not** advertise `cap`.
    pub fn missing(&self, cap: &str) -> Vec<MemberId> {
        self.advertised
            .iter()
            .filter_map(|(m, set)| if set.contains(cap) { None } else { Some(*m) })
            .collect()
    }

    /// Number of members that advertised something.
    pub fn len(&self) -> usize {
        self.advertised.len()
    }

    /// Empty?
    pub fn is_empty(&self) -> bool {
        self.advertised.is_empty()
    }

    /// Clears all advertisements. Call on epoch advance for symmetry with
    /// [`GapClient::sync_epoch`], [`GtpClient::sync_epoch`] and
    /// [`GspClient::sync_epoch`].
    pub fn reset_for_epoch(&mut self) {
        self.advertised.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_is_lowest_common() {
        let mut n = CapabilitiesNegotiator::new();
        n.advertise(1, ["opus", "fec", "h264"]);
        n.advertise(2, ["opus", "fec"]);
        n.advertise(3, ["opus", "av1"]);
        let common = n.intersection();
        assert!(common.contains("opus"));
        assert!(!common.contains("fec"));
        assert!(!common.contains("h264"));
        assert_eq!(n.missing("fec"), vec![3]);
    }

    #[test]
    fn group_supports_requires_everyone() {
        let mut n = CapabilitiesNegotiator::new();
        n.advertise(1, ["opus"]);
        n.advertise(2, ["opus"]);
        assert!(n.group_supports("opus"));
        n.advertise(3, [] as [&str; 0]);
        assert!(!n.group_supports("opus"));
    }
}
