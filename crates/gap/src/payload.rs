//! GAP audio payload codec. Five CBOR keys.

use gbp::CodecError;
use gbp_core::PayloadCodec;
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
    /// Builds a 20 ms Opus frame at 48 kHz (960 samples).
    pub fn opus_20ms(
        media_source_id: u32,
        rtp_sequence: u16,
        key_phase: u32,
        opus: Vec<u8>,
    ) -> Self {
        Self {
            media_source_id,
            rtp_sequence: rtp_sequence as u32,
            rtp_timestamp: 960,
            key_phase,
            opus_frame: ByteBuf::from(opus),
        }
    }

    /// Builds an Opus frame with an explicit `rtp_timestamp`.
    /// Prefer [`GapPayload::opus_20ms`] for the common 48 kHz / 20 ms case.
    pub fn with_timestamp(
        media_source_id: u32,
        rtp_sequence: u16,
        rtp_timestamp: u64,
        key_phase: u32,
        opus: Vec<u8>,
    ) -> Self {
        Self {
            media_source_id,
            rtp_sequence: rtp_sequence as u32,
            rtp_timestamp,
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

    /// Encodes using the given codec.
    pub fn to_bytes(&self, codec: PayloadCodec) -> Vec<u8> {
        match codec {
            PayloadCodec::Cbor => self.to_cbor(),
            PayloadCodec::Protobuf => {
                use prost::Message as _;
                gbp_proto::gap::GapPayload::from(self).encode_to_vec()
            }
            PayloadCodec::FlatBuffers => {
                let mut b = gbp_flat::planus::Builder::new();
                b.finish(gbp_flat::gap::GapPayload::from(self), None).to_vec()
            }
        }
    }

    /// Decodes from the given codec.
    pub fn from_bytes(data: &[u8], codec: PayloadCodec) -> Result<Self, CodecError> {
        match codec {
            PayloadCodec::Cbor => Self::from_cbor(data),
            PayloadCodec::Protobuf => {
                use prost::Message as _;
                let p = gbp_proto::gap::GapPayload::decode(data)
                    .map_err(|e| CodecError::Decode(e.to_string()))?;
                Ok(Self::from(p))
            }
            PayloadCodec::FlatBuffers => {
                use gbp_flat::planus::ReadAsRoot as _;
                let r = gbp_flat::gap::GapPayloadRef::read_as_root(data)
                    .map_err(|e| CodecError::Decode(e.to_string()))?;
                Self::try_from(r).map_err(|_| CodecError::Decode("flatbuffers field error".into()))
            }
        }
    }
}

// ── Proto conversions ─────────────────────────────────────────────────────────

impl From<&GapPayload> for gbp_proto::gap::GapPayload {
    fn from(p: &GapPayload) -> Self {
        Self {
            media_source_id: p.media_source_id,
            rtp_sequence: p.rtp_sequence,
            rtp_timestamp: p.rtp_timestamp,
            key_phase: p.key_phase,
            opus_frame: p.opus_frame.to_vec(),
        }
    }
}

impl From<gbp_proto::gap::GapPayload> for GapPayload {
    fn from(p: gbp_proto::gap::GapPayload) -> Self {
        Self {
            media_source_id: p.media_source_id,
            rtp_sequence: p.rtp_sequence,
            rtp_timestamp: p.rtp_timestamp,
            key_phase: p.key_phase,
            opus_frame: ByteBuf::from(p.opus_frame),
        }
    }
}

// ── FlatBuffers conversions ───────────────────────────────────────────────────

impl From<&GapPayload> for gbp_flat::gap::GapPayload {
    fn from(p: &GapPayload) -> Self {
        Self {
            media_source_id: p.media_source_id,
            rtp_sequence: p.rtp_sequence,
            rtp_timestamp: p.rtp_timestamp,
            key_phase: p.key_phase,
            opus_frame: Some(p.opus_frame.to_vec()),
        }
    }
}

impl<'a> TryFrom<gbp_flat::gap::GapPayloadRef<'a>> for GapPayload {
    type Error = ();
    fn try_from(r: gbp_flat::gap::GapPayloadRef<'a>) -> Result<Self, ()> {
        let opus_frame = r.opus_frame().map_err(|_| ())?.unwrap_or(&[]).to_vec();
        Ok(Self {
            media_source_id: r.media_source_id().map_err(|_| ())?,
            rtp_sequence: r.rtp_sequence().map_err(|_| ())?,
            rtp_timestamp: r.rtp_timestamp().map_err(|_| ())?,
            key_phase: r.key_phase().map_err(|_| ())?,
            opus_frame: ByteBuf::from(opus_frame),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GapPayload {
        GapPayload::opus_20ms(1, 42, 7, vec![0xAB, 0xCD, 0xEF])
    }

    #[test]
    fn cbor_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::Cbor);
        let decoded = GapPayload::from_bytes(&bytes, PayloadCodec::Cbor).unwrap();
        assert_eq!(decoded.media_source_id, orig.media_source_id);
        assert_eq!(decoded.rtp_sequence, orig.rtp_sequence);
        assert_eq!(decoded.key_phase, orig.key_phase);
        assert_eq!(decoded.opus_frame.as_ref(), orig.opus_frame.as_ref());
    }

    #[test]
    fn protobuf_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::Protobuf);
        let decoded = GapPayload::from_bytes(&bytes, PayloadCodec::Protobuf).unwrap();
        assert_eq!(decoded.media_source_id, orig.media_source_id);
        assert_eq!(decoded.rtp_sequence, orig.rtp_sequence);
        assert_eq!(decoded.opus_frame.as_ref(), orig.opus_frame.as_ref());
    }

    #[test]
    fn flatbuffers_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::FlatBuffers);
        let decoded = GapPayload::from_bytes(&bytes, PayloadCodec::FlatBuffers).unwrap();
        assert_eq!(decoded.media_source_id, orig.media_source_id);
        assert_eq!(decoded.rtp_sequence, orig.rtp_sequence);
        assert_eq!(decoded.opus_frame.as_ref(), orig.opus_frame.as_ref());
    }

    #[test]
    fn codec_bytes_differ() {
        let p = sample();
        let cbor = p.to_bytes(PayloadCodec::Cbor);
        let proto = p.to_bytes(PayloadCodec::Protobuf);
        let flat = p.to_bytes(PayloadCodec::FlatBuffers);
        assert_ne!(cbor, proto);
        assert_ne!(cbor, flat);
        assert_ne!(proto, flat);
    }
}
