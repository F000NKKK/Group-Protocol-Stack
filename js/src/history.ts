/** Bounded message log + per-sender resync watermark for GTP. */

/** One entry in {@link MessageHistory}. */
export interface MessageEntry {
    senderId: number;
    messageId: bigint;
    text: string;
}

/**
 * Bounded ring buffer of recent GTP messages.
 *
 * Used to serve resync requests from re-joining peers — keep a few
 * thousand recent messages, then `since(watermark)` returns everything
 * above the caller's high-water mark.
 */
export class MessageHistory {
    private readonly capacity: number;
    private buffer: MessageEntry[] = [];

    constructor(capacity: number) {
        if (capacity <= 0) throw new Error("capacity must be > 0");
        this.capacity = capacity;
    }

    get length(): number { return this.buffer.length; }

    /** Record a message. Returns `true` if newly added. */
    push(entry: MessageEntry): boolean {
        if (this.contains(entry.senderId, entry.messageId)) return false;
        if (this.buffer.length === this.capacity) this.buffer.shift();
        this.buffer.push(entry);
        return true;
    }

    /** `true` if `(senderId, messageId)` is present. */
    contains(senderId: number, messageId: bigint): boolean {
        return this.buffer.some(e => e.senderId === senderId && e.messageId === messageId);
    }

    /** Yields every message produced after the given watermark. */
    *since(watermark: Watermark): Iterable<MessageEntry> {
        for (const m of this.buffer) {
            const hw = watermark.lastSeen(m.senderId);
            if (hw === null || m.messageId > hw) yield m;
        }
    }

    /** Yields messages from a single sender newer than `sinceMessageId`. */
    *sinceForSender(senderId: number, sinceMessageId: bigint): Iterable<MessageEntry> {
        for (const m of this.buffer)
            if (m.senderId === senderId && m.messageId > sinceMessageId) yield m;
    }

    /** Drops every message in the buffer. */
    clear(): void { this.buffer = []; }
}

/** Per-sender high-water mark of accepted GTP `message_id`s. */
export class Watermark {
    private readonly _lastSeen = new Map<number, bigint>();

    /** Record that `messageId` from `senderId` has been observed. */
    observe(senderId: number, messageId: bigint): void {
        const prev = this._lastSeen.get(senderId) ?? 0n;
        if (messageId > prev) this._lastSeen.set(senderId, messageId);
    }

    /** Last observed `messageId` for `senderId`, or `null`. */
    lastSeen(senderId: number): bigint | null {
        return this._lastSeen.get(senderId) ?? null;
    }

    /** Returns a copy of the underlying map. */
    snapshot(): Map<number, bigint> { return new Map(this._lastSeen); }

    get size(): number { return this._lastSeen.size; }

    /** Drops every entry. */
    clear(): void { this._lastSeen.clear(); }
}
