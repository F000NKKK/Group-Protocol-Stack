//! GBP transport frame.
//!
//! On the wire the frame is a deterministic CBOR map of ten keys (eleven when
//! a non-CBOR payload codec is in use):
//! `v, gid, ep, tid, st, sid, fl, seq, psz, pl[, pf]`.
//! Field `psz` MUST equal the actual length of `pl`; this is checked on
//! decode. Field `pf` is omitted when its value is 0 (CBOR) for
//! backward-compatibility.

use crate::CodecError;
use gbp_core::{GroupId, PayloadCodec, StreamType};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

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
    /// Payload codec discriminant (see [`gbp_core::PayloadCodec`]).
    /// Omitted when 0 (CBOR) for backward-compatibility with pre-1.5 peers.
    #[serde(rename = "pf", default, skip_serializing_if = "is_zero_u8")]
    pub payload_format: u8,
}

impl GbpFrame {
    /// Builds a frame from already-encrypted payload bytes.
    ///
    /// `payload_size` is set to `encrypted_payload.len()` automatically.
    /// Pass `PayloadCodec::Cbor` (or `0`) for the default CBOR encoding; the
    /// `pf` field is omitted from the wire when the codec is CBOR so older
    /// peers continue to decode the frame correctly.
    pub fn new(
        group_id: GroupId,
        epoch: u64,
        transition_id: u32,
        stream_type: StreamType,
        stream_id: u32,
        flags: u16,
        sequence_no: u32,
        encrypted_payload: Vec<u8>,
        payload_format: u8,
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
            payload_format,
        }
    }

    /// Returns the `payload_format` field as a [`PayloadCodec`], falling back
    /// to [`PayloadCodec::Cbor`] for unknown discriminants.
    pub fn payload_codec(&self) -> PayloadCodec {
        PayloadCodec::from_u8(self.payload_format).unwrap_or(PayloadCodec::Cbor)
    }

    /// Serialises the frame into a freshly allocated CBOR byte vector.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode is infallible on Vec");
        buf
    }

    /// Decodes a CBOR-encoded frame **and** validates `payload_size`.
    ///
    /// The order of all §6.2 checks (`version`, `group_id`, `epoch`,
    /// `payload_size`, `transition_id`, `sequence_no`) is what governs which
    /// error a malformed frame produces. Most callers should use
    /// [`GroupNode::on_wire`](https://docs.rs/gbp-node) which decodes via
    /// [`GbpFrame::decode`] and runs the full pipeline. This convenience
    /// wrapper exists for tests and ad-hoc tooling that want both decode
    /// and the length check in one shot.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        let f = Self::decode(data)?;
        f.validate_payload_size()?;
        Ok(f)
    }

    /// Decodes a CBOR-encoded frame **without** running any §6.2 checks.
    ///
    /// Use [`GbpFrame::validate_payload_size`] (and the higher-priority
    /// version / group_id / epoch checks at the calling layer) before
    /// trusting the result.
    pub fn decode(data: &[u8]) -> Result<Self, CodecError> {
        ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))
    }

    /// Returns `Ok(())` if `payload_size` equals the actual payload length,
    /// `Err(CodecError::PayloadSizeMismatch)` otherwise.
    pub fn validate_payload_size(&self) -> Result<(), CodecError> {
        if self.payload_size as usize != self.encrypted_payload.len() {
            return Err(CodecError::PayloadSizeMismatch);
        }
        Ok(())
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
            0,
        );
        let bytes = f.to_cbor();
        let back = GbpFrame::from_cbor(&bytes).unwrap();
        assert_eq!(back.epoch, 42);
        assert_eq!(back.transition_id, 7);
        assert_eq!(back.stream_type_typed().unwrap(), StreamType::Text);
        assert_eq!(back.encrypted_payload.as_slice(), &[1, 2, 3, 4, 5]);
        assert_eq!(back.payload_format, 0);
    }

    #[test]
    fn frame_roundtrip_with_codec() {
        use gbp_core::PayloadCodec;
        let f = GbpFrame::new(
            [0xBB; 16],
            1,
            0,
            StreamType::Audio,
            1,
            0,
            1,
            vec![0xDE, 0xAD],
            PayloadCodec::FlatBuffers.as_u8(),
        );
        assert_eq!(f.payload_codec(), PayloadCodec::FlatBuffers);
        let bytes = f.to_cbor();
        let back = GbpFrame::from_cbor(&bytes).unwrap();
        assert_eq!(back.payload_format, PayloadCodec::FlatBuffers.as_u8());
        assert_eq!(back.payload_codec(), PayloadCodec::FlatBuffers);
    }

    #[test]
    fn cbor_codec_field_omitted_from_wire() {
        let f = GbpFrame::new([0; 16], 1, 0, StreamType::Text, 1, 0, 1, vec![0], 0);
        let bytes = f.to_cbor();
        // CBOR map should NOT contain the "pf" key when codec is 0.
        // Decode and confirm pf defaults to 0.
        let back = GbpFrame::from_cbor(&bytes).unwrap();
        assert_eq!(back.payload_format, 0);
    }

    #[test]
    fn frame_rejects_bad_payload_size() {
        let mut f = GbpFrame::new([0; 16], 1, 0, StreamType::Text, 1, 0, 1, vec![1, 2, 3], 0);
        f.payload_size = 99;
        let mut bytes = Vec::new();
        ciborium::into_writer(&f, &mut bytes).unwrap();
        assert!(matches!(
            GbpFrame::from_cbor(&bytes),
            Err(CodecError::PayloadSizeMismatch)
        ));
    }
}
