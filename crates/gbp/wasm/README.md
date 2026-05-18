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
npm install @voluntas-progressus/gbp-stack-wasm
```

## Quick start

```ts
import init, { MlsContext, GroupNode, GtpClient } from "@voluntas-progressus/gbp-stack-wasm";

// Load and compile the WASM module once at startup.
await init();

// ── MLS key exchange ────────────────────────────────────────────────
const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");

// Bob exports a key package so Alice can invite him.
// (In a real app Bob sends this over the signaling channel.)
const welcome = aliceMls.invite(bobMls.keyPackage);
bobMls.acceptWelcome(welcome);
// aliceMls.epoch === bobMls.epoch === 1n

// ── GBP group nodes ─────────────────────────────────────────────────
const groupId   = aliceMls.groupId;            // shared 16-byte id
const aliceNode = GroupNode.create(1, groupId);
const bobNode   = GroupNode.create(2, groupId);

aliceNode.bootstrapAsCreator(aliceMls.epoch);
bobNode.bootstrapAsJoiner(bobMls.epoch, 0);

// ── GTP clients ─────────────────────────────────────────────────────
const gtpAlice = GtpClient.create();
const gtpBob   = GtpClient.create();

// Alice sends a message to Bob (target = 2, messageId = 1n)
const frame = gtpAlice.send(aliceNode, aliceMls, 2, 1n, "hello bob!");
// frame.wire: Uint8Array — hand to your WebSocket / WebRTC transport

// Bob receives it
for (const ev of bobNode.onWire(bobMls, frame.wire)) {
  if (ev.kind === "payload_received" && ev.streamType === 2 /* Text */) {
    const r = gtpBob.accept(ev.plaintext, bobMls.epoch);
    if (r) console.log(r.text);   // → "hello bob!"
  }
}
```

## API reference

### `MlsContext`

| Member | Description |
|--------|-------------|
| `MlsContext.create(userId: string)` | Creates a new member identity and an empty MLS group |
| `.keyPackage: Uint8Array` | TLS-serialised key package (pass to the inviter's `invite`) |
| `.epoch: bigint` | Current MLS group epoch |
| `.groupId: Uint8Array` | 16-byte group identifier |
| `.invite(keyPackageBytes: Uint8Array): Uint8Array` | Invites another member; returns Welcome bytes |
| `.acceptWelcome(welcomeBytes: Uint8Array): void` | Joins a group from a Welcome produced by `invite` |

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
| `.accept(plaintext, epoch)` | Decodes a GTP payload; returns `{ text, messageId, senderId, status }` or `null` |
| `.reset()` | Clears the idempotency set |

`accept` return value:

| Field | Type | Description |
|-------|------|-------------|
| `text` | `string` | Decoded message text |
| `messageId` | `bigint` | Sender-assigned message identifier |
| `senderId` | `number` | Leaf index of the sender |
| `status` | `"new" \| "duplicate"` | Whether this message id was seen before |

### `NodeEvent` shape

Every event object from `onWire` / `checkTimeouts` carries `kind: string`.

| `kind` | Extra fields |
|--------|-------------|
| `"payload_received"` | `streamType: number`, `plaintext: Uint8Array`, `sequenceNo: number`, `codec: number` |
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

## Examples

| File | Description |
|------|-------------|
| [`examples/gtpChat.ts`](examples/gtpChat.ts) | Two-member encrypted text chat (TypeScript, Node.js ≥ 18) |
| [`examples/gtpChat.html`](examples/gtpChat.html) | Standalone browser demo (no bundler needed) |

## Testing

Tests are written with [`wasm-bindgen-test`](https://rustwasm.github.io/wasm-bindgen/wasm-bindgen-test/index.html) and run inside Node.js:

```sh
# Install wasm-pack if needed
cargo install wasm-pack

# Run all WASM tests
wasm-pack test --node crates/gbp/wasm
```

The test suite covers:

- `MlsContext`: create, epoch, keyPackage, groupId, invite / acceptWelcome
- `GroupNode`: bootstrap (creator + joiner), onWire, checkTimeouts, getters
- `GtpClient`: send, accept (roundtrip, unicode, duplicate, sequential, reset)
- Two-member group: Alice→Bob, Bob→Alice, bidirectional, epoch sync

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
