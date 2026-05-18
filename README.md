# Group Protocol Stack

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

[![Release](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/release.yml/badge.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/release.yml)
[![Dependabot](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/dependabot/dependabot-updates/badge.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/dependabot/dependabot-updates)

[![Crates IO](https://img.shields.io/crates/v/gbp-stack?logo=rust&label=crates.io)](https://crates.io/crates/gbp-stack)
[![NuGet](https://img.shields.io/nuget/v/GBPStack?logo=nuget&label=NuGet)](https://www.nuget.org/packages/GBPStack)
[![PyPI](https://img.shields.io/pypi/v/gbp-stack?logo=pypi&label=PyPI&logoColor=white)](https://pypi.org/project/gbp-stack/)
[![NPM](https://img.shields.io/npm/v/%40voluntas-progressus%2Fgbp-stack?logo=npm&label=npm)](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack)
[![NPM WASM](https://img.shields.io/npm/v/%40voluntas-progressus%2Fgbp-stack-wasm?logo=npm&label=npm%20(wasm))](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack-wasm)
[![GitHub release](https://img.shields.io/github/v/release/F000NKKK/Group-Protocol-Stack?label=latest%20release&logo=github)](https://github.com/F000NKKK/Group-Protocol-Stack/releases)
[![GitHub release date](https://img.shields.io/github/release-date/F000NKKK/Group-Protocol-Stack?label=released)](https://github.com/F000NKKK/Group-Protocol-Stack/releases)

[![GitHub Issues](https://img.shields.io/github/issues/F000NKKK/Group-Protocol-Stack?logo=github)](https://github.com/F000NKKK/Group-Protocol-Stack/issues)
[![GitHub Discussions](https://img.shields.io/github/discussions/F000NKKK/Group-Protocol-Stack?logo=github)](https://github.com/F000NKKK/Group-Protocol-Stack/discussions)

A layered, end-to-end encrypted group-messaging protocol family built on top
of [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420). One base
protocol — **GBP** — provides framed, replay-protected, AEAD-encrypted
delivery; three sub-protocols — **GTP**, **GAP**, **GSP** — provide message
semantics for text, audio and signalling, the way TCP and UDP build on top
of IP.

## Architecture

```
┌── application ──────────────────────────────────────────────────────┐
│   GTP · GAP · GSP    (TCP / UDP / SCTP-like)                        │
├─────────────────────────────────────────────────────────────────────┤
│   GBP                (the IP-like base)                             │
│   - frame codec  - AEAD  - replay window  - FSM  - control plane    │
├─────────────────────────────────────────────────────────────────────┤
│   MLS (RFC 9420)     (group key agreement, exporter)                │
└─────────────────────────────────────────────────────────────────────┘
```

## Bindings

The same protocol family is published from one source tree to every major
package manager. Each binding ships pre-built native binaries for `win-x64`,
`win-arm64`, `linux-x64`, `linux-arm64`, `osx-x64` and `osx-arm64`.

| Language | Package | Source | README | Examples |
| --- | --- | --- | --- | --- |
| Rust    | [`gbp-stack`](https://crates.io/crates/gbp-stack)                              | [`crates/gbp/stack`](crates/gbp/stack) | [README](crates/gbp/stack/README.md) | [gtp_chat](crates/gbp/stack/examples/gtp_chat.rs) · [gap_audio](crates/gbp/stack/examples/gap_audio.rs) · [gsp_signals](crates/gbp/stack/examples/gsp_signals.rs) |
| .NET    | [`GBPStack`](https://www.nuget.org/packages/GBPStack)                          | [`csharp/GBPStack`](csharp/GBPStack)   | [README](csharp/README.md)           | [GtpChat](csharp/examples/GtpChat.cs) · [GapAudio](csharp/examples/GapAudio.cs) · [GspSignals](csharp/examples/GspSignals.cs) |
| Python  | [`gbp-stack`](https://pypi.org/project/gbp-stack/)                             | [`python`](python)                     | [README](python/README.md)           | [gtp_chat](python/examples/gtp_chat.py) · [gap_audio](python/examples/gap_audio.py) · [gsp_signals](python/examples/gsp_signals.py) |
| Node.js | [`@voluntas-progressus/gbp-stack`](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack) | [`js`](js) | [README](js/README.md) | [gtpChat](js/examples/gtpChat.ts) · [gapAudio](js/examples/gapAudio.ts) · [gspSignals](js/examples/gspSignals.ts) |
| Browser (WASM) | [`@voluntas-progressus/gbp-stack-wasm`](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack-wasm) | [`crates/gbp/wasm`](crates/gbp/wasm) | [README](crates/gbp/wasm/README.md) | — |

## Rust crates

The reference implementation is a Rust workspace. The umbrella crate
[`gbp-stack`](crates/gbp/stack) re-exports every layer; smaller consumers
can depend on individual crates directly.

| Crate                                                            | Purpose                                                       |
| ---------------------------------------------------------------- | ------------------------------------------------------------- |
| [`gbp-core`](https://crates.io/crates/gbp-core)                  | Shared type vocabulary (StreamType, flags, FSMs, error codes) |
| [`gbp-protocol`](https://crates.io/crates/gbp-protocol)          | Base GBP layer: GbpFrame, control plane, ErrorObject          |
| [`gtp-protocol`](https://crates.io/crates/gtp-protocol)          | Group Text Protocol (text + attachments + history/watermark)  |
| [`gap-protocol`](https://crates.io/crates/gap-protocol)          | Group Audio Protocol (Opus + key-overlap buffer + jitter)     |
| [`gsp-protocol`](https://crates.io/crates/gsp-protocol)          | Group Signaling Protocol (signals + per-signal validation + roles + capabilities) |
| [`gbp-mls`](https://crates.io/crates/gbp-mls)                    | MLS (RFC 9420) integration via openmls                        |
| [`gbp-transport`](https://crates.io/crates/gbp-transport)        | Length-prefixed TCP adapter + async QUIC transport (quinn)    |
| [`gbp-node`](https://crates.io/crates/gbp-node)                  | GBP-layer group node (FSM, replay, control plane, coordinator handover, timer engine, tie-break) |
| [`gbp-sframe`](https://crates.io/crates/gbp-sframe)              | SFrame (draft-ietf-sframe-enc) E2EE for GAP audio streams     |
| [`gbp-proto`](https://crates.io/crates/gbp-proto)                | Protobuf schemas for GBP/GTP/GAP/GSP (via prost, no protoc)  |
| [`gbp-flat`](https://crates.io/crates/gbp-flat)                  | FlatBuffers schemas for GBP/GTP/GAP/GSP (via planus, no flatc) |
| [`gbp-stack`](https://crates.io/crates/gbp-stack)                | Top-level facade re-exporting every layer                     |
| [`gbp-stack-ffi`](https://crates.io/crates/gbp-stack-ffi)        | C ABI / cdylib for non-Rust consumers                         |
| [`gbp-cli`](https://crates.io/crates/gbp-cli)                    | Reference CLI (`gbp-node listen|connect`)                     |

## Payload codec

Every sub-protocol (GTP, GAP, GSP) can encode its payload as **CBOR**,
**Protobuf**, or **FlatBuffers**. The codec is negotiated per-frame via
the `pf` field of the enclosing GBP frame. The default (`pf=0`, CBOR) is
omitted from the wire for backward compatibility.

```
PayloadCodec::Cbor        (0) — default; pf field omitted when 0
PayloadCodec::Protobuf    (1) — via gbp-proto / prost
PayloadCodec::FlatBuffers (2) — via gbp-flat / planus
```

Callers pass `codec` to every `send` / `accept` call; the chosen codec is
echoed back in the `payload_received` event so the receiver can decode correctly.

## Specifications

See [`docs/`](docs/) for the protocol specifications (English in
`docs/en/`, Russian in `docs/ru/`).

## License

Licensed under [Apache License, Version 2.0](LICENSE).
