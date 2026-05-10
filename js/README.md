# @voluntas-progressus/gbp-stack — Node.js bindings for the Group Protocol Stack

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE)

Node.js bindings for the [Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack):
a layered, end-to-end encrypted group-messaging protocol family built on top
of [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420).

The package wraps the native `gbp_stack` shared library through
[`koffi`](https://www.npmjs.com/package/koffi). Pre-built native binaries
for every supported runtime identifier (`win-x64`, `win-arm64`,
`linux-x64`, `linux-arm64`, `osx-x64`, `osx-arm64`) are bundled in the
package — `npm install @voluntas-progressus/gbp-stack` works out of the box.

## Layers

```
┌── application ──────────────────────────────────────────────────────┐
│   GtpClient · GapClient · GspClient   (TCP / UDP / SCTP-like)       │
├─────────────────────────────────────────────────────────────────────┤
│   GroupNode (GBP — IP-like base)                                    │
├─────────────────────────────────────────────────────────────────────┤
│   MlsContext (RFC 9420)                                             │
└─────────────────────────────────────────────────────────────────────┘
```

## Sub-protocol toolkits

Beyond the protocol clients, the package ships ready-made helpers:

* `MessageHistory` + `Watermark` — bounded GTP message log + per-sender
  high-water mark for serving and consuming resync requests.
* `JitterBuffer` — bounded GAP reorder window keyed by `mediaSourceId`,
  with `push`, `popInOrder`, `popForce` and late-frame detection.
* `RoleRegistry` + `Permissions` — bind numeric role ids to permission
  bit-masks and check them with `require` / `has`.
* `CapabilitiesNegotiator` — track per-member advertisements and query the
  `intersection()`, `union()`, `groupSupports()` and `missing()` views.

## Install

```sh
npm install @voluntas-progressus/gbp-stack@1.1.1
```

## Quick start

```ts
import { MlsContext, GroupNode, GtpClient, StreamType } from "@voluntas-progressus/gbp-stack";

const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");

const bobKp   = bobMls.exportKeyPackage();
const welcome = aliceMls.invite(bobKp);
bobMls.acceptWelcome(welcome);

const gid = aliceMls.groupId;
const alice = GroupNode.create(1, gid);
const bob   = GroupNode.create(2, gid);
alice.bootstrapAsCreator(aliceMls.epoch);
bob.bootstrapAsJoiner(bobMls.epoch);

const gtpAlice = GtpClient.create();
const gtpBob   = GtpClient.create();

const frame = gtpAlice.send(alice, aliceMls, 2, 0xCAFEF00Dn, "hello");
for (const ev of bob.onWire(bobMls, frame.wire)) {
  if (ev.kind === "payload_received" && ev.streamType === StreamType.Text) {
    console.log(gtpBob.accept(ev.plaintext!).text);
  }
}
```

## License

Licensed under [Apache License, Version 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE).
