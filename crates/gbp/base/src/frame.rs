//! GBP transport frame.
//!
//! On the wire the frame is a deterministic CBOR map of ten keys:
//! `v, gid, ep, tid, st, sid, fl, seq, psz, pl`. Field `psz` MUST equal the
//! actual length of `pl`; this is checked on decode.

use crate::CodecError;
use gbp_core::{GroupId, StreamType};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

/// CBOR-encoded GBP frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GbpFrame {
    /// Protocol version (currently `1`).
    #[serde(rename = "v")]
    pub version: u8,
    /// 16-byte group identifier.
    #[serde(rename = "gid")]
    pub group_id: ByteBuf,
    /// Sender's current epoch.
    #[serde(rename = "ep")]
    pub epoch: u64,
    /// Last applied `transition_id`.
    #[serde(rename = "tid")]
    pub transition_id: u32,
    /// StreamType as a `u8` (see `gbp_core::StreamType`).
    #[serde(rename = "st")]
    pub stream_type: u8,
    /// Logical stream identifier within the session.
    #[serde(rename = "sid")]
    pub stream_id: u32,
    /// Delivery flags (see `gbp_core::GbpFlags`).
    #[serde(rename = "fl")]
    pub flags: u16,
    /// Per-stream sequence number (replay window key).
    #[serde(rename = "seq")]
    pub sequence_no: u32,
    /// Declared payload length; MUST equal `encrypted_payload.len()`.
    #[serde(rename = "psz")]
    pub payload_size: u32,
    /// Encrypted payload (an opaque byte string).
    #[serde(rename = "pl")]
    pub encrypted_payload: ByteBuf,
}

impl GbpFrame {
    /// Builds a frame from already-encrypted payload bytes.
    ///
    /// `payload_size` is set to `encrypted_payload.len()` automatically.
    pub fn new(
        group_id: GroupId,
        epoch: u64,
        transition_id: u32,
        stream_type: StreamType,
        stream_id: u32,
        flags: u16,
        sequence_no: u32,
        encrypted_payload: Vec<u8>,
    ) -> Self {
        Self {
            version: 1,
            group_id: ByteBuf::from(group_id.to_vec()),
            epoch,
            transition_id,
            stream_type: stream_type as u8,
            stream_id,
            flags,
            sequence_no,
            payload_size: encrypted_payload.len() as u32,
            encrypted_payload: ByteBuf::from(encrypted_payload),
        }
    }

    /// Serialises the frame into a freshly allocated CBOR byte vector.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode is infallible on Vec");
        buf
    }

    /// Decodes a CBOR-encoded frame and validates `payload_size`.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        let f: Self = ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))?;
        if f.payload_size as usize != f.encrypted_payload.len() {
            return Err(CodecError::PayloadSizeMismatch);
        }
        Ok(f)
    }

    /// Returns the typed `StreamType`, or `CodecError::UnknownEnumValue` for
    /// unknown stream classes.
    pub fn stream_type_typed(&self) -> Result<StreamType, CodecError> {
        StreamType::try_from(self.stream_type as u32).map_err(CodecError::UnknownEnumValue)
    }

    /// Returns `group_id` as a 16-byte array, padding with zeros or
    /// truncating if necessary.
    pub fn group_id_array(&self) -> GroupId {
        let mut out = [0u8; 16];
        let n = self.group_id.len().min(16);
        out[..n].copy_from_slice(&self.group_id[..n]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbp_core::GbpFlags;

    #[test]
    fn frame_roundtrip() {
        let f = GbpFrame::new(
            [0xAA; 16],
            42,
            7,
            StreamType::Text,
            201,
            GbpFlags::ORDERED | GbpFlags::RELIABLE,
            1,
            vec![1, 2, 3, 4, 5],
        );
        let bytes = f.to_cbor();
        let back = GbpFrame::from_cbor(&bytes).unwrap();
        assert_eq!(back.epoch, 42);
        assert_eq!(back.transition_id, 7);
        assert_eq!(back.stream_type_typed().unwrap(), StreamType::Text);
        assert_eq!(back.encrypted_payload.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn frame_rejects_bad_payload_size() {
        let mut f = GbpFrame::new([0; 16], 1, 0, StreamType::Text, 1, 0, 1, vec![1, 2, 3]);
        f.payload_size = 99;
        let mut bytes = Vec::new();
        ciborium::into_writer(&f, &mut bytes).unwrap();
        assert!(matches!(
            GbpFrame::from_cbor(&bytes),
            Err(CodecError::PayloadSizeMismatch)
        ));
    }
}
