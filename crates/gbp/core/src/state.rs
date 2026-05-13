//! Finite state machines defined by the state-machine specification.
//!
//! Only the enum values and the transition validator live here; the side
//! effects (timers, retries, etc.) belong in `gbp-node`.

use core::fmt;

/// Group node FSM.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NodeState {
    /// Initial state, before any transport is opened.
    Idle,
    /// QUIC / TLS handshake in progress.
    Connecting,
    /// MLS Welcome / ratchet tree exchange in progress.
    EstablishingGroup,
    /// Normal operating state.
    Active,
    /// `ERR_EPOCH_MISMATCH` (or equivalent) was raised; digest-based resync
    /// is in progress.
    Resyncing,
    /// Fatal error; the node MUST NOT transmit application data.
    Failed,
    /// The node performed a graceful shutdown.
    Closed,
}

impl NodeState {
    /// Returns `true` if the transition `self -> next` is allowed by the
    /// state-machine specification.
    pub fn can_transition_to(self, next: NodeState) -> bool {
        use NodeState::*;
        matches!(
            (self, next),
            (Idle, Connecting)
                | (Idle, Failed)
                | (Connecting, EstablishingGroup)
                | (Connecting, Failed)
                | (EstablishingGroup, Active)
                | (EstablishingGroup, Failed)
                | (Active, Resyncing)
                | (Active, Closed)
                | (Active, Failed)
                | (Resyncing, Active)
                | (Resyncing, Failed)
                | (_, Closed)
        )
    }
}

impl fmt::Display for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Idle => "IDLE",
            Self::Connecting => "CONNECTING",
            Self::EstablishingGroup => "ESTABLISHING_GROUP",
            Self::Active => "ACTIVE",
            Self::Resyncing => "RESYNCING",
            Self::Failed => "FAILED",
            Self::Closed => "CLOSED",
        })
    }
}

/// Epoch transition FSM.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransitionState {
    /// No pending commit.
    TIdle,
    /// `PREPARE_TRANSITION` was issued or received.
    TPrepared,
    /// MLS commit was processed and the local ratchet was applied.
    TCommitProcessed,
    /// Every member acknowledged with `READY_FOR_TRANSITION`.
    TReady,
    /// `EXECUTE_TRANSITION` has been applied; epoch was advanced.
    TExecuted,
    /// Transition was aborted (`ABORT_TRANSITION` or timeout).
    TAborted,
}

impl TransitionState {
    /// Returns `true` if the transition `self -> next` is allowed by the
    /// state-machine specification (gbp-state-machine §4).
    pub fn can_transition_to(self, next: TransitionState) -> bool {
        use TransitionState::*;
        matches!(
            (self, next),
            (TIdle, TPrepared)
                | (TPrepared, TCommitProcessed)
                | (TPrepared, TAborted)
                | (TCommitProcessed, TReady)
                | (TCommitProcessed, TAborted)
                | (TReady, TExecuted)
                | (TReady, TAborted)
                | (TExecuted, TIdle)
                | (TAborted, TIdle)
        )
    }
}

/// Timeout defaults normative for interoperable deployments (gbp-state-machine §6).
pub mod timeouts {
    /// Coordinator: max wait for READY quorum after issuing PREPARE_TRANSITION.
    pub const T_PREPARE_MAX_MS: u64 = 5_000;
    /// Member: max time to complete local commit / welcome processing.
    pub const T_READY_MAX_MS: u64 = 5_000;
    /// Member: max wait for EXECUTE_TRANSITION after sending READY_FOR_TRANSITION.
    pub const T_EXECUTE_MAX_MS: u64 = 10_000;
    /// Coordinator: extra slack before declaring quorum failure.
    pub const T_QUORUM_GRACE_MS: u64 = 2_000;
    /// Member: silence threshold before triggering coordinator handover.
    pub const T_COORDINATOR_GRACE_MS: u64 = 10_000;
}

/// Sub-protocol activation FSM.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SubprotocolState {
    /// Sub-protocol is disabled.
    Disabled,
    /// Capability negotiation is in progress (`CAPABILITIES_ADVERTISE`).
    Negotiating,
    /// Sub-protocol is active.
    Enabled,
    /// Sub-protocol is active in degraded mode (e.g. lost FEC).
    Degraded,
    /// Sub-protocol is temporarily suspended (`MUTE` / `STREAM_STOP`).
    Suspended,
}
