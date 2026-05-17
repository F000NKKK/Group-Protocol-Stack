/**
 * Integration tests for the @voluntas-progressus/gbp-stack Node.js bindings.
 *
 * Covers:
 *  - MLS context lifecycle and invite flows
 *  - GBP GroupNode bootstrap, send, onWire
 *  - GTP text messaging (bidirectional, duplicates, unicode, reset)
 *  - GAP audio frames (multiple sources, reset)
 *  - GSP signals (all signal types, duplicates)
 *  - User lifecycle: 3-member group, leave, rejoin
 *  - Coordinator event kinds
 *  - MessageHistory + Watermark
 *  - JitterBuffer
 *  - RoleRegistry + CapabilitiesNegotiator
 *  - SFrame E2EE (AES-128, AES-256, extra AAD)
 *  - Utility functions: encodeGbpFrame, lookupError
 */

import { MlsContext } from "../src/mls";
import {
    GroupNode, StreamType, NodeState, ControlOpcode,
    encodeGbpFrame, lookupError, NodeEvent,
} from "../src/node";
import { GtpClient } from "../src/gtp";
import { GapClient } from "../src/gap";
import { GspClient, SignalType } from "../src/gsp";
import { SFrameSession, AES_128_GCM, AES_256_GCM } from "../src/sframe";
import { MessageHistory, Watermark } from "../src/history";
import { JitterBuffer } from "../src/jitter";
import { RoleRegistry, Permissions, RoleError } from "../src/roles";
import { CapabilitiesNegotiator } from "../src/capabilities";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function twoMemberGroup() {
    const aliceMls = MlsContext.create("alice");
    const bobMls   = MlsContext.create("bob");
    bobMls.acceptWelcome(aliceMls.invite(bobMls.exportKeyPackage()));
    const gid       = aliceMls.groupId;
    const aliceNode = GroupNode.create(1, gid);
    const bobNode   = GroupNode.create(2, gid);
    aliceNode.bootstrapAsCreator(aliceMls.epoch);
    bobNode.bootstrapAsJoiner(bobMls.epoch);
    return { aliceMls, aliceNode, bobMls, bobNode };
}

function textEvents(evs: NodeEvent[]) {
    return evs.filter(e => e.kind === "payload_received" && e.streamType === StreamType.Text);
}
function audioEvents(evs: NodeEvent[]) {
    return evs.filter(e => e.kind === "payload_received" && e.streamType === StreamType.Audio);
}
function signalEvents(evs: NodeEvent[]) {
    return evs.filter(e => e.kind === "payload_received" && e.streamType === StreamType.Signal);
}

// ---------------------------------------------------------------------------
// MLS context
// ---------------------------------------------------------------------------

describe("MlsContext", () => {
    test("create — epoch is 0n, identity preserved", () => {
        const ctx = MlsContext.create("alice");
        try {
            expect(ctx.epoch).toBe(0n);
            expect(ctx.identity).toBe("alice");
        } finally { ctx.close(); }
    });

    test("groupId is 16 bytes", () => {
        const ctx = MlsContext.create("alice");
        try { expect(ctx.groupId.length).toBe(16); }
        finally { ctx.close(); }
    });

    test("exportKeyPackage returns non-empty buffer", () => {
        const ctx = MlsContext.create("alice");
        try {
            const kp = ctx.exportKeyPackage();
            expect(kp.length).toBeGreaterThan(0);
        } finally { ctx.close(); }
    });

    test("invite + acceptWelcome syncs epoch", () => {
        const alice = MlsContext.create("alice");
        const bob   = MlsContext.create("bob");
        try {
            bob.acceptWelcome(alice.invite(bob.exportKeyPackage()));
            expect(bob.epoch).toBe(alice.epoch);
        } finally { alice.close(); bob.close(); }
    });

    test("inviteFull returns commit and welcome", () => {
        const alice = MlsContext.create("alice");
        const bob   = MlsContext.create("bob");
        try {
            const { commit, welcome } = alice.inviteFull(bob.exportKeyPackage());
            expect(commit.length).toBeGreaterThan(0);
            expect(welcome.length).toBeGreaterThan(0);
            const epochBefore = alice.epoch;
            alice.finalizeCommit();
            expect(alice.epoch).toBeGreaterThan(epochBefore);
            bob.acceptWelcome(welcome);
            expect(bob.epoch).toBe(alice.epoch);
        } finally { alice.close(); bob.close(); }
    });

    test("inviteFull — 3 members", () => {
        const alice = MlsContext.create("alice");
        const bob   = MlsContext.create("bob");
        const carol = MlsContext.create("carol");
        try {
            bob.acceptWelcome(alice.invite(bob.exportKeyPackage()));
            const { commit, welcome: welcomeCarol } = alice.inviteFull(carol.exportKeyPackage());
            alice.finalizeCommit();
            bob.processMessage(commit);
            bob.finalizeCommit();
            carol.acceptWelcome(welcomeCarol);
            expect(alice.epoch).toBe(bob.epoch);
            expect(alice.epoch).toBe(carol.epoch);
        } finally { alice.close(); bob.close(); carol.close(); }
    });

    test("clearPendingCommit does not throw", () => {
        const alice = MlsContext.create("alice");
        const bob   = MlsContext.create("bob");
        try {
            alice.inviteFull(bob.exportKeyPackage());
            alice.clearPendingCommit();
        } finally { alice.close(); bob.close(); }
    });

    test("processMessage returns 'commit'", () => {
        const alice = MlsContext.create("alice");
        const bob   = MlsContext.create("bob");
        const carol = MlsContext.create("carol");
        try {
            bob.acceptWelcome(alice.invite(bob.exportKeyPackage()));
            const { commit } = alice.inviteFull(carol.exportKeyPackage());
            alice.finalizeCommit();
            expect(bob.processMessage(commit)).toBe("commit");
        } finally { alice.close(); bob.close(); carol.close(); }
    });
});

