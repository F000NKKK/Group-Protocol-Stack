//! GTP attachment support: chunking, manifest, and integrity verification
//! (gtp_rfc §6).
//!
//! Flow:
//! 1. Sender splits data into chunks via [`AttachmentSender::new`] and sends
//!    the manifest message followed by individual [`AttachmentChunk`] messages.
//! 2. Receiver feeds chunks to [`AttachmentAssembler`]; when all chunks arrive
//!    it verifies the SHA-256 hash and returns the complete payload.

use gbp::CodecError;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Default chunk size: 64 KiB.
pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;

/// Manifest sent as the body of a `ContentType::AttachmentRef` message.
/// Describes the full attachment so the receiver can pre-allocate and verify.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachmentManifest {
    /// Unique attachment identifier (sender-scoped).
    #[serde(rename = "aid")]
    pub attachment_id: u64,
    /// Original filename (UTF-8, no path components).
    #[serde(rename = "name")]
    pub filename: String,
    /// MIME type string (e.g. `"image/png"`).
    #[serde(rename = "mime")]
    pub mime_type: String,
    /// Total byte length of the reassembled payload.
    #[serde(rename = "size")]
    pub total_size: u64,
    /// Number of chunks.
    #[serde(rename = "nc")]
    pub chunk_count: u32,
    /// SHA-256 hash of the complete payload (32 bytes).
    #[serde(rename = "hash")]
    pub sha256: ByteBuf,
}

impl AttachmentManifest {
    /// CBOR-encodes the manifest.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode");
        buf
    }

    /// Decodes a CBOR manifest.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))
    }
}

/// One chunk of an attachment payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachmentChunk {
    /// Attachment this chunk belongs to.
    #[serde(rename = "aid")]
    pub attachment_id: u64,
    /// Zero-based chunk index.
    #[serde(rename = "idx")]
    pub chunk_index: u32,
    /// Total number of chunks (redundant but useful for validation).
    #[serde(rename = "nc")]
    pub chunk_count: u32,
    /// Chunk payload bytes.
    #[serde(rename = "data")]
    pub data: ByteBuf,
}

impl AttachmentChunk {
    /// CBOR-encodes the chunk.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf).expect("cbor encode");
        buf
    }

    /// Decodes a CBOR chunk.
    pub fn from_cbor(data: &[u8]) -> Result<Self, CodecError> {
        ciborium::from_reader(data).map_err(|e| CodecError::Decode(e.to_string()))
    }
}

/// Errors from attachment assembly.
#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    /// CBOR decode failed.
    #[error("decode: {0}")]
    Decode(#[from] CodecError),
    /// `chunk_index` is out of range for the declared `chunk_count`.
    #[error("chunk index {idx} out of range (count={count})")]
    ChunkOutOfRange {
        /// The out-of-range index.
        idx: u32,
        /// Declared total chunk count.
        count: u32,
    },
    /// The reassembled payload's SHA-256 does not match the manifest.
    #[error("integrity check failed: hash mismatch")]
    HashMismatch,
    /// Not all chunks have arrived yet.
    #[error("incomplete: {received}/{total} chunks received")]
    Incomplete {
        /// Number of chunks received so far.
        received: u32,
        /// Total expected chunks.
        total: u32,
    },
}

/// Splits a payload into chunks and produces the manifest.
pub struct AttachmentSender {
    /// The manifest to broadcast first.
    pub manifest: AttachmentManifest,
    /// Ready-to-send CBOR-encoded chunks.
    pub chunks: Vec<Vec<u8>>,
}

impl AttachmentSender {
    /// Splits `data` into chunks of `chunk_size` bytes (last chunk may be
    /// smaller), computes the SHA-256 manifest, and encodes everything.
    pub fn new(
        attachment_id: u64,
        filename: impl Into<String>,
        mime_type: impl Into<String>,
        data: &[u8],
        chunk_size: usize,
    ) -> Self {
        let hash = Sha256::digest(data);
        let chunk_size = chunk_size.max(1);
        let raw_chunks: Vec<&[u8]> = data.chunks(chunk_size).collect();
        let chunk_count = raw_chunks.len() as u32;

        let manifest = AttachmentManifest {
            attachment_id,
            filename: filename.into(),
            mime_type: mime_type.into(),
            total_size: data.len() as u64,
            chunk_count,
            sha256: ByteBuf::from(hash.as_slice().to_vec()),
        };

        let chunks = raw_chunks
            .into_iter()
            .enumerate()
            .map(|(i, slice)| {
                AttachmentChunk {
                    attachment_id,
                    chunk_index: i as u32,
                    chunk_count,
                    data: ByteBuf::from(slice.to_vec()),
                }
                .to_cbor()
            })
            .collect();

        Self { manifest, chunks }
    }
}

/// Reassembles incoming chunks and verifies integrity when complete.
pub struct AttachmentAssembler {
    manifest: AttachmentManifest,
    received: HashMap<u32, Vec<u8>>,
}

impl AttachmentAssembler {
    /// Creates an assembler for the given manifest.
    pub fn new(manifest: AttachmentManifest) -> Self {
        Self {
            manifest,
            received: HashMap::new(),
        }
    }

    /// Returns the manifest.
    pub fn manifest(&self) -> &AttachmentManifest {
        &self.manifest
    }

    /// How many chunks have been received so far.
    pub fn received_count(&self) -> u32 {
        self.received.len() as u32
    }

    /// Returns `true` when all chunks have arrived.
    pub fn is_complete(&self) -> bool {
        self.received.len() as u32 == self.manifest.chunk_count
    }

