# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP / MLS Binding

## Abstract
This document defines the contractual relationship between the GBP control plane and the underlying MLS (RFC 9420) state machine. It clarifies which MLS messages are visible at which GBP layer, who is responsible for distributing them, and how MLS epochs and proposal types map to GBP TransitionIDs.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.

## 1. Conventions
BCP 14 keywords from [RFC2119] and [RFC8174] apply.

## 2. MLS Message Visibility
RFC 9420 defines two relevant message types for membership changes:
- **Welcome** — sent to *new* members only; carries enough state to bootstrap the joiner's MLS group.
- **Commit** — sent to *existing* members; instructs them to apply a set of proposals (Add/Update/Remove) and advance epoch.

GBP REQUIRES distinct distribution paths:
- Welcome MUST be **unicast** to the joiner.
- Commit MUST be **broadcast** to all existing members, embedded as `args.commit` in the `PREPARE_TRANSITION` control message.

A bug-class implementation that distributes only Welcome (RFC 9420 §11) leaves existing members unable to advance their MLS epoch and breaks all subsequent application traffic. Implementations MUST expose both messages from their MLS API.

## 3. Required MLS API Surface
The GBP MLS wrapper MUST expose:

```
mls.invite(key_packages: [KeyPackage]) -> { commit: bytes, welcome: bytes }
mls.remove_members(leaf_indices: [u32]) -> { commit: bytes }
mls.process_message(message: bytes) -> ProcessedMessageKind
mls.accept_welcome(welcome: bytes) -> ()
mls.epoch() -> u64
mls.group_id() -> [u8; 16]
mls.export_key_package() -> bytes
```

`process_message` MUST handle Commit messages and is REQUIRED by every existing member. `ProcessedMessageKind` distinguishes Commit, Application, Proposal so callers know which path to take; for the GBP control plane only Commit is relevant.

The `invite` and `remove_members` calls MUST advance the local MLS state immediately (via `merge_pending_commit`) so that the Coordinator's view matches the post-transition state used to derive PREPARE bytes.

## 4. Mapping MLS Epoch to GBP TransitionID
- Each accepted MLS Commit advances `mls.epoch` by 1.
- Each `EXECUTE_TRANSITION` carries the same `transition_id` that was announced in the corresponding `PREPARE_TRANSITION`.
- Implementations MUST maintain the invariant `node.current_epoch == mls.epoch()` at every steady state (post-EXECUTE, pre-next-PREPARE).
- During a transition: `mls.epoch()` advances when the Commit is processed (step 5 of leave / step 5 of add); `node.current_epoch` advances on `apply_transition` (step 7). Between these two points, the node is in `T_READY` and MUST NOT send application data.

## 5. DS Responsibilities
A Delivery Service implementation handling GBP MUST:
1. Forward `PREPARE_TRANSITION` (target=0) to every Active member except the original sender.
2. Forward Welcome unicasts addressed to a specific MemberID (target=N).
3. Detect transport closures and emit a `MemberLeft { member_id, reason }` notification to the Coordinator within `T_coordinator_grace`.
4. Provide a per-DS monotonic sequence on forwarded control messages to satisfy `gbp_rfc.md` §8 tie-break ordering.

P2P fallback deployments (no DS) MUST simulate items 1-3 in the Coordinator process; item 4 reduces to local accept order.

## 6. Joiner State Bootstrap
A joiner that receives a Welcome MUST:
1. `mls.accept_welcome(welcome_bytes)` — sets up MLS group at the post-commit epoch.
2. Read `mls.epoch()` and `mls.group_id()` from the resulting state.
3. Construct GBP node with `gbp_node_create(member_id, group_id_16)`.
4. Call `gbp_node_bootstrap_joiner(epoch=0)` — the GBP node's `current_epoch` starts at 0 and advances only via the upcoming `EXECUTE_TRANSITION`. The MLS epoch returned in step 2 is informational only at this point; it WILL be matched after the joiner emits READY and receives EXECUTE for the same TransitionID that admitted them.
5. Wait for `PREPARE_TRANSITION` carrying the joiner's own admission Commit. The Coordinator MUST send this even though the joiner already has the post-commit state from Welcome — the joiner's GBP layer needs the explicit transition record to advance `current_epoch` and `last_transition_id`.

## 7. Coordinator State After Invite
The Coordinator that calls `mls.invite`:
1. Has `mls.epoch()` already advanced (via `merge_pending_commit`).
2. MUST NOT send any application data frame yet — `node.current_epoch` is still old.
3. MUST send `PREPARE_TRANSITION` with the new `transition_id` and embed the commit bytes.
4. MUST NOT call `apply_transition` locally until after broadcasting `EXECUTE_TRANSITION`. The Coordinator goes through the same `T_PREPARED -> T_COMMIT_PROCESSED -> T_READY -> T_EXECUTED` sequence as any other member, with itself implicitly counted in the READY quorum.

## 8. Out-of-Order Welcome and Commit
The DS does not guarantee that a joiner's Welcome arrives before the existing members' PREPARE+Commit, or vice versa. Both orderings are legal:
- Existing member receives PREPARE before joiner accepts Welcome — quorum count waits for the joiner's READY (potentially up to `T_ready_max`).
- Joiner accepts Welcome before existing members process Commit — joiner waits in `T_PREPARED` for the missing `args.commit` to arrive embedded in PREPARE; if PREPARE arrived first and Commit was extracted, joiner is already in `T_COMMIT_PROCESSED`.

Implementations MUST be robust to both orderings.

## 9. Security Considerations
- The Coordinator's MLS state advances eagerly (step 1 of §7). If the transition aborts, the Coordinator MUST be able to roll back to the pre-commit state. RFC 9420 §12 supports this only if `merge_pending_commit` has not been called yet. Implementations SHOULD therefore defer the merge until READY quorum is observed; if the wrapper merges eagerly, an abort requires re-bootstrap of the Coordinator's MLS context (acceptable in deployments where Coordinator-side abort is rare, but MUST be documented as a known cost).
- Welcome messages MUST be sent over a confidential transport. Disclosure of the Welcome to any party other than the intended joiner allows that party to reconstruct the new epoch's secrets.
- An attacker who replays a stale PREPARE+Commit MUST be detected via TransitionID monotonicity (`gbp_rfc.md` §8).

## 10. References
### 10.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
- `gbp_rfc.md`
- `gbp-control-plane.md`
- `gbp-state-machine.md`
- `gbp-leave-flow.md`
