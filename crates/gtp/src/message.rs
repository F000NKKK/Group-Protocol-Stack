//! GTP message codec.

use gbp::CodecError;
use gbp_core::PayloadCodec;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

/// Body content type.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GtpContentType {
    /// UTF-8 plaintext.
    Plain = 0,
    /// CommonMark.
    Markdown = 1,
    /// Opaque binary blob.
    Binary = 2,
    /// Reference to an out-of-band attachment.
    AttachmentRef = 3,
}

/// GTP message envelope. Eight CBOR keys.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GtpMessage {
    /// Message identifier (used for idempotency).
    #[serde(rename = "mid")]
    pub message_id: u64,
    /// Sender member identifier.
    #[serde(rename = "sid")]
    pub sender_id: u32,
    /// Send timestamp in milliseconds since the Unix epoch.
    #[serde(rename = "ts")]
    pub timestamp_ms: u64,
    /// Request identifier (echoed in ACK / NACK).
    #[serde(rename = "rid")]
    pub request_id: u32,
    /// Message flag bits (`urgent` / `ephemeral` / `persistent`).
    #[serde(rename = "fl")]
    pub flags: u8,
    /// content_type (see [`GtpContentType`]).
    #[serde(rename = "ct")]
    pub content_type: u8,
    /// Declared length of [`content`](Self::content).
    #[serde(rename = "len")]
    pub content_length: u32,
    /// Body bytes.
    #[serde(rename = "body")]
    pub content: ByteBuf,
}

impl GtpMessage {
    /// Builds a plaintext (UTF-8) message.
    pub fn plain(sender_id: u32, message_id: u64, text: &str) -> Self {
        let body = text.as_bytes().to_vec();
        Self {
            message_id,
            sender_id,
            timestamp_ms: 0,
            request_id: 0,
            flags: 0x01,
            content_type: GtpContentType::Plain as u8,
            content_length: body.len() as u32,
            content: ByteBuf::from(body),
        }
    }

    /// CBOR-encodes the message.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode");
        buf
    }

    /// Decodes a CBOR-encoded message and validates `content_length`.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        let m: Self = ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))?;
        if m.content_length as usize != m.content.len() {
            return Err(CodecError::PayloadSizeMismatch);
        }
        Ok(m)
    }

    /// Returns the body as a `&str` when it is valid UTF-8.
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.content).ok()
    }

    /// Encodes the message using the given codec.
    pub fn to_bytes(&self, codec: PayloadCodec) -> Vec<u8> {
        match codec {
            PayloadCodec::Cbor => self.to_cbor(),
            PayloadCodec::Protobuf => {
                use prost::Message as _;
                gbp_proto::gtp::GtpMessage::from(self).encode_to_vec()
            }
            PayloadCodec::FlatBuffers => {
                let mut b = gbp_flat::planus::Builder::new();
                b.finish(gbp_flat::gtp::GtpMessage::from(self), None).to_vec()
            }
        }
    }

    /// Decodes a message from the given codec.
    pub fn from_bytes(data: &[u8], codec: PayloadCodec) -> Result<Self, CodecError> {
        match codec {
            PayloadCodec::Cbor => Self::from_cbor(data),
            PayloadCodec::Protobuf => {
                use prost::Message as _;
                let p = gbp_proto::gtp::GtpMessage::decode(data)
                    .map_err(|e| CodecError::Decode(e.to_string()))?;
                Self::try_from(p).map_err(|_| CodecError::PayloadSizeMismatch)
            }
            PayloadCodec::FlatBuffers => {
                use gbp_flat::planus::ReadAsRoot as _;
                let r = gbp_flat::gtp::GtpMessageRef::read_as_root(data)
                    .map_err(|e| CodecError::Decode(e.to_string()))?;
                Self::try_from(r).map_err(|_| CodecError::PayloadSizeMismatch)
            }
        }
    }
}

// ── Proto conversions ─────────────────────────────────────────────────────────

