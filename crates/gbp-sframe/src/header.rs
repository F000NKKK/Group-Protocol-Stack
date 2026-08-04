//! SFrame Key ID scheme.
//!
//! The on-the-wire header (RFC 9605 §4.3) is produced and parsed entirely by
//! the [`sframe`] crate. This module only decides how gbp packs
//! `(epoch, leaf_index)` into the RFC's MLS Key ID, using sframe's own
//! [`MlsKeyId`] definition (RFC 9605 §5.2).

use sframe::mls::{MlsKeyId, MlsKeyIdBitRange};

/// Bits of the MLS epoch encoded in the KID (its least-significant bits).
pub const EPOCH_BITS: u8 = 16;
/// Bits of the sender's member index encoded in the KID (up to 65 535 senders).
pub const INDEX_BITS: u8 = 16;

/// Context ID component of the KID; `0` yields the shortest RFC Key ID.
const CONTEXT_ID: u64 = 0;

fn bit_range() -> MlsKeyIdBitRange {
    MlsKeyIdBitRange::new(EPOCH_BITS, INDEX_BITS)
}

/// SFrame Key ID helper, backed by sframe's [`MlsKeyId`] (RFC 9605 §5.2).
///
/// Packs `(epoch_lsb, member_index)` into the Key ID that the `sframe` crate
/// carries in the RFC header, and unpacks it again on receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SFrameHeader;

impl SFrameHeader {
    /// Builds the KID for `(epoch, leaf_index)`.
    #[inline]
    pub fn kid_from(epoch: u64, leaf_index: u32) -> u64 {
        MlsKeyId::new(CONTEXT_ID, epoch, u64::from(leaf_index), bit_range()).into()
    }

    /// Extracts the epoch's low [`EPOCH_BITS`] bits from a KID.
    #[inline]
    pub fn epoch_from_kid(kid: u64) -> u64 {
        MlsKeyId::from_key_id(kid, bit_range()).epoch_lsb()
    }

    /// Reduces a full epoch to the low [`EPOCH_BITS`] bits stored in a KID, so
    /// it can be compared against [`epoch_from_kid`](Self::epoch_from_kid).
    #[inline]
    pub fn epoch_lsb(epoch: u64) -> u64 {
        Self::epoch_from_kid(Self::kid_from(epoch, 0))
    }

    /// Extracts the sender's leaf/member index from a KID.
    #[inline]
    pub fn leaf_from_kid(kid: u64) -> u32 {
        MlsKeyId::from_key_id(kid, bit_range()).member_index() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kid_round_trip() {
        let epoch = 42u64;
        let leaf = 7u32;
        let kid = SFrameHeader::kid_from(epoch, leaf);
        assert_eq!(SFrameHeader::epoch_from_kid(kid), epoch);
        assert_eq!(SFrameHeader::leaf_from_kid(kid), leaf);
    }
}
