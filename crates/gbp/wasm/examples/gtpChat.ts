/**
 * GTP Chat — minimal two-member end-to-end encrypted text chat in the browser.
 *
 * Demonstrates the full GBP stack lifecycle:
 *   1. MLS identity creation and key exchange (invite / acceptWelcome)
 *   2. GBP group node bootstrapping (creator + joiner)
 *   3. GTP text send / receive with idempotency deduplication
 *
 * Run with: npx ts-node gtpChat.ts  (Node.js ≥ 18, typescript ≥ 5)
 * Or bundle for the browser with Vite / webpack 5 / Rollup.
 */

import init, {
  MlsContext,
  GroupNode,
  GtpClient,
} from "@voluntas-progressus/gbp-stack-wasm";

// --- types -----------------------------------------------------------------

interface SendFrame {
  wire: Uint8Array;
  to: number;
}

interface GtpResult {
  text: string;
  messageId: bigint;
  senderId: number;
  status: "new" | "duplicate";
}

interface NodeEvent {
  kind: string;
  streamType?: number;
  plaintext?: Uint8Array;
  sequenceNo?: number;
  from?: string | number;
  to?: string;
  epoch?: bigint;
  transitionId?: number;
  code?: number;
  reason?: string;
  fatal?: boolean;
  retryable?: boolean;
  opcode?: number;
}

// --- StreamType constants --------------------------------------------------

const StreamType = { Control: 0, Audio: 1, Text: 2, Signal: 3 } as const;

// --- helpers ---------------------------------------------------------------

function send(
  gtp: GtpClient,
  node: GroupNode,
  mls: MlsContext,
  target: number,
  messageId: bigint,
  text: string,
): SendFrame {
  const frame = gtp.send(node, mls, target, messageId, text) as SendFrame | null;
  if (!frame) throw new Error("send() returned null");
  return frame;
}

function accept(gtp: GtpClient, plaintext: Uint8Array, epoch: bigint): GtpResult | null {
  return gtp.accept(plaintext, epoch) as GtpResult | null;
}

function textEvents(events: NodeEvent[]): NodeEvent[] {
  return events.filter(
    (ev) => ev.kind === "payload_received" && ev.streamType === StreamType.Text,
  );
}

// --- main ------------------------------------------------------------------

async function main() {
  await init();

  // ── Step 1: MLS identities ──────────────────────────────────────────────
  const aliceMls = MlsContext.create("alice");
  const bobMls = MlsContext.create("bob");

  console.log(`Alice epoch before invite: ${aliceMls.epoch}`);
  console.log(`Bob   epoch before invite: ${bobMls.epoch}`);

  // Alice invites Bob: returns Welcome bytes Bob must accept.
  const welcome = aliceMls.invite(bobMls.keyPackage);
  bobMls.acceptWelcome(welcome);

  console.log(`Alice epoch after invite:  ${aliceMls.epoch}`);
  console.log(`Bob   epoch after invite:  ${bobMls.epoch}`);
  console.log(
    `Group IDs match: ${
      JSON.stringify(Array.from(aliceMls.groupId)) ===
      JSON.stringify(Array.from(bobMls.groupId))
    }`,
  );

  // ── Step 2: GBP group nodes ─────────────────────────────────────────────
  const groupId = aliceMls.groupId; // shared 16-byte identifier

  const aliceNode = GroupNode.create(1, groupId);
  const bobNode = GroupNode.create(2, groupId);

  aliceNode.bootstrapAsCreator(aliceMls.epoch);
  bobNode.bootstrapAsJoiner(bobMls.epoch, 0);

  console.log(`\naliceNode.currentEpoch: ${aliceNode.currentEpoch}`);
  console.log(`bobNode.currentEpoch:   ${bobNode.currentEpoch}`);

  // ── Step 3: GTP clients ─────────────────────────────────────────────────
  const gtpAlice = GtpClient.create();
  const gtpBob = GtpClient.create();

  // Alice → Bob
  console.log("\n── Alice → Bob ──");
  const frame1 = send(gtpAlice, aliceNode, aliceMls, 2 /* bobId */, 1n, "hello bob!");
  console.log(`Wire frame size: ${frame1.wire.length} bytes`);

  const events1 = bobNode.onWire(bobMls, frame1.wire) as NodeEvent[];
  for (const ev of textEvents(events1)) {
    const result = accept(gtpBob, ev.plaintext!, bobMls.epoch);
    if (result) {
      console.log(`Bob received [${result.status}]: "${result.text}" (msgId=${result.messageId})`);
    }
  }

  // Bob → Alice
  console.log("\n── Bob → Alice ──");
  const frame2 = send(gtpBob, bobNode, bobMls, 1 /* aliceId */, 1n, "hi alice!");
  const events2 = aliceNode.onWire(aliceMls, frame2.wire) as NodeEvent[];
  for (const ev of textEvents(events2)) {
    const result = accept(gtpAlice, ev.plaintext!, aliceMls.epoch);
    if (result) {
      console.log(`Alice received [${result.status}]: "${result.text}" (msgId=${result.messageId})`);
    }
  }

  // Duplicate detection
  console.log("\n── Duplicate detection ──");
  const events3 = aliceNode.onWire(aliceMls, frame2.wire) as NodeEvent[];
  for (const ev of textEvents(events3)) {
    const result = accept(gtpAlice, ev.plaintext!, aliceMls.epoch);
    if (result) {
      console.log(`Alice received [${result.status}]: "${result.text}"`);
    }
  }

  // Broadcast (target = 0)
  console.log("\n── Broadcast from Alice ──");
  const frame3 = send(gtpAlice, aliceNode, aliceMls, 0 /* broadcast */, 2n, "everyone can see this");
  const events4 = bobNode.onWire(bobMls, frame3.wire) as NodeEvent[];
  for (const ev of textEvents(events4)) {
    const result = accept(gtpBob, ev.plaintext!, bobMls.epoch);
    if (result) {
      console.log(`Bob received broadcast [${result.status}]: "${result.text}"`);
    }
  }

  console.log("\nDone.");
}

main().catch(console.error);
