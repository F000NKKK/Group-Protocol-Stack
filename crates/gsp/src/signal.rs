//! GSP signal codec. Six CBOR keys.

use gbp::CodecError;
use gbp_core::PayloadCodec;
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

    /// Encodes using the given codec.
    pub fn to_bytes(&self, codec: PayloadCodec) -> Vec<u8> {
        match codec {
            PayloadCodec::Cbor => self.to_cbor(),
            PayloadCodec::Protobuf => {
                use prost::Message as _;
                gbp_proto::gsp::GspSignal::from(self).encode_to_vec()
            }
            PayloadCodec::FlatBuffers => {
                let mut b = gbp_flat::planus::Builder::new();
                b.finish(gbp_flat::gsp::GspSignal::from(self), None).to_vec()
            }
        }
    }

    /// Decodes from the given codec.
    pub fn from_bytes(data: &[u8], codec: PayloadCodec) -> Result<Self, CodecError> {
        match codec {
            PayloadCodec::Cbor => Self::from_cbor(data),
            PayloadCodec::Protobuf => {
                use prost::Message as _;
                let p = gbp_proto::gsp::GspSignal::decode(data)
                    .map_err(|e| CodecError::Decode(e.to_string()))?;
                Self::try_from(p).map_err(|_| CodecError::PayloadSizeMismatch)
            }
            PayloadCodec::FlatBuffers => {
                use gbp_flat::planus::ReadAsRoot as _;
                let r = gbp_flat::gsp::GspSignalRef::read_as_root(data)
                    .map_err(|e| CodecError::Decode(e.to_string()))?;
                Self::try_from(r).map_err(|_| CodecError::PayloadSizeMismatch)
            }
        }
    }
}

// ── Proto conversions ─────────────────────────────────────────────────────────

impl From<&GspSignal> for gbp_proto::gsp::GspSignal {
    fn from(s: &GspSignal) -> Self {
        Self {
            signal_type: s.signal_type,
            request_id: s.request_id,
            sender_id: s.sender_id,
            role_claim: s.role_claim,
            args_length: s.args_length,
            args: s.args.to_vec(),
        }
    }
}

impl TryFrom<gbp_proto::gsp::GspSignal> for GspSignal {
    type Error = ();
    fn try_from(p: gbp_proto::gsp::GspSignal) -> Result<Self, ()> {
        if p.args_length as usize != p.args.len() {
            return Err(());
        }
        Ok(Self {
            signal_type: p.signal_type,
            request_id: p.request_id,
            sender_id: p.sender_id,
            role_claim: p.role_claim,
            args_length: p.args_length,
            args: ByteBuf::from(p.args),
        })
    }
}

// ── FlatBuffers conversions ───────────────────────────────────────────────────

impl From<&GspSignal> for gbp_flat::gsp::GspSignal {
    fn from(s: &GspSignal) -> Self {
        Self {
            signal_type: s.signal_type,
            request_id: s.request_id,
            sender_id: s.sender_id,
            role_claim: s.role_claim,
            args_length: s.args_length,
            args: if s.args.is_empty() {
                None
            } else {
                Some(s.args.to_vec())
            },
        }
    }
}

impl<'a> TryFrom<gbp_flat::gsp::GspSignalRef<'a>> for GspSignal {
    type Error = ();
    fn try_from(r: gbp_flat::gsp::GspSignalRef<'a>) -> Result<Self, ()> {
        let args = r.args().map_err(|_| ())?.unwrap_or(&[]).to_vec();
        let args_length = r.args_length().map_err(|_| ())?;
        if args_length as usize != args.len() {
            return Err(());
        }
        Ok(Self {
            signal_type: r.signal_type().map_err(|_| ())?,
            request_id: r.request_id().map_err(|_| ())?,
            sender_id: r.sender_id().map_err(|_| ())?,
            role_claim: r.role_claim().map_err(|_| ())?,
            args_length,
            args: ByteBuf::from(args),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GspSignal {
        GspSignal::bare(1, 99, 5)
    }

    #[test]
    fn cbor_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::Cbor);
        let decoded = GspSignal::from_bytes(&bytes, PayloadCodec::Cbor).unwrap();
        assert_eq!(decoded.signal_type, orig.signal_type);
        assert_eq!(decoded.request_id, orig.request_id);
        assert_eq!(decoded.sender_id, orig.sender_id);
    }

    #[test]
    fn protobuf_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::Protobuf);
        let decoded = GspSignal::from_bytes(&bytes, PayloadCodec::Protobuf).unwrap();
        assert_eq!(decoded.signal_type, orig.signal_type);
        assert_eq!(decoded.request_id, orig.request_id);
        assert_eq!(decoded.sender_id, orig.sender_id);
    }

    #[test]
    fn flatbuffers_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::FlatBuffers);
        let decoded = GspSignal::from_bytes(&bytes, PayloadCodec::FlatBuffers).unwrap();
        assert_eq!(decoded.signal_type, orig.signal_type);
        assert_eq!(decoded.request_id, orig.request_id);
        assert_eq!(decoded.sender_id, orig.sender_id);
    }

    #[test]
    fn codec_bytes_differ() {
        let sig = sample();
        let cbor = sig.to_bytes(PayloadCodec::Cbor);
        let proto = sig.to_bytes(PayloadCodec::Protobuf);
        let flat = sig.to_bytes(PayloadCodec::FlatBuffers);
        assert_ne!(cbor, proto);
        assert_ne!(cbor, flat);
        assert_ne!(proto, flat);
    }
}
