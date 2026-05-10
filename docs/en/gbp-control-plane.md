# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Control Plane Messages

## Abstract
This document defines GBP-Control messages, opcodes, and transition/recovery procedures.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
GBP-Control is carried on StreamType 0 and stream_id 0.

## 2. Conventions
BCP 14 requirement words apply.

## 3. Control Message Header
```
GBPControl {
  uint16 opcode;
  uint32 request_id;
  uint32 sender_id;
  uint32 transition_id;
  uint32 args_length;
  bytes  args_cbor;
}
```

## 4. Opcode Registry (Initial)
- `0x0001 PREPARE_TRANSITION`
- `0x0002 READY_FOR_TRANSITION`
- `0x0003 EXECUTE_TRANSITION`
- `0x0004 ABORT_TRANSITION`
- `0x0005 GROUP_STATE_DIGEST_REQUEST`
- `0x0006 GROUP_STATE_DIGEST_RESPONSE`
- `0x0007 REPORT_INVALID_COMMIT`
- `0x0008 CAPABILITIES_ADVERTISE`
- `0x0009 ACK`
- `0x000A NACK`

## 5. Transition Procedures
### 5.1 Prepare
Coordinator sends `PREPARE_TRANSITION` with target epoch metadata.
Receivers MUST create local pending transition context.

### 5.2 Ready
Members send `READY_FOR_TRANSITION` only after commit validation and local readiness.

### 5.3 Execute
Coordinator sends `EXECUTE_TRANSITION` when readiness quorum is met or timeout policy resolves.
Receivers MUST atomically switch epoch context.

### 5.4 Abort
Coordinator or policy engine sends `ABORT_TRANSITION`.
Receivers MUST discard pending transition state.

## 6. Recovery Procedures
### 6.1 Invalid Commit Recovery
Receiver sends `REPORT_INVALID_COMMIT` with error details and offending transition context.

### 6.2 Resync
Client sends `GROUP_STATE_DIGEST_REQUEST`.
Server responds with digest + recent control range metadata.

## 7. Capability Negotiation
Endpoints MUST advertise supported protocol versions and optional features before entering ACTIVE state.
Downgrade to unsupported profiles MUST be rejected unless explicit policy permits.

## 8. IANA Considerations
This document requests creation of a GBP Control Opcode registry.

## 9. Security Considerations
Control messages MUST be authenticated, replay-protected, and tied to transition ordering constraints.

## 10. References
### 10.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
