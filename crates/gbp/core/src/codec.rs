//! Payload codec discriminant.

use core::fmt;

/// Codec used to encode the sub-protocol payload inside a `GbpFrame`.
///
/// The `pf` (payload_format) field in the frame header carries this value.
/// `Cbor` (0) is the default and is backward-compatible with all existing
/// implementations; frames that omit the `pf` field are treated as CBOR.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PayloadCodec {
    /// CBOR (RFC 7049) — default and backward-compatible.
    #[default]
    Cbor = 0,
    /// Protocol Buffers (proto3).
    Protobuf = 1,
    /// FlatBuffers.
    FlatBuffers = 2,
}

impl PayloadCodec {
    /// Returns the on-wire byte discriminant.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Converts from the on-wire discriminant. Returns `None` for unknown values.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Cbor),
            1 => Some(Self::Protobuf),
            2 => Some(Self::FlatBuffers),
            _ => None,
        }
    }
}

impl fmt::Display for PayloadCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cbor => "cbor",
            Self::Protobuf => "protobuf",
            Self::FlatBuffers => "flatbuffers",
        })
    }
}
