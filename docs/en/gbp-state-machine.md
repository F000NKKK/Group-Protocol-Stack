# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP State Machine Specification

## Abstract
This document defines normative state machines for GBP endpoint lifecycle, group epoch transitions, and subprotocol activation.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
This document is a companion to `gbp_rfc.md` and is normative for transition correctness.

## 2. Conventions
BCP 14 keywords from [RFC2119] and [RFC8174] apply.

## 3. Endpoint Lifecycle State Machine
States:
- `IDLE`
- `CONNECTING`
- `ESTABLISHING_GROUP`
- `ACTIVE`
- `RESYNCING`
- `FAILED`
- `CLOSED`

Mandatory transitions:
- `IDLE -> CONNECTING` on join intent.
- `CONNECTING -> ESTABLISHING_GROUP` on authenticated transport.
- `ESTABLISHING_GROUP -> ACTIVE` on valid epoch establishment.
- `ACTIVE -> RESYNCING` on epoch/transition mismatch.
- `RESYNCING -> ACTIVE` on successful digest replay.
- any state -> `FAILED` on fatal policy/crypto violation.
- `FAILED -> CLOSED` on teardown.

## 4. Epoch Transition State Machine
States:
- `T_IDLE`
- `T_PREPARED`
- `T_COMMIT_PROCESSED`
- `T_READY`
- `T_EXECUTED`
- `T_ABORTED`

Flow:
1. `T_IDLE -> T_PREPARED` on `PREPARE_TRANSITION`.
2. `T_PREPARED -> T_COMMIT_PROCESSED` after valid commit.
3. `T_COMMIT_PROCESSED -> T_READY` when local prerequisites are met.
4. `T_READY -> T_EXECUTED` on `EXECUTE_TRANSITION`.
5. any pre-execute state -> `T_ABORTED` on timeout or invalid transition.

## 5. Subprotocol Activation State Machine
Each stream type has:
- `DISABLED`
- `NEGOTIATING`
- `ENABLED`
- `DEGRADED`
- `SUSPENDED`

Rules:
- System policy MUST gate `NEGOTIATING -> ENABLED`.
- Auth or schema failures SHOULD move to `DEGRADED`.
- Repeated fatal failures MUST move to `SUSPENDED`.

## 6. Timeout Semantics
Endpoints MUST implement:
- `T_prepare_max`
- `T_ready_max`
- `T_execute_max`

Timeout expiration MUST trigger deterministic fallback policy.

## 7. IANA Considerations
No additional IANA actions.

## 8. Security Considerations
State machine divergence is a security risk. Implementations MUST reject invalid transitions and MUST NOT apply side effects before state validation.

## 9. References
### 9.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
