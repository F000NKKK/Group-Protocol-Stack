# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# Group Base Protocol (GBP)

## Abstract
This document specifies the Group Base Protocol (GBP), a secure group transport substrate over QUIC, TLS 1.3, and MLS. GBP defines frame structure, stream multiplexing, epoch transition control, commit ordering behavior, replay boundaries, and recovery procedures for dependent protocols.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
GBP defines common transport and cryptographic mechanics for multi-party applications. Upper protocols (GAP, GTP, GSP) inherit membership and epoch semantics from GBP.

## 2. Conventions and Terminology
The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT",
"SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and
"OPTIONAL" are to be interpreted as described in BCP 14 [RFC2119]
[RFC8174] when, and only when, they appear in all capitals.

Terms used in this document:
- GroupID: globally unique group identifier.
- MemberID: unique participant identifier in GroupID scope. The DS assigns MemberID monotonically in the order each member is admitted to the group; assigned values are NEVER reused after a member leaves. Each MemberID MUST correspond 1:1 with the member's MLS LeafIndex at the moment of admission. After an MLS Remove, the freed LeafIndex MAY be reused by future MLS adds, but the corresponding GBP MemberID MUST NOT be — implementations MUST keep a mapping table for the lifetime of the group.
- Epoch: MLS generation for active traffic secrets.
- TransitionID: monotonic identifier for protocol/epoch transition. TransitionID is global per-GroupID, not per-Coordinator; on coordinator handover the new Coordinator MUST continue the existing sequence.
- AS: Authentication Service.
- DS: Delivery Service. The DS provides a canonical receive order for control messages by tagging each forwarded frame with a per-DS monotonic sequence; in P2P fallback (no DS) the Coordinator's local accept order serves as the canonical order.
- Coordinator: the single Active member authorized to issue PREPARE/EXECUTE/ABORT for a given epoch. See `gbp-control-plane.md` §5.1.

## 3. Architecture and Trust Model
GBP runs over QUIC [RFC9000] with TLS 1.3 [RFC8446] and MLS [RFC9420].

An endpoint:
- MUST maintain one authenticated QUIC connection per active group session.
- MUST assign Stream 0 to GBP-Control.
- MUST support StreamType values 0..3.
- SHOULD isolate congestion and retransmission across stream classes.

Service boundaries:
- AS binds identities, credentials, and authorization assertions.
- DS distributes control and data messages and may reorder, delay, or replay traffic.

GBP endpoints MUST treat DS as untrusted for confidentiality and integrity of application payloads.

## 4. Group State Model
An endpoint state tuple contains:
- GroupID
- CurrentEpoch
- ActiveTransitionID
- MemberSet
- ActiveStreams
- CommitLog
- ReplayWindow per stream

Membership changes:
- MUST be driven by valid MLS commit processing.
- MUST advance epoch monotonically.
- MUST reject stale commits and unknown credential chains.

## 5. Stream Registry
Initial StreamType registry:
- 0 control
- 1 audio
- 2 text
- 3 signal

Unknown StreamType values in non-critical frames MAY be ignored.
Unknown StreamType values in critical/system context MUST generate a protocol error.

### 5.1 Stream Multiplexing
Each member exposes one logical stream per (StreamType, member) pair. The per-member `stream_id` is computed as:

```
stream_id = base_class + member_id * 100
```

where `base_class` is the protocol-wide base for the stream class:
- Control: `base = 0`
- Text (GTP): `base = 1`
- Audio (GAP): `base = 2`
- Signal (GSP): `base = 3`

The factor `100` ensures non-overlapping `stream_id` ranges for up to `1_000_000` members per group. Implementations MUST enforce `member_id < 1_000_000` via `debug_assert!`.

## 6. Wire Format

### 6.1 GBP Frame
```
GBPFrame {
  uint8    version;
  uint128  group_id;
  uint64   epoch;
  uint32   transition_id;
  uint8    stream_type;
  uint32   stream_id;
  uint16   flags;
  uint32   sequence_no;
  uint32   payload_size;
  bytes    encrypted_payload;
  uint8    pf;               // optional; omitted when 0 (CBOR) for backward-compat
}
```

`pf` (payload format) identifies the codec used to encode the sub-protocol message inside `encrypted_payload` before AEAD sealing. See §6.5 of the Serialization Profile (schemas.md) for the PayloadCodec registry. Receivers MUST treat an absent `pf` field as `0` (CBOR).

Flags:
- `0x0001` O ordered delivery
- `0x0002` R reliable delivery
- `0x0004` A acknowledgment requested
- `0x0008` S system frame
- `0x0010` C critical extension frame

### 6.2 Validation
Receiver MUST validate version, group_id, epoch, transition_id ordering policy, payload_size, and replay window constraints prior to payload dispatch.

