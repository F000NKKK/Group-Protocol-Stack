/**
 * GSP Signals — two-member call signalling in the browser.
 *
 * Demonstrates the signalling sub-protocol that drives call membership and
 * media state:
 *   1. MLS identity creation and key exchange
 *   2. GBP group node bootstrapping
 *   3. GSP bare signals (JOIN / LEAVE) via `send`
 *   4. GSP signals carrying CBOR args (MUTE / STREAM_START) via `sendWithArgs`
 *
 * Run with: npx ts-node gspSignals.ts  (Node.js ≥ 18, typescript ≥ 5)
 * Or bundle for the browser with Vite / webpack 5 / Rollup.
 */

import init, {
  MlsContext,
  GroupNode,
  GspClient,
  SignalType,
} from "@voluntas-progressus/gbp-stack-wasm";

// --- types -----------------------------------------------------------------

interface SendFrame {
  wire: Uint8Array;
  to: number;
}

interface GspResult {
  status: "new" | "duplicate";
  signal?: string;
  signalCode?: number;
  sender?: number;
  roleClaim?: number;
  requestId: number;
}

interface NodeEvent {
  kind: string;
  streamType?: number;
  plaintext?: Uint8Array;
}

const StreamType = { Control: 0, Audio: 1, Text: 2, Signal: 3 } as const;

function signalEvents(events: NodeEvent[]): NodeEvent[] {
  return events.filter(
    (ev) => ev.kind === "payload_received" && ev.streamType === StreamType.Signal,
  );
}

/**
 * Minimal CBOR encoder for a `{ key: uint }` map — enough for the GSP arg
 * schemas (MUTE/UNMUTE `{0: target}`, STREAM_* `{0: stream_type}`,
 * CODEC_UPDATE `{0: codec_id}`). A real app would use a CBOR library.
 */
function cborUintMap(entries: Array<[number, number]>): Uint8Array {
  const out: number[] = [0xa0 | entries.length]; // map of N pairs (N < 24)
  const uint = (n: number) => (n < 24 ? [n] : [0x18, n]); // uint < 256
  for (const [k, v] of entries) out.push(...uint(k), ...uint(v));
  return new Uint8Array(out);
}

function deliver(
  from: GspClient,
  fromNode: GroupNode,
  fromMls: MlsContext,
  toClient: GspClient,
  toNode: GroupNode,
  toMls: MlsContext,
  label: string,
  frame: SendFrame,
) {
  const events = toNode.onWire(toMls, frame.wire) as NodeEvent[];
  for (const ev of signalEvents(events)) {
    const r = toClient.accept(ev.plaintext!, toMls.epoch) as GspResult;
    console.log(`  ${label}: [${r.status}] ${r.signal} (code=${r.signalCode}, req=${r.requestId})`);
  }
}

// --- main ------------------------------------------------------------------

async function main() {
  await init();

  // ── Setup: two-member group ─────────────────────────────────────────────
  const aliceMls = MlsContext.create("alice");
  const bobMls = MlsContext.create("bob");
  bobMls.acceptWelcome(aliceMls.invite(bobMls.keyPackage));

  const groupId = aliceMls.groupId;
  const aliceNode = GroupNode.create(1, groupId);
  const bobNode = GroupNode.create(2, groupId);
  aliceNode.bootstrapAsCreator(aliceMls.epoch);
  bobNode.bootstrapAsJoiner(bobMls.epoch, 0);

  const gspAlice = GspClient.create();
  const gspBob = GspClient.create();

  // ── JOIN (bare signal, no args) ─────────────────────────────────────────
  console.log("── Alice announces JOIN ──");
  const join = gspAlice.send(aliceNode, aliceMls, 2 /* bob */, SignalType.Join, 0, 1) as SendFrame;
  deliver(gspAlice, aliceNode, aliceMls, gspBob, bobNode, bobMls, "Bob sees", join);

  // ── MUTE (signal with CBOR args {0: target_member_id}) ──────────────────
  console.log("\n── Alice requests MUTE of member 2 ──");
  const muteArgs = cborUintMap([[0, 2]]);
  const mute = gspAlice.sendWithArgs(
    aliceNode, aliceMls, 2, SignalType.Mute, 0, 2, muteArgs,
  ) as SendFrame;
  deliver(gspAlice, aliceNode, aliceMls, gspBob, bobNode, bobMls, "Bob sees", mute);

  // ── STREAM_START (signal with CBOR args {0: stream_type}) ───────────────
  console.log("\n── Bob announces STREAM_START (audio = 1) ──");
  const streamArgs = cborUintMap([[0, StreamType.Audio]]);
  const stream = gspBob.sendWithArgs(
    bobNode, bobMls, 1 /* alice */, SignalType.StreamStart, 0, 3, streamArgs,
  ) as SendFrame;
  deliver(gspBob, bobNode, bobMls, gspAlice, aliceNode, aliceMls, "Alice sees", stream);

  console.log("\nDone.");
}

main().catch(console.error);
