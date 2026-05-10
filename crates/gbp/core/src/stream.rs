//! GBP stream classes.

use core::fmt;

/// Stream class. Each [`StreamType`] is a separate sub-protocol with its own
/// delivery and reliability policy (see the GBP interop profile).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StreamType {
    /// Stream 0 — the GBP control plane.
    Control = 0,
    /// Group Audio Protocol (Opus media frames).
    Audio = 1,
    /// Group Text Protocol (textual messages).
    Text = 2,
    /// Group Signaling Protocol (membership and policy signals).
    Signal = 3,
}

impl StreamType {
    /// Returns the on-wire byte representation.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u32> for StreamType {
    type Error = u32;
    fn try_from(v: u32) -> Result<Self, u32> {
        match v {
            0 => Ok(Self::Control),
            1 => Ok(Self::Audio),
            2 => Ok(Self::Text),
            3 => Ok(Self::Signal),
            other => Err(other),
        }
    }
}

impl TryFrom<u8> for StreamType {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        StreamType::try_from(v as u32).map_err(|_| v)
    }
}

impl fmt::Display for StreamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Control => "control",
            Self::Audio => "audio",
            Self::Text => "text",
            Self::Signal => "signal",
        })
    }
}
