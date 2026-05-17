//! FlatBuffers codec for the Group Protocol Stack (GBP/GTP/GAP/GSP).
//!
//! Alternative wire format to CBOR per gbp_rfc §12.2. Schemas are compiled
//! from `.fbs` files at build time via planus; no `flatc` binary required.
//!
//! # Modules (generated)
//! - [`gbp`] — GbpFrame, ControlMessage, ErrorObject
//! - [`gtp`] — GtpMessage, AttachmentManifest, AttachmentChunk
//! - [`gap`] — GapPayload
//! - [`gsp`] — GspSignal
//!
//! # Quick start
//! ```
//! use gbp_flat::gbp::{GbpFrame, GbpFrameRef};
//! use planus::{Builder, ReadAsRoot};
//!
//! let frame = GbpFrame { version: 1, epoch: 42, ..Default::default() };
//! let mut builder = Builder::new();
//! let bytes = builder.finish(frame, None).to_vec();
//! let read = GbpFrameRef::read_as_root(&bytes).unwrap();
//! assert_eq!(read.version().unwrap(), 1);
//! ```

// Re-export planus so downstream crates can use it without adding it directly.
pub use planus;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[cfg(test)]
mod tests {
    use planus::{Builder, ReadAsRoot};

    // ── GBP ──────────────────────────────────────────────────────────────────

    #[test]
    fn gbp_frame_round_trip() {
        let frame = crate::gbp::GbpFrame {
            version: 1,
            group_id: Some(vec![0u8; 16]),
            epoch: 42,
            transition_id: 7,
            stream_type: 1,
            stream_id: 0,
            flags: 0,
            sequence_no: 100,
            encrypted_payload: Some(b"hello".to_vec()),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(frame.clone(), None).to_vec();
        let r = crate::gbp::GbpFrameRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.version().unwrap(), 1);
        assert_eq!(r.epoch().unwrap(), 42);
        assert_eq!(r.sequence_no().unwrap(), 100);
        assert_eq!(r.encrypted_payload().unwrap(), Some(b"hello".as_slice()));
        let owned: crate::gbp::GbpFrame = r.try_into().unwrap();
        assert_eq!(owned.version, frame.version);
        assert_eq!(owned.encrypted_payload, frame.encrypted_payload);
    }

    #[test]
    fn gbp_frame_defaults_on_empty() {
        let frame = crate::gbp::GbpFrame::default();
        let mut builder = Builder::new();
        let bytes = builder.finish(frame, None).to_vec();
        let r = crate::gbp::GbpFrameRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.version().unwrap(), 0);
        assert_eq!(r.epoch().unwrap(), 0);
    }

    #[test]
    fn control_message_round_trip() {
        let msg = crate::gbp::ControlMessage {
            opcode: 3,
            request_id: 99,
            sender_id: 1,
            transition_id: 2,
            args: Some(vec![0xA0]),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(msg, None).to_vec();
        let r = crate::gbp::ControlMessageRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.opcode().unwrap(), 3);
        assert_eq!(r.request_id().unwrap(), 99);
        assert_eq!(r.args().unwrap(), Some([0xA0u8].as_slice()));
    }

    #[test]
    fn error_object_round_trip() {
        let err = crate::gbp::ErrorObject {
            code: 404,
            class: 1,
            retryable: true,
            fatal: false,
            reason: Some("not found".to_string()),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(err, None).to_vec();
        let r = crate::gbp::ErrorObjectRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.code().unwrap(), 404);
        assert_eq!(r.retryable().unwrap(), true);
        assert_eq!(r.reason().unwrap(), Some("not found"));
    }

    // ── GTP ──────────────────────────────────────────────────────────────────

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
            content: Some(b"hello".to_vec()),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(msg, None).to_vec();
        let r = crate::gtp::GtpMessageRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.message_id().unwrap(), 12345);
        assert_eq!(r.timestamp_ms().unwrap(), 1_700_000_000_000);
        assert_eq!(r.content().unwrap(), Some(b"hello".as_slice()));
    }

    #[test]
    fn attachment_manifest_round_trip() {
        let m = crate::gtp::AttachmentManifest {
            attachment_id: 1,
            filename: Some("test.png".to_string()),
            mime_type: Some("image/png".to_string()),
            total_size: 1024,
            chunk_count: 1,
            sha256: Some(vec![0u8; 32]),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(m, None).to_vec();
        let r = crate::gtp::AttachmentManifestRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.attachment_id().unwrap(), 1);
        assert_eq!(r.filename().unwrap(), Some("test.png"));
        assert_eq!(r.chunk_count().unwrap(), 1);
        assert_eq!(r.sha256().unwrap().map(|s| s.len()), Some(32));
    }

    #[test]
    fn attachment_chunk_round_trip() {
        let c = crate::gtp::AttachmentChunk {
            attachment_id: 1,
            chunk_index: 0,
            chunk_count: 1,
            data: Some(vec![1, 2, 3, 4]),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(c, None).to_vec();
        let r = crate::gtp::AttachmentChunkRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.attachment_id().unwrap(), 1);
        assert_eq!(r.data().unwrap(), Some([1u8, 2, 3, 4].as_slice()));
    }

    // ── GAP ──────────────────────────────────────────────────────────────────

    #[test]
    fn gap_payload_round_trip() {
        let p = crate::gap::GapPayload {
            media_source_id: 5,
            rtp_sequence: 100,
            rtp_timestamp: 960,
            key_phase: 2,
            opus_frame: Some(vec![0xAB, 0xCD]),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(p, None).to_vec();
        let r = crate::gap::GapPayloadRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.media_source_id().unwrap(), 5);
        assert_eq!(r.rtp_timestamp().unwrap(), 960);
        assert_eq!(r.opus_frame().unwrap(), Some([0xABu8, 0xCD].as_slice()));
    }

    // ── GSP ──────────────────────────────────────────────────────────────────

    #[test]
    fn gsp_signal_round_trip() {
        let s = crate::gsp::GspSignal {
            signal_type: 1,
            request_id: 42,
            sender_id: 3,
            role_claim: 0,
            args_length: 0,
            args: None,
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(s, None).to_vec();
        let r = crate::gsp::GspSignalRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.signal_type().unwrap(), 1);
        assert_eq!(r.request_id().unwrap(), 42);
        assert_eq!(r.sender_id().unwrap(), 3);
    }

    #[test]
    fn gsp_signal_with_args_round_trip() {
        let s = crate::gsp::GspSignal {
            signal_type: 3,
            request_id: 1,
            sender_id: 2,
            role_claim: 1,
            args_length: 2,
            args: Some(vec![0xA1, 0x00]),
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(s, None).to_vec();
        let r = crate::gsp::GspSignalRef::read_as_root(&bytes).unwrap();
        assert_eq!(r.args_length().unwrap(), 2);
        assert_eq!(r.args().unwrap(), Some([0xA1u8, 0x00].as_slice()));
    }

    // ── Cross-type ───────────────────────────────────────────────────────────

    #[test]
    fn encoded_bytes_are_nonempty() {
        let frame = crate::gbp::GbpFrame {
            version: 1,
            ..Default::default()
        };
        let mut builder = Builder::new();
        let bytes = builder.finish(frame, None).to_vec();
        assert!(!bytes.is_empty());
    }
}
