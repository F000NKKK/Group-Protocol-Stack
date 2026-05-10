# Group Protocol Stack

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![release](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/release.yml/badge.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/release.yml)
[![crates.io](https://img.shields.io/crates/v/gbp-stack?logo=rust&label=crates.io)](https://crates.io/crates/gbp-stack)
[![NuGet](https://img.shields.io/nuget/v/GBPStack?logo=nuget&label=NuGet)](https://www.nuget.org/packages/GBPStack)
[![PyPI](https://img.shields.io/pypi/v/gbp-stack?logo=pypi&label=PyPI&logoColor=white)](https://pypi.org/project/gbp-stack/)
[![npm](https://img.shields.io/npm/v/%40voluntas-progressus%2Fgbp-stack?logo=npm&label=npm)](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack)

A layered, end-to-end encrypted group-messaging protocol family built on top
of [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420). One base
protocol — **GBP** — provides framed, replay-protected, AEAD-encrypted
delivery; three sub-protocols — **GTP**, **GAP**, **GSP** — provide message
semantics for text, audio and signalling, the way TCP and UDP build on top
of IP.

## Crates

The reference implementation is a Rust workspace published as several
focused crates:

| Crate                                                            | Purpose                                                       |
| ---------------------------------------------------------------- | ------------------------------------------------------------- |
| [`gbp-core`](https://crates.io/crates/gbp-core)                  | Shared type vocabulary (StreamType, flags, FSMs, error codes) |
| [`gbp-protocol`](https://crates.io/crates/gbp-protocol)          | Base GBP layer: GbpFrame, control plane, ErrorObject          |
| [`gtp-protocol`](https://crates.io/crates/gtp-protocol)          | Group Text Protocol (text)                                    |
| [`gap-protocol`](https://crates.io/crates/gap-protocol)          | Group Audio Protocol (Opus)                                   |
| [`gsp-protocol`](https://crates.io/crates/gsp-protocol)          | Group Signaling Protocol                                      |
| [`gbp-mls`](https://crates.io/crates/gbp-mls)                    | MLS (RFC 9420) integration via openmls                        |
| [`gbp-transport`](https://crates.io/crates/gbp-transport)        | Length-prefixed TCP transport adapter                         |
| [`gbp-node`](https://crates.io/crates/gbp-node)                  | GBP-layer group node (FSM, replay, control plane)             |
| [`gbp-stack`](https://crates.io/crates/gbp-stack)                | Top-level facade re-exporting every layer                     |
| [`gbp-stack-ffi`](https://crates.io/crates/gbp-stack-ffi)        | C ABI / cdylib for non-Rust consumers                         |
| [`gbp-cli`](https://crates.io/crates/gbp-cli)                    | Reference CLI (`gbp-node listen|connect`)                     |

## Architecture

```
┌── application ──────────────────────────────────────────────────────┐
│   GTP · GAP · GSP    (TCP / UDP / SCTP-like)                        │
├──────────────────────────────────────────────────────────────────────┤
│   GBP                (the IP-like base)                             │
│   - frame codec  - AEAD  - replay window  - FSM  - control plane    │
├──────────────────────────────────────────────────────────────────────┤
│   MLS (RFC 9420)     (group key agreement, exporter)                │
└──────────────────────────────────────────────────────────────────────┘
```

## Quick start

```toml
[dependencies]
gbp-stack = "0.2"
```

```rust,ignore
use gbp_stack::{GroupNode, GtpClient, MlsContext};

let (mut alice_mls, _) = MlsContext::new_member(b"alice")?;
let (mut bob_mls, kp)  = MlsContext::new_member(b"bob")?;
// … MLS handshake (publish KeyPackage, invite, accept welcome) …

let mut alice = GroupNode::new(1, alice_mls.group_id_16());
let mut bob   = GroupNode::new(2, bob_mls.group_id_16());
alice.bootstrap_as_creator(alice_mls.epoch());
bob.bootstrap_as_joiner(bob_mls.epoch());

let mut gtp_alice = GtpClient::new();
let mut gtp_bob   = GtpClient::new();

let frame = gtp_alice.send(&mut alice, &mut alice_mls, 2, 0xCAFE_F00D, "hi")?;
for ev in bob.on_wire(&mut bob_mls, &frame.wire)? {
    // dispatch payload_received to gtp_bob.accept(...)
}
```

## Specifications

See [`docs/`](docs/) for the protocol specifications (English in
`docs/en/`, Russian in `docs/ru/`).

## License

Licensed under [Apache License, Version 2.0](LICENSE).
