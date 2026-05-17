//! GBP control plane message envelope. Six CBOR keys.

use crate::CodecError;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

/// Control plane message that travels on Stream 0.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlMessage {
    /// Opcode (see `gbp_core::ControlOpcode`).
    #[serde(rename = "op")]
    pub opcode: u16,
    /// Request identifier (echoed in ACK / NACK).
    #[serde(rename = "rid")]
    pub request_id: u32,
    /// Sender's member id.
    #[serde(rename = "sid")]
    pub sender_id: u32,
    /// Related `transition_id`.
    #[serde(rename = "tid")]
    pub transition_id: u32,
    /// Declared length of [`args`](Self::args).
    #[serde(rename = "alen")]
    pub args_length: u32,
    /// Opcode-specific CBOR arguments.
    #[serde(rename = "args")]
    pub args: ByteBuf,
}

impl ControlMessage {
    /// Builds an argument-less message.
    pub fn bare(opcode: u16, request_id: u32, sender_id: u32, transition_id: u32) -> Self {
        Self {
            opcode,
            request_id,
            sender_id,
            transition_id,
            args_length: 0,
            args: ByteBuf::new(),
        }
    }

    /// Builds a message with raw CBOR arguments.
    pub fn with_args(
        opcode: u16,
        request_id: u32,
        sender_id: u32,
        transition_id: u32,
        args: Vec<u8>,
    ) -> Self {
        Self {
            opcode,
            request_id,
            sender_id,
            transition_id,
            args_length: args.len() as u32,
            args: ByteBuf::from(args),
        }
    }

    /// CBOR-encodes the message.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode");
        buf
    }

    /// Decodes a CBOR-encoded message and validates `args_length`.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        let m: Self = ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))?;
        if m.args_length as usize != m.args.len() {
            return Err(CodecError::PayloadSizeMismatch);
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_message_round_trip() {
        let msg = ControlMessage::bare(3, 99, 1, 2);
        let bytes = msg.to_cbor();
        let decoded = ControlMessage::from_cbor(&bytes).unwrap();
        assert_eq!(decoded.opcode, 3);
        assert_eq!(decoded.request_id, 99);
        assert_eq!(decoded.sender_id, 1);
        assert_eq!(decoded.transition_id, 2);
        assert_eq!(decoded.args_length, 0);
        assert!(decoded.args.is_empty());
    }

    #[test]
    fn with_args_round_trip() {
        let args = vec![0xA1u8, 0x00, 0xF5];
        let msg = ControlMessage::with_args(7, 1, 2, 3, args.clone());
        assert_eq!(msg.args_length, 3);
        let decoded = ControlMessage::from_cbor(&msg.to_cbor()).unwrap();
        assert_eq!(decoded.args.as_ref(), args.as_slice());
        assert_eq!(decoded.args_length, 3);
    }

    #[test]
    fn args_length_mismatch_rejected() {
        let mut msg = ControlMessage::bare(1, 0, 0, 0);
        msg.args = serde_bytes::ByteBuf::from(vec![0xFFu8; 5]);
        // args_length still 0, args has 5 bytes → mismatch
        let bytes = {
            let mut buf = Vec::new();
            ciborium::into_writer(&msg, &mut buf).unwrap();
            buf
        };
        assert!(matches!(
            ControlMessage::from_cbor(&bytes),
            Err(CodecError::PayloadSizeMismatch)
        ));
    }

    #[test]
    fn invalid_cbor_returns_decode_error() {
        assert!(matches!(
            ControlMessage::from_cbor(b"\xFF\xFF"),
            Err(CodecError::Decode(_))
        ));
    }
}
