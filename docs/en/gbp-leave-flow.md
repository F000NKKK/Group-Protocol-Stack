# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Leave Flow

## Abstract
This document specifies the normative procedure for removing a member from a GBP group, covering both voluntary leave (member-initiated via GSP) and involuntary removal (transport disconnect, policy enforcement, or moderator action). It complements `gbp_rfc.md`, `gbp-control-plane.md`, and `gbp-mls-binding.md`.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Conventions
BCP 14 keywords from [RFC2119] and [RFC8174] apply.

## 2. Scope
The leave flow rotates the MLS epoch so that the departed member's traffic secrets are no longer valid for new application data. Forward secrecy after the executed transition is guaranteed by the underlying MLS Commit (RFC 9420 §12.3). The leave flow does NOT recover messages encrypted before the transition; those remain decryptable by the departed member if they retained ratchet state.

## 3. Triggers
A leave transition MUST be initiated by the Coordinator when one of the following is observed:

1. **Voluntary leave** — a member sends `GSP { signal_type = LEAVE (101) }` (see `gsp_rfc.md`). The Coordinator validates authorization per the GSP role-authorization matrix.
2. **Involuntary disconnect** — the DS notifies the Coordinator of transport closure for an Active member, and the silence persists beyond `T_coordinator_grace`. The DS notification mechanism is deployment-specific; see `gbp-mls-binding.md` §5.
3. **Moderator removal** — a member with the `moderator` role sends `GSP { signal_type = LEAVE, target = X }` against another member.
4. **Policy enforcement** — repeated fatal protocol violations from a member exceed deployment-defined thresholds.

If the Coordinator itself is the departing member, coordinator handover (`gbp-control-plane.md` §5.1) MUST complete first; the new Coordinator then drives the leave transition.

## 4. Procedure

```
Step  Actor          Action
----  -----          ------
 1    Coordinator    Validate trigger and identify target leaf_index in MLS tree.
 2    Coordinator    mls.remove_members([leaf_index]) -> commit_bytes
                     Coordinator's MLS state advances locally to new epoch.
 3    Coordinator    Compute next_tid = last_transition_id + 1.
 4    Coordinator    Broadcast PREPARE_TRANSITION to all remaining Active members
                     (target = 0), args = { commit: commit_bytes, removed: target }.
                     The departing member is NOT a recipient.
 5    Each remaining Apply commit via mls.process_message(commit_bytes); MLS state
      member         advances. Send READY_FOR_TRANSITION (target = coordinator_id).
 6    Coordinator    On READY quorum within T_ready_max + T_quorum_grace:
                     broadcast EXECUTE_TRANSITION (target = 0).
                     On timeout: broadcast ABORT_TRANSITION; retry as new tid.
 7    Each remaining apply_transition(next_tid) -> current_epoch++,
      member         last_transition_id = next_tid, replay window cleared.
 8    Coordinator    Same as step 7 locally.
 9    Departed       MAY observe PREPARE_TRANSITION transit through DS. It MUST NOT
      member         attempt to participate. Its own MLS state does not advance.
                     Application traffic frames it sends post-step 8 will be
                     rejected by remaining members with ERR_DECRYPT_FAILED.
```

## 5. Concurrent Leaves
If two leave triggers fire concurrently (e.g. moderator removal of A while B sends voluntary LEAVE), the Coordinator MUST:

1. Queue both into the pending-transition queue.
2. Issue them as two separate transitions OR batch them as one MLS commit with two Remove proposals — implementation choice. If batched, the single PREPARE_TRANSITION carries both `removed: [A, B]` in args.
3. Never collapse a leave into an in-flight add transition; the add MUST complete or abort first.

## 6. Crash and Re-Bootstrap
If a member crashes mid-leave-transition (e.g. sent READY but never received EXECUTE), recovery follows `gbp-control-plane.md` §6.2 (Resync). The member requests a `GROUP_STATE_DIGEST`, replays missing `EXECUTE_TRANSITION`s, and resumes Active.

If the Coordinator crashes between step 4 and step 6, the new Coordinator (handover per §5.1) MUST treat the in-flight leave as ABORTED, re-derive the commit, and issue a fresh PREPARE on the next tid.

## 7. Departed-Member Recovery
A departed member that wishes to rejoin MUST:
1. Generate a fresh KeyPackage (do not reuse the previous one).
2. Publish it via the standard add flow.
3. Be assigned a new MemberID; the previous MemberID is permanently retired (`gbp_rfc.md` §2).

## 8. Security Considerations
- Forward secrecy: after EXECUTE_TRANSITION, the new epoch's traffic secrets are derived from a commit that excludes the departed member's leaf. The departed member cannot decrypt new traffic.
- Past traffic: anything encrypted before the transition remains decryptable by anyone who held the old epoch's keys, including the departed member. Applications requiring deniability or post-compromise security against past traffic MUST rotate keys proactively, not rely on leave-on-departure alone.
- Ghost members: the Coordinator MUST NOT include in PREPARE a member whose transport is closed but who has not yet been formally removed; doing so risks an indefinite quorum stall.
- Replay: the freed MemberID MUST NOT be reused (`gbp_rfc.md` §2). A future joiner with the same MemberID would create ambiguity in replay-window state across history.

## 9. References
### 9.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
- `gbp_rfc.md`
- `gbp-control-plane.md`
- `gbp-mls-binding.md`
- `gsp_rfc.md`
