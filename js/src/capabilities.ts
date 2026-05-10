/** Capability negotiation helper for GSP. */

/**
 * Per-member capability advertisement plus intersection / union queries
 * for negotiating optional features (codecs, extensions, version flags).
 */
export class CapabilitiesNegotiator {
    private readonly advertised = new Map<number, Set<string>>();

    /** Record an advertisement (replaces any prior one). */
    advertise(memberId: number, capabilities: Iterable<string>): void {
        this.advertised.set(memberId, new Set(capabilities));
    }

    /** Remove a member's advertisement. */
    forget(memberId: number): void { this.advertised.delete(memberId); }

    /** Current advertisement for `memberId` (a copy). */
    capabilitiesOf(memberId: number): Set<string> | null {
        const s = this.advertised.get(memberId);
        return s ? new Set(s) : null;
    }

    /** `true` iff every advertised member supports `capability`. */
    groupSupports(capability: string): boolean {
        if (this.advertised.size === 0) return false;
        for (const s of this.advertised.values()) if (!s.has(capability)) return false;
        return true;
    }

    /** Intersection — capabilities every member advertises (safe-to-use set). */
    intersection(): Set<string> {
        const iter = this.advertised.values();
        const first = iter.next();
        if (first.done) return new Set();
        const acc = new Set(first.value);
        let n = iter.next();
        while (!n.done) {
            for (const c of acc) if (!n.value.has(c)) acc.delete(c);
            n = iter.next();
        }
        return acc;
    }

    /** Union — every capability advertised by any member. */
    union(): Set<string> {
        const acc = new Set<string>();
        for (const s of this.advertised.values()) for (const c of s) acc.add(c);
        return acc;
    }

    /** Members that did not advertise `capability`. */
    missing(capability: string): number[] {
        const out: number[] = [];
        for (const [m, s] of this.advertised) if (!s.has(capability)) out.push(m);
        return out;
    }

    get size(): number { return this.advertised.size; }
}
