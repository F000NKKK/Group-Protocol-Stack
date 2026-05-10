# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Signaling Protocol (GSP) over GBP

## Abstract
This document specifies GSP, the control-plane signaling protocol for group state transitions, moderation actions, and media session control over GBP StreamType 3.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
GSP carries stateful control operations for membership and media lifecycle management in group sessions.

## 2. Conventions and Terminology
BCP 14 requirement words are interpreted per [RFC2119] and [RFC8174].

## 3. Protocol Binding
GSP messages MUST be carried in GBP frames with `stream_type=3`.
GSP messages MUST use reliable delivery, and request-response signals MUST request ACK.

## 4. Signal Message Format
```
GSPSignal {
  uint16 signal_type;
  uint32 request_id;
  uint32 sender_id;
  uint32 role_claim;
  uint32 args_length;
  bytes  args;
}
```

`args` MUST contain valid CBOR for the specified signal type.

## 5. Signal Type Registry
- `100` JOIN
- `101` LEAVE
- `102` ROLE_CHANGE
- `200` MUTE
- `201` UNMUTE
- `300` STREAM_START
- `301` STREAM_STOP
- `400` CODEC_UPDATE

## 6. Processing and Authorization
Receiver:
1. MUST authenticate/decrypt payload.
2. MUST validate sender authority for requested action.
3. MUST validate argument schema.
4. MUST apply side effects atomically.
5. MUST respond with ACK/NACK where required.

### 6.1 Role Authorization Matrix
- JOIN, LEAVE: member or moderator policy.
- ROLE_CHANGE: moderator or admin only.
- MUTE, UNMUTE: moderator or self for local media.
- STREAM_START, STREAM_STOP: member with media privilege.
- CODEC_UPDATE: moderator or negotiated auto-control role.

Unauthorized requests MUST return `ERR_GSP_UNAUTHORIZED` with reason code.

## 7. Request Lifecycle
`request_id` is unique per sender within replay window.

Lifecycle states:
`received -> validated -> applied -> acked` or `received -> rejected -> nacked`

## 8. Recovery
Implementations SHOULD retain recent control history and MUST provide deterministic replay via snapshot + delta application.

## 9. Errors
Defined errors:
- `ERR_GSP_BAD_SCHEMA`
- `ERR_GSP_UNAUTHORIZED`
- `ERR_GSP_UNKNOWN_SIGNAL`
- `ERR_GSP_DUPLICATE_REQUEST`
- `ERR_GSP_STATE_CONFLICT`
- `ERR_GSP_PRECONDITION_FAILED`

## 10. ACK/NACK Schema
```
GSPAck {
  uint32 request_id;
  uint16 status_code;
  string reason;
  bytes  details_cbor;
}
```

## 11. Message Schemas

### 11.1 CBOR Map Schema
```
{
  "t": uint,      ; signal_type
  "rid": uint,    ; request_id
  "sid": uint,    ; sender_id
  "rc": uint,     ; role_claim
  "alen": uint,   ; args_length
  "args": any     ; CBOR structure by signal type
}
```

### 11.2 Protobuf Schema
```proto
syntax = "proto3";
package gsp;

message GSPSignal {
  uint32 signal_type = 1;
  uint32 request_id = 2;
  uint32 sender_id = 3;
  uint32 role_claim = 4;
  uint32 args_length = 5;
  bytes args_cbor = 6;
}
```

### 11.3 FlatBuffers Schema
```fbs
namespace gsp;

table GSPSignal {
  signal_type:ushort;
  request_id:uint;
  sender_id:uint;
  role_claim:uint;
  args_length:uint;
  args:[ubyte]; // CBOR bytes
}

root_type GSPSignal;
```

## 12. IANA Considerations
This document relies on GBP registry policy for signal type extensibility.

GSP error allocations MUST use the `0x3000-0x3FFF` range from the common registry profile.

## 13. Security Considerations
Unauthorized control operations are a primary risk. Implementations MUST bind authorization checks to authenticated sender identity and current group role state.

## 14. References
### 14.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC8949] Bormann, C. and P. Hoffman, "Concise Binary Object Representation (CBOR)".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
