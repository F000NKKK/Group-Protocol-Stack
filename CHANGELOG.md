## 1.4.0 (2026-05-17)

### Features

- feat: integration tests, API fixes, and documentation audit (v1.3.0) (89fbd52)
- feat(bindings): add coordinator events and frame/error helpers to all bindings (c8afc00)
- feat(transport): add QUIC transport via quinn (b2a8eeb)
- feat(gbp-flat): add FlatBuffers codec crate for GBP/GTP/GAP/GSP (f5c4e11)
- feat(gbp-proto): add Protobuf codec crate for GBP/GTP/GAP/GSP (b2cae42)
- feat(gsp): per-signal args validation; feat(core): ConformanceClass A/B/C (4b7bcf9)
- feat(node): timer engine, coordinator handover, tie-break; feat(gap): T_overlap buffer (e47ee86)

### Refactoring

- refactor(csharp): one type per file — split all multi-type .cs files (0425324)
- refactor(csharp): move GbpHelpers to its own file (502fae1)

### Tests

- test: add unit tests for mls, gtp, gsp, gbp-base, fix FFI event match (91a9cf6)

### Documentation

- docs(changelog): remove manually written Unreleased block (auto-generated) (c09cc92)
- docs: update all READMEs and CHANGELOG for 1.4.0 feature set (9093d0c)
- docs(gbp-proto): add README.md required for crates.io publish (a13efbb)

### Chores

- chore(deps): bump prost from 0.13.5 to 0.14.3 (#14) (b73fbda)
- chore(deps): bump rcgen from 0.13.2 to 0.14.8 (#13) (05a3357)

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



















