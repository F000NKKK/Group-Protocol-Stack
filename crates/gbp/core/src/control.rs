//! GBP control plane opcode registry.

/// Control plane opcode. Width: u16.
#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ControlOpcode {
    /// Coordinator announces an upcoming epoch change.
    PrepareTransition = 0x0001,
    /// Member acknowledges that it is ready to apply the commit.
    ReadyForTransition = 0x0002,
    /// Coordinator: apply the new epoch.
    ExecuteTransition = 0x0003,
    /// Coordinator: abort the pending transition.
    AbortTransition = 0x0004,
    /// Request a digest of the current group state.
    GroupStateDigestRequest = 0x0005,
    /// Response to a [`GroupStateDigestRequest`](Self::GroupStateDigestRequest).
    GroupStateDigestResponse = 0x0006,
    /// Report an invalid MLS commit.
    ReportInvalidCommit = 0x0007,
    /// Capability advertisement (interoperability profile).
    CapabilitiesAdvertise = 0x0008,
    /// Positive acknowledgement.
    Ack = 0x0009,
    /// Negative acknowledgement carrying an `ErrorObject`.
    Nack = 0x000A,
}

impl ControlOpcode {
    /// Stable human-readable name suitable for logs and dumps.
    pub fn name(self) -> &'static str {
        use ControlOpcode::*;
        match self {
            PrepareTransition => "PREPARE_TRANSITION",
            ReadyForTransition => "READY_FOR_TRANSITION",
            ExecuteTransition => "EXECUTE_TRANSITION",
            AbortTransition => "ABORT_TRANSITION",
            GroupStateDigestRequest => "GROUP_STATE_DIGEST_REQUEST",
            GroupStateDigestResponse => "GROUP_STATE_DIGEST_RESPONSE",
            ReportInvalidCommit => "REPORT_INVALID_COMMIT",
            CapabilitiesAdvertise => "CAPABILITIES_ADVERTISE",
            Ack => "ACK",
            Nack => "NACK",
        }
    }
}

impl TryFrom<u16> for ControlOpcode {
    type Error = u16;
    fn try_from(v: u16) -> Result<Self, u16> {
        use ControlOpcode::*;
        Ok(match v {
            0x0001 => PrepareTransition,
            0x0002 => ReadyForTransition,
            0x0003 => ExecuteTransition,
            0x0004 => AbortTransition,
            0x0005 => GroupStateDigestRequest,
            0x0006 => GroupStateDigestResponse,
            0x0007 => ReportInvalidCommit,
            0x0008 => CapabilitiesAdvertise,
            0x0009 => Ack,
            0x000A => Nack,
            other => return Err(other),
        })
    }
}
