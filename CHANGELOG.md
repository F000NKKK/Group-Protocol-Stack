## [Unreleased] — 1.4.0

### Features

- feat(gbp-proto): Protobuf schemas for GBP/GTP/GAP/GSP via prost (no protoc required)
- feat(gbp-flat): FlatBuffers schemas for GBP/GTP/GAP/GSP via planus (no flatc required)
- feat(gbp-transport): async QUIC transport via quinn + rustls (`quic` module); same `u32-LE length || bytes` framing as TCP
- feat(gbp-node): automatic coordinator handover — FSM emits `CoordinatorElectionNeeded`, `BecameCoordinator`, `CoordinatorClaim` events
- feat(gbp-node): timer engine for FSM transition timeouts
- feat(gbp-node): tie-break logic for competing commits (deterministic coordinator selection)
- feat(gtp): chunked attachment transfers with SHA-256 integrity (`AttachmentManifest` + `AttachmentChunk`)
- feat(gbp-core): conformance class declarations A / B / C
- feat(gap): per-epoch key-overlap buffer (`T_overlap`) for seamless epoch transitions
- feat(gsp): per-signal args schema validation (gsp_rfc §6)
- feat(bindings): coordinator event kinds in `NodeEvent` for C#, Python, JS
- feat(bindings): `encodeGbpFrame` / `lookupError` utilities in Python and JS; `GbpHelpers` static class in C#
- refactor(csharp): one type per file — split all multi-type `.cs` files into individual files

### Tests

- test(gbp-mls): 15 unit tests covering stream labels, seal/open, two-member invite, epoch staging, key export
- test(gtp): 8 unit tests covering dedup, epoch advance, reset, invalid CBOR
- test(gsp): 9 unit tests covering JOIN/LEAVE membership, mute cleanup, duplicate reject, epoch advance, unknown signal
- test(gbp-base): 7 unit tests for ControlMessage and ErrorObject round-trips and validation

---

## 1.3.0 (2026-05-16)

### Bug Fixes

- fix(js): add node types and ESNext.Disposable to tsconfig (0380f4b)
- fix(release): fall back to latest non-deprecated patch; update SECURITY.md (1.2.2 supported, 1.2.3 deprecated) (ca34e58)

### Documentation

- docs: move FAQ to root, implement CONTRIBUTING, update CoC link (5a1091b)

---

## 1.2.3 (2026-05-16)

### Bug Fixes

- fix(release): use .Contains() instead of .ContainsKey() for PS 5.1 compatibility (5f35371)

### Chores

- chore: remove root CoC and Contributing (live in .github), update release script with auto SECURITY.md (566b02a)

---

## 1.2.2 (2026-05-16)

### Bug Fixes

- fix(csharp): use fixed instead of lambdas for ReadOnlySpan in SFrameSession (21c4cfb)

### Chores

- chore: update Cargo.lock (eb69395)

---

## 1.2.1 (2026-05-16)

### CI

- ci: add gbp-sframe to crates.io publish order (970a5e1)

### Chores

- chore: update Cargo.lock (4ba2b16)

---

## 1.2.0 (2026-05-16)

### Features

- feat(gbp-sframe): add SFrame E2EE crate for GAP audio streams (dbf75c5)

### Chores

- chore: remove manual Unreleased section (release script auto-generates) (30517fa)

---

## 1.1.4 (2026-05-13)

_No conventional commits found in this range._

---

## 1.1.3 (2026-05-13)

_No conventional commits found in this range._

---

## 1.1.2 (2026-05-10)

_No conventional commits found in this range._

---

## 1.1.1 (2026-05-10)

_No conventional commits found in this range._

---

## 1.1.0 (2026-05-10)

_No conventional commits found in this range._

---

## 1.0.1 (2026-05-10)

_No conventional commits found in this range._

---

## 1.0.0 (2026-05-10)

_No conventional commits found in this range._

---

## 1.0.0-rc6. (2026-05-10)

### Bug Fixes

- fix(audit): respect ┬з6.2 validation order; clamp GAP rtp_sequence; auto-reset clients on epoch change (da6a2ed)

### Chores

- chore(release): 1.0.0-rc5. (4d01f6f)

---

## 1.0.0-rc5. (2026-05-10)

### Bug Fixes

- fix(audit): respect ┬з6.2 validation order; clamp GAP rtp_sequence; auto-reset clients on epoch change (da6a2ed)

---

## 1.0.0-rc3 (2026-05-10)

_No conventional commits found in this range._

---

## 1.0.0-rc1 (2026-05-10)

_No conventional commits found in this range._

---

# Changelog

## 0.2.1 (2026-05-10)

_No conventional commits found in this range._


















