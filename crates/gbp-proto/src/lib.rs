//! Protobuf codec for the Group Protocol Stack (GBP/GTP/GAP/GSP).
//!
//! Alternative wire format to CBOR per gbp_rfc §12.2. All message types
//! derive [`prost::Message`] so they can be encoded/decoded without protoc.
//!
//! # Modules
//! - [`gbp`] — GbpFrame, ControlMessage, ErrorObject
//! - [`gtp`] — GtpMessage, AttachmentManifest, AttachmentChunk
//! - [`gap`] — GapPayload
//! - [`gsp`] — GspSignal

/// GBP (Group Base Protocol) messages.
pub mod gbp {
    /// A framed GBP envelope carrying an encrypted payload.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct GbpFrame {
        /// Protocol version.
        #[prost(uint32, tag = "1")]
        pub version: u32,
        /// 16-byte group identifier.
        #[prost(bytes = "vec", tag = "2")]
        pub group_id: Vec<u8>,
        /// MLS epoch.
        #[prost(uint64, tag = "3")]
        pub epoch: u64,
        /// Transition identifier.
        #[prost(uint32, tag = "4")]
        pub transition_id: u32,
        /// Stream type discriminant.
        #[prost(uint32, tag = "5")]
        pub stream_type: u32,
        /// Stream identifier.
        #[prost(uint32, tag = "6")]
        pub stream_id: u32,
        /// Frame flags bitmask.
        #[prost(uint32, tag = "7")]
        pub flags: u32,
        /// Monotonic sequence number.
        #[prost(uint64, tag = "8")]
        pub sequence_no: u64,
        /// AEAD-encrypted payload bytes.
        #[prost(bytes = "vec", tag = "9")]
        pub encrypted_payload: Vec<u8>,
    }

    /// GBP control-plane message.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct ControlMessage {
        /// Control opcode.
        #[prost(uint32, tag = "1")]
        pub opcode: u32,
        /// Request/response correlation identifier.
        #[prost(uint32, tag = "2")]
        pub request_id: u32,
        /// Sender member identifier.
        #[prost(uint32, tag = "3")]
        pub sender_id: u32,
        /// Transition this message belongs to.
        #[prost(uint32, tag = "4")]
        pub transition_id: u32,
        /// Opcode-specific CBOR-encoded arguments.
        #[prost(bytes = "vec", tag = "5")]
        pub args: Vec<u8>,
    }

    /// GBP structured error.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct ErrorObject {
        /// Numeric error code.
        #[prost(uint32, tag = "1")]
        pub code: u32,
        /// Error class (e.g. transport, crypto, protocol).
        #[prost(uint32, tag = "2")]
        pub class: u32,
        /// Whether the sender may retry after a back-off.
        #[prost(bool, tag = "3")]
        pub retryable: bool,
        /// Whether this error is unrecoverable for the session.
        #[prost(bool, tag = "4")]
        pub fatal: bool,
        /// Human-readable reason string.
        #[prost(string, tag = "5")]
        pub reason: String,
    }
}

/// GTP (Group Text Protocol) messages.
pub mod gtp {
    /// A GTP text message envelope.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct GtpMessage {
        /// Globally unique message identifier.
        #[prost(uint64, tag = "1")]
        pub message_id: u64,
        /// Sender member identifier.
        #[prost(uint32, tag = "2")]
        pub sender_id: u32,
        /// Wall-clock send time in milliseconds since Unix epoch.
        #[prost(uint64, tag = "3")]
        pub timestamp_ms: u64,
        /// Request/response correlation identifier.
        #[prost(uint32, tag = "4")]
        pub request_id: u32,
        /// Message flags bitmask.
        #[prost(uint32, tag = "5")]
        pub flags: u32,
        /// Content-type discriminant.
        #[prost(uint32, tag = "6")]
        pub content_type: u32,
        /// Byte length of [`content`](Self::content).
        #[prost(uint32, tag = "7")]
        pub content_length: u32,
        /// Message payload bytes (text, JSON, or attachment reference).
        #[prost(bytes = "vec", tag = "8")]
        pub content: Vec<u8>,
    }

    /// Attachment manifest sent before the chunk stream.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct AttachmentManifest {
        /// Unique attachment identifier (sender-scoped).
        #[prost(uint64, tag = "1")]
        pub attachment_id: u64,
        /// Original filename (UTF-8, no path components).
        #[prost(string, tag = "2")]
        pub filename: String,
        /// MIME type string.
        #[prost(string, tag = "3")]
        pub mime_type: String,
        /// Total byte length of the reassembled payload.
        #[prost(uint64, tag = "4")]
        pub total_size: u64,
        /// Number of chunks.
        #[prost(uint32, tag = "5")]
        pub chunk_count: u32,
        /// SHA-256 hash of the complete payload (32 bytes).
        #[prost(bytes = "vec", tag = "6")]
        pub sha256: Vec<u8>,
    }

    /// One chunk of an attachment payload.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct AttachmentChunk {
        /// Attachment this chunk belongs to.
        #[prost(uint64, tag = "1")]
        pub attachment_id: u64,
        /// Zero-based chunk index.
        #[prost(uint32, tag = "2")]
        pub chunk_index: u32,
        /// Total number of chunks.
        #[prost(uint32, tag = "3")]
        pub chunk_count: u32,
        /// Chunk payload bytes.
        #[prost(bytes = "vec", tag = "4")]
        pub data: Vec<u8>,
    }
}

