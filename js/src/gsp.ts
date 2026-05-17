/** Group Signaling Protocol client wrapper. */

import * as N from "./native";
import { PayloadCodec } from "./native";
import { MlsContext } from "./mls";
import { GroupNode, OutboundFrame, unpackOutbound } from "./node";

/** Signal opcode registry. */
export enum SignalType {
    Join = 100,
    Leave = 101,
    RoleChange = 102,
    Mute = 200,
    Unmute = 201,
    StreamStart = 300,
    StreamStop = 301,
    CodecUpdate = 400,
}

/** Outcome of {@link GspClient.accept}. */
export interface GspAcceptResult {
    status: "new" | "error" | string;
    signal?: string;
    signalCode?: SignalType;
    sender?: number;
    roleClaim?: number;
    requestId?: number;
    reason?: string;
}

function parseAccept(json: string): GspAcceptResult {
    if (!json) return { status: "?" };
    const d = JSON.parse(json) as Record<string, unknown>;
    const sc = typeof d["signal_code"] === "number" ? (d["signal_code"] as number) : undefined;
    return {
        status: typeof d["status"] === "string" ? (d["status"] as string) : "?",
        signal: typeof d["signal"] === "string" ? (d["signal"] as string) : undefined,
        signalCode: sc !== undefined ? (sc as SignalType) : undefined,
        sender: typeof d["sender"] === "number" ? (d["sender"] as number) : undefined,
        roleClaim: typeof d["role_claim"] === "number" ? (d["role_claim"] as number) : undefined,
        requestId: typeof d["request_id"] === "number" ? (d["request_id"] as number) : undefined,
        reason: typeof d["reason"] === "string" ? (d["reason"] as string) : undefined,
    };
}

/**
 * Group Signaling Protocol client.
 *
 * Tracks ``request_id`` deduplication and exposes accepted signals to the
 * application.
 */
export class GspClient {
    /** Native handle (i32). */
    public handle: number;

    private constructor(handle: number) { this.handle = handle; }

    /** Create a fresh GSP client. */
    static create(): GspClient {
        const h = N.gsp_client_create() as number;
        if (h <= 0) throw new Error("gsp_client_create");
        return new GspClient(h);
    }

    /**
     * Send a signal without signal-specific args.
     * @param codec - payload encoding (default: CBOR).
     */
    send(
        node: GroupNode,
        mls: MlsContext,
        target: number,
        signal: SignalType,
        roleClaim: number,
        requestId: number,
        codec: PayloadCodec = PayloadCodec.Cbor,
    ): OutboundFrame {
        const buf = N.gsp_client_send(
            this.handle, node.handle, mls.handle, target,
            signal, roleClaim, requestId, codec,
        ) as N.GbpBuffer;
        return unpackOutbound(buf, "gsp_client_send");
    }

    /**
     * Send a signal with opcode-specific args bytes.
     * @param codec - payload encoding (default: CBOR).
     */
    sendWithArgs(
        node: GroupNode,
        mls: MlsContext,
        target: number,
        signal: SignalType,
        roleClaim: number,
        requestId: number,
        args: Buffer,
        codec: PayloadCodec = PayloadCodec.Cbor,
    ): OutboundFrame {
        const buf = N.gsp_client_send_with_args(
            this.handle, node.handle, mls.handle, target,
            signal, roleClaim, requestId,
            args, args.length, codec,
        ) as N.GbpBuffer;
        return unpackOutbound(buf, "gsp_client_send_with_args");
    }

    /**
     * Accept a plaintext payload delivered by the GBP layer.
     * `currentEpoch` lets the client auto-reset its dedup state when the
     * epoch advances. `codec` must match the `codec` field of the
     * `payload_received` event.
     */
    accept(plaintext: Buffer, currentEpoch: bigint | number,
        codec: PayloadCodec = PayloadCodec.Cbor): GspAcceptResult {
        const ptr = N.gsp_client_accept(
            this.handle, BigInt(currentEpoch), plaintext, plaintext.length, codec,
        ) as number;
        return parseAccept(N.takeCString(ptr));
    }

    /** Clear the request-id deduplication set. Intended for use after an epoch change. */
    reset(): void { N.gsp_client_reset(this.handle); }

    /** Release the native handle. Idempotent. */
    close(): void {
        if (this.handle) { N.gsp_client_destroy(this.handle); this.handle = 0; }
    }

    [Symbol.dispose](): void { this.close(); }
}
