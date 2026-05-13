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
- `IDLE -> FAILED` on initialisation or pre-join fatal error.
- `CONNECTING -> ESTABLISHING_GROUP` on authenticated transport.
- `CONNECTING -> FAILED` on transport failure.
- `ESTABLISHING_GROUP -> ACTIVE` on valid epoch establishment.
- `ESTABLISHING_GROUP -> FAILED` on MLS/Welcome failure.
- `ACTIVE -> RESYNCING` on epoch/transition mismatch.
- `ACTIVE -> FAILED` on fatal policy/crypto violation.
- `ACTIVE -> CLOSED` on graceful shutdown.
- `RESYNCING -> ACTIVE` on successful digest replay.
- `RESYNCING -> FAILED` on irrecoverable resync failure.
- any state -> `CLOSED` on teardown.

## 4. Epoch Transition State Machine
States:
- `T_IDLE`
- `T_PREPARED`
- `T_COMMIT_PROCESSED`
- `T_READY`
- `T_EXECUTED`
- `T_ABORTED`

Normative transition matrix (`TransitionState::can_transition_to`):
| From | To | Trigger |
|---|---|---|
| `T_IDLE` | `T_PREPARED` | `PREPARE_TRANSITION` issued or received |
| `T_PREPARED` | `T_COMMIT_PROCESSED` | Local MLS commit/welcome processed |
| `T_PREPARED` | `T_ABORTED` | Timeout or invalid transition |
| `T_COMMIT_PROCESSED` | `T_READY` | Local prerequisites met; member sends `READY_FOR_TRANSITION` |
| `T_COMMIT_PROCESSED` | `T_ABORTED` | Timeout or coordinator abort |
| `T_READY` | `T_EXECUTED` | `EXECUTE_TRANSITION` received |
| `T_READY` | `T_ABORTED` | `ABORT_TRANSITION` or coordinator timeout |
| `T_EXECUTED` | `T_IDLE` | Epoch advance complete; return to idle for next transition |
| `T_ABORTED` | `T_IDLE` | Cleanup complete; return to idle |

All other transitions are forbidden. Implementations MUST enforce this matrix.

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
Endpoints MUST implement the following timers. Default values are normative for interoperable deployments; deployment policy MAY tighten them but MUST NOT loosen without an explicit downgrade negotiation.

| Timer | Default | Owner | Started on | Expires when |
|---|---|---|---|---|
| `T_prepare_max` | 5 s | Coordinator | issuing `PREPARE_TRANSITION` | quorum of `READY_FOR_TRANSITION` not yet reached |
| `T_ready_max` | 5 s | Member | receiving `PREPARE_TRANSITION` | local commit/welcome processing not finished |
| `T_execute_max` | 10 s | Member | sending `READY_FOR_TRANSITION` | `EXECUTE_TRANSITION` not received |
| `T_quorum_grace` | 2 s | Coordinator | `T_prepare_max` expiry | extra slack before declaring quorum failure |
| `T_coordinator_grace` | 10 s | Member | observing Coordinator silence | Coordinator handover may be claimed |

Timeout expiration MUST trigger the following deterministic fallback:
- **`T_prepare_max + T_quorum_grace` on Coordinator**: send `ABORT_TRANSITION` with `reason_code = ERR_READY_TIMEOUT`. Coordinator MAY re-issue PREPARE on the next epoch, omitting any member that the transport has reported unreachable.
- **`T_ready_max` on Member**: drop the local pending transition (return to `T_IDLE`). The member MUST NOT send `READY_FOR_TRANSITION` retroactively. If the member subsequently observes `EXECUTE_TRANSITION` for a tid it never readied, it MUST enter `RESYNCING` and request a digest.
- **`T_execute_max` on Member**: assume the Coordinator failed. Trigger `RESYNCING`; if the Coordinator is confirmed lost (transport closed), participate in coordinator handover per `gbp-control-plane.md` §5.1.

## 7. IANA Considerations
No additional IANA actions.

## 8. Security Considerations
State machine divergence is a security risk. Implementations MUST reject invalid transitions and MUST NOT apply side effects before state validation.

## 9. References
### 9.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
