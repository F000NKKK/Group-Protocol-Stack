//! GTP message codec.

use gbp::CodecError;
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
}
