# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP SFrame Extension — E2EE for GAP Audio Streams

## Abstract

This document specifies the SFrame key derivation and header encoding used by
the Group Protocol Stack (GBP) to provide end-to-end encryption for GAP
(Group Audio Protocol) media payloads.  The scheme derives per-sender AES-GCM
keys from the MLS ExportSecret, encodes sender identity in a compact SFrame
header, and maintains per-sender 1024-entry sliding-window replay protection.

## Status of This Memo

This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.

## 1. Introduction

GBP's GAP sub-protocol (StreamType 1) delivers Opus audio frames through an
SFU that performs selective forwarding, packet pacing and NACK handling.  The
SFU requires visibility of RTP/transport headers but MUST NOT have access to
media payload.

SFrame [draft-ietf-sframe-enc] solves this by encrypting only the media
payload while leaving transport headers in the clear.  GBP adopts SFrame with
its own KDF label, KID encoding and HKDF info strings as specified in this
document.

## 2. Conventions

BCP 14 requirement words from [RFC2119] and [RFC8174] apply.

## 3. Protocol Position

```
┌──────────────────────────────────────────────────┐
│         Transport encryption (SRTP / DTLS)       │  ← client ↔ SFU
│  ┌────────────────────────────────────────────┐  │
│  │      SFrame (this specification)           │  │  ← E2E client ↔ client
│  │   ┌──────────────────────────────────────┐ │  │
│  │   │  Encoded media frame (Opus / VP8)    │ │  │
│  │   └──────────────────────────────────────┘ │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

SFrame payloads are carried as the `encrypted_payload` field of GAP-type
GBP frames (StreamType = 1) after MLS AEAD has been removed (i.e. the GBP
node delivers plaintext bytes which are themselves the SFrame payload).

## 4. Key Schedule

### 4.1 Base Key Derivation

After every MLS commit, each member MUST derive:

```
sframe_base_key = MLS.ExportSecret(
    label   = <application label>,     ; e.g. "gbp/sframe v1"
    context = epoch.to_be_bytes(),     ; 8 bytes big-endian
    length  = 32                       ; 256 bits
)
```

The `label` is application-defined and MUST be agreed out of band.  GBP
implementations SHOULD use `"gbp/sframe v1"` unless a deployment-specific
label is required.

### 4.2 Per-Sender Key Derivation

For each participant with MLS leaf index `i`:

```
participant_key_i = HKDF-Expand(
    PRK  = sframe_base_key,
    info = "gbp sframe key " || leaf_index_i.to_be_bytes(),  ; 4 bytes BE
    L    = L_key                                             ; 16 or 32 bytes
)

participant_salt_i = HKDF-Expand(
    PRK  = sframe_base_key,
    info = "gbp sframe salt " || leaf_index_i.to_be_bytes(), ; 4 bytes BE
    L    = 12
)
```

`L_key` is determined by the ciphersuite:
- AES-128-GCM: `L_key = 16`.
- AES-256-GCM: `L_key = 32`.

The hash function for HKDF is SHA-256 in all cases.

### 4.3 Frame Nonce

The per-frame 12-byte nonce is:

```
nonce = participant_salt_i XOR (CTR.to_le_bytes() || 0x00_00_00_00)
```

where `CTR.to_le_bytes()` is the 8-byte little-endian representation of the
per-sender counter and the trailing 4 bytes are zero.

## 5. SFrame Header

The wire header follows [draft-ietf-sframe-enc §4.3]:

```
 0                   1                   2
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- - -
|V| K (3-bit) |  C (4-bit)  |  KID ...  CTR ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- - -
```

* `V` (1 bit) — version, MUST be `0`.
* `K` (3 bits) — KID length in bytes, minus one (0 → 1 byte, 7 → 8 bytes).
* `C` (4 bits) — CTR length in bytes, minus one (0 → 1 byte, 15 → 16 bytes).
* `KID` — Key ID, variable-length big-endian unsigned integer.
* `CTR` — per-sender counter, variable-length big-endian unsigned integer.

### 5.1 KID Encoding

```
KID = (epoch << 16) | (leaf_index & 0xFFFF)
```

The lower 16 bits carry the sender's MLS leaf index (supports up to 65 535
concurrent senders per epoch).  Bits 16 and above carry the MLS epoch.

### 5.2 Full SFrame Payload Layout

```
SFrame payload = SFrame_header || AEAD_ciphertext || AEAD_tag
```

The payload is placed into the `encrypted_payload` field of the GBP frame.

## 6. AEAD Encryption

```
ciphertext, tag = AEAD.Seal(
    key    = participant_key_i,
    nonce  = nonce,
    plain  = encoded_media_frame,
    aad    = SFrame_header || extra_aad
)
```

`extra_aad` is caller-supplied additional data (e.g. an RTP header); it MUST
be identical on both encrypt and decrypt sides.  If no extra AAD is needed,
callers MUST pass an empty byte string.

## 7. Key Rotation

`sframe_base_key` MUST be re-derived on every MLS commit.  This provides
post-compromise security at the group level: a compromised epoch's key is
inaccessible after the next commit.

Applications MUST create a new `SFrameSession` after every successful
`EXECUTE_TRANSITION` and MUST discard encryptors and decryptors from the
previous epoch.

## 8. Replay Protection

Implementations MUST maintain a per-`(KID, sender)` 1024-entry sliding-window
replay window.

* Counters more than 1024 positions behind the current highest-seen counter
  MUST be rejected.
* Duplicate counters within the window MUST be rejected before decryption.
* The replay window MUST be reset when the epoch changes.

## 9. Ciphersuite

| Suite           | Key length | AEAD        |
|-----------------|-----------|-------------|
| AES-128-GCM     | 16 bytes  | AES-128-GCM |
| AES-256-GCM     | 32 bytes  | AES-256-GCM |

The default ciphersuite is AES-128-GCM.  AES-256-GCM is available for
high-assurance deployments.

## 10. Implementation Notes

The `gbp-sframe` Rust crate implements this specification.  The
`gbp_sframe_*` FFI family in `gbp-stack-ffi` exposes it to .NET, Node.js
and Python consumers.

## 11. Security Considerations

- Key reuse: the per-sender nonce construction ensures no two frames with the
  same `(epoch, leaf_index, CTR)` tuple share a nonce.
- Counter wrap: counters are 64-bit; wrap-around is not a practical concern.
- Replay: the 1024-entry window provides replay protection for out-of-order
  networks; very old frames are rejected.
- Forward secrecy: base key rotation on every commit provides FS at epoch
  granularity.

## 12. IANA Considerations

None.  KID encoding and HKDF info strings are GBP-internal.

## 13. References

### 13.1 Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
- [draft-ietf-sframe-enc] Jennings, C., et al., "Secure Frame (SFrame)".
- `gap_rfc.md` — GAP specification.
- `gbp-mls-binding.md` — GBP/MLS binding.
