/** MLS (RFC 9420) context wrapper. */

import koffi from "koffi";
import * as N from "./native";

/**
 * Managed wrapper around an MLS context owned by the native library.
 *
 * Owns a single-member group plus a published ``KeyPackage`` that can be
 * used to invite this member into another group. Always call
 * {@link MlsContext.close} (or use `try/finally`) to release the native
 * handle.
 */
export class MlsContext {
    /** Native handle (i32). */
    public handle: number;

    /** Application-level identity. */
    public readonly identity: string;

    private constructor(handle: number, identity: string) {
        this.handle = handle;
        this.identity = identity;
    }

    /** Create a fresh MLS context. */
    static create(identity: string): MlsContext {
        const data = Buffer.from(identity, "utf8");
        const h = N.gbp_mls_create(data, data.length) as number;
        if (h <= 0) throw new Error(`gbp_mls_create: ${N.lastError()}`);
        return new MlsContext(h, identity);
    }

    /** Current group epoch. */
    get epoch(): bigint {
        return BigInt(N.gbp_mls_epoch(this.handle) as number | bigint);
    }

    /** 16-byte group identifier. */
    get groupId(): Buffer {
        const out = Buffer.alloc(16);
        if (!(N.gbp_mls_group_id(this.handle, out) as boolean)) {
            throw new Error(`group_id: ${N.lastError()}`);
        }
        return out;
    }

    /** Export this member's TLS-serialised KeyPackage. */
    exportKeyPackage(): Buffer {
        const buf = N.gbp_mls_export_key_package(this.handle) as N.GbpBuffer;
        const out = N.takeBuffer(buf);
        if (out.length === 0) throw new Error(`export_key_package: ${N.lastError()}`);
        return out;
    }

    /**
     * Invite the given KeyPackage into the local group; returns the
     * Welcome only. Use {@link MlsContext.inviteFull} to also obtain the
     * Commit message that must be broadcast to existing members
     * (RFC 9420 §11/§12.4).
     */
    invite(keyPackage: Buffer): Buffer {
        const buf = N.gbp_mls_invite(this.handle, keyPackage, keyPackage.length) as N.GbpBuffer;
        const out = N.takeBuffer(buf);
        if (out.length === 0) throw new Error(`invite: ${N.lastError()}`);
        return out;
    }

    /**
     * Invite the given KeyPackage and return BOTH the MLS Commit (broadcast
     * to existing members) and the Welcome (unicast to the new joiner).
     */
    inviteFull(keyPackage: Buffer): { commit: Buffer; welcome: Buffer } {
        const buf = N.gbp_mls_invite_full(this.handle, keyPackage, keyPackage.length) as N.GbpBuffer;
        const out = N.takeBuffer(buf);
        if (out.length < 4) throw new Error(`invite_full: ${N.lastError() || "truncated"}`);
        const commitLen = out.readUInt32LE(0);
        if (commitLen < 0 || 4 + commitLen > out.length) throw new Error(`invite_full: bad commit_len`);
        const commit = out.subarray(4, 4 + commitLen);
        const welcome = out.subarray(4 + commitLen);
        return { commit: Buffer.from(commit), welcome: Buffer.from(welcome) };
    }

    /**
     * Remove the member at the given MLS LeafIndex; returns the Commit that
     * remaining members must apply via {@link MlsContext.processMessage}.
     */
    removeMember(leafIndex: number): Buffer {
        const buf = N.gbp_mls_remove(this.handle, leafIndex >>> 0) as N.GbpBuffer;
        const out = N.takeBuffer(buf);
        if (out.length === 0) throw new Error(`remove: ${N.lastError()}`);
        return out;
    }

    /**
     * Apply a Commit (or staged Proposal) to the local MLS group. Returns
     * the kind of message that was processed.
     */
    processMessage(message: Buffer): "commit" | "application" | "proposal" | "external" {
        const code = N.gbp_mls_process_message(this.handle, message, message.length) as number;
        switch (code) {
            case 1: return "commit";
            case 2: return "application";
            case 3: return "proposal";
            case 4: return "external";
            default: throw new Error(`process_message: ${N.lastError()}`);
        }
    }

    /**
     * Merge any pending commit produced by {@link MlsContext.inviteFull} or
     * {@link MlsContext.removeMember}. Idempotent.
     */
    finalizeCommit(): void {
        const ok = N.gbp_mls_finalize_commit(this.handle) as boolean;
        if (!ok) throw new Error(`finalize_commit: ${N.lastError()}`);
    }

    /** Discard any pending commit without applying it (used on ABORT). */
    clearPendingCommit(): void {
        const ok = N.gbp_mls_clear_pending_commit(this.handle) as boolean;
        if (!ok) throw new Error(`clear_pending_commit: ${N.lastError()}`);
    }

    /** Replace the local group with the one described by the Welcome. */
    acceptWelcome(welcome: Buffer): void {
        const ok = N.gbp_mls_accept_welcome(this.handle, welcome, welcome.length) as boolean;
        if (!ok) throw new Error(`accept_welcome: ${N.lastError()}`);
    }

    /**
     * Serialise the full MLS state into an opaque blob that
     * {@link MlsContext.restoreState} can reconstruct, so a consumer can
     * persist the context across restarts. The blob contains **private key
     * material** — store it encrypted at rest.
     */
    exportState(): Buffer {
        const buf = N.gbp_mls_export_state(this.handle) as N.GbpBuffer;
        const out = N.takeBuffer(buf);
        if (out.length === 0) throw new Error(`export_state: ${N.lastError()}`);
        return out;
    }

    /**
     * Reconstruct a context from a blob produced by {@link MlsContext.exportState}.
     * The restored context is at the same epoch / group state and can send and
     * receive again. `identity` is informational (the real identity is in the blob).
     */
    static restoreState(state: Buffer, identity = ""): MlsContext {
        const h = N.gbp_mls_restore_state(state, state.length) as number;
        if (h <= 0) throw new Error(`restore_state: ${N.lastError()}`);
        return new MlsContext(h, identity);
    }

    /** Release the native handle. Idempotent. */
    close(): void {
        if (this.handle) {
            N.gbp_mls_destroy(this.handle);
            this.handle = 0;
        }
    }

    [Symbol.dispose](): void { this.close(); }
}
