/** Group Text Protocol client wrapper. */

import * as N from "./native";
import { MlsContext } from "./mls";
import { GroupNode, OutboundFrame, unpackOutbound } from "./node";

/** Outcome of {@link GtpClient.accept}. */
export interface GtpAcceptResult {
    status: "new" | "duplicate" | "error" | string;
    sender?: number;
    messageId?: bigint;
    text?: string;
    reason?: string;
}

function parseAccept(json: string): GtpAcceptResult {
    if (!json) return { status: "?" };
    const d = JSON.parse(json) as Record<string, unknown>;
    return {
        status: typeof d["status"] === "string" ? (d["status"] as string) : "?",
        sender: typeof d["sender"] === "number" ? (d["sender"] as number) : undefined,
        messageId: typeof d["message_id"] === "number" ? BigInt(d["message_id"] as number) : undefined,
        text: typeof d["text"] === "string" ? (d["text"] as string) : undefined,
        reason: typeof d["reason"] === "string" ? (d["reason"] as string) : undefined,
    };
}

/**
 * Group Text Protocol client.
 *
 * Tracks idempotency by ``(sender_id, message_id)``.
 */
export class GtpClient {
    /** Native handle (i32). */
    public handle: number;

    private constructor(handle: number) { this.handle = handle; }

    /** Create a fresh GTP client. */
    static create(): GtpClient {
        const h = N.gtp_client_create() as number;
        if (h <= 0) throw new Error("gtp_client_create");
        return new GtpClient(h);
    }

    /** Send a text message. */
    send(node: GroupNode, mls: MlsContext, target: number, messageId: bigint, text: string): OutboundFrame {
        const data = Buffer.from(text, "utf8");
        const buf = N.gtp_client_send(
            this.handle, node.handle, mls.handle, target, messageId,
            data, data.length,
        ) as N.GbpBuffer;
        return unpackOutbound(buf, "gtp_client_send");
    }

    /** Accept a plaintext payload delivered by the GBP layer. */
    accept(plaintext: Buffer): GtpAcceptResult {
        const ptr = N.gtp_client_accept(this.handle, plaintext, plaintext.length) as number;
        return parseAccept(N.takeCString(ptr));
    }

    /** Clear the idempotency state. Intended for use after an epoch change. */
    reset(): void { N.gtp_client_reset(this.handle); }

    /** Release the native handle. Idempotent. */
    close(): void {
        if (this.handle) { N.gtp_client_destroy(this.handle); this.handle = 0; }
    }

    [Symbol.dispose](): void { this.close(); }
}
