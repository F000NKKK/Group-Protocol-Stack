/**
 * GAP Audio + SFrame — two-member end-to-end encrypted voice in the browser.
 *
 * Demonstrates the full real-time-call pipeline:
 *   1. MLS identity creation and key exchange (invite / acceptWelcome)
 *   2. GBP group node bootstrapping (creator + joiner)
 *   3. SFrame media E2EE keyed off the MLS epoch (createEncryptor / decrypt)
 *   4. GAP audio send / accept (Opus frames are opaque bytes — encode/decode
 *      with WebCodecs or libopus.wasm in a real app)
 *
 * The Opus payload is wrapped with SFrame *before* GAP carries it, so the
 * server (delivery layer) never sees plaintext audio.
 *
 * Run with: npx ts-node gapAudio.ts  (Node.js ≥ 18, typescript ≥ 5)
 * Or bundle for the browser with Vite / webpack 5 / Rollup.
 */

import init, {
  MlsContext,
  GroupNode,
  GapClient,
  SFrameSession,
  PayloadCodec,
  CipherSuite,
} from "@voluntas-progressus/gbp-stack-wasm";

// --- types -----------------------------------------------------------------

interface SendFrame {
  wire: Uint8Array;
  to: number;
}

interface GapResult {
  status: "new" | "late";
  source: number;
  seq: number;
  rtpTimestamp: bigint;
  opus: Uint8Array;
}

interface SFramePlain {
  plaintext: Uint8Array;
  senderLeaf: number;
}

interface NodeEvent {
  kind: string;
  streamType?: number;
  plaintext?: Uint8Array;
  codec?: number;
}

const StreamType = { Control: 0, Audio: 1, Text: 2, Signal: 3 } as const;
const SFRAME_LABEL = "gbp/sframe v1";

function audioEvents(events: NodeEvent[]): NodeEvent[] {
  return events.filter(
    (ev) => ev.kind === "payload_received" && ev.streamType === StreamType.Audio,
  );
}

// --- main ------------------------------------------------------------------

async function main() {
  await init();

  // ── Step 1: MLS identities ──────────────────────────────────────────────
  const aliceMls = MlsContext.create("alice");
  const bobMls = MlsContext.create("bob");
  const welcome = aliceMls.invite(bobMls.keyPackage);
  bobMls.acceptWelcome(welcome);
  console.log(`Shared epoch: alice=${aliceMls.epoch}, bob=${bobMls.epoch}`);

  // ── Step 2: GBP group nodes ─────────────────────────────────────────────
  const groupId = aliceMls.groupId;
  const aliceNode = GroupNode.create(1, groupId);
  const bobNode = GroupNode.create(2, groupId);
  aliceNode.bootstrapAsCreator(aliceMls.epoch);
  bobNode.bootstrapAsJoiner(bobMls.epoch, 0);

  // ── Step 3: SFrame sessions (one per MLS epoch) ─────────────────────────
  // Both members derive the SAME base key from the shared MLS exporter secret.
  // After any membership change (invite/remove → new epoch) create fresh ones.
  const aliceLeaf = 0; // creator's MLS leaf index
  const aliceEnc = SFrameSession.create(aliceMls, SFRAME_LABEL, CipherSuite.Aes128Gcm)
    .createEncryptor(aliceMls, aliceLeaf, SFRAME_LABEL, CipherSuite.Aes128Gcm);
  const bobSession = SFrameSession.create(bobMls, SFRAME_LABEL, CipherSuite.Aes128Gcm);

  // ── Step 4: GAP clients ─────────────────────────────────────────────────
  const gapAlice = GapClient.create();
  const gapBob = GapClient.create();

  console.log("\n── Alice speaks → Bob hears ──");
  const mediaSourceId = 7; // Alice's microphone
  for (let i = 0; i < 3; i++) {
    // In a real app this is an encoded Opus frame from WebCodecs.
    const opus = new Uint8Array([0xaa, 0xbb, i, i + 1]);
    const rtpTimestamp = BigInt(960 * (i + 1)); // 20 ms @ 48 kHz

    // E2EE: wrap the Opus frame with SFrame before GAP carries it.
    const sframe = aliceEnc.encrypt(opus, new Uint8Array());

    // GAP send — FlatBuffers codec for lowest decode latency on audio.
    const frame = gapAlice.send(
      aliceNode, aliceMls, 2 /* bobId */, mediaSourceId, rtpTimestamp,
      sframe, PayloadCodec.FlatBuffers,
    ) as SendFrame | null;
    if (!frame) throw new Error("gap.send returned null");

    const events = bobNode.onWire(bobMls, frame.wire) as NodeEvent[];
    for (const ev of audioEvents(events)) {
      const acc = gapBob.accept(ev.plaintext!, bobMls.epoch, ev.codec) as GapResult | null;
      if (!acc) continue;
      // Decrypt the SFrame payload back to the raw Opus frame.
      const plain = bobSession.decrypt(acc.opus, new Uint8Array()) as SFramePlain;
      console.log(
        `Bob got [${acc.status}] src=${acc.source} seq=${acc.seq} ` +
        `from leaf ${plain.senderLeaf}: ${plain.plaintext.length} opus bytes`,
      );
    }
  }

  console.log("\nDone.");
}

main().catch(console.error);
