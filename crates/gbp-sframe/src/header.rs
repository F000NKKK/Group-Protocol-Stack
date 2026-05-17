use crate::error::SFrameError;

/// Decoded SFrame header fields.
///
/// Wire format (draft-ietf-sframe-enc §4.3):
/// ```text
/// ┌─┬──────────┬──────────┬───────────────┬───────────────┐
/// │V│  K (3)   │  C (4)   │  KID  (var)   │  CTR  (var)   │
/// └─┴──────────┴──────────┴───────────────┴───────────────┘
/// ```
/// * `V` (1 bit) — version, always `0` for SFrame v1.
/// * `K` (3 bits) — KID length in bytes minus one (0 → 1 byte, 7 → 8 bytes).
/// * `C` (4 bits) — CTR length in bytes minus one (0 → 1 byte, 15 → 16 bytes).
/// * `KID` — Key ID: `(epoch << 16) | leaf_index`.
/// * `CTR` — monotonic per-sender counter, big-endian.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SFrameHeader {
    /// Key ID: encodes both epoch and sender leaf index.
    pub kid: u64,
    /// Per-sender monotonic counter.
    pub ctr: u64,
}

impl SFrameHeader {
    /// Constructs the KID from an epoch and a leaf index.
    ///
    /// Uses the lower 16 bits of `leaf_index` (supports up to 65 535 senders
    /// per epoch) and the full 48 available bits of `epoch`.
    #[inline]
    pub fn kid_from(epoch: u64, leaf_index: u32) -> u64 {
        (epoch << 16) | (u64::from(leaf_index) & 0xFFFF)
    }

    /// Extracts the epoch from a KID.
    #[inline]
    pub fn epoch_from_kid(kid: u64) -> u64 {
        kid >> 16
    }

    /// Extracts the leaf index from a KID.
    #[inline]
    pub fn leaf_from_kid(kid: u64) -> u32 {
        (kid & 0xFFFF) as u32
    }

    /// Encodes the header into bytes.
    ///
    /// Layout: `[flags_byte] [kid_bytes...] [ctr_bytes...]`
    /// where `flags_byte = 0 | ((kid_len-1) << 4) | (ctr_len-1)`.
    pub fn encode(self) -> Vec<u8> {
        let kid_bytes = encode_uint(self.kid);
        let ctr_bytes = encode_uint(self.ctr);

        let k = (kid_bytes.len() as u8 - 1) & 0x07; // 3 bits
        let c = (ctr_bytes.len() as u8 - 1) & 0x0F; // 4 bits
        // V=0 (bit 7), K (bits 6-4), C (bits 3-0)
        let flags = (k << 4) | c;

        let mut out = Vec::with_capacity(1 + kid_bytes.len() + ctr_bytes.len());
        out.push(flags);
        out.extend_from_slice(&kid_bytes);
        out.extend_from_slice(&ctr_bytes);
        out
    }

    /// Decodes a header from the front of `data`.
    ///
    /// Returns the decoded header and the number of bytes consumed.
    pub fn decode(data: &[u8]) -> Result<(Self, usize), SFrameError> {
        if data.is_empty() {
            return Err(SFrameError::Header("empty payload".into()));
        }
        let flags = data[0];
        // V must be 0.
        if flags & 0x80 != 0 {
            return Err(SFrameError::Header(format!(
                "unsupported SFrame version (V bit set in flags {flags:#04x})"
            )));
        }
        let kid_len = ((flags >> 4) & 0x07) as usize + 1; // 1-8 bytes
        let ctr_len = (flags & 0x0F) as usize + 1; // 1-16 bytes
        let total = 1 + kid_len + ctr_len;

        if data.len() < total {
            return Err(SFrameError::Header(format!(
                "need {total} bytes for header but only {} available",
                data.len()
            )));
        }

        let kid = decode_uint_be(&data[1..1 + kid_len]);
        let ctr = decode_uint_be(&data[1 + kid_len..total]);
        Ok((Self { kid, ctr }, total))
    }
}

/// Encodes an integer as the minimal number of big-endian bytes (at least 1).
fn encode_uint(v: u64) -> Vec<u8> {
    let needed = ((64 - v.leading_zeros() + 7) / 8) as usize;
    let needed = needed.max(1);
    v.to_be_bytes()[8 - needed..].to_vec()
}

/// Decodes a big-endian byte slice into a u64.
fn decode_uint_be(data: &[u8]) -> u64 {
    data.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        for (kid, ctr) in [
            (0, 0),
            (1, 1),
            (0xFFFF, 0),
            (0x0001_FFFF, 0xDEAD),
            (u64::MAX >> 16, u64::from(u16::MAX)),
        ] {
            let hdr = SFrameHeader { kid, ctr };
            let encoded = hdr.encode();
            let (decoded, consumed) = SFrameHeader::decode(&encoded).unwrap();
            assert_eq!(decoded, hdr, "kid={kid:#x} ctr={ctr:#x}");
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn kid_round_trip() {
        let epoch = 42u64;
        let leaf = 7u32;
        let kid = SFrameHeader::kid_from(epoch, leaf);
        assert_eq!(SFrameHeader::epoch_from_kid(kid), epoch);
        assert_eq!(SFrameHeader::leaf_from_kid(kid), leaf);
    }
}
