//! GSP signal codec. Six CBOR keys.

use gbp::CodecError;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

/// GSP signal envelope. `args` carries opcode-specific CBOR bytes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GspSignal {
    /// `SignalType` widened to `u32` for CBOR uint compatibility.
    #[serde(rename = "t")]
    pub signal_type: u32,
    /// Request identifier (echoed in ACK / NACK and used for deduplication).
    #[serde(rename = "rid")]
    pub request_id: u32,
    /// Sender member id.
    #[serde(rename = "sid")]
    pub sender_id: u32,
    /// Role claim (used by `ROLE_CHANGE`).
    #[serde(rename = "rc")]
    pub role_claim: u32,
    /// Declared length of [`args`](Self::args).
    #[serde(rename = "alen")]
    pub args_length: u32,
    /// Opcode-specific CBOR-encoded arguments.
    #[serde(rename = "args")]
    pub args: ByteBuf,
}

impl GspSignal {
    /// Builds a signal with no arguments.
    pub fn bare(signal_type: u32, request_id: u32, sender_id: u32) -> Self {
        Self {
            signal_type,
            request_id,
            sender_id,
            role_claim: 0,
            args_length: 0,
            args: ByteBuf::new(),
        }
    }

    /// CBOR-encodes the signal.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode");
        buf
    }

    /// Decodes a CBOR-encoded signal and validates `args_length`.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        let s: Self = ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))?;
        if s.args_length as usize != s.args.len() {
            return Err(CodecError::PayloadSizeMismatch);
        }
        Ok(s)
    }
}
