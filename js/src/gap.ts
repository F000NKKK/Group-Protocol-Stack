/** Group Audio Protocol client wrapper. */

import * as N from "./native";
import { PayloadCodec } from "./native";
import { MlsContext } from "./mls";
import { GroupNode, OutboundFrame, unpackOutbound } from "./node";

/** Outcome of {@link GapClient.accept}. */
export interface GapAcceptResult {
    status: "new" | "late" | "error" | string;
    source?: number;
    seq?: number;
    bytes?: number;
    reason?: string;
}

function parseAccept(json: string): GapAcceptResult {
    if (!json) return { status: "?" };
    const d = JSON.parse(json) as Record<string, unknown>;
    return {
        status: typeof d["status"] === "string" ? (d["status"] as string) : "?",
        source: typeof d["source"] === "number" ? (d["source"] as number) : undefined,
        seq: typeof d["seq"] === "number" ? (d["seq"] as number) : undefined,
        bytes: typeof d["bytes"] === "number" ? (d["bytes"] as number) : undefined,
        reason: typeof d["reason"] === "string" ? (d["reason"] as string) : undefined,
    };
}

/**
 * Group Audio Protocol client.
 *
 * Maintains a per-source ``rtp_sequence`` window and validates
 * ``key_phase`` against the current group epoch.
 */
export class GapClient {
    /** Native handle (i32). */
    public handle: number;

    private constructor(handle: number) { this.handle = handle; }

    /** Create a fresh GAP client. */
    static create(): GapClient {
        const h = N.gap_client_create() as number;
        if (h <= 0) throw new Error("gap_client_create");
        return new GapClient(h);
    }

    /**
     * Send an Opus audio frame.
     * @param codec - payload encoding; use `PayloadCodec.FlatBuffers` for lowest latency.
     */
    send(
        node: GroupNode,
        mls: MlsContext,
        target: number,
        mediaSourceId: number,
        rtpTimestamp: bigint,
        opus: Buffer,
        codec: PayloadCodec = PayloadCodec.Cbor,
    ): OutboundFrame {
        const buf = N.gap_client_send(
            this.handle, node.handle, mls.handle, target,
            mediaSourceId, rtpTimestamp, opus, opus.length, codec,
        ) as N.GbpBuffer;
        return unpackOutbound(buf, "gap_client_send");
    }

    /**
     * Accept a plaintext payload delivered by the GBP layer.
     * `codec` must match the `codec` field of the `payload_received` event.
     */
    accept(plaintext: Buffer, currentEpoch: bigint | number,
        codec: PayloadCodec = PayloadCodec.Cbor): GapAcceptResult {
        const ptr = N.gap_client_accept(
            this.handle, BigInt(currentEpoch), plaintext, plaintext.length, codec,
        ) as number;
        return parseAccept(N.takeCString(ptr));
    }

    /** Clear the replay window. Intended for use after an epoch change. */
    reset(): void { N.gap_client_reset(this.handle); }

    /** Release the native handle. Idempotent. */
    close(): void {
        if (this.handle) { N.gap_client_destroy(this.handle); this.handle = 0; }
    }

    [Symbol.dispose](): void { this.close(); }
}
