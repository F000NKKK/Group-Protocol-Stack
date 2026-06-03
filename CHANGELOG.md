## 1.8.0 (2026-06-03)

### Features

- feat(node): export/restore outbound sequence counters (survive rebuild) (6231c70)
- feat(wasm): inviteMany — add several members in one Add commit (abef8be)

### Bug Fixes

- feat(node): export/restore outbound sequence counters (survive rebuild) (6231c70)

### Tests

- test(mls): restored pre-key still accepts a Welcome (4f1e5e0)

### Chores

- chore(deps): bump koffi from 3.0.1 to 3.0.2 in /js (#28) (89800a4)

---

## 1.7.0 (2026-06-02)

### Features

- feat(mls): export/restore MLS state across all bindings (45f4411)

---

## 1.6.0 (2026-06-01)

### Features

- feat(wasm): expose gap, gsp, sframe + full MLS lifecycle and control plane (1e2a5e8)

### Tests

- feat(wasm): expose gap, gsp, sframe + full MLS lifecycle and control plane (1e2a5e8)

### Chores

- chore(deps-dev): bump ts-jest from 29.4.9 to 29.4.11 in /js (#25) (3ef32bf)
- chore(deps-dev): bump @types/node from 25.9.0 to 25.9.1 in /js (#24) (cff35b7)
- chore(deps): bump serde_json from 1.0.149 to 1.0.150 (#27) (466a0fd)
- chore(deps): bump koffi from 3.0.0 to 3.0.1 in /js (#26) (e5aad48)
- chore(deps): bump actions/cache from 4 to 5 (#22) (373e744)
- chore(deps-dev): bump @types/node from 25.8.0 to 25.9.0 in /js (#23) (0f9ad2a)

---

## 1.5.5 (2026-05-20)

_No conventional commits found in this range._

---

## 1.5.4 (2026-05-18)

### Features

- feat(wasm): implement MLS invite/acceptWelcome, add tests, examples, CI (696b557)

### Bug Fixes

- fix(wasm): pass Uint8Array as &[u8] via .to_vec() in wasm-bindgen-test tests
- fix(wasm): remove redundant static_method_of from impl-block #[wasm_bindgen] attrs; fixes unused-variable warnings
- fix(wasm): remove unused StreamType import from lib.rs; move to tests module

### Documentation

- docs(readme): remove file extensions from WASM example links in bindings table

---

## 1.5.2 (2026-05-17)

### Documentation

- docs: add C#/Python/JS examples, fix gbp-node README, add Examples column to README (0d247ff)
- docs: fix README link, expand gbp-node README, add gtp/gap/gsp examples (bba146f)

### Chores

- chore: update Cargo.lock for anyhow dev-dep in gbp-stack examples (9e7402a)

---

## 1.5.1 (2026-05-17)

### CI

- ci: publish gbp-proto and gbp-flat before sub-protocols (56455fb)

---

## 1.5.0 (2026-05-17)

### Features

- feat(codec): add PayloadCodec — per-frame CBOR/Protobuf/FlatBuffers selection (365d0fa)

### Documentation

- docs(readme): remove legacy badge (db4bc6a)
- docs(readme): mark v1.4.2 as legacy/deprecated (190db51)

---

## 1.4.2 (2026-05-17)

### Bug Fixes

- fix(sframe): use Default::default() to break CodeQL taint source on HKDF output buffer (353af1f)
- fix(sframe): add lgtm suppression for CodeQL false positive on HKDF label (d19cb2d)
- fix(js): use moduleResolution=node for koffi 3.x TS compatibility (1157ffc)
- fix(sframe): rename salt->base_nonce to suppress CodeQL false positive (1ac3924)
- fix(ci): add workflow-level permissions and clarify HKDF domain labels (201be72)
- fix(security): only latest minor series is supported, not top-2 (00dd20a)
- fix(release): correct SECURITY.md supported versions and Update-SecurityPolicy false-positive (6162529)

---

## 1.4.1 (2026-05-17)

### Bug Fixes

- fix(release): replace em-dash with ASCII dash to fix PowerShell 5.1 parser (2c37498)
- fix: koffi 3.x type rename, csharp README path, release script lock-file regen; drop tracked .vs/ files (24c0ecc)
- fix(release): correct SECURITY.md supported versions table and fix regex to match padded header (32f750e)

### Chores

- chore(deps-dev): bump jest and @types/jest in /js (#15) (3933b98)
- chore(csharp): move README.md to csharp/ root (a07845d)
- chore(deps): bump koffi from 2.16.2 to 3.0.0 in /js (#16) (be53c2e)

---

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





























