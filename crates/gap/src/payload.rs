//! GAP audio payload codec. Five CBOR keys.

use gbp::CodecError;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

/// Audio frame payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GapPayload {
    /// Audio source identifier (microphone or device).
    #[serde(rename = "msid")]
    pub media_source_id: u32,
    /// 16-bit `rtp_sequence` widened to `u32` for CBOR uint compatibility.
    #[serde(rename = "seq")]
    pub rtp_sequence: u32,
    /// 48 kHz timestamp.
    #[serde(rename = "ts")]
    pub rtp_timestamp: u64,
    /// Key phase (binds the payload to a specific MLS epoch).
    #[serde(rename = "kp")]
    pub key_phase: u32,
    /// Opus frame bytes.
    #[serde(rename = "opus")]
    pub opus_frame: ByteBuf,
}

impl GapPayload {
    /// Builds a 20 ms Opus frame at 48 kHz (`rtp_timestamp = 960`).
    pub fn opus_20ms(media_source_id: u32, rtp_sequence: u16, key_phase: u32, opus: Vec<u8>) -> Self {
        Self {
            media_source_id,
            rtp_sequence: rtp_sequence as u32,
            rtp_timestamp: 960,
            key_phase,
            opus_frame: ByteBuf::from(opus),
        }
    }

    /// CBOR-encodes the payload.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode");
        buf
    }

    /// Decodes a CBOR-encoded payload.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))
    }
}
