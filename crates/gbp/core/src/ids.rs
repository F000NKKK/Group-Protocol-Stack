//! Identifier type aliases used across the stack.

/// Group identifier. On the wire, a 16-byte big-endian unsigned integer.
pub type GroupId = [u8; 16];

/// Application-level group member identifier.
pub type MemberId = u32;

/// Group epoch counter. Strictly increases after every accepted MLS commit.
pub type Epoch = u64;

/// Identifier of a pending or applied control plane transition.
pub type TransitionId = u32;

/// Logical stream identifier within a session.
pub type StreamId = u32;

/// Per-stream monotonic sequence number used for the replay window.
pub type SequenceNo = u32;
