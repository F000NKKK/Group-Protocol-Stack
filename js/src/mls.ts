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
        return N.gbp_mls_epoch(this.handle) as bigint;
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

    /** Invite the given KeyPackage into the local group; returns the Welcome. */
    invite(keyPackage: Buffer): Buffer {
        const buf = N.gbp_mls_invite(this.handle, keyPackage, keyPackage.length) as N.GbpBuffer;
        const out = N.takeBuffer(buf);
        if (out.length === 0) throw new Error(`invite: ${N.lastError()}`);
        return out;
    }

    /** Replace the local group with the one described by the Welcome. */
    acceptWelcome(welcome: Buffer): void {
        const ok = N.gbp_mls_accept_welcome(this.handle, welcome, welcome.length) as boolean;
        if (!ok) throw new Error(`accept_welcome: ${N.lastError()}`);
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
