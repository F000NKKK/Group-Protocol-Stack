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

### 5.1 Coordinator Role
At any time exactly one Active member MUST act as the **Coordinator** for a given GroupID. The Coordinator is the only member authorized to issue `PREPARE_TRANSITION`, `EXECUTE_TRANSITION`, and `ABORT_TRANSITION`.

Selection rules:
- The group **creator** is the initial Coordinator immediately after `bootstrap_creator`.
- If the Coordinator becomes unreachable (transport disconnect, fatal error, voluntary leave), the **next-lowest active MemberID** assumes the Coordinator role and MUST broadcast a `CAPABILITIES_ADVERTISE` message carrying a `coordinator_claim=true` flag in args.
- Members MUST accept a coordinator claim only after observing the prior Coordinator's `MemberLeft` notification or a `T_coordinator_grace = 2 * T_ready_max` silence.
- Two simultaneous claims are resolved by lowest claimant MemberID; the loser MUST self-demote.

A member that is not Coordinator MUST silently drop any `PREPARE_TRANSITION` / `EXECUTE_TRANSITION` / `ABORT_TRANSITION` it originates by mistake (defense in depth) and MUST log a protocol error.

### 5.2 Single-Pending Transition Invariant
The Coordinator MUST NOT have more than one outstanding transition at any time. Concurrent `add` / `remove` requests MUST be queued and serialized into one `PREPARE_TRANSITION` per `transition_id`. Multiple proposals MAY be batched into a single commit, but they MUST share one TransitionID.

### 5.3 Welcome / PREPARE Ordering for Add
When the Coordinator admits a new member:
1. The Coordinator computes the MLS commit + Welcome (`mls.invite`) producing both messages.
2. The Coordinator broadcasts `PREPARE_TRANSITION` (target=0) to existing members carrying the new `transition_id` and the **MLS Commit** message in `args.commit`.
3. The Coordinator unicasts the **MLS Welcome** to the new member's transport address (target=joiner_member_id_to_be) **in parallel** with step 2.
4. The new member becomes a "candidate" for the duration of the transition. It is included in the READY quorum only after it has accepted the Welcome and emitted its own `READY_FOR_TRANSITION`.

A receiver MUST process the embedded Commit *before* emitting `READY_FOR_TRANSITION`. If `args.commit` is missing, malformed, or rejected by MLS, the receiver MUST reply with `NACK { code = ERR_COMMIT_INVALID }` and MUST NOT advance state.

### 5.4 Prepare
Coordinator sends `PREPARE_TRANSITION` with the new `transition_id`, the new `epoch` value (post-commit), and the MLS Commit message bytes in `args.commit`. Receivers MUST create local pending transition context, validate version/group_id, decrypt and apply the MLS Commit, then transition `T_IDLE -> T_PREPARED -> T_COMMIT_PROCESSED`.

### 5.5 Ready
Members send `READY_FOR_TRANSITION` only after their MLS state has advanced to the new epoch (Commit applied or Welcome accepted) and all local prerequisites are met. Sender MUST set the same `transition_id` echoed from PREPARE. Members move `T_COMMIT_PROCESSED -> T_READY`.

### 5.6 Execute
Coordinator sends `EXECUTE_TRANSITION` when the readiness quorum is met or timeout policy resolves. Default quorum is **all Active members**, including the new candidate (if any), within `T_ready_max`. If even one Active member fails to ack within `T_ready_max + T_quorum_grace`, the Coordinator MUST send `ABORT_TRANSITION` and re-issue PREPARE on the next epoch, omitting the silent member from the ready set if it has been declared unreachable by the transport.

Receivers MUST atomically apply `node.apply_transition(tid)`: increment `current_epoch`, set `last_transition_id = tid`, clear replay window, transition `T_READY -> T_EXECUTED`.

### 5.7 Abort
Coordinator or policy engine sends `ABORT_TRANSITION` with `args.reason_code`. Receivers MUST discard pending transition state, roll back any locally-staged MLS commit (or recover via Resync if not possible), and return to `T_IDLE`.

## 6. Recovery Procedures

### 6.1 Invalid Commit Recovery
A receiver that detects an invalid Commit (signature failure, epoch out of range, malformed proposal list) MUST emit `REPORT_INVALID_COMMIT`. The opcode args carry a CBOR map:

```
ReportInvalidCommitArgs = {
  "transition_id": uint,         ; mandatory; the offending TransitionID
  "reason_code":   uint,         ; mandatory; ErrorCode from gbp-errors-registry
  "commit_hash":   bstr / nil,   ; optional; SHA-256 of the offending commit bytes
  ? "details":     tstr          ; optional human-readable string, MUST NOT include
                                 ;   secrets, plaintext, or stable identifiers
}
```

After emitting, the reporter MUST clear local pending transition state, request a fresh KeyPackage workflow (re-publish own KeyPackage if it was the joiner; otherwise initiate Resync per §6.2), and refuse to send any application-data frames until the Coordinator's next `EXECUTE_TRANSITION`.

The Coordinator on receipt of `REPORT_INVALID_COMMIT`:
- MUST broadcast `ABORT_TRANSITION` with `reason_code = ERR_COMMIT_INVALID`.
- MUST NOT retry the same commit byte-for-byte.
- SHOULD re-derive the commit (e.g. with a fresh ratchet step) and issue a new PREPARE.

### 6.2 Resync
A client whose state diverged (frame rejected with `ERR_EPOCH_MISMATCH` or `ERR_TRANSITION_MISMATCH`) MUST transition to `RESYNCING` and send `GROUP_STATE_DIGEST_REQUEST` to the Coordinator (or any Active member if Coordinator unknown).

### 6.3 GROUP_STATE_DIGEST Format
The args of `GROUP_STATE_DIGEST_REQUEST` (0x0005) is:

```
GroupStateDigestRequest = {
  "since_tid": uint,             ; the requester's last_transition_id
  ? "since_epoch": uint           ; optional; requester's current_epoch
}
```

The args of `GROUP_STATE_DIGEST_RESPONSE` (0x0006) is:

```
GroupStateDigestResponse = {
  "epoch":                uint,           ; mandatory; responder's current_epoch
  "last_transition_id":   uint,           ; mandatory
  "member_set_root_hash": bstr,           ; mandatory; SHA-256 over canonical CBOR
                                          ;   encoding of sorted MemberID array
  "control_log_tail":     [ ControlLogEntry ], ; mandatory; up to 64 entries since
                                               ;   `since_tid`, oldest first
  ? "coordinator_id":     uint            ; optional; current coordinator MemberID
}

ControlLogEntry = {
  "transition_id": uint,
  "opcode":        uint,
  "sender_id":     uint,
  "args_digest":   bstr     ; SHA-256 of args_cbor of the original control msg
}
```

If the requester's `since_tid` is older than the responder's oldest retained entry (default retention: 64 transitions), the responder SHOULD reply with the full digest and set a `truncated=true` flag, leaving full state recovery to a higher-level rejoin (re-publish KeyPackage and treat as fresh joiner).

After processing the response, the requester MUST verify `member_set_root_hash` matches its own MLS view, replay any missing `EXECUTE_TRANSITION`s in order, and transition `RESYNCING -> ACTIVE`. On hash mismatch, the requester MUST emit `REPORT_INVALID_COMMIT` and treat the session as fatally divergent (re-bootstrap as joiner).

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
