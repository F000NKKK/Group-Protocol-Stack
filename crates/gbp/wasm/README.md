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
│   GtpClient · GapClient · GspClient   (text · audio · signalling)   │
│   SFrameSession / SFrameEncryptor      (media E2EE over GAP)         │
├─────────────────────────────────────────────────────────────────────┤
│   GroupNode (GBP — IP-like base)                                    │
├─────────────────────────────────────────────────────────────────────┤
│   MlsContext (RFC 9420)                                             │
└─────────────────────────────────────────────────────────────────────┘
```

The browser/WASM binding now exposes the **same** gap (audio) / gsp
(signalling) / gtp (text) / sframe (media E2EE) surface as the C#, Python and
Node bindings — a browser client can place a fully end-to-end-encrypted call.

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
| `.invite(keyPackageBytes: Uint8Array): Uint8Array` | Invites another member (merges immediately); returns Welcome bytes |
| `.acceptWelcome(welcomeBytes: Uint8Array): void` | Joins a group from a Welcome produced by `invite` |
| `.inviteFull(keyPackageBytes: Uint8Array): { commit, welcome }` | Two-phase invite — stages a pending commit; returns both Commit and Welcome bytes. Broadcast the Commit, then `finalizeCommit` (or `clearPendingCommit` to roll back) |
| `.removeMember(leafIndex: number): Uint8Array` | Stages a Remove commit for `leafIndex`; returns the Commit to broadcast |
| `.processMessage(msgBytes: Uint8Array): string` | Applies an inbound MLS message; returns `"commit"`, `"application"`, `"proposal"` or `"external"` |
| `.finalizeCommit(): void` | Merges a pending commit from `inviteFull`/`removeMember` (advances the epoch) |
| `.clearPendingCommit(): void` | Discards a pending commit without applying it (use on `ABORT_TRANSITION`) |

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
| `.sendControl(mls, target, opcode, transitionId, requestId, args): { wire, to }` | Sends a control-plane message (`ControlOpcode`) — transition coordination, capabilities, ACK/NACK |
| `.applyTransition(tid: number)` | Applies an epoch transition locally |
| `.drainEvents(): NodeEvent[]` | Drains queued events without consuming wire bytes |

### `GtpClient`

| Member | Description |
|--------|-------------|
| `GtpClient.create()` | Creates an empty GTP client |
| `.send(node, mls, target, messageId, text, codec?)` | Encrypts and frames a text message; returns `{ wire: Uint8Array, to: number }`. `codec` is an optional `PayloadCodec` (defaults to CBOR) |
| `.accept(plaintext, epoch, codec?)` | Decodes a GTP payload; returns `{ text, messageId, senderId, status }` or `null` |
| `.reset()` | Clears the idempotency set |

`accept` return value:

| Field | Type | Description |
|-------|------|-------------|
| `text` | `string` | Decoded message text |
| `messageId` | `bigint` | Sender-assigned message identifier |
| `senderId` | `number` | Leaf index of the sender |
| `status` | `"new" \| "duplicate"` | Whether this message id was seen before |

### `GapClient` (audio)

Group Audio Protocol — Opus frame delivery with per-source replay protection.
The Opus payload is opaque bytes (encode/decode audio with WebCodecs or
libopus.wasm); combine with `SFrameSession` for media E2EE.

| Member | Description |
|--------|-------------|
| `GapClient.create()` | Creates an empty GAP client |
| `.send(node, mls, target, mediaSourceId, rtpTimestamp, opus, codec?)` | Frames one Opus frame; returns `{ wire, to }` or `null`. Prefer `PayloadCodec.FlatBuffers` for audio |
| `.accept(plaintext, epoch, codec?)` | Decodes a GAP payload; returns `{ status, source, seq, rtpTimestamp, opus }` (`status` is `"new"`/`"late"`) or `null` |
| `.reset()` | Clears outbound counters + replay window (use after an epoch change) |

### `GspClient` (signalling)

Group Signaling Protocol — membership / role / stream / codec control signals
that drive call membership and mute/stream state.

| Member | Description |
|--------|-------------|
| `GspClient.create()` | Creates an empty GSP client |
| `.send(node, mls, target, signalType, roleClaim, requestId, codec?)` | Sends a bare signal (e.g. `SignalType.Join`/`Leave`); returns `{ wire, to }` |
| `.sendWithArgs(node, mls, target, signalType, roleClaim, requestId, args, codec?)` | Sends a signal carrying CBOR `args` (MUTE/UNMUTE/ROLE_CHANGE/STREAM_START/STREAM_STOP/CODEC_UPDATE) |
| `.accept(plaintext, epoch, codec?)` | Decodes a signal; returns `{ status, signal, signalCode, sender, roleClaim, requestId }` (or `{ status: "duplicate", requestId }`) |
| `.reset()` | Clears dedup state |

### `SFrameSession` / `SFrameEncryptor` (media E2EE)

SFrame ([draft-ietf-sframe-enc](https://datatracker.ietf.org/doc/draft-ietf-sframe-enc/))
end-to-end encryption for media frames, keyed off the MLS epoch via the group
exporter secret. Derive a fresh session after every epoch change
(invite/remove/commit). Wrap each Opus frame with `encrypt` **before** handing
it to `GapClient.send`, and `decrypt` after `GapClient.accept`.

| Member | Description |
|--------|-------------|
| `SFrameSession.create(mls, label, suite)` | Derives a session from the MLS exporter secret. `suite` is a `CipherSuite` value |
| `.createEncryptor(mls, leafIndex, label, suite): SFrameEncryptor` | Creates a sender-side encryptor for `leafIndex` |
| `.decrypt(payload, aad): { plaintext, senderLeaf }` | Decrypts an SFrame payload (throws on failure) |
| `SFrameEncryptor.encrypt(plaintext, aad): Uint8Array` | Encrypts one frame → `header ‖ ciphertext ‖ tag` |

### Enums

Exported for parity with the C#/Python/JS SDKs (each is a plain numeric value in JS):

| Enum | Values |
|------|--------|
| `PayloadCodec` | `Cbor = 0`, `Protobuf = 1`, `FlatBuffers = 2` |
| `SignalType` | `Join = 100`, `Leave = 101`, `RoleChange = 102`, `Mute = 200`, `Unmute = 201`, `StreamStart = 300`, `StreamStop = 301`, `CodecUpdate = 400` |
| `ControlOpcode` | `PrepareTransition = 1` … `CapabilitiesAdvertise = 8`, `Ack = 9`, `Nack = 10` |
| `CipherSuite` | `Aes128Gcm = 0`, `Aes256Gcm = 1` |

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
| [`examples/gapAudio.ts`](examples/gapAudio.ts) | Two-member encrypted voice — GAP audio wrapped with SFrame E2EE |
| [`examples/gspSignals.ts`](examples/gspSignals.ts) | Call signalling — JOIN, MUTE, STREAM_START via GSP |

## Testing

Tests are written with [`wasm-bindgen-test`](https://rustwasm.github.io/wasm-bindgen/wasm-bindgen-test/index.html) and run inside Node.js:

```sh
# Install wasm-pack if needed
cargo install wasm-pack

# Run all WASM tests
wasm-pack test --node crates/gbp/wasm
```

The test suite covers:

- `MlsContext`: create, epoch, keyPackage, groupId, invite / acceptWelcome, inviteFull / finalizeCommit / clearPendingCommit, removeMember, processMessage
- `GroupNode`: bootstrap (creator + joiner), onWire, checkTimeouts, getters, sendControl, drainEvents
- `GtpClient`: send, accept (roundtrip, unicode, duplicate, sequential, reset, codec selection)
- `GapClient`: audio send / accept roundtrip (CBOR + FlatBuffers)
- `GspClient`: JOIN / MUTE-with-args signal roundtrip, bad-signal rejection
- `SFrameSession` / `SFrameEncryptor`: encrypt → decrypt roundtrip, AAD mismatch, full GAP+SFrame audio pipeline
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
