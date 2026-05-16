# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Audio Protocol (GAP) over GBP

## Abstract
This document specifies GAP, the low-latency media subprotocol over GBP StreamType 1. GAP defines media payloads, sender ratchets, replay windows, epoch overlap behavior, and operational failure handling.

GAP payloads MAY be additionally protected with SFrame end-to-end encryption as specified in `gbp-sframe.md`.  When SFrame is in use the GBP node delivers SFrame payloads (not raw Opus) to the application layer; the application is responsible for SFrame encrypt/decrypt before submitting frames to `GapClient`.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
GAP carries group audio using Opus framing with keys derived from MLS exporter context.

## 2. Conventions and Terminology
BCP 14 requirement words from [RFC2119] and [RFC8174] apply.

## 3. Protocol Binding
GAP payloads MUST be sent only in GBP frames with `stream_type=1`.
Receivers MUST reject GAP payloads in other stream types.

## 4. Key Schedule and Ratchets
Per epoch, sender key material:
- MediaMasterKey
- MediaSalt
- SenderBaseKey = HKDF(MediaMasterKey, MediaSalt, "audio/sender")
- SenderAuthKey = HKDF(MediaMasterKey, MediaSalt, "audio/auth")

Rules:
- Sender keys MUST rotate on epoch transition.
- Old sender keys SHOULD be retained for overlap window `T_overlap`.
- `T_overlap` default is 10 seconds and MUST be deployment-configurable.

## 5. Payload Format
```
GAPPayload {
  uint32 media_source_id;
  uint16 rtp_sequence;
  uint64 rtp_timestamp;
  uint32 key_phase;
  bytes  opus_frame;
}
```

## 6. Replay and Ordering
Receiver MUST maintain per-source replay windows keyed by `(media_source_id, key_phase, rtp_sequence)`.
Packets outside window MUST be dropped.

`rtp_sequence` wrap and nonce wrap:
- Implementations MUST support `rtp_sequence` modulo `2^16`.
- Implementations MUST handle long-lived epoch nonce-generation wrap without key reuse.

## 7. Sender and Receiver Processing
Receiver pipeline:
1. Validate epoch and transition context.
2. Select active or overlap decryption key by key_phase.
3. Verify authentication tag.
4. Replay check and reorder buffering.
5. Decode Opus and submit to playout.

If decryption fails with active key and overlap key exists, receiver SHOULD attempt overlap key once.

## 8. Performance and Congestion
- Opus 48kHz support: REQUIRED.
- 20ms packetization: RECOMMENDED.
- Opus FEC: RECOMMENDED.
- Reliable retransmission for conversational audio: NOT RECOMMENDED.

## 9. Error Handling
GAP-specific errors:
- `ERR_GAP_BAD_SOURCE_ID`
- `ERR_GAP_DECODE_FAILED`
- `ERR_GAP_AUTH_FAILED`
- `ERR_GAP_REPLAY_DETECTED`
- `ERR_GAP_EPOCH_STALE`
- `ERR_GAP_KEY_PHASE_UNKNOWN`

NACK payloads for media MAY be batched using GSP control channel.

## 10. Message Schemas

### 10.1 CBOR Map Schema
```
{
  "msid": uint,   ; media_source_id
  "seq": uint,    ; rtp_sequence
  "ts": uint,     ; rtp_timestamp (64-bit)
  "kp": uint,     ; key_phase
  "opus": bstr    ; opus_frame
}
```

### 10.2 Protobuf Schema
```proto
syntax = "proto3";
package gap;

message GAPPayload {
  uint32 media_source_id = 1;
  uint32 rtp_sequence = 2;
  uint64 rtp_timestamp = 3;
  uint32 key_phase = 4;
  bytes opus_frame = 5;
}
```

### 10.3 FlatBuffers Schema
```fbs
namespace gap;

table GAPPayload {
  media_source_id:uint;
  rtp_sequence:ushort;
  rtp_timestamp:ulong;
  key_phase:uint;
  opus_frame:[ubyte];
}

root_type GAPPayload;
```

## 11. IANA Considerations
This document registers GAP error names and GAP key phase behavior under GBP registries.

Registry values are allocated from the GAP range (`0x1000-0x1FFF`) defined in `schemas.md` and `gbp-errors-registry.md`.

## 12. Security Considerations
Implementations MUST ensure no sender-key/nonce reuse across epoch boundaries and key phases. Replay and downgrade protections are mandatory.

## 13. References
### 13.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC3711] Baugher, M., et al., "The Secure Real-time Transport Protocol (SRTP)".
- [RFC6716] Valin, J., et al., "Definition of the Opus Audio Codec".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
- `gbp-sframe.md` — SFrame E2EE for GAP audio payloads.
