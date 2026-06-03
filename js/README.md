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

## Payload codec

Each sub-protocol payload can be encoded as **CBOR** (default), **Protobuf**,
or **FlatBuffers**. Pass `PayloadCodec` to `send` and `accept`; the chosen
codec is surfaced in `ev.codec` on `payload_received` events.

```ts
import { PayloadCodec } from "@voluntas-progressus/gbp-stack";

// FlatBuffers for low-latency audio, Protobuf for text archival, CBOR default
const frame = gtpAlice.send(alice, aliceMls, 2, 0xCAFEn, "hi", PayloadCodec.FlatBuffers);
for (const ev of bob.onWire(bobMls, frame.wire)) {
  if (ev.kind === "payload_received") {
    const r = gtpBob.accept(ev.plaintext!, bobMls.epoch, ev.codec ?? PayloadCodec.Cbor);
    console.log(r.text);
  }
}
```

| Value | Name | Description |
|-------|------|-------------|
| `0`   | `PayloadCodec.Cbor`         | Default; `pf` field omitted from wire |
| `1`   | `PayloadCodec.Protobuf`     | Protobuf via `gbp-proto` |
| `2`   | `PayloadCodec.FlatBuffers`  | FlatBuffers via `gbp-flat`; lowest latency |

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
* `SFrameSession` + `SFrameEncryptor` — SFrame (draft-ietf-sframe-enc) E2EE
  for GAP audio frames; per-sender AES-GCM keys derived from MLS exporter,
  1024-entry sliding-window replay protection.
* `encodeGbpFrame` — low-level helper to construct a raw CBOR GBP frame.
* `lookupError` — return the CBOR `ErrorObject` for a known error code.

### Coordinator events

`NodeEvent` surfaces three new event kinds for coordinator election:

| `kind` | Extra fields | Meaning |
|--------|-------------|---------|
| `coordinator_election_needed` | — | The local node should initiate GSP `COORDINATOR_CLAIM` |
| `became_coordinator` | — | This node won the election |
| `coordinator_claim` | `claimant` | A peer sent `COORDINATOR_CLAIM` with this member id |

## Install

```sh
npm install @voluntas-progressus/gbp-stack@1.8.0
```

## Quick start

```ts
import { MlsContext, GroupNode, GtpClient, StreamType } from "@voluntas-progressus/gbp-stack";

const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");

const bobKp   = bobMls.exportKeyPackage();
const welcome = aliceMls.invite(bobKp);   // alice auto-finalizes; epoch advances to 1n
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
    const r = gtpBob.accept(ev.plaintext!, bobMls.epoch);
    console.log(r.text);   // → "hello"
    // r.status is "new" (first message from this sender)
    // subsequent messages → "new"; duplicates → "duplicate"
  }
}
```

## GSP signals with per-signal arguments

Signals that target a specific member or resource require CBOR-encoded arguments.
Use `GspClient.sendWithArgs` for these signals:

```ts
import { GspClient, SignalType } from "@voluntas-progressus/gbp-stack";

// Minimal CBOR helpers
function cborUint(n: number): number[] {
  if (n <= 23)     return [n];
  if (n <= 0xFF)   return [0x18, n];
  if (n <= 0xFFFF) return [0x19, (n >> 8) & 0xFF, n & 0xFF];
  return [0x1A, (n>>24)&0xFF, (n>>16)&0xFF, (n>>8)&0xFF, n&0xFF];
}
function cborMap1(k: number, v: number): Buffer {
  return Buffer.from([0xA1, ...cborUint(k), ...cborUint(v)]);
}

// Signal-specific args schemas:
//   MUTE / Unmute  → {0: target_member_id}
//   RoleChange     → {0: target_member_id, 1: new_role_id}
//   StreamStart / StreamStop → {0: stream_type}
//   CodecUpdate    → {0: codec_id}
//   Join / Leave   → no args; use GspClient.send

const gsp = GspClient.create();

// Mute member 3
const frame = gsp.sendWithArgs(
  aliceNode, aliceMls,
  0,                        // target: 0 = broadcast
  SignalType.Mute,
  0,                        // roleClaim
  1,                        // requestId
  cborMap1(0, 3),           // args: {0: target_member_id=3}
);
```

## MLS multi-member group pattern

When inviting a member to an **existing** group (not the first invite), use
`inviteFull` so that existing members can process the commit:

```ts
// Alice adds Carol to an alice+bob group
const { commit, welcome } = aliceMls.inviteFull(carolMls.exportKeyPackage());
aliceMls.finalizeCommit();          // alice's epoch advances
bobMls.processMessage(commit);      // bob stages the commit
bobMls.finalizeCommit();            // bob's epoch advances to match alice
carolMls.acceptWelcome(welcome);    // carol joins
console.assert(aliceMls.epoch === bobMls.epoch && bobMls.epoch === carolMls.epoch);
```

## Persisting MLS state

Serialise a context so it survives a restart, then restore it later — the
restored context is at the same epoch and can send / receive again. The blob
holds **private key material**, so store it encrypted at rest.

```ts
const blob = mls.exportState();             // persist (encrypted) to disk
// ... later / after restart ...
const restored = MlsContext.restoreState(blob, "alice");
// restored.epoch === mls.epoch
```

## License

Licensed under [Apache License, Version 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE).
