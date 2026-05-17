/**
 * GSP (signaling) examples: JOIN, MUTE with CBOR args, ROLE_CHANGE.
 *
 * Demonstrates:
 *   - Signals without args via GspClient.send
 *   - Signals with per-signal CBOR args via GspClient.sendWithArgs
 *
 * Run from repo root:
 *   npx ts-node js/examples/gspSignals.ts
 */

import { MlsContext } from "../src/mls";
import { GroupNode, StreamType } from "../src/node";
import { GspClient, SignalType } from "../src/gsp";
import { PayloadCodec } from "../src/native";

// Minimal CBOR helpers.
function cborUint(n: number): number[] {
    if (n <= 23)      return [n];
    if (n <= 0xFF)    return [0x18, n];
    if (n <= 0xFFFF)  return [0x19, (n >> 8) & 0xFF, n & 0xFF];
    return [0x1A, (n>>24)&0xFF, (n>>16)&0xFF, (n>>8)&0xFF, n&0xFF];
}
function cborMap1(k: number, v: number): Buffer {
    return Buffer.from([0xA1, ...cborUint(k), ...cborUint(v)]);
}
function cborMap2(k0: number, v0: number, k1: number, v1: number): Buffer {
    return Buffer.from([0xA2, ...cborUint(k0), ...cborUint(v0), ...cborUint(k1), ...cborUint(v1)]);
}

const aliceMls = MlsContext.create("alice");
const bobMls   = MlsContext.create("bob");
bobMls.acceptWelcome(aliceMls.invite(bobMls.exportKeyPackage()));

const gid   = aliceMls.groupId;
const alice = GroupNode.create(1, gid);
const bob   = GroupNode.create(2, gid);
alice.bootstrapAsCreator(aliceMls.epoch);
bob.bootstrapAsJoiner(bobMls.epoch);

const gspAlice = GspClient.create();
const gspBob   = GspClient.create();

function recv(wire: Buffer, label: string): void {
    for (const ev of bob.onWire(bobMls, wire)) {
        if (ev.kind === "payload_received" && ev.streamType === StreamType.Signal) {
            const r = gspBob.accept(ev.plaintext!, bobMls.epoch, ev.codec ?? PayloadCodec.Cbor);
            console.log(`${label}: signal=${r.signal}  sender=${r.sender}  requestId=${r.requestId}`);
        }
    }
}

// 1. JOIN — no args.
recv(gspAlice.send(alice, aliceMls, 0, SignalType.Join, 0, 1).wire, "JOIN");

// 2. MUTE member 2 — args: {0: target_member_id=2}.
recv(gspAlice.sendWithArgs(alice, aliceMls, 0, SignalType.Mute, 0, 2, cborMap1(0, 2)).wire, "MUTE");

// 3. ROLE_CHANGE member 2 → role 1 — args: {0: target=2, 1: new_role=1}.
recv(gspAlice.sendWithArgs(alice, aliceMls, 0, SignalType.RoleChange, 1, 3, cborMap2(0, 2, 1, 1)).wire, "ROLE_CHANGE");

[aliceMls, bobMls, alice, bob, gspAlice, gspBob].forEach(x => x.close());