/// GAP (Group Audio Protocol) messages.
pub mod gap {
    /// An encrypted audio frame payload.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct GapPayload {
        /// Audio source identifier.
        #[prost(uint32, tag = "1")]
        pub media_source_id: u32,
        /// RTP sequence number (16-bit widened to u32).
        #[prost(uint32, tag = "2")]
        pub rtp_sequence: u32,
        /// 48 kHz RTP timestamp.
        #[prost(uint64, tag = "3")]
        pub rtp_timestamp: u64,
        /// Key phase (MLS epoch binding).
        #[prost(uint32, tag = "4")]
        pub key_phase: u32,
        /// Opus-encoded frame bytes.
        #[prost(bytes = "vec", tag = "5")]
        pub opus_frame: Vec<u8>,
    }
}

/// GSP (Group Signaling Protocol) messages.
pub mod gsp {
    /// A GSP signal envelope.
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct GspSignal {
        /// Signal type discriminant.
        #[prost(uint32, tag = "1")]
        pub signal_type: u32,
        /// Request/response correlation identifier.
        #[prost(uint32, tag = "2")]
        pub request_id: u32,
        /// Sender member identifier.
        #[prost(uint32, tag = "3")]
        pub sender_id: u32,
        /// Role claim (used by ROLE_CHANGE signals).
        #[prost(uint32, tag = "4")]
        pub role_claim: u32,
        /// Declared byte length of [`args`](Self::args).
        #[prost(uint32, tag = "5")]
        pub args_length: u32,
        /// Opcode-specific CBOR-encoded arguments.
        #[prost(bytes = "vec", tag = "6")]
        pub args: Vec<u8>,
    }
}

#[cfg(test)]
mod tests {
    use prost::Message;

    #[test]
    fn gbp_frame_round_trip() {
        let frame = crate::gbp::GbpFrame {
            version: 1,
            group_id: vec![0u8; 16],
            epoch: 42,
            transition_id: 7,
            stream_type: 1,
            stream_id: 0,
            flags: 0,
            sequence_no: 100,
            encrypted_payload: b"hello".to_vec(),
        };
        let encoded = frame.encode_to_vec();
        let decoded = crate::gbp::GbpFrame::decode(encoded.as_slice()).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn control_message_round_trip() {
        let msg = crate::gbp::ControlMessage {
            opcode: 3,
            request_id: 99,
            sender_id: 1,
            transition_id: 2,
            args: vec![0xA0],
        };
        let encoded = msg.encode_to_vec();
        let decoded = crate::gbp::ControlMessage::decode(encoded.as_slice()).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn error_object_round_trip() {
        let err = crate::gbp::ErrorObject {
            code: 404,
            class: 1,
            retryable: true,
            fatal: false,
            reason: "not found".to_string(),
        };
        let encoded = err.encode_to_vec();
        let decoded = crate::gbp::ErrorObject::decode(encoded.as_slice()).unwrap();
        assert_eq!(err, decoded);
    }

    #[test]
    fn gtp_message_round_trip() {
        let msg = crate::gtp::GtpMessage {
            message_id: 12345,
            sender_id: 1,
            timestamp_ms: 1_700_000_000_000,
            request_id: 0,
            flags: 0,
            content_type: 1,
            content_length: 5,
            content: b"hello".to_vec(),
        };
        let encoded = msg.encode_to_vec();
        let decoded = crate::gtp::GtpMessage::decode(encoded.as_slice()).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn attachment_manifest_round_trip() {
        let m = crate::gtp::AttachmentManifest {
            attachment_id: 1,
            filename: "test.png".to_string(),
            mime_type: "image/png".to_string(),
            total_size: 1024,
            chunk_count: 1,
            sha256: vec![0u8; 32],
        };
        let encoded = m.encode_to_vec();
        let decoded = crate::gtp::AttachmentManifest::decode(encoded.as_slice()).unwrap();
        assert_eq!(m, decoded);
    }

    #[test]
    fn attachment_chunk_round_trip() {
        let c = crate::gtp::AttachmentChunk {
            attachment_id: 1,
            chunk_index: 0,
            chunk_count: 1,
            data: vec![1, 2, 3, 4],
        };
        let encoded = c.encode_to_vec();
        let decoded = crate::gtp::AttachmentChunk::decode(encoded.as_slice()).unwrap();
        assert_eq!(c, decoded);
    }

    #[test]
    fn gap_payload_round_trip() {
        let p = crate::gap::GapPayload {
            media_source_id: 5,
            rtp_sequence: 100,
            rtp_timestamp: 960,
            key_phase: 2,
            opus_frame: vec![0xAB, 0xCD],
        };
        let encoded = p.encode_to_vec();
        let decoded = crate::gap::GapPayload::decode(encoded.as_slice()).unwrap();
        assert_eq!(p, decoded);
    }

    #[test]
    fn gsp_signal_round_trip() {
        let s = crate::gsp::GspSignal {
            signal_type: 1,
            request_id: 42,
            sender_id: 3,
            role_claim: 0,
            args_length: 0,
            args: vec![],
        };
        let encoded = s.encode_to_vec();
        let decoded = crate::gsp::GspSignal::decode(encoded.as_slice()).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn empty_frame_decodes_to_defaults() {
        let decoded = crate::gbp::GbpFrame::decode(&[][..]).unwrap();
        assert_eq!(decoded.version, 0);
        assert!(decoded.encrypted_payload.is_empty());
    }

    #[test]
    fn encoded_size_is_nonzero_for_nonempty_frame() {
        let frame = crate::gbp::GbpFrame {
            version: 1,
            sequence_no: 1,
            ..Default::default()
        };
        assert!(!frame.encode_to_vec().is_empty());
    }
}
