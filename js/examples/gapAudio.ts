/**
 * Two-party GAP (audio) frame exchange — minimal in-memory end-to-end example.
 *
 * Demonstrates:
 *   - Sending synthetic Opus frames with FlatBuffers codec (recommended for audio)
 *   - Sequential frames: rtp_sequence advances automatically inside GapClient
 *
 * Run from repo root:
 *   npx ts-node js/examples/gapAudio.ts
 */

import { MlsContext } from "../src/mls";
import { GroupNode, StreamType } from "../src/node";
import { GapClient } from "../src/gap";
import { PayloadCodec } from "../src/native";

const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");
bobMls.acceptWelcome(aliceMls.invite(bobMls.exportKeyPackage()));

const gid   = aliceMls.groupId;
const alice = GroupNode.create(1, gid);
const bob   = GroupNode.create(2, gid);
alice.bootstrapAsCreator(aliceMls.epoch);
bob.bootstrapAsJoiner(bobMls.epoch);

const gapAlice = GapClient.create();
const gapBob   = GapClient.create();

// Synthetic 20 ms Opus frame (zeroed; real usage: encode from PCM).
const opus = Buffer.alloc(40);

for (let i = 0; i < 3; i++) {
    const frame = gapAlice.send(
        alice, aliceMls,
        /*target*/          2,
        /*mediaSourceId*/   1,
        /*rtpTimestamp*/    BigInt(i * 960),
        opus,
        PayloadCodec.FlatBuffers,  // lowest decode latency
    );
    for (const ev of bob.onWire(bobMls, frame.wire)) {
        if (ev.kind === "payload_received" && ev.streamType === StreamType.Audio) {
            const r = gapBob.accept(ev.plaintext!, bobMls.epoch, ev.codec ?? PayloadCodec.Cbor);
            console.log(`frame ${i + 1}: status=${r.status}  seq=${r.seq}  codec=${ev.codec}`);
        }
    }
}

[aliceMls, bobMls, alice, bob, gapAlice, gapBob].forEach(x => x.close());
