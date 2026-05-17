# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Text Protocol (GTP) over GBP

## Abstract
This document specifies GTP, a group text and binary message protocol carried over GBP StreamType 2. GTP defines payload structure, delivery guarantees, attachment referencing, and synchronization behavior.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
GTP provides ordered and reliable messaging semantics for group text communication in a shared GBP/MLS context.

## 2. Conventions and Terminology
BCP 14 terms are interpreted per [RFC2119] and [RFC8174].

## 3. Protocol Binding
GTP payloads MUST be carried only in GBP frames where `stream_type=2`.

## 4. Message Format
```
GTPMessage {
  uint64 message_id;
  uint32 sender_id;
  uint64 timestamp_ms;
  uint32 request_id;
  uint8  flags;
  uint8  content_type;
  uint32 content_length;
  bytes  content;
}
```

Flag bits:
- `0x01` urgent
- `0x02` ephemeral
- `0x04` persistent

ContentType registry:
- `0` plain
- `1` markdown
- `2` binary
- `3` attachment_ref

## 5. Delivery Semantics and Idempotency
For conversational chat traffic, sender SHOULD set `O|R|A`.

Endpoints:
- MUST treat repeated `(sender_id, message_id)` as idempotent duplicates.
- MUST verify `content_length` against actual body size.
- SHOULD preserve order for streams with ordered delivery.
- MUST preserve per-sender dedupe cache for the current epoch. The default implementation uses an LRU-bounded set of 10 000 entries per epoch; time-based eviction (`max(2*RTT, 30s)`) is an acceptable alternative for deployments with predictable RTT bounds.
- SHOULD support ACK and NACK envelopes carrying `request_id`.

## 6. Attachments
Attachments SHOULD be chunked or moved to dedicated attachment channels, with parent message linkage and integrity metadata.

Attachment metadata MUST include digest, total size, and chunk index.

## 7. Resynchronization
Rejoining clients MUST provide a known-message watermark. Peers SHOULD replay retained messages in-order within retention limits.

## 8. Error Handling
Defined errors:
- `ERR_GTP_BAD_LENGTH`
- `ERR_GTP_UNSUPPORTED_CONTENT_TYPE`
- `ERR_GTP_DUPLICATE_MESSAGE`
- `ERR_GTP_POLICY_REJECTED`
- `ERR_GTP_ATTACHMENT_INTEGRITY`
- `ERR_GTP_REQUEST_TIMEOUT`

## 9. ACK/NACK Envelope
```
GTPAck {
  uint32 request_id;
  uint64 message_id;
  uint16 status_code;
  string reason;
}
```

## 10. Message Schemas

### 10.1 CBOR Map Schema
```
{
  "mid": uint,      ; message_id
  "sid": uint,      ; sender_id
  "ts": uint,       ; timestamp_ms
  "rid": uint,      ; request_id
  "fl": uint,       ; flags bitset
  "ct": uint,       ; content_type
  "len": uint,      ; content_length
  "body": bstr      ; content
}
```

### 10.2 Protobuf Schema
```proto
syntax = "proto3";
package gtp;

message GTPMessage {
  uint64 message_id = 1;
  uint32 sender_id = 2;
  uint64 timestamp_ms = 3;
  uint32 request_id = 4;
  uint32 flags = 5;
  uint32 content_type = 6;
  uint32 content_length = 7;
  bytes content = 8;
}
```

### 10.3 FlatBuffers Schema
```fbs
namespace gtp;

table GTPMessage {
  message_id:ulong;
  sender_id:uint;
  timestamp_ms:ulong;
  request_id:uint;
  flags:ubyte;
  content_type:ubyte;
  content_length:uint;
  content:[ubyte];
}

root_type GTPMessage;
```

## 10.4 Payload Codec Selection
The codec used to encode a GTPMessage is signalled by the `pf` (payload format) field of the enclosing
GBP frame (see `gbp_rfc.md` §6.1 and `schemas.md` §6.5).  The default value `0` (CBOR) is
backward-compatible with pre-1.5 implementations.  Senders pass the desired codec to the `send`
API; receivers read the codec from the `payload_received` event and MUST pass the same value to
the `accept` API.

## 11. IANA Considerations
This document does not require additional IANA actions beyond GBP registries.

GTP-specific error allocations MUST use the `0x2000-0x2FFF` range from the common registry profile.

## 12. Security Considerations
GTP inherits confidentiality and integrity from MLS/GBP. Implementations MUST enforce authorization policy for retention, ephemeral message handling, and replay protection.

## 13. References
### 13.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
