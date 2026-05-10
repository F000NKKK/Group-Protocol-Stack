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

## Install

The same protocol family is available from every major package manager.
Each binding ships pre-built native binaries for `win-x64`, `win-arm64`,
`linux-x64`, `linux-arm64`, `osx-x64` and `osx-arm64`.

### Rust (crates.io)

```toml
[dependencies]
gbp-stack = "1.0.0-rc4"
```

### .NET (NuGet)

```sh
dotnet add package GBPStack --version 1.0.0-rc4
```

### Python (PyPI)

```sh
pip install gbp-stack==1.0.0-rc4
```

### Node.js (npm)

```sh
npm install @voluntas-progressus/gbp-stack@1.0.0-rc4
```

## Crates

The reference implementation is a Rust workspace published as several
focused crates:

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
| [`gbp-stack`](https://crates.io/crates/gbp-stack)                | Top-level facade re-exporting every layer                     |
| [`gbp-stack-ffi`](https://crates.io/crates/gbp-stack-ffi)        | C ABI / cdylib for non-Rust consumers                         |
| [`gbp-cli`](https://crates.io/crates/gbp-cli)                    | Reference CLI (`gbp-node listen|connect`)                     |

## Quick start

The same flow expressed in each binding:

### Rust

```rust,ignore
use gbp_stack::{GroupNode, GtpClient, MlsContext};

let (mut alice_mls, _)      = MlsContext::new_member(b"alice")?;
let (mut bob_mls,   bob_kp) = MlsContext::new_member(b"bob")?;
let welcome = alice_mls.invite(&[bob_kp.key_package().clone()])?;
bob_mls.accept_welcome(&welcome)?;

let group_id = alice_mls.group_id_16();
let mut alice = GroupNode::new(1, group_id);
let mut bob   = GroupNode::new(2, group_id);
alice.bootstrap_as_creator(alice_mls.epoch());
bob.bootstrap_as_joiner(bob_mls.epoch());

let mut gtp_alice = GtpClient::new();
let mut gtp_bob   = GtpClient::new();

let frame = gtp_alice.send(&mut alice, &mut alice_mls, 2, 0xCAFE_F00D, "hi")?;
for ev in bob.on_wire(&mut bob_mls, &frame.wire)? {
    if let gbp_stack::Event::PayloadReceived(p) = ev {
        if let Ok(gbp_stack::GtpAccept::New(m)) = gtp_bob.accept(&p.plaintext) {
            println!("{}", m.text().unwrap_or_default());
        }
    }
}
```

### .NET

```csharp
using GBPStack;

using var aliceMls = MlsContext.Create("alice");
using var bobMls   = MlsContext.Create("bob");
bobMls.AcceptWelcome(aliceMls.Invite(bobMls.ExportKeyPackage()));

using var alice = GroupNode.Create(memberId: 1, aliceMls.GroupId);
using var bob   = GroupNode.Create(memberId: 2, bobMls.GroupId);
alice.BootstrapAsCreator(aliceMls.Epoch);
bob.BootstrapAsJoiner(bobMls.Epoch);

using var gtpAlice = GtpClient.Create();
using var gtpBob   = GtpClient.Create();

var frame = gtpAlice.Send(alice, aliceMls, target: 2, messageId: 0xCAFE_F00D, "hi");
foreach (var ev in bob.OnWire(bobMls, frame.Wire))
    if (ev.Kind == "payload_received" && ev.StreamType == StreamType.Text)
        Console.WriteLine(gtpBob.Accept(ev.Plaintext!).Text);
```

### Python

```python
from gbp_stack import MlsContext, GroupNode, GtpClient, StreamType

with MlsContext.create("alice") as alice_mls, \
     MlsContext.create("bob")   as bob_mls:
    bob_mls.accept_welcome(alice_mls.invite(bob_mls.export_key_package()))

    gid = alice_mls.group_id
    with GroupNode.create(1, gid) as alice, \
         GroupNode.create(2, gid) as bob, \
         GtpClient.create() as gtp_alice, \
         GtpClient.create() as gtp_bob:

        alice.bootstrap_as_creator(alice_mls.epoch)
        bob.bootstrap_as_joiner(bob_mls.epoch)

        frame = gtp_alice.send(alice, alice_mls, target=2,
                               message_id=0xCAFE_F00D, text="hi")
        for ev in bob.on_wire(bob_mls, frame.wire):
            if ev.kind == "payload_received" and ev.stream_type == StreamType.TEXT:
                print(gtp_bob.accept(ev.plaintext).text)
```

### Node.js / TypeScript

```ts
import { MlsContext, GroupNode, GtpClient, StreamType } from "@voluntas-progressus/gbp-stack";

const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");
bobMls.acceptWelcome(aliceMls.invite(bobMls.exportKeyPackage()));

const gid = aliceMls.groupId;
const alice = GroupNode.create(1, gid);
const bob   = GroupNode.create(2, gid);
alice.bootstrapAsCreator(aliceMls.epoch);
bob.bootstrapAsJoiner(bobMls.epoch);

const gtpAlice = GtpClient.create();
const gtpBob   = GtpClient.create();

const frame = gtpAlice.send(alice, aliceMls, 2, 0xCAFEF00Dn, "hi");
for (const ev of bob.onWire(bobMls, frame.wire)) {
    if (ev.kind === "payload_received" && ev.streamType === StreamType.Text) {
        console.log(gtpBob.accept(ev.plaintext!).text);
    }
}
```

## Specifications

See [`docs/`](docs/) for the protocol specifications (English in
`docs/en/`, Russian in `docs/ru/`).

## License

Licensed under [Apache License, Version 2.0](LICENSE).
