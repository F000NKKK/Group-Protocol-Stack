/** GBP-layer group node wrapper (the IP-like base). */

import * as N from "./native";
import { PayloadCodec } from "./native";
import { MlsContext } from "./mls";

/** Stream class. */
export enum StreamType {
    Control = 0,
    Audio = 1,
    Text = 2,
    Signal = 3,
}

/** Node FSM state. */
export enum NodeState {
    Idle = 0,
    Connecting,
    EstablishingGroup,
    Active,
    Resyncing,
    Failed,
    Closed,
}

/** Control plane opcode. */
export enum ControlOpcode {
    PrepareTransition = 0x0001,
    ReadyForTransition = 0x0002,
    ExecuteTransition = 0x0003,
    AbortTransition = 0x0004,
    GroupStateDigestRequest = 0x0005,
    GroupStateDigestResponse = 0x0006,
    ReportInvalidCommit = 0x0007,
    CapabilitiesAdvertise = 0x0008,
    Ack = 0x0009,
    Nack = 0x000A,
}

/** Wire frame produced by the native library. */
export interface OutboundFrame {
    target: number;
    wire: Buffer;
}

/**
 * Event surfaced by the GBP layer.
 *
 * `kind` determines which optional fields are set:
 * - `state_changed` — `fromState`, `toState`
 * - `payload_received` — `streamTypeName`, `streamType`, `streamId`, `sequenceNo`, `flags`, `plaintext`
 * - `control` — `sender`, `opcode`, `opcodeCode`, `transitionId`
 * - `error` — `code`, `codeHex`, `classCode`, `retryable`, `fatal`, `reason`
 * - `epoch_advanced` — `epoch`, `transitionId`
 * - `coordinator_election_needed` — no extra fields; the local node should start the coordinator-election handshake
 * - `became_coordinator` — no extra fields; this node won the election
 * - `coordinator_claim` — `claimant` (member id of the peer that sent COORDINATOR_CLAIM)
 */
export interface NodeEvent {
    kind: string;
    fromState?: string;
    toState?: string;
    streamTypeName?: string;
    streamType?: StreamType;
    streamId?: number;
    sequenceNo?: number;
    flags?: number;
    codec?: PayloadCodec;
    plaintext?: Buffer;
    sender?: number;
    opcode?: string;
    opcodeCode?: number;
    transitionId?: number;
    code?: number;
    codeHex?: string;
    classCode?: number;
    retryable?: boolean;
    fatal?: boolean;
    reason?: string;
    epoch?: bigint;
    claimant?: number;
}

function parseEvents(json: string): NodeEvent[] {
    if (!json || json === "[]") return [];
    const arr = JSON.parse(json) as Array<Record<string, unknown>>;
    return arr.map((d): NodeEvent => {
        const num = (k: string) => (typeof d[k] === "number" ? (d[k] as number) : undefined);
        const big = (k: string) =>
            typeof d[k] === "number" ? BigInt(d[k] as number) :
            typeof d[k] === "bigint" ? (d[k] as bigint) : undefined;
        const str = (k: string) => (typeof d[k] === "string" ? (d[k] as string) : undefined);
        const bool = (k: string) => (typeof d[k] === "boolean" ? (d[k] as boolean) : undefined);
        const stCode = num("stream_type_code");
        const b64 = str("plaintext_b64");
        return {
            kind: String(d["kind"]),
            fromState: str("from"),
            toState: str("to"),
            streamTypeName: str("stream_type"),
            streamType: stCode !== undefined ? (stCode as StreamType) : undefined,
            streamId: num("stream_id"),
            sequenceNo: num("sequence_no"),
            flags: num("flags"),
            codec: num("codec") !== undefined ? (num("codec") as PayloadCodec) : undefined,
            plaintext: b64 ? Buffer.from(b64, "base64") : undefined,
            sender: typeof d["from"] === "number" ? (d["from"] as number) : undefined,
            opcode: str("opcode"),
            opcodeCode: num("opcode_code"),
            transitionId: num("transition_id"),
            code: num("code"),
            codeHex: str("code_hex"),
            classCode: num("class"),
            retryable: bool("retryable"),
            fatal: bool("fatal"),
            reason: str("reason"),
            epoch: big("epoch"),
            claimant: num("claimant"),
        };
    });
}

/**
 * Encode a raw GBP frame to CBOR bytes.
 *
 * Low-level helper — most callers should use {@link GroupNode.sendControl}
 * or the sub-protocol `send` methods instead.
 */
export function encodeGbpFrame(
    version: number,
    groupId: Buffer,
    epoch: bigint | number,
    transitionId: number,
    streamType: number,
    streamId: number,
    flags: number,
    sequenceNo: number,
    payload: Buffer,
): Buffer {
    if (groupId.length !== 16) throw new Error("groupId must be 16 bytes");
    const buf = N.gbp_frame_encode_v(
        version, groupId, BigInt(epoch), transitionId, streamType,
        streamId, flags, sequenceNo, payload, payload.length,
    ) as N.GbpBuffer;
    return N.takeBuffer(buf);
}

/**
 * Return the CBOR-encoded `ErrorObject` for `code`, or `null` if unknown.
 */