Malformed frames MUST be discarded and SHOULD generate NACK with structured error code for control streams.

## 7. Transition Protocol
GBP defines control messages:
- `PREPARE_TRANSITION`
- `READY_FOR_TRANSITION`
- `EXECUTE_TRANSITION`
- `ABORT_TRANSITION`

Rules:
- TransitionID MUST be strictly monotonic per GroupID.
- All commit-capable members MUST process commit before READY_FOR_TRANSITION.
- EXECUTE_TRANSITION MUST only occur after all required READY messages or timeout expiration.
- Timeout fallback MUST be deterministic and documented by deployment policy.

## 8. Commit Ordering and Tie-Break
If multiple commits are received for the same epoch window:
1. Prefer the first valid commit by DS receive order. "DS receive order" is defined as the per-DS monotonic forwarding sequence; in P2P fallback the Coordinator's local accept order is used.
2. Break ties by lowest committer MemberID.
3. Discard all non-winning commits for that TransitionID.

Clients MUST process only one winning commit per TransitionID.

The Coordinator is the sole legitimate committer per `gbp-control-plane.md` §5.1, so under normal operation tie-break only fires during coordinator-handover races (two members claiming the role simultaneously). Implementations MUST detect such collisions and resolve them per the rules above before any application traffic in the new epoch.

Coordinators MUST NOT enqueue more than one outstanding transition. Membership change requests received while a transition is in flight MUST be queued in FIFO order and processed as separate transitions, each with its own `transition_id`. Batching multiple proposals into one MLS commit is permitted within a single transition.

## 9. Replay and Duplicate Handling
GBP does not fully prevent insider replay.
Applications MUST include stream-local uniqueness (`sequence_no` + sender context) and maintain replay windows.

Frames outside replay window:
- MUST be dropped.
- SHOULD be metered for abuse detection.

## 10. Recovery
On reconnect:
1. Request `GroupStateDigest`.
2. Compare epoch and transition_id.
3. Request missing control log range.
4. Perform MLS resync if needed.
5. Reopen mandatory streams.

Invalid commit or welcome recovery:
- Endpoint MUST emit `REPORT_INVALID_COMMIT`.
- Endpoint MUST reset local pending state.
- Endpoint MUST request fresh key package workflow.

## 11. Error Handling
Global codes are defined in `gbp-errors-registry.md`.

Core GBP errors:
- `ERR_UNSUPPORTED_VERSION`
- `ERR_UNKNOWN_GROUP`
- `ERR_EPOCH_MISMATCH`
- `ERR_TRANSITION_MISMATCH`
- `ERR_REPLAY_DETECTED`
- `ERR_DECRYPT_FAILED`
- `ERR_COMMIT_INVALID`
- `ERR_STREAM_POLICY_VIOLATION`

Each error MUST define class, retryability, and fatality.

## 12. Schemas

### 12.1 CBOR
```
{
  "v": uint,
  "gid": bstr,
  "ep": uint,
  "tid": uint,
  "st": uint,
  "sid": uint,
  "fl": uint,
  "seq": uint,
  "psz": uint,
  "pl": bstr
}
```

### 12.2 Protobuf
```proto
syntax = "proto3";
package gbp;

message GBPFrame {
  uint32 version = 1;
  bytes group_id = 2;
  uint64 epoch = 3;
  uint32 transition_id = 4;
  uint32 stream_type = 5;
  uint32 stream_id = 6;
  uint32 flags = 7;
  uint32 sequence_no = 8;
  uint32 payload_size = 9;
  bytes encrypted_payload = 10;
}
```

### 12.3 FlatBuffers
```fbs
namespace gbp;

table GBPFrame {
  version:ubyte;
  group_id:[ubyte];
  epoch:ulong;
  transition_id:uint;
  stream_type:ubyte;
  stream_id:uint;
  flags:ushort;
  sequence_no:uint;
  payload_size:uint;
  encrypted_payload:[ubyte];
}

root_type GBPFrame;
```

## 13. IANA Considerations
IANA is requested to create:
- GBP StreamType Registry
- GBP Control Message Type Registry
- GBP Error Code Registry

Initial values are provided in this document and companion registries.

## 14. Security Considerations
Security depends on MLS state convergence and endpoint authorization enforcement. Implementations MUST enforce downgrade resistance, strict epoch/transition validation, replay window checks, and key erasure after epoch rollover. DS compromise MUST be assumed in threat analysis.

## 15. References
### 15.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC8446] Rescorla, E., "The Transport Layer Security (TLS) Protocol Version 1.3".
- [RFC8949] Bormann, C. and P. Hoffman, "Concise Binary Object Representation (CBOR)".
- [RFC9000] Iyengar, J. and M. Thomson, "QUIC: A UDP-Based Multiplexed and Secure Transport".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
