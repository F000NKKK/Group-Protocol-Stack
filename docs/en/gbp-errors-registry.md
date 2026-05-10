# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Error Code Registry

## Abstract
This document defines the unified error taxonomy for GBP, GAP, GTP, and GSP.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Conventions
BCP 14 requirement keywords apply.

## 2. Error Object
All protocol NACK/error responses SHOULD include:
```
ErrorObject {
  uint16 code;
  uint8  class;
  bool   retryable;
  bool   fatal;
  string reason;
  bytes  details_cbor;
}
```

## 3. Classes
- `0x01` TRANSPORT
- `0x02` CRYPTO
- `0x03` STATE
- `0x04` POLICY
- `0x05` SCHEMA
- `0x06` AUTHZ

## 4. Code Ranges
- `0x0000-0x0FFF` GBP
- `0x1000-0x1FFF` GAP
- `0x2000-0x2FFF` GTP
- `0x3000-0x3FFF` GSP
- `0xF000-0xFFFF` Private use

## 5. Initial GBP Codes
- `0x0001 ERR_UNSUPPORTED_VERSION`
- `0x0002 ERR_UNKNOWN_GROUP`
- `0x0003 ERR_EPOCH_MISMATCH`
- `0x0004 ERR_TRANSITION_MISMATCH`
- `0x0005 ERR_REPLAY_DETECTED`
- `0x0006 ERR_DECRYPT_FAILED`
- `0x0007 ERR_COMMIT_INVALID`
- `0x0008 ERR_STREAM_POLICY_VIOLATION`
- `0x0010 ERR_PREPARE_TIMEOUT`        ; Coordinator timed out waiting for READY quorum
- `0x0011 ERR_READY_TIMEOUT`          ; Member timed out before completing local commit processing
- `0x0012 ERR_EXECUTE_TIMEOUT`        ; Member timed out waiting for EXECUTE_TRANSITION after READY
- `0x0013 ERR_COORDINATOR_GONE`       ; Coordinator transport lost; handover required
- `0x0014 ERR_DIGEST_MISMATCH`        ; member_set_root_hash mismatch on Resync

## 6. Initial GAP Codes
- `0x1001 ERR_GAP_BAD_SOURCE_ID`
- `0x1002 ERR_GAP_DECODE_FAILED`
- `0x1003 ERR_GAP_AUTH_FAILED`
- `0x1004 ERR_GAP_REPLAY_DETECTED`
- `0x1005 ERR_GAP_EPOCH_STALE`
- `0x1006 ERR_GAP_KEY_PHASE_UNKNOWN`

## 7. Initial GTP Codes
- `0x2001 ERR_GTP_BAD_LENGTH`
- `0x2002 ERR_GTP_UNSUPPORTED_CONTENT_TYPE`
- `0x2003 ERR_GTP_DUPLICATE_MESSAGE`
- `0x2004 ERR_GTP_POLICY_REJECTED`
- `0x2005 ERR_GTP_ATTACHMENT_INTEGRITY`
- `0x2006 ERR_GTP_REQUEST_TIMEOUT`

## 8. Initial GSP Codes
- `0x3001 ERR_GSP_BAD_SCHEMA`
- `0x3002 ERR_GSP_UNAUTHORIZED`
- `0x3003 ERR_GSP_UNKNOWN_SIGNAL`
- `0x3004 ERR_GSP_DUPLICATE_REQUEST`
- `0x3005 ERR_GSP_STATE_CONFLICT`
- `0x3006 ERR_GSP_PRECONDITION_FAILED`

## 9. Retryability and Fatality
Each specification MUST declare retryability/fatality per code. The matrix below is normative for the GBP base codes; sub-protocols MAY tighten but MUST NOT loosen.

| Code | Retryable | Fatal | Rationale |
|---|---|---|---|
| `0x0001 ERR_UNSUPPORTED_VERSION` | false | true | Cannot recover without re-negotiation |
| `0x0002 ERR_UNKNOWN_GROUP` | false | true | Wrong group; session is invalid |
| `0x0003 ERR_EPOCH_MISMATCH` | true | false | Recover via Resync (`gbp-control-plane` §5.2) |
| `0x0004 ERR_TRANSITION_MISMATCH` | true | false | Recover via Resync |
| `0x0005 ERR_REPLAY_DETECTED` | false | false | Drop frame; keep going |
| `0x0006 ERR_DECRYPT_FAILED` | true | false | A frame sealed under a different MLS epoch (e.g. PREPARE reaching a fresh joiner) cannot be opened, but the node MUST keep running for the next EXECUTE on the shared epoch. Resync MAY be initiated on repeated failures. |
| `0x0007 ERR_COMMIT_INVALID` | false | true | Stack-level integrity violation; abort transition and require fresh KeyPackage |
| `0x0008 ERR_STREAM_POLICY_VIOLATION` | false | false | Drop frame; deployment policy decides escalation |
| `0x0010 ERR_PREPARE_TIMEOUT` | true | false | Coordinator MAY re-issue PREPARE on next tid |
| `0x0011 ERR_READY_TIMEOUT` | true | false | Member returns to `T_IDLE`; resync if EXECUTE later observed |
| `0x0012 ERR_EXECUTE_TIMEOUT` | true | false | Trigger Resync; participate in coordinator handover |
| `0x0013 ERR_COORDINATOR_GONE` | true | false | Members elect handover per §4.1 |
| `0x0014 ERR_DIGEST_MISMATCH` | false | true | Re-bootstrap as joiner |

Default classes for codes without an explicit row:
- SCHEMA in data path: non-fatal.
- POLICY/AUTHZ: non-fatal unless repeated threshold is exceeded.

## 10. IANA Considerations
This document requests creation of the GBP Error Code registry with ranges in Section 4.

## 11. Security Considerations
Error payloads MUST NOT leak key material, plaintext, or sensitive identity metadata.

## 12. References
### 12.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
