## Unreleased

### Features

- feat(gbp-sframe): new crate implementing SFrame (draft-ietf-sframe-enc) E2EE for GAP audio streams
  - Per-sender AES-128-GCM / AES-256-GCM keys derived from MLS ExportSecret via HKDF
  - KID encoding: `(epoch << 16) | leaf_index`; nonce: `salt XOR CTR_LE64`
  - 1024-entry sliding-window replay protection per sender
  - `SFrameSession::from_mls`, `SFrameEncryptor`, `SFrameDecryptor` APIs
- feat(gbp-mls): add `export_raw(label, context, len)` for custom MLS exporter label access
- feat(gbp-stack): re-export `gbp_sframe` module
- feat(gbp-stack-ffi): add `gbp_sframe_*` FFI family (session_create, session_free, encryptor_create, encryptor_free, encrypt, decrypt)
- feat(csharp): add `SFrameSession`, `SFrameEncryptor`, `SFrameCipherSuite` managed wrappers
- feat(js): add `SFrameSession`, `SFrameEncryptor` TypeScript wrappers
- feat(python): add `SFrameSession`, `SFrameEncryptor`, `SFrameDecryptResult` Python wrappers

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













