# @voluntas-progressus/gbp-stack-wasm — Browser/WASM bindings for the Group Protocol Stack

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE)
[![NPM](https://img.shields.io/npm/v/%40voluntas-progressus%2Fgbp-stack-wasm?logo=npm&label=npm)](https://www.npmjs.com/package/@voluntas-progressus/gbp-stack-wasm)

Browser-native WebAssembly bindings for the [Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack):
a layered, end-to-end encrypted group-messaging protocol family built on top
of [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420).

All cryptography runs **entirely in the browser** — no native binaries, no
server-side key material. The WASM module is compiled from the same Rust
source as the other language bindings and exposes an ergonomic JS/TS API via
[wasm-bindgen](https://github.com/rustwasm/wasm-bindgen).

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

## Bundler compatibility

The package is built with `wasm-pack --target bundler`. It works out of the box with:

| Bundler | Notes |
|---------|-------|
| **Vite** | Add `@vitejs/plugin-react` or use `vite-plugin-wasm` |
| **webpack 5** | Requires `experiments: { asyncWebAssembly: true }` |
| **Rollup** | Use `@rollup/plugin-wasm` |
| **Next.js** | Wrap imports in `dynamic(() => import('...'), { ssr: false })` |

## Install

```sh
npm install @voluntas-progressus/gbp-stack-wasm@1.5.3
```

## Quick start

```ts
import init, { MlsContext, GroupNode, GtpClient } from "@voluntas-progressus/gbp-stack-wasm";

// Load and compile the WASM module once at startup.
await init();

// ── MLS identities ─────────────────────────────────────────────────
const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");

// ── GBP nodes ──────────────────────────────────────────────────────
const groupId = crypto.getRandomValues(new Uint8Array(16));

const alice = GroupNode.create(1, groupId);
const bob   = GroupNode.create(2, groupId);

alice.bootstrapAsCreator(aliceMls.epoch);
bob.bootstrapAsJoiner(bobMls.epoch, 0);

// ── GTP clients ────────────────────────────────────────────────────
const gtpAlice = GtpClient.create();
const gtpBob   = GtpClient.create();

// Send a text message (broadcast: target = 0)
const frame = gtpAlice.send(alice, aliceMls, 0, 1n, "hello from browser!");
// frame.wire: Uint8Array — hand to your WebSocket / WebRTC transport

// On the receiver side
for (const ev of bob.onWire(bobMls, frame.wire)) {
  if (ev.kind === "payload_received" && ev.streamType === 2 /* Text */) {
    const r = gtpBob.accept(ev.plaintext, bobMls.epoch);
    if (r) console.log(r.text);   // → "hello from browser!"
  }
}
```

## API reference

### `MlsContext`

| Member | Description |
|--------|-------------|
| `MlsContext.create(userId: string)` | Creates a new member identity and an empty MLS group |
| `.epoch: bigint` | Current MLS group epoch |

### `GroupNode`

| Member | Description |
|--------|-------------|
| `GroupNode.create(leafIndex: number, groupId: Uint8Array)` | Creates a GBP node for the given member id and 16-byte group id |
| `.bootstrapAsCreator(epoch: bigint)` | Drives the node to `ACTIVE` as the group creator |
| `.bootstrapAsJoiner(epoch: bigint, expectedFirstTid: number)` | Drives the node to `ACTIVE` as a joiner |
| `.onWire(mls: MlsContext, wireBytes: Uint8Array): NodeEvent[]` | Delivers a wire frame; returns decoded events |
| `.checkTimeouts(): NodeEvent[]` | Polls timeout events — call ~every 500 ms from the app loop |
| `.lastTransitionId: number` | `transition_id` of the last applied epoch transition |
| `.currentEpoch: bigint` | Current epoch as seen by the GBP layer |
| `.memberId: number` | This node's member id (leaf index) |

### `GtpClient`

| Member | Description |
|--------|-------------|
| `GtpClient.create()` | Creates an empty GTP client |
| `.send(node, mls, target, messageId, text)` | Encrypts and frames a text message; returns `{ wire: Uint8Array, to: number }` |
| `.accept(plaintext, epoch)` | Decodes a GTP payload; returns `{ text: string, messageId: bigint, senderId: number }` or `null` |
| `.reset()` | Clears the idempotency set |

### `NodeEvent` shape

Every event object from `onWire` / `checkTimeouts` carries `kind: string`.

| `kind` | Extra fields |
|--------|-------------|
| `"payload_received"` | `streamType: number`, `plaintext: Uint8Array`, `sequenceNo: number` |
| `"state_changed"` | `from: string`, `to: string` |
| `"epoch_advanced"` | `epoch: bigint`, `transitionId: number` |
| `"error"` | `code: number`, `reason: string`, `fatal: boolean`, `retryable: boolean` |
| `"control"` | `from: number`, `opcode: number`, `transitionId: number` |

### Stream types

| Value | Name | Sub-protocol |
|-------|------|--------------|
| `0` | Control | GBP control plane |
| `1` | Audio | GAP (Group Audio Protocol) |
| `2` | Text | GTP (Group Text Protocol) |
| `3` | Signal | GSP (Group Signaling Protocol) |

## Multi-member group pattern

```ts
await init();

const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");

// Bob exports a key package so Alice can invite him.
// (In a real app Bob would send this over the signaling channel.)
// Currently MlsContext exposes group operations at the Rust level.
// Use the Node.js package for full multi-member key exchange in tests.
```

> **Note:** Full multi-member MLS key exchange (invite, commit, Welcome)
> is available in the WASM API as low-level calls.
> The high-level `invite` / `acceptWelcome` wrappers will be added in a
> future release. Single-member groups (creator only) and pre-established
> epoch groups are fully supported today.

## Vite configuration example

```ts
// vite.config.ts
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
});
```

Then in your entry point:

```ts
import init from "@voluntas-progressus/gbp-stack-wasm";
await init();
```

## License

Licensed under [Apache License, Version 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE).