    /// Feeds a decoded chunk. Duplicate indices are silently ignored.
    pub fn push(&mut self, chunk: AttachmentChunk) -> Result<(), AttachmentError> {
        if chunk.chunk_index >= self.manifest.chunk_count {
            return Err(AttachmentError::ChunkOutOfRange {
                idx: chunk.chunk_index,
                count: self.manifest.chunk_count,
            });
        }
        self.received
            .entry(chunk.chunk_index)
            .or_insert_with(|| chunk.data.into_vec());
        Ok(())
    }

    /// Assembles the payload once all chunks have arrived and verifies the
    /// SHA-256 hash against the manifest. Returns the complete byte vector.
    pub fn assemble(self) -> Result<Vec<u8>, AttachmentError> {
        let total = self.manifest.chunk_count;
        let received = self.received.len() as u32;
        if received < total {
            return Err(AttachmentError::Incomplete { received, total });
        }
        let mut payload = Vec::with_capacity(self.manifest.total_size as usize);
        for i in 0..total {
            payload.extend_from_slice(self.received.get(&i).unwrap());
        }
        let hash = Sha256::digest(&payload);
        if hash.as_slice() != self.manifest.sha256.as_ref() {
            return Err(AttachmentError::HashMismatch);
        }
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn round_trip_small_payload() {
        let data = sample_data(100);
        let sender = AttachmentSender::new(
            1,
            "file.bin",
            "application/octet-stream",
            &data,
            DEFAULT_CHUNK_SIZE,
        );
        assert_eq!(sender.manifest.chunk_count, 1);
        let mut asm = AttachmentAssembler::new(sender.manifest);
        for cbor in &sender.chunks {
            let chunk = AttachmentChunk::from_cbor(cbor).unwrap();
            asm.push(chunk).unwrap();
        }
        let result = asm.assemble().unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn round_trip_multi_chunk() {
        let data = sample_data(300);
        let sender = AttachmentSender::new(2, "multi.bin", "application/octet-stream", &data, 100);
        assert_eq!(sender.manifest.chunk_count, 3);
        let mut asm = AttachmentAssembler::new(sender.manifest);
        for cbor in &sender.chunks {
            let chunk = AttachmentChunk::from_cbor(cbor).unwrap();
            asm.push(chunk).unwrap();
        }
        assert!(asm.is_complete());
        let result = asm.assemble().unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn out_of_order_chunks_reassemble_correctly() {
        let data = sample_data(250);
        let sender = AttachmentSender::new(3, "ooo.bin", "application/octet-stream", &data, 100);
        let mut asm = AttachmentAssembler::new(sender.manifest);
        // Feed chunks in reverse order.
        for cbor in sender.chunks.iter().rev() {
            let chunk = AttachmentChunk::from_cbor(cbor).unwrap();
            asm.push(chunk).unwrap();
        }
        let result = asm.assemble().unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn duplicate_chunk_ignored() {
        let data = sample_data(100);
        let sender = AttachmentSender::new(
            4,
            "dup.bin",
            "application/octet-stream",
            &data,
            DEFAULT_CHUNK_SIZE,
        );
        let mut asm = AttachmentAssembler::new(sender.manifest);
        let chunk = AttachmentChunk::from_cbor(&sender.chunks[0]).unwrap();
        asm.push(chunk.clone()).unwrap();
        asm.push(chunk).unwrap(); // duplicate — should not error
        let result = asm.assemble().unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn hash_mismatch_detected() {
        let data = sample_data(100);
        let sender = AttachmentSender::new(
            5,
            "bad.bin",
            "application/octet-stream",
            &data,
            DEFAULT_CHUNK_SIZE,
        );
        let mut manifest = sender.manifest;
        // Corrupt the hash.
        manifest.sha256[0] ^= 0xFF;
        let mut asm = AttachmentAssembler::new(manifest);
        let chunk = AttachmentChunk::from_cbor(&sender.chunks[0]).unwrap();
        asm.push(chunk).unwrap();
        assert!(matches!(asm.assemble(), Err(AttachmentError::HashMismatch)));
    }

    #[test]
    fn incomplete_returns_error() {
        let data = sample_data(300);
        let sender = AttachmentSender::new(6, "inc.bin", "application/octet-stream", &data, 100);
        let mut asm = AttachmentAssembler::new(sender.manifest);
        // Feed only the first chunk.
        let chunk = AttachmentChunk::from_cbor(&sender.chunks[0]).unwrap();
        asm.push(chunk).unwrap();
        assert!(matches!(
            asm.assemble(),
            Err(AttachmentError::Incomplete { .. })
        ));
    }

    #[test]
    fn chunk_out_of_range_rejected() {
        let data = sample_data(100);
        let sender = AttachmentSender::new(
            7,
            "oor.bin",
            "application/octet-stream",
            &data,
            DEFAULT_CHUNK_SIZE,
        );
        let mut asm = AttachmentAssembler::new(sender.manifest);
        let bad_chunk = AttachmentChunk {
            attachment_id: 7,
            chunk_index: 99,
            chunk_count: 1,
            data: ByteBuf::new(),
        };
        assert!(matches!(
            asm.push(bad_chunk),
            Err(AttachmentError::ChunkOutOfRange { .. })
        ));
    }

    #[test]
    fn manifest_cbor_round_trip() {
        let data = sample_data(50);
        let sender = AttachmentSender::new(8, "rt.bin", "text/plain", &data, DEFAULT_CHUNK_SIZE);
        let encoded = sender.manifest.to_cbor();
        let decoded = AttachmentManifest::from_cbor(&encoded).unwrap();
        assert_eq!(decoded.attachment_id, 8);
        assert_eq!(decoded.filename, "rt.bin");
        assert_eq!(decoded.chunk_count, 1);
    }
}
