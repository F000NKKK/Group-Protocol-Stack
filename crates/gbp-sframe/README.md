# gbp-sframe

[![Crates.io](https://img.shields.io/crates/v/gbp-sframe.svg)](https://crates.io/crates/gbp-sframe)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE)

SFrame ([draft-ietf-sframe-enc]) E2EE for GAP audio streams in the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

## What is SFrame?

SFrame sits *inside* transport-level encryption (SRTP / DTLS) and provides
**end-to-end** confidentiality for media payloads.  An SFU can forward packets
based on RTP headers without ever seeing the Opus frame content.

```
┌──────────────────────────────────────────────────┐
│              Transport encryption                │  ← client ↔ SFU
│  ┌────────────────────────────────────────────┐  │
│  │               SFrame (this crate)          │  │  ← E2E client ↔ client
│  │   ┌──────────────────────────────────────┐  │  │
│  │   │   Encoded media (Opus / VP8 / VP9)   │  │  │
│  │   └──────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

## Key derivation

After each MLS epoch change:

1. **Base key** — `MLS.ExportSecret(label, context=epoch_be8, length=32)`.
2. **Per-sender key** — `HKDF-Expand(base_key, "gbp sframe key " ‖ leaf_be4, L)`.
3. **Per-sender salt** — `HKDF-Expand(base_key, "gbp sframe salt " ‖ leaf_be4, 12)`.
4. **Frame nonce** — `salt XOR (CTR_LE64 ‖ 0x00_00_00_00)`.

The `label` is application-defined (`"gbp/sframe v1"` by convention), so
different deployments can use distinct key universes without changing any
protocol parameter.

## Usage

```rust
use gbp_sframe::{SFrameSession, CipherSuite};
use gbp_mls::MlsContext;

// After MLS handshake — both sides derive a session for the current epoch.
let session = SFrameSession::from_mls(&mls, "gbp/sframe v1", CipherSuite::Aes128Gcm)?;

// Sender (leaf_index = 0):
let mut enc = session.encryptor(0);
let payload = enc.encrypt(opus_frame, rtp_header)?;

// Receiver:
let mut dec = session.decryptor();
let (plaintext, sender_leaf) = dec.decrypt(&payload, rtp_header)?;
```

## Ciphersuite

| Suite           | Key | Salt | AEAD        |
|-----------------|-----|------|-------------|
| `Aes128Gcm`     | 16 B | 12 B | AES-128-GCM |
| `Aes256Gcm`     | 32 B | 12 B | AES-256-GCM |

## Replay protection

A 1024-entry sliding-window replay window is maintained per sender.  Duplicate
or overly-old counters are rejected before decryption.

## SFrame header wire format

```
┌─┬──────────┬──────────┬───────────────┬───────────────┐
│V│  K (3)   │  C (4)   │  KID  (var)   │  CTR  (var)   │
└─┴──────────┴──────────┴───────────────┴───────────────┘
```

* `V` = 0 (SFrame v1).
* `K` — KID length in bytes minus one.
* `C` — CTR length in bytes minus one.
* `KID = (epoch << 16) | leaf_index`.
* `CTR` — big-endian per-sender monotonic counter.

## License

[Apache 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE)

[draft-ietf-sframe-enc]: https://datatracker.ietf.org/doc/draft-ietf-sframe-enc/
