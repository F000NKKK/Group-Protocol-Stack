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
