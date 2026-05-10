//! GSP signal opcode registry.

/// Signal opcode. Allocation:
/// 1xx — membership, 2xx — media permissions, 3xx — stream, 4xx — config.
#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SignalType {
    /// Member announces that it has joined the group.
    Join = 100,
    /// Member announces that it is leaving.
    Leave = 101,
    /// Member changes its role.
    RoleChange = 102,
    /// Mute request or notification.
    Mute = 200,
    /// Unmute request or notification.
    Unmute = 201,
    /// Start of a stream publication.
    StreamStart = 300,
    /// End of a stream publication.
    StreamStop = 301,
    /// Codec or SDP parameters update.
    CodecUpdate = 400,
}

impl TryFrom<u32> for SignalType {
    type Error = u32;
    fn try_from(v: u32) -> Result<Self, u32> {
        use SignalType::*;
        Ok(match v {
            100 => Join,
            101 => Leave,
            102 => RoleChange,
            200 => Mute,
            201 => Unmute,
            300 => StreamStart,
            301 => StreamStop,
            400 => CodecUpdate,
            other => return Err(other),
        })
    }
}

impl SignalType {
    /// Stable human-readable name suitable for logs.
    pub fn name(self) -> &'static str {
        use SignalType::*;
        match self {
            Join => "JOIN",
            Leave => "LEAVE",
            RoleChange => "ROLE_CHANGE",
            Mute => "MUTE",
            Unmute => "UNMUTE",
            StreamStart => "STREAM_START",
            StreamStop => "STREAM_STOP",
            CodecUpdate => "CODEC_UPDATE",
        }
    }
}
