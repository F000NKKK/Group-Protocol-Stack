/** Bounded reorder buffer for GAP audio frames. */

/** One frame held by {@link JitterBuffer}. */
export interface AudioFrame {
    mediaSourceId: number;
    rtpSequence: number;
    plaintext: Buffer;
}

/** Outcome of {@link JitterBuffer.push}. */
export type JitterPushOutcome = "accepted" | "late" | "evicted";

/** Result returned by {@link JitterBuffer.push}. */
export interface JitterPushResult {
    outcome: JitterPushOutcome;
    evicted?: AudioFrame;
}

interface SourceState {
    waiting: AudioFrame[];
    next: number | null;
}

/**
 * Bounded reorder window keyed by `mediaSourceId`. Holds incoming GAP
 * frames briefly so the decoder consumes them in `rtpSequence` order;
 * drops anything older than the next-expected sequence as late.
 */
export class JitterBuffer {
    private readonly capacityPerSource: number;
    private readonly sources = new Map<number, SourceState>();

    constructor(capacityPerSource: number) {
        if (capacityPerSource <= 0) throw new Error("capacityPerSource must be > 0");
        this.capacityPerSource = capacityPerSource;
    }

    /** Insert a frame into the buffer. */
    push(frame: AudioFrame): JitterPushResult {
        let state = this.sources.get(frame.mediaSourceId);
        if (!state) {
            state = { waiting: [], next: null };
            this.sources.set(frame.mediaSourceId, state);
        }
        if (state.next !== null && frame.rtpSequence < state.next) {
            return { outcome: "late" };
        }
        if (state.waiting.some(f => f.rtpSequence === frame.rtpSequence)) {
            return { outcome: "accepted" };
        }
        const idx = state.waiting.findIndex(f => f.rtpSequence > frame.rtpSequence);
        if (idx === -1) state.waiting.push(frame);
        else state.waiting.splice(idx, 0, frame);

        if (state.waiting.length > this.capacityPerSource) {
            const evicted = state.waiting.shift()!;
            return { outcome: "evicted", evicted };
        }
        return { outcome: "accepted" };
    }

    /** Pop the next frame if its `rtpSequence` is contiguous; otherwise `null`. */
    popInOrder(mediaSourceId: number): AudioFrame | null {
        const state = this.sources.get(mediaSourceId);
        if (!state || state.waiting.length === 0) return null;
        const head = state.waiting[0];
        if (state.next !== null && head.rtpSequence !== state.next) return null;
        state.waiting.shift();
        state.next = (head.rtpSequence + 1) >>> 0;
        return head;
    }

    /** Pop the next frame regardless of contiguity (skip gaps). */
    popForce(mediaSourceId: number): AudioFrame | null {
        const state = this.sources.get(mediaSourceId);
        if (!state || state.waiting.length === 0) return null;
        const head = state.waiting.shift()!;
        state.next = (head.rtpSequence + 1) >>> 0;
        return head;
    }

    /** Number of frames buffered for the given source. */
    lengthFor(mediaSourceId: number): number {
        return this.sources.get(mediaSourceId)?.waiting.length ?? 0;
    }

    /** Drops every queued frame. */
    clear(): void { this.sources.clear(); }
}
