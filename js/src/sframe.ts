import * as N from "./native.js";
import { MlsContext } from "./mls.js";

/**
 * SFrame ciphersuite.
 *
 * * `0` — AES-128-GCM (16-byte key). Default.
 * * `1` — AES-256-GCM (32-byte key).
 */
export type SFrameCipherSuite = 0 | 1;
export const AES_128_GCM: SFrameCipherSuite = 0;
export const AES_256_GCM: SFrameCipherSuite = 1;

/**
 * Result returned by {@link SFrameSession.decrypt}.
 */
export interface SFrameDecryptResult {
    /** Decrypted Opus frame. */
    plaintext: Buffer;
    /** MLS leaf index of the sender. */
    senderLeaf: number;
}

/**
 * SFrame E2EE session for one MLS epoch.
 *
 * Derives `sframe_base_key` from the MLS `ExportSecret` and provides
 * {@link createEncryptor} and {@link decrypt} for send and receive paths.
 *
 * Create a new session after every MLS commit (epoch change).
 *
 * @example
 * ```ts
 * const session = SFrameSession.create(mls, "gbp/sframe v1");
 * const enc = session.createEncryptor(mls, myLeafIndex);
 *
 * // Sender side:
 * const payload = enc.encrypt(opusFrame);
 *
 * // Receiver side:
 * const { plaintext, senderLeaf } = session.decrypt(payload);
 * ```
 */
export class SFrameSession {
    private readonly handle: number;

    private constructor(handle: number) {
        this.handle = handle;
    }

    /**
     * Creates an SFrame session from the current MLS context.
     *
     * @param mls     The active MLS context.
     * @param label   Export label, e.g. `"gbp/sframe v1"`.
     * @param suite   Ciphersuite — `AES_128_GCM` (default) or `AES_256_GCM`.
     */
    static create(
        mls: MlsContext,
        label = "gbp/sframe v1",
        suite: SFrameCipherSuite = AES_128_GCM,
    ): SFrameSession {
        const labelBuf = Buffer.from(label, "utf8");
        const handle = N.gbp_sframe_session_create(
            mls.handle, suite, labelBuf, labelBuf.byteLength,
        ) as number;
        if (!handle) throw new Error(`gbp_sframe_session_create: ${N.lastError()}`);
        return new SFrameSession(handle);
    }

    /**
     * Creates a per-sender {@link SFrameEncryptor} for `leafIndex`.
     *
     * The encryptor holds the derived key+salt for this sender and maintains an
     * internal counter.  **Do not share across threads.**
     *
     * @param mls       Must be the same MLS context used in {@link create}.
     * @param leafIndex The local sender's MLS leaf index.
     * @param label     Must match the label used in {@link create}.
     * @param suite     Must match the suite used in {@link create}.
     */
    createEncryptor(
        mls: MlsContext,
        leafIndex: number,
        label = "gbp/sframe v1",
        suite: SFrameCipherSuite = AES_128_GCM,
    ): SFrameEncryptor {
        const labelBuf = Buffer.from(label, "utf8");
        const encHandle = N.gbp_sframe_encryptor_create(
            mls.handle, this.handle, leafIndex, suite,
            labelBuf, labelBuf.byteLength,
        ) as number;
        if (!encHandle)
            throw new Error(`gbp_sframe_encryptor_create: ${N.lastError()}`);
        return new SFrameEncryptor(encHandle);
    }

    /**
     * Decrypts one SFrame payload.
     *
     * @param payload  Full SFrame payload (header + ciphertext + tag).
     * @param extraAad Additional authenticated data passed on the sender side;
     *                 empty `Buffer` or `undefined` if none.
     */
    decrypt(payload: Buffer, extraAad?: Buffer): SFrameDecryptResult {
        const aad = extraAad ?? Buffer.alloc(0);
        const leafOut = new Uint32Array(1);
        const raw = N.gbp_sframe_decrypt(
            this.handle,
            payload, payload.byteLength,
            aad, aad.byteLength,
            leafOut,
        ) as N.GbpBuffer;
        const plaintext = N.takeBuffer(raw);
        if (plaintext.byteLength === 0 && payload.byteLength > 0)
            throw new Error(`gbp_sframe_decrypt: ${N.lastError()}`);
        return { plaintext, senderLeaf: leafOut[0] };
    }

    /** Frees the native session handle. */
    close(): void {
        N.gbp_sframe_session_free(this.handle);
    }

    [Symbol.dispose](): void {
        this.close();
    }
}

/**
 * Stateful per-sender SFrame encryptor.
 *
 * Maintains an internal counter that increments on every {@link encrypt} call.
 * Obtain via {@link SFrameSession.createEncryptor}.
 */
export class SFrameEncryptor {
    private readonly handle: number;

    constructor(handle: number) {
        this.handle = handle;
    }

    /**
     * Encrypts one audio frame.
     *
     * @param plaintext Raw Opus frame bytes.
     * @param extraAad  Additional authenticated data; pass `undefined` or an
     *                  empty `Buffer` if none.
     * @returns SFrame payload: `sframe_header ‖ ciphertext ‖ GCM-tag`.
     */
    encrypt(plaintext: Buffer, extraAad?: Buffer): Buffer {
        const aad = extraAad ?? Buffer.alloc(0);
        const raw = N.gbp_sframe_encrypt(
            this.handle,
            plaintext, plaintext.byteLength,
            aad, aad.byteLength,
        ) as N.GbpBuffer;
        const result = N.takeBuffer(raw);
        if (result.byteLength === 0 && plaintext.byteLength > 0)
            throw new Error(`gbp_sframe_encrypt: ${N.lastError()}`);
        return result;
    }

    /** Frees the native encryptor handle. */
    close(): void {
        N.gbp_sframe_encryptor_free(this.handle);
    }

    [Symbol.dispose](): void {
        this.close();
    }
}
