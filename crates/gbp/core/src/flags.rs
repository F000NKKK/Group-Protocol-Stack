//! GBP frame delivery flags.
//!
//! * `O` (ordered)  — preserve in-stream ordering.
//! * `R` (reliable) — retransmit until ACK / NACK / timeout.
//! * `A` (ack-req)  — acknowledgement is mandatory.
//! * `S` (system)   — control plane frame; receiver MUST process it before
//!   application delivery.
//! * `C` (critical) — frame MUST match the receiver's current `transition_id`.

/// Bit set on top of `u16`. Hand-rolled to keep `gbp-core` free of any
/// dependency tree.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GbpFlags(pub u16);

impl GbpFlags {
    /// Bit 0: ordered.
    pub const ORDERED: u16 = 0x0001;
    /// Bit 1: reliable.
    pub const RELIABLE: u16 = 0x0002;
    /// Bit 2: ack required.
    pub const ACK_REQ: u16 = 0x0004;
    /// Bit 3: system frame.
    pub const SYSTEM: u16 = 0x0008;
    /// Bit 4: critical (must match the current transition_id).
    pub const CRITICAL: u16 = 0x0010;

    /// Constructs a flag set from raw bits.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns `true` if the given bit is set.
    pub const fn has(self, bit: u16) -> bool {
        self.0 & bit != 0
    }

    /// `O | R | A` profile — chat / signaling messages.
    pub const fn ordered_reliable_ack() -> u16 {
        Self::ORDERED | Self::RELIABLE | Self::ACK_REQ
    }

    /// `O | R | S` profile — control plane (system).
    pub const fn ordered_reliable_system() -> u16 {
        Self::ORDERED | Self::RELIABLE | Self::SYSTEM
    }

    /// `O` profile — best-effort ordered (voice and other real-time media).
    pub const fn ordered_only() -> u16 {
        Self::ORDERED
    }
}
