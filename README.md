# Group Protocol Stack

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

[![Release](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/release.yml/badge.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/release.yml)
[![Dependabot](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/dependabot/dependabot-updates/badge.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/actions/workflows/dependabot/dependabot-updates)

[![Crates IO](https://img.shields.io/crates/v/gbp-stack?logo=rust&label=crates.io)](https://crates.io/crates/gbp-stack)
[![NuGet](https://img.shields.io/nuget/v/GBPStack?logo=nuget&label=NuGet)](https://www.nuget.org/packages/GBPStack)
[![PyPI](https://img.shields.io/pypi/v/gbp-stack?logo=pypi&label=PyPI&logoColor=white)](https://pypi.org/project/gbp-stack/)
[![NPM](https://img.shields.io/npm/v/%40voluntas-progressus%2Fgbp-stack?logo=npm&label=npm)](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack)

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

| Language | Package | Source | README |
| --- | --- | --- | --- |
| Rust    | [`gbp-stack`](https://crates.io/crates/gbp-stack)                              | [`crates/gbp/stack`](crates/gbp/stack)   | [README](crates/gbp/stack/README.md) |
| .NET    | [`GBPStack`](https://www.nuget.org/packages/GBPStack)                          | [`csharp/GBPStack`](csharp/GBPStack)     | [README](csharp/GBPStack/README.md) |
| Python  | [`gbp-stack`](https://pypi.org/project/gbp-stack/)                             | [`python`](python)                       | [README](python/README.md) |
| Node.js | [`@voluntas-progressus/gbp-stack`](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack) | [`js`](js) | [README](js/README.md) |

## Rust crates

The reference implementation is a Rust workspace. The umbrella crate
[`gbp-stack`](crates/gbp/stack) re-exports every layer; smaller consumers
can depend on individual crates directly.

| Crate                                                            | Purpose                                                       |
| ---------------------------------------------------------------- | ------------------------------------------------------------- |
| [`gbp-core`](https://crates.io/crates/gbp-core)                  | Shared type vocabulary (StreamType, flags, FSMs, error codes) |
| [`gbp-protocol`](https://crates.io/crates/gbp-protocol)          | Base GBP layer: GbpFrame, control plane, ErrorObject          |
| [`gtp-protocol`](https://crates.io/crates/gtp-protocol)          | Group Text Protocol (text + history/watermark)                |
| [`gap-protocol`](https://crates.io/crates/gap-protocol)          | Group Audio Protocol (Opus + jitter buffer)                   |
| [`gsp-protocol`](https://crates.io/crates/gsp-protocol)          | Group Signaling Protocol (signals + roles + capabilities)     |
| [`gbp-mls`](https://crates.io/crates/gbp-mls)                    | MLS (RFC 9420) integration via openmls                        |
| [`gbp-transport`](https://crates.io/crates/gbp-transport)        | Length-prefixed TCP transport adapter                         |
| [`gbp-node`](https://crates.io/crates/gbp-node)                  | GBP-layer group node (FSM, replay, control plane)             |
| [`gbp-sframe`](https://crates.io/crates/gbp-sframe)              | SFrame (draft-ietf-sframe-enc) E2EE for GAP audio streams     |
| [`gbp-stack`](https://crates.io/crates/gbp-stack)                | Top-level facade re-exporting every layer                     |
| [`gbp-stack-ffi`](https://crates.io/crates/gbp-stack-ffi)        | C ABI / cdylib for non-Rust consumers                         |
| [`gbp-cli`](https://crates.io/crates/gbp-cli)                    | Reference CLI (`gbp-node listen|connect`)                     |

## Specifications

See [`docs/`](docs/) for the protocol specifications (English in
`docs/en/`, Russian in `docs/ru/`).

## License

Licensed under [Apache License, Version 2.0](LICENSE).