export function lookupError(code: number): Buffer | null {
    const buf = N.gbp_error_lookup(code) as N.GbpBuffer;
    const data = N.takeBuffer(buf);
    return data.length > 0 ? data : null;
}

/** @internal Decode the (target, wire) pair returned by ``send_*``. */
export function unpackOutbound(buf: N.GbpBuffer, what: string): OutboundFrame {
    const raw = N.takeBuffer(buf);
    if (raw.length === 0) throw new Error(`${what}: ${N.lastError()}`);
    if (raw.length < 4) throw new Error(`${what}: buffer too short`);
    const target = raw.readUInt32LE(0);
    return { target, wire: raw.subarray(4) };
}

/**
 * GBP-layer group node.
 *
 * Owns the framing, AEAD, replay window, FSM and control plane.
 * Sub-protocol semantics live in {@link GtpClient}, {@link GapClient} and
 * {@link GspClient}.
 */
export class GroupNode {
    /** Native handle (i32). */
    public handle: number;

    /** This node's member id. */
    public readonly memberId: number;

    private readonly _groupId: Buffer;

    private constructor(handle: number, memberId: number, groupId: Buffer) {
        this.handle = handle;
        this.memberId = memberId;
        this._groupId = Buffer.from(groupId);
    }

    /** Create a node bound to ``groupId`` (which MUST be 16 bytes). */
    static create(memberId: number, groupId: Buffer): GroupNode {
        if (groupId.length !== 16) throw new Error("group_id must be 16 bytes");
        const h = N.gbp_node_create(memberId, groupId) as number;
        if (h <= 0) throw new Error(`node_create: ${N.lastError()}`);
        return new GroupNode(h, memberId, groupId);
    }

    /** Current FSM state. */
    get state(): NodeState { return N.gbp_node_state(this.handle) as NodeState; }

    /** Current node epoch. */
    get epoch(): bigint { return BigInt(N.gbp_node_epoch(this.handle) as number | bigint); }

    /** Last applied ``transition_id``. */
    get lastTransitionId(): number { return N.gbp_node_last_transition_id(this.handle) as number; }

    /** 16-byte group identifier. */
    get groupId(): Buffer { return Buffer.from(this._groupId); }

    /** Drive the node from ``IDLE`` to ``ACTIVE`` as a creator. */
    bootstrapAsCreator(epoch: bigint | number): void {
        if (!(N.gbp_node_bootstrap_creator(this.handle, BigInt(epoch)) as boolean))
            throw new Error(N.lastError());
    }

    /**
     * Drive the node from ``IDLE`` to ``ACTIVE`` as a joiner.
     *
     * `expectedFirstTid` pre-arms `pending_transition_id` so the next
     * `EXECUTE_TRANSITION` is accepted by `handle_control`'s validation
     * matrix. The matching PREPARE was sealed under the pre-Welcome MLS
     * epoch and is therefore undecryptable to the joiner; the joiner is
     * brought into the group when EXECUTE arrives on the new shared epoch.
     * Pass `0` if the joiner recovered out-of-band and is already current.
     */
    bootstrapAsJoiner(epoch: bigint | number, expectedFirstTid: number = 0): void {
        if (!(N.gbp_node_bootstrap_joiner(this.handle, BigInt(epoch), expectedFirstTid >>> 0) as boolean))
            throw new Error(N.lastError());
    }

    /** Forcibly override ``current_epoch`` (intended for tests of late peers). */
    setEpochForTesting(epoch: bigint | number): void {
        if (!(N.gbp_node_set_epoch(this.handle, BigInt(epoch)) as boolean))
            throw new Error(N.lastError());
    }

    /** Apply an epoch transition locally. */
    applyTransition(transitionId: number): void {
        if (!(N.gbp_node_apply_transition(this.handle, transitionId) as boolean))
            throw new Error(N.lastError());
    }

    /** Send a control plane message on Stream 0. */
    sendControl(
        mls: MlsContext,
        target: number,
        opcode: ControlOpcode,
        transitionId: number,
        requestId: number,
        args: Buffer = Buffer.alloc(0),
    ): OutboundFrame {
        const buf = N.gbp_node_send_control(
            this.handle, mls.handle, target, opcode, transitionId, requestId,
            args, args.length,
        ) as N.GbpBuffer;
        return unpackOutbound(buf, "send_control");
    }

    /** Feed wire bytes to the node and return the resulting events. */
    onWire(mls: MlsContext, wire: Buffer): NodeEvent[] {
        const ptr = N.gbp_node_on_wire(this.handle, mls.handle, wire, wire.length) as number;
        return parseEvents(N.takeCString(ptr));
    }

    /** Drain queued events without consuming any wire bytes. */
    drainEvents(): NodeEvent[] {
        const ptr = N.gbp_node_drain_events(this.handle) as number;
        return parseEvents(N.takeCString(ptr));
    }

    /** Release the native handle. Idempotent. */
    close(): void {
        if (this.handle) {
            N.gbp_node_destroy(this.handle);
            this.handle = 0;
        }
    }

    [Symbol.dispose](): void { this.close(); }
}