impl From<&GtpMessage> for gbp_proto::gtp::GtpMessage {
    fn from(m: &GtpMessage) -> Self {
        Self {
            message_id: m.message_id,
            sender_id: m.sender_id,
            timestamp_ms: m.timestamp_ms,
            request_id: m.request_id,
            flags: m.flags as u32,
            content_type: m.content_type as u32,
            content_length: m.content_length,
            content: m.content.to_vec(),
        }
    }
}

impl TryFrom<gbp_proto::gtp::GtpMessage> for GtpMessage {
    type Error = ();
    fn try_from(p: gbp_proto::gtp::GtpMessage) -> Result<Self, ()> {
        if p.content_length as usize != p.content.len() {
            return Err(());
        }
        Ok(Self {
            message_id: p.message_id,
            sender_id: p.sender_id,
            timestamp_ms: p.timestamp_ms,
            request_id: p.request_id,
            flags: p.flags as u8,
            content_type: p.content_type as u8,
            content_length: p.content_length,
            content: ByteBuf::from(p.content),
        })
    }
}

// ── FlatBuffers conversions ───────────────────────────────────────────────────

impl From<&GtpMessage> for gbp_flat::gtp::GtpMessage {
    fn from(m: &GtpMessage) -> Self {
        Self {
            message_id: m.message_id,
            sender_id: m.sender_id,
            timestamp_ms: m.timestamp_ms,
            request_id: m.request_id,
            flags: m.flags as u32,
            content_type: m.content_type as u32,
            content_length: m.content_length,
            content: Some(m.content.to_vec()),
        }
    }
}

impl<'a> TryFrom<gbp_flat::gtp::GtpMessageRef<'a>> for GtpMessage {
    type Error = ();
    fn try_from(r: gbp_flat::gtp::GtpMessageRef<'a>) -> Result<Self, ()> {
        let content = r.content().map_err(|_| ())?.unwrap_or(&[]).to_vec();
        let content_length = r.content_length().map_err(|_| ())?;
        if content_length as usize != content.len() {
            return Err(());
        }
        Ok(Self {
            message_id: r.message_id().map_err(|_| ())?,
            sender_id: r.sender_id().map_err(|_| ())?,
            timestamp_ms: r.timestamp_ms().map_err(|_| ())?,
            request_id: r.request_id().map_err(|_| ())?,
            flags: r.flags().map_err(|_| ())? as u8,
            content_type: r.content_type().map_err(|_| ())? as u8,
            content_length,
            content: ByteBuf::from(content),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GtpMessage {
        GtpMessage::plain(42, 0xDEAD_BEEF, "codec roundtrip")
    }

    #[test]
    fn cbor_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::Cbor);
        let decoded = GtpMessage::from_bytes(&bytes, PayloadCodec::Cbor).unwrap();
        assert_eq!(decoded.message_id, orig.message_id);
        assert_eq!(decoded.sender_id, orig.sender_id);
        assert_eq!(decoded.text().unwrap(), "codec roundtrip");
    }

    #[test]
    fn protobuf_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::Protobuf);
        let decoded = GtpMessage::from_bytes(&bytes, PayloadCodec::Protobuf).unwrap();
        assert_eq!(decoded.message_id, orig.message_id);
        assert_eq!(decoded.sender_id, orig.sender_id);
        assert_eq!(decoded.text().unwrap(), "codec roundtrip");
    }

    #[test]
    fn flatbuffers_roundtrip() {
        let orig = sample();
        let bytes = orig.to_bytes(PayloadCodec::FlatBuffers);
        let decoded = GtpMessage::from_bytes(&bytes, PayloadCodec::FlatBuffers).unwrap();
        assert_eq!(decoded.message_id, orig.message_id);
        assert_eq!(decoded.sender_id, orig.sender_id);
        assert_eq!(decoded.text().unwrap(), "codec roundtrip");
    }

    #[test]
    fn codec_bytes_differ() {
        let msg = sample();
        let cbor = msg.to_bytes(PayloadCodec::Cbor);
        let proto = msg.to_bytes(PayloadCodec::Protobuf);
        let flat = msg.to_bytes(PayloadCodec::FlatBuffers);
        assert_ne!(cbor, proto);
        assert_ne!(cbor, flat);
        assert_ne!(proto, flat);
    }
}
