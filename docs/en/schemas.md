# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Stack Serialization and Interoperability Profile

## Abstract
This document defines common schema and interoperability conventions for the GBP protocol family. It specifies shared field encoding requirements, registry expectations, and optional common envelopes for CBOR, Protobuf, and FlatBuffers.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
This profile is used to keep GBP-family protocol messages interoperable across language runtimes and deployments.

## 2. Conventions
BCP 14 language applies as defined in [RFC2119] and [RFC8174].

## 3. Baseline Dependencies
- Security: MLS [RFC9420], TLS 1.3 [RFC8446].
- Transport: QUIC [RFC9000].
- Media (where applicable): Opus [RFC6716], RTP [RFC3550], SRTP [RFC3711].
- Traversal (deployment-specific): ICE [RFC8445], STUN [RFC5389], TURN [RFC8656].

## 4. Cross-Protocol Requirements
- Endpoints MUST reject malformed length fields.
- Endpoints MUST reject authentication failures.
- Endpoints MUST enforce monotonic epoch progression.
- Control-plane streams MUST use reliable delivery.
- Replay behavior SHOULD be deterministic for resync.
- Every protocol MUST define retryability and fatality for each error code.

## 5. Shared Encoding Rules
- Unsigned integers are default unless explicitly documented otherwise.
- Binary blobs are `bstr` (CBOR), `bytes` (Protobuf), `[ubyte]` (FlatBuffers).
- Unknown enum values MUST be handled safely (ignore, preserve, or explicit NACK by profile).

## 6. Common Registries
### 6.1 StreamType
- `0` control
- `1` audio
- `2` text
- `3` signal
- `4-32767` standards action
- `32768-65535` private use

### 6.2 GBP Flags
- `0x0001` ordered (`O`)
- `0x0002` reliable (`R`)
- `0x0004` ack requested (`A`)
- `0x0008` system (`S`)
- `0x0010` critical extension (`C`)

### 6.3 Error Code Range Policy
- `0x0000-0x0FFF`: GBP core
- `0x1000-0x1FFF`: GAP
- `0x2000-0x2FFF`: GTP
- `0x3000-0x3FFF`: GSP
- `0xF000-0xFFFF`: private use

### 6.4 Extension Policy
All registries in this profile use "Specification Required", except private ranges.

## 7. Shared Protobuf Envelope (Optional)
```proto
syntax = "proto3";
package gbpstack;

message Envelope {
  uint32 version = 1;
  bytes group_id = 2;
  uint64 epoch = 3;
  uint32 stream_type = 4;
  uint32 stream_id = 5;
  uint32 flags = 6;
  bytes payload = 7; // protocol-specific serialized body
}
```

## 8. Shared FlatBuffers Envelope (Optional)
```fbs
namespace gbpstack;

table Envelope {
  version:ubyte;
  group_id:[ubyte];
  epoch:ulong;
  stream_type:ubyte;
  stream_id:uint;
  flags:ushort;
  payload:[ubyte];
}

root_type Envelope;
```

## 9. Shared CBOR Envelope (Optional)
```
{
  "v": uint,
  "gid": bstr,
  "ep": uint,
  "st": uint,
  "sid": uint,
  "fl": uint,
  "pl": bstr
}
```

## 10. Validation Checklist
- Version compatibility check.
- Group membership and authorization check.
- Epoch check and resync gate.
- Payload length and schema validation.
- ACK/NACK generation per delivery policy.

Implementations SHOULD treat this checklist as minimum conformance criteria for interoperability testing.

## 11. IANA Considerations
This document defines allocation policy for GBP-family registries and requests IANA registry creation for StreamType, Error Codes, and Control Message Types.

## 12. Security Considerations
Schema interoperability does not replace cryptographic validation. Receivers MUST perform cryptographic and authorization checks before applying side effects.

## 13. References
### 13.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC8446] Rescorla, E., "The Transport Layer Security (TLS) Protocol Version 1.3".
- [RFC8949] Bormann, C. and P. Hoffman, "Concise Binary Object Representation (CBOR)".
- [RFC9000] Iyengar, J. and M. Thomson, "QUIC: A UDP-Based Multiplexed and Secure Transport".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
