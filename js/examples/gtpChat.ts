/**
 * Two-party GTP (text) chat — minimal in-memory end-to-end example.
 *
 * Demonstrates:
 *   - MLS two-party handshake
 *   - GBP node bootstrap
 *   - Sending text messages with CBOR and FlatBuffers codecs
 *   - Idempotency: duplicate (sender, messageId) returns status "duplicate"
 *
 * Run from repo root:
 *   npx ts-node js/examples/gtpChat.ts
 */

import { MlsContext } from "../src/mls";
import { GroupNode, StreamType } from "../src/node";
import { GtpClient, PayloadCodec } from "../src/gtp";

// --- MLS handshake ----------------------------------------------------------
const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");
bobMls.acceptWelcome(aliceMls.invite(bobMls.exportKeyPackage()));
console.log(`MLS epoch: alice=${aliceMls.epoch}  bob=${bobMls.epoch}`);

// --- GBP nodes --------------------------------------------------------------
const gid   = aliceMls.groupId;
const alice = GroupNode.create(1, gid);
const bob   = GroupNode.create(2, gid);
alice.bootstrapAsCreator(aliceMls.epoch);
bob.bootstrapAsJoiner(bobMls.epoch);

// --- GTP clients ------------------------------------------------------------
const gtpAlice = GtpClient.create();
const gtpBob   = GtpClient.create();

// Send "hello" with default CBOR codec.
const frame = gtpAlice.send(alice, aliceMls, 2, 1n, "hello");
for (const ev of bob.onWire(bobMls, frame.wire)) {
    if (ev.kind === "payload_received" && ev.streamType === StreamType.Text) {
        const r = gtpBob.accept(ev.plaintext!, bobMls.epoch, ev.codec ?? PayloadCodec.Cbor);
        console.log(`new (cbor):   text=${r.text}  status=${r.status}`);
    }
}

// Send with FlatBuffers codec.
const frame2 = gtpAlice.send(alice, aliceMls, 2, 2n, "hello flatbuffers", PayloadCodec.FlatBuffers);
for (const ev of bob.onWire(bobMls, frame2.wire)) {
    if (ev.kind === "payload_received" && ev.streamType === StreamType.Text) {
        const r = gtpBob.accept(ev.plaintext!, bobMls.epoch, ev.codec ?? PayloadCodec.Cbor);
        console.log(`new (fbs):    text=${r.text}  codec=${ev.codec}`);
    }
}

// Replay: same messageId=1n must come back as "duplicate".
const dup = gtpAlice.send(alice, aliceMls, 2, 1n, "hello");
for (const ev of bob.onWire(bobMls, dup.wire)) {
    if (ev.kind === "payload_received" && ev.streamType === StreamType.Text) {
        const r = gtpBob.accept(ev.plaintext!, bobMls.epoch);
        console.log(`replay:       status=${r.status}`);  // → duplicate
    }
}

// Cleanup.
[aliceMls, bobMls, alice, bob, gtpAlice, gtpBob].forEach(x => x.close());