// ---------------------------------------------------------------------------
// GroupNode
// ---------------------------------------------------------------------------

describe("GroupNode", () => {
    test("bootstrap creator → Active state", () => {
        const mls  = MlsContext.create("alice");
        const node = GroupNode.create(1, mls.groupId);
        try {
            expect(node.state).toBe(NodeState.Idle);
            node.bootstrapAsCreator(mls.epoch);
            expect(node.state).toBe(NodeState.Active);
        } finally { mls.close(); node.close(); }
    });

    test("groupId preserved", () => {
        const mls = MlsContext.create("alice");
        const gid = mls.groupId;
        const node = GroupNode.create(1, gid);
        try { expect(node.groupId).toEqual(gid); }
        finally { mls.close(); node.close(); }
    });

    test("invalid groupId throws", () => {
        expect(() => GroupNode.create(1, Buffer.alloc(15))).toThrow();
    });

    test("epoch matches MLS", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        try { expect(aliceNode.epoch).toBe(aliceMls.epoch); }
        finally {
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("sendControl produces frame", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        try {
            const frame = aliceNode.sendControl(aliceMls, 2, ControlOpcode.Ack, 0, 1);
            expect(frame.wire.length).toBeGreaterThan(0);
        } finally {
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });
});

// ---------------------------------------------------------------------------
// GTP — text messaging
// ---------------------------------------------------------------------------

describe("GtpClient", () => {
    test("basic send/receive", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gtpA = GtpClient.create();
        const gtpB = GtpClient.create();
        try {
            const frame = gtpA.send(aliceNode, aliceMls, 2, 1n, "hello");
            const evs   = textEvents(bobNode.onWire(bobMls, frame.wire));
            expect(evs).toHaveLength(1);
            const r = gtpB.accept(evs[0].plaintext!, bobMls.epoch);
            expect(r.status).toBe("new");
            expect(r.text).toBe("hello");
        } finally {
            gtpA.close(); gtpB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("bidirectional messaging", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gtpA = GtpClient.create();
        const gtpB = GtpClient.create();
        try {
            const frame = gtpB.send(bobNode, bobMls, 1, 10n, "hi alice");
            const evs   = textEvents(aliceNode.onWire(aliceMls, frame.wire));
            const r = gtpA.accept(evs[0].plaintext!, aliceMls.epoch);
            expect(r.text).toBe("hi alice");
            expect(r.sender).toBe(2);
        } finally {
            gtpA.close(); gtpB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("multiple messages", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gtpA = GtpClient.create();
        const gtpB = GtpClient.create();
        try {
            const msgs = ["first", "second", "third"];
            for (let i = 0; i < msgs.length; i++) {
                const frame = gtpA.send(aliceNode, aliceMls, 2, BigInt(i + 1), msgs[i]);
                const evs   = textEvents(bobNode.onWire(bobMls, frame.wire));
                expect(gtpB.accept(evs[0].plaintext!, bobMls.epoch).text).toBe(msgs[i]);
            }
        } finally {
            gtpA.close(); gtpB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("duplicate rejection", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gtpA = GtpClient.create();
        const gtpB = GtpClient.create();
        try {
            const frame = gtpA.send(aliceNode, aliceMls, 2, 99n, "once");
            const pt    = textEvents(bobNode.onWire(bobMls, frame.wire))[0].plaintext!;
            expect(gtpB.accept(pt, bobMls.epoch).status).toBe("new");
            expect(gtpB.accept(pt, bobMls.epoch).status).toBe("duplicate");
        } finally {
            gtpA.close(); gtpB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("unicode text", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gtpA = GtpClient.create();
        const gtpB = GtpClient.create();
        try {
            const text = "Привет мир 🌍";
            const frame = gtpA.send(aliceNode, aliceMls, 2, 5n, text);
            const evs   = textEvents(bobNode.onWire(bobMls, frame.wire));
            expect(gtpB.accept(evs[0].plaintext!, bobMls.epoch).text).toBe(text);
        } finally {
            gtpA.close(); gtpB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("reset clears dedup", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gtpA = GtpClient.create();
        const gtpB = GtpClient.create();
        try {
            const frame = gtpA.send(aliceNode, aliceMls, 2, 7n, "test");
            const pt    = textEvents(bobNode.onWire(bobMls, frame.wire))[0].plaintext!;
            expect(gtpB.accept(pt, bobMls.epoch).status).toBe("new");
            gtpB.reset();
            expect(gtpB.accept(pt, bobMls.epoch).status).toBe("new");
        } finally {
            gtpA.close(); gtpB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });
});

// ---------------------------------------------------------------------------
// GAP — audio
// ---------------------------------------------------------------------------

describe("GapClient", () => {
    test("basic audio frame", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gapA = GapClient.create();
        const gapB = GapClient.create();
        try {
            const frame = gapA.send(aliceNode, aliceMls, 2, 1, 0n, Buffer.alloc(60));
            const evs   = audioEvents(bobNode.onWire(bobMls, frame.wire));
            expect(evs).toHaveLength(1);
            const r = gapB.accept(evs[0].plaintext!, bobMls.epoch);
            expect(r.status).toBe("new");
            expect(r.source).toBe(1);
        } finally {
            gapA.close(); gapB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("multiple frames in order", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gapA = GapClient.create();
        const gapB = GapClient.create();
        try {
            for (let i = 0; i < 5; i++) {
                const frame = gapA.send(aliceNode, aliceMls, 2, 1, BigInt(i * 960), Buffer.from([i]));
                const evs   = audioEvents(bobNode.onWire(bobMls, frame.wire));
                const r = gapB.accept(evs[0].plaintext!, bobMls.epoch);
                expect(r.status).toBe("new");
                expect(r.seq).toBe(i + 1);
            }
        } finally {
            gapA.close(); gapB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("multiple sources", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gapA = GapClient.create();
        const gapB = GapClient.create();
        try {
            for (const src of [10, 20]) {
                const frame = gapA.send(aliceNode, aliceMls, 2, src, 0n, Buffer.from([src]));
                const evs   = audioEvents(bobNode.onWire(bobMls, frame.wire));
                const r = gapB.accept(evs[0].plaintext!, bobMls.epoch);
                expect(r.status).toBe("new");
                expect(r.source).toBe(src);
            }
        } finally {
            gapA.close(); gapB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("reset does not throw", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gapA = GapClient.create();
        const gapB = GapClient.create();
        try {
            const frame = gapA.send(aliceNode, aliceMls, 2, 1, 0n, Buffer.alloc(5));
            const evs   = audioEvents(bobNode.onWire(bobMls, frame.wire));
            gapB.accept(evs[0].plaintext!, bobMls.epoch);
            gapB.reset();
        } finally {
            gapA.close(); gapB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });
});

// ---------------------------------------------------------------------------
// GSP — signals
// ---------------------------------------------------------------------------

function cborUint(n: number): number[] {
    if (n <= 23)     return [n];
    if (n <= 0xFF)   return [0x18, n];
    if (n <= 0xFFFF) return [0x19, (n >> 8) & 0xFF, n & 0xFF];
    return [0x1A, (n >> 24) & 0xFF, (n >> 16) & 0xFF, (n >> 8) & 0xFF, n & 0xFF];
}
function cborMap1(k: number, v: number): Buffer {
    return Buffer.from([0xA1, ...cborUint(k), ...cborUint(v)]);
}
function cborMap2(k0: number, v0: number, k1: number, v1: number): Buffer {
    return Buffer.from([0xA2, ...cborUint(k0), ...cborUint(v0), ...cborUint(k1), ...cborUint(v1)]);
}

function sendSignal(
    signal: SignalType, requestId: number,
    aliceMls: MlsContext, aliceNode: GroupNode,
    bobMls: MlsContext, bobNode: GroupNode,
    roleClaim = 0, args: Buffer | null = null,
) {
    const gspA = GspClient.create();
    const gspB = GspClient.create();
    try {
        const frame = args
            ? gspA.sendWithArgs(aliceNode, aliceMls, 2, signal, roleClaim, requestId, args)
            : gspA.send(aliceNode, aliceMls, 2, signal, roleClaim, requestId);
        const evs   = signalEvents(bobNode.onWire(bobMls, frame.wire));
        expect(evs).toHaveLength(1);
        return gspB.accept(evs[0].plaintext!, bobMls.epoch);
    } finally { gspA.close(); gspB.close(); }
}

describe("GspClient", () => {
    test.each([
        ["join",         SignalType.Join,        1, null],
        ["leave",        SignalType.Leave,       2, null],
        ["mute",         SignalType.Mute,        3, cborMap1(0, 2)],
        ["unmute",       SignalType.Unmute,      4, cborMap1(0, 2)],
        ["role_change",  SignalType.RoleChange,  5, cborMap2(0, 2, 1, 0)],
        ["stream_start", SignalType.StreamStart, 6, cborMap1(0, 1)],
        ["stream_stop",  SignalType.StreamStop,  7, cborMap1(0, 1)],
        ["codec_update", SignalType.CodecUpdate, 8, cborMap1(0, 1)],
    ])("%s signal", (_, signal, reqId, args) => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        try {
            const r = sendSignal(signal as SignalType, reqId as number, aliceMls, aliceNode, bobMls, bobNode, 0, args as Buffer | null);
            expect(r.status).toBe("new");
            expect(r.signalCode).toBe(signal);
        } finally {
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("duplicate signal rejected", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        const gspA = GspClient.create();
        const gspB = GspClient.create();
        try {
            const frame = gspA.sendWithArgs(aliceNode, aliceMls, 2, SignalType.Mute, 0, 50, cborMap1(0, 2));
            const pt    = signalEvents(bobNode.onWire(bobMls, frame.wire))[0].plaintext!;
            expect(gspB.accept(pt, bobMls.epoch).status).toBe("new");
            expect(gspB.accept(pt, bobMls.epoch).status).toBe("duplicate");
        } finally {
            gspA.close(); gspB.close();
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });

    test("sender id", () => {
        const { aliceMls, aliceNode, bobMls, bobNode } = twoMemberGroup();
        try {
            const r = sendSignal(SignalType.Join, 9, aliceMls, aliceNode, bobMls, bobNode);
            expect(r.sender).toBe(1);
        } finally {
            aliceMls.close(); aliceNode.close();
            bobMls.close(); bobNode.close();
        }
    });
});

// ---------------------------------------------------------------------------
// User lifecycle
// ---------------------------------------------------------------------------

describe("UserLifecycle", () => {
    test("three member group broadcast", () => {
        const aliceMls = MlsContext.create("alice");
        const bobMls   = MlsContext.create("bob");
        const carolMls = MlsContext.create("carol");
        try {
            bobMls.acceptWelcome(aliceMls.invite(bobMls.exportKeyPackage()));
            const { commit, welcome: wc } = aliceMls.inviteFull(carolMls.exportKeyPackage());
            aliceMls.finalizeCommit();
            bobMls.processMessage(commit);
            bobMls.finalizeCommit();
            carolMls.acceptWelcome(wc);

            expect(aliceMls.epoch).toBe(bobMls.epoch);
            expect(aliceMls.epoch).toBe(carolMls.epoch);

            const gid       = aliceMls.groupId;
            const aliceNode = GroupNode.create(1, gid);
            const bobNode   = GroupNode.create(2, gid);
            const carolNode = GroupNode.create(3, gid);
            aliceNode.bootstrapAsCreator(aliceMls.epoch);
            bobNode.bootstrapAsJoiner(bobMls.epoch);
            carolNode.bootstrapAsJoiner(carolMls.epoch);

            const gtpA = GtpClient.create();
            const gtpB = GtpClient.create();
            const gtpC = GtpClient.create();

            const frame = gtpA.send(aliceNode, aliceMls, 0, 1n, "hi all");
            for (const [node, mls, client] of [
                [bobNode,   bobMls,   gtpB] as const,
                [carolNode, carolMls, gtpC] as const,
            ]) {
                const evs = textEvents(node.onWire(mls, frame.wire));
                expect(evs).toHaveLength(1);
                expect(client.accept(evs[0].plaintext!, mls.epoch).text).toBe("hi all");
            }

            for (const x of [aliceNode, bobNode, carolNode, gtpA, gtpB, gtpC]) x.close();
        } finally {
            aliceMls.close(); bobMls.close(); carolMls.close();
        }
    });

    test("member rejoin", () => {
        const aliceMls = MlsContext.create("alice");
        const bobMls   = MlsContext.create("bob");
        try {
            bobMls.acceptWelcome(aliceMls.invite(bobMls.exportKeyPackage()));
            const gid       = aliceMls.groupId;
            const aliceNode = GroupNode.create(1, gid);
            aliceNode.bootstrapAsCreator(aliceMls.epoch);
            { const bobNode = GroupNode.create(2, gid); bobNode.bootstrapAsJoiner(bobMls.epoch); bobNode.close(); }

            aliceMls.removeMember(1);
            aliceMls.finalizeCommit();
            aliceNode.setEpochForTesting(aliceMls.epoch);

            const bobMls2  = MlsContext.create("bob2");
            bobMls2.acceptWelcome(aliceMls.invite(bobMls2.exportKeyPackage()));
            aliceNode.setEpochForTesting(aliceMls.epoch);
            const bobNode2 = GroupNode.create(3, gid);
            bobNode2.bootstrapAsJoiner(bobMls2.epoch);

            const gtpA  = GtpClient.create();
            const gtpB2 = GtpClient.create();

            const frame = gtpA.send(aliceNode, aliceMls, 3, 1n, "welcome back");
            const evs   = textEvents(bobNode2.onWire(bobMls2, frame.wire));
            expect(gtpB2.accept(evs[0].plaintext!, bobMls2.epoch).text).toBe("welcome back");

            for (const x of [aliceNode, bobNode2, gtpA, gtpB2]) x.close();
            bobMls2.close();
        } finally {
            aliceMls.close(); bobMls.close();
        }
    });
});

// ---------------------------------------------------------------------------
// Coordinator events
// ---------------------------------------------------------------------------

describe("CoordinatorEvents", () => {
    test("NodeEvent interface has claimant field", () => {
        const ev: NodeEvent = { kind: "coordinator_claim", claimant: 42 };
        expect(ev.claimant).toBe(42);
    });

    test("election_needed event — no claimant", () => {
        const ev: NodeEvent = { kind: "coordinator_election_needed" };
        expect(ev.claimant).toBeUndefined();
    });

    test("became_coordinator event — no claimant", () => {
        const ev: NodeEvent = { kind: "became_coordinator" };
        expect(ev.claimant).toBeUndefined();
        expect(ev.kind).toBe("became_coordinator");
    });

    test("coordinator_claim with claimant=7", () => {
        const ev: NodeEvent = { kind: "coordinator_claim", claimant: 7 };
        expect(ev.kind).toBe("coordinator_claim");
        expect(ev.claimant).toBe(7);
    });
});

// ---------------------------------------------------------------------------
// MessageHistory + Watermark
// ---------------------------------------------------------------------------

describe("MessageHistory", () => {
    test("push and contains", () => {
        const h = new MessageHistory(10);
        expect(h.push({ senderId: 1, messageId: 100n, text: "hi" })).toBe(true);
        expect(h.contains(1, 100n)).toBe(true);
    });

    test("duplicate push returns false", () => {
        const h = new MessageHistory(10);
        h.push({ senderId: 1, messageId: 1n, text: "x" });
        expect(h.push({ senderId: 1, messageId: 1n, text: "x" })).toBe(false);
    });

    test("capacity limit", () => {
        const h = new MessageHistory(3);
        for (let i = 0; i < 5; i++) h.push({ senderId: 1, messageId: BigInt(i), text: String(i) });
        expect(h.length).toBe(3);
    });

    test("since watermark", () => {
        const h = new MessageHistory(100);
        for (let i = 0; i < 5; i++) h.push({ senderId: 1, messageId: BigInt(i), text: String(i) });
        const w = new Watermark();
        w.observe(1, 2n);
        const ids = [...h.since(w)].map(m => m.messageId);
        expect(ids).toEqual([3n, 4n]);
    });

    test("sinceForSender", () => {
        const h = new MessageHistory(100);
        h.push({ senderId: 1, messageId: 10n, text: "a" });
        h.push({ senderId: 2, messageId: 20n, text: "b" });
        h.push({ senderId: 1, messageId: 30n, text: "c" });
        const msgs = [...h.sinceForSender(1, 10n)];
        expect(msgs).toHaveLength(1);
        expect(msgs[0].messageId).toBe(30n);
    });

    test("clear", () => {
        const h = new MessageHistory(10);
        h.push({ senderId: 1, messageId: 1n, text: "hi" });
        h.clear();
        expect(h.length).toBe(0);
    });

    test("zero capacity throws", () => {
        expect(() => new MessageHistory(0)).toThrow();
    });
});

describe("Watermark", () => {
    test("observe and lastSeen", () => {
        const w = new Watermark();
        w.observe(1, 5n); w.observe(1, 3n); w.observe(2, 10n);
        expect(w.lastSeen(1)).toBe(5n);
        expect(w.lastSeen(2)).toBe(10n);
        expect(w.lastSeen(3)).toBeNull();
    });

    test("snapshot", () => {
        const w = new Watermark();
        w.observe(1, 5n); w.observe(2, 10n);
        const s = w.snapshot();
        expect(s.get(1)).toBe(5n);
        expect(s.get(2)).toBe(10n);
    });

    test("clear removes entries", () => {
        const w = new Watermark();
        w.observe(1, 1n);
        w.clear();
        expect(w.lastSeen(1)).toBeNull();
        expect(w.size).toBe(0);
    });
});

// ---------------------------------------------------------------------------
// JitterBuffer
// ---------------------------------------------------------------------------

describe("JitterBuffer", () => {
    test("accepted frame", () => {
        const buf = new JitterBuffer(10);
        const r   = buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.from([1]) });
        expect(r.outcome).toBe("accepted");
        expect(r.evicted).toBeUndefined();
    });

    test("popInOrder", () => {
        const buf = new JitterBuffer(10);
        buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.alloc(1) });
        buf.push({ mediaSourceId: 1, rtpSequence: 1, plaintext: Buffer.alloc(1) });
        expect(buf.popInOrder(1)?.rtpSequence).toBe(0);
        expect(buf.popInOrder(1)?.rtpSequence).toBe(1);
    });

    test("popForce skips gap", () => {
        const buf = new JitterBuffer(10);
        buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.alloc(1) });
        buf.popInOrder(1);
        buf.push({ mediaSourceId: 1, rtpSequence: 5, plaintext: Buffer.alloc(1) });
        expect(buf.popForce(1)?.rtpSequence).toBe(5);
    });

    test("late frame", () => {
        const buf = new JitterBuffer(10);
        buf.push({ mediaSourceId: 1, rtpSequence: 5, plaintext: Buffer.alloc(1) });
        buf.popForce(1);
        expect(buf.push({ mediaSourceId: 1, rtpSequence: 3, plaintext: Buffer.alloc(1) }).outcome).toBe("late");
    });

    test("eviction on overflow", () => {
        const buf = new JitterBuffer(2);
        buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.alloc(1) });
        buf.push({ mediaSourceId: 1, rtpSequence: 1, plaintext: Buffer.alloc(1) });
        const r = buf.push({ mediaSourceId: 1, rtpSequence: 2, plaintext: Buffer.alloc(1) });
        expect(r.outcome).toBe("evicted");
        expect(r.evicted).toBeDefined();
    });

    test("multiple sources independent", () => {
        const buf = new JitterBuffer(5);
        buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.from([0xAA]) });
        buf.push({ mediaSourceId: 2, rtpSequence: 0, plaintext: Buffer.from([0xBB]) });
        expect(buf.popInOrder(1)?.plaintext[0]).toBe(0xAA);
        expect(buf.popInOrder(2)?.plaintext[0]).toBe(0xBB);
    });

    test("lengthFor", () => {
        const buf = new JitterBuffer(10);
        expect(buf.lengthFor(1)).toBe(0);
        buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.alloc(1) });
        expect(buf.lengthFor(1)).toBe(1);
    });

    test("clear", () => {
        const buf = new JitterBuffer(10);
        buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.alloc(1) });
        buf.clear();
        expect(buf.lengthFor(1)).toBe(0);
    });

    test("out-of-order reordering", () => {
        const buf = new JitterBuffer(10);
        buf.push({ mediaSourceId: 1, rtpSequence: 2, plaintext: Buffer.alloc(1) });
        buf.push({ mediaSourceId: 1, rtpSequence: 0, plaintext: Buffer.alloc(1) });
        buf.push({ mediaSourceId: 1, rtpSequence: 1, plaintext: Buffer.alloc(1) });
        const seqs: number[] = [];
        let f;
        while ((f = buf.popInOrder(1))) seqs.push(f.rtpSequence);
        expect(seqs).toEqual([0, 1, 2]);
    });

    test("invalid capacity throws", () => {
        expect(() => new JitterBuffer(0)).toThrow();
    });
});

// ---------------------------------------------------------------------------
// RoleRegistry + CapabilitiesNegotiator
// ---------------------------------------------------------------------------

describe("RoleRegistry", () => {
    test("define and assign", () => {
        const reg = new RoleRegistry();
        reg.defineRole(1, "mod", Permissions.SendText | Permissions.MuteOthers);
        reg.assign(42, 1);
        expect(reg.permissionsOf(42)).toBe(Permissions.SendText | Permissions.MuteOthers);
    });

    test("has permission", () => {
        const reg = new RoleRegistry();
        reg.defineRole(2, "viewer", Permissions.SendText);
        reg.assign(1, 2);
        expect(reg.has(1, Permissions.SendText)).toBe(true);
        expect(reg.has(1, Permissions.MuteOthers)).toBe(false);
    });

    test("require throws on missing", () => {
        const reg = new RoleRegistry();
        reg.defineRole(1, "guest", Permissions.None);
        reg.assign(5, 1);
        expect(() => reg.require(5, Permissions.SendText)).toThrow(RoleError);
    });

    test("require passes when has permission", () => {
        const reg = new RoleRegistry();
        reg.defineRole(1, "admin", Permissions.CloseGroup | Permissions.AssignRoles);
        reg.assign(5, 1);
        expect(() => reg.require(5, Permissions.CloseGroup)).not.toThrow();
    });

    test("unknown role throws", () => {
        expect(() => new RoleRegistry().assign(1, 999)).toThrow(RoleError);
    });

    test("no role gives None permissions", () => {
        expect(new RoleRegistry().permissionsOf(99)).toBe(Permissions.None);
    });

    test("roleOf", () => {
        const reg = new RoleRegistry();
        reg.defineRole(3, "speaker", Permissions.SendAudio);
        reg.assign(10, 3);
        expect(reg.roleOf(10)?.name).toBe("speaker");
    });

    test("all permission bits", () => {
        const all = Permissions.SendText | Permissions.SendAudio | Permissions.SendSignal
                  | Permissions.MuteOthers | Permissions.AssignRoles | Permissions.Invite
                  | Permissions.RemoveMembers | Permissions.CloseGroup;
        const reg = new RoleRegistry();
        reg.defineRole(10, "superadmin", all);
        reg.assign(1, 10);
        for (const bit of [Permissions.SendText, Permissions.CloseGroup]) {
            expect(reg.has(1, bit)).toBe(true);
        }
    });
});

describe("CapabilitiesNegotiator", () => {
    test("advertise and groupSupports", () => {
        const neg = new CapabilitiesNegotiator();
        neg.advertise(1, ["audio", "video"]);
        neg.advertise(2, ["audio"]);
        expect(neg.groupSupports("audio")).toBe(true);
        expect(neg.groupSupports("video")).toBe(false);
    });

    test("intersection", () => {
        const neg = new CapabilitiesNegotiator();
        neg.advertise(1, ["a", "b", "c"]);
        neg.advertise(2, ["b", "c"]);
        expect(neg.intersection()).toEqual(new Set(["b", "c"]));
    });

    test("union", () => {
        const neg = new CapabilitiesNegotiator();
        neg.advertise(1, ["a"]);
        neg.advertise(2, ["b"]);
        expect(neg.union()).toEqual(new Set(["a", "b"]));
    });

    test("missing", () => {
        const neg = new CapabilitiesNegotiator();
        neg.advertise(1, ["x"]);
        neg.advertise(2, []);
        const m = neg.missing("x");
        expect(m).toContain(2);
        expect(m).not.toContain(1);
    });

    test("forget", () => {
        const neg = new CapabilitiesNegotiator();
        neg.advertise(1, ["a"]);
        neg.forget(1);
        expect(neg.size).toBe(0);
    });

    test("capabilitiesOf", () => {
        const neg = new CapabilitiesNegotiator();
        neg.advertise(5, ["alpha", "beta"]);
        const caps = neg.capabilitiesOf(5);
        expect(caps?.has("alpha")).toBe(true);
    });

    test("empty intersection", () => {
        expect(new CapabilitiesNegotiator().intersection()).toEqual(new Set());
    });

    test("groupSupports false when empty", () => {
        expect(new CapabilitiesNegotiator().groupSupports("x")).toBe(false);
    });

    test("update advertisement replaces", () => {
        const neg = new CapabilitiesNegotiator();
        neg.advertise(1, ["a"]);
        neg.advertise(1, ["b"]);
        const caps = neg.capabilitiesOf(1);
        expect(caps?.has("b")).toBe(true);
        expect(caps?.has("a")).toBe(false);
    });
});

// ---------------------------------------------------------------------------
// SFrame E2EE
// ---------------------------------------------------------------------------

function mlsPair() {
    const alice = MlsContext.create("alice");
    const bob   = MlsContext.create("bob");
    bob.acceptWelcome(alice.invite(bob.exportKeyPackage()));
    return { alice, bob };
}

describe("SFrameSession", () => {
    test("AES-128 encrypt/decrypt", () => {
        const { alice, bob } = mlsPair();
        try {
            const aliceSess = SFrameSession.create(alice, "gbp/sframe v1", AES_128_GCM);
            const bobSess   = SFrameSession.create(bob, "gbp/sframe v1", AES_128_GCM);
            const enc       = aliceSess.createEncryptor(alice, 0, "gbp/sframe v1", AES_128_GCM);
            const plaintext = Buffer.from("opus_data");
            const ct        = enc.encrypt(plaintext);
            expect(ct).not.toEqual(plaintext);
            const { plaintext: pt, senderLeaf } = bobSess.decrypt(ct);
            expect(pt).toEqual(plaintext);
            expect(senderLeaf).toBe(0);
            enc.close(); aliceSess.close(); bobSess.close();
        } finally { alice.close(); bob.close(); }
    });

    test("AES-256 encrypt/decrypt", () => {
        const { alice, bob } = mlsPair();
        try {
            const aliceSess = SFrameSession.create(alice, "gbp/sframe v1", AES_256_GCM);
            const bobSess   = SFrameSession.create(bob, "gbp/sframe v1", AES_256_GCM);
            const enc       = aliceSess.createEncryptor(alice, 0, "gbp/sframe v1", AES_256_GCM);
            const pt = Buffer.from([1, 2, 3]);
            const { plaintext } = bobSess.decrypt(enc.encrypt(pt));
            expect(plaintext).toEqual(pt);
            enc.close(); aliceSess.close(); bobSess.close();
        } finally { alice.close(); bob.close(); }
    });

    test("extra AAD", () => {
        const { alice, bob } = mlsPair();
        try {
            const aliceSess = SFrameSession.create(alice);
            const bobSess   = SFrameSession.create(bob);
            const enc       = aliceSess.createEncryptor(alice, 0);
            const aad = Buffer.from("stream-42");
            const pt  = Buffer.from([0xFF]);
            const ct  = enc.encrypt(pt, aad);
            const { plaintext } = bobSess.decrypt(ct, aad);
            expect(plaintext).toEqual(pt);
            enc.close(); aliceSess.close(); bobSess.close();
        } finally { alice.close(); bob.close(); }
    });

    test("multiple frames", () => {
        const { alice, bob } = mlsPair();
        try {
            const aliceSess = SFrameSession.create(alice);
            const bobSess   = SFrameSession.create(bob);
            const enc       = aliceSess.createEncryptor(alice, 0);
            for (let i = 0; i < 10; i++) {
                const pt = Buffer.from([i]);
                const { plaintext } = bobSess.decrypt(enc.encrypt(pt));
                expect(plaintext).toEqual(pt);
            }
            enc.close(); aliceSess.close(); bobSess.close();
        } finally { alice.close(); bob.close(); }
    });

    test("wrong AAD fails", () => {
        const { alice, bob } = mlsPair();
        try {
            const aliceSess = SFrameSession.create(alice);
            const bobSess   = SFrameSession.create(bob);
            const enc       = aliceSess.createEncryptor(alice, 0);
            const ct = enc.encrypt(Buffer.from([1]), Buffer.from("correct"));
            expect(() => bobSess.decrypt(ct, Buffer.from("wrong"))).toThrow();
            enc.close(); aliceSess.close(); bobSess.close();
        } finally { alice.close(); bob.close(); }
    });
});

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

describe("Utilities", () => {
    test("encodeGbpFrame returns buffer", () => {
        const mls = MlsContext.create("alice");
        try {
            const r = encodeGbpFrame(1, mls.groupId, mls.epoch, 0, 2, 0, 0, 1, Buffer.from("hello"));
            expect(r.length).toBeGreaterThan(0);
        } finally { mls.close(); }
    });

    test("encodeGbpFrame bad groupId throws", () => {
        expect(() => encodeGbpFrame(1, Buffer.alloc(5), 1n, 0, 2, 0, 0, 1, Buffer.alloc(1))).toThrow();
    });

    test("lookupError unknown code returns null", () => {
        expect(lookupError(0xFFFF)).toBeNull();
    });

    test("lookupError known code", () => {
        const r = lookupError(0x0001);
        if (r !== null) expect(r.length).toBeGreaterThan(0);
    });

    test("encodeGbpFrame empty payload", () => {
        const mls = MlsContext.create("alice");
        try {
            const r = encodeGbpFrame(1, mls.groupId, mls.epoch, 0, 0, 0, 0, 0, Buffer.alloc(0));
            expect(Buffer.isBuffer(r)).toBe(true);
        } finally { mls.close(); }
    });
});
