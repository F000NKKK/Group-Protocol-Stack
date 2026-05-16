/**
 * Low-level bindings to the native ``gbp_stack`` shared library via
 * ``koffi``.
 *
 * The library is loaded from a platform-specific subdirectory of
 * ``native/`` (so it can be packaged inside the npm tarball), with a
 * fallback to the OS loader path. Call sites should not depend on the
 * symbols in this module directly — use the high-level wrappers exported
 * from ``index``.
 */

import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import koffi from "koffi";

function rid(): string {
    const arch = process.arch;
    if (process.platform === "win32") return arch === "arm64" ? "win-arm64" : "win-x64";
    if (process.platform === "darwin") return arch === "arm64" ? "osx-arm64" : "osx-x64";
    return arch === "arm64" ? "linux-arm64" : "linux-x64";
}

function libName(): string {
    if (process.platform === "win32") return "gbp_stack.dll";
    if (process.platform === "darwin") return "libgbp_stack.dylib";
    return "libgbp_stack.so";
}

function candidates(): string[] {
    const here = resolve(__dirname, "..");
    const r = rid();
    const name = libName();
    return [
        join(here, "native", r, name),
        join(here, "native", name),
        name,
    ];
}

function load(): koffi.IKoffiLib {
    let lastErr: unknown;
    for (const path of candidates()) {
        try {
            if (path.includes("/") || path.includes("\\")) {
                if (!existsSync(path)) continue;
            }
            return koffi.load(path);
        } catch (e) {
            lastErr = e;
        }
    }
    throw new Error(
        "failed to load native gbp_stack library; tried: " +
        candidates().join(", ") +
        (lastErr ? `; last error: ${String(lastErr)}` : "")
    );
}

const lib = load();

// FFI buffer (ptr, len, cap)
export const GbpBuffer = koffi.struct("GbpBuffer", {
    ptr: "void *",
    len: "size_t",
    cap: "size_t",
});
export type GbpBuffer = { ptr: number | null; len: number; cap: number };

// Memory protocol
export const gbp_buffer_free = lib.func("void gbp_buffer_free(GbpBuffer)");
export const gbp_string_free = lib.func("void gbp_string_free(void *)");
export const gbp_last_error = lib.func("void *gbp_last_error()");
export const gbp_version = lib.func("void *gbp_version()");

// MLS
export const gbp_mls_create = lib.func("int32_t gbp_mls_create(void *, size_t)");
export const gbp_mls_destroy = lib.func("void gbp_mls_destroy(int32_t)");
export const gbp_mls_epoch = lib.func("uint64_t gbp_mls_epoch(int32_t)");
export const gbp_mls_group_id = lib.func("bool gbp_mls_group_id(int32_t, void *)");
export const gbp_mls_export_key_package = lib.func("GbpBuffer gbp_mls_export_key_package(int32_t)");
export const gbp_mls_invite = lib.func("GbpBuffer gbp_mls_invite(int32_t, void *, size_t)");
export const gbp_mls_invite_full = lib.func("GbpBuffer gbp_mls_invite_full(int32_t, void *, size_t)");
export const gbp_mls_remove = lib.func("GbpBuffer gbp_mls_remove(int32_t, uint32_t)");
export const gbp_mls_process_message = lib.func("uint32_t gbp_mls_process_message(int32_t, void *, size_t)");
export const gbp_mls_finalize_commit = lib.func("bool gbp_mls_finalize_commit(int32_t)");
export const gbp_mls_clear_pending_commit = lib.func("bool gbp_mls_clear_pending_commit(int32_t)");
export const gbp_mls_accept_welcome = lib.func("bool gbp_mls_accept_welcome(int32_t, void *, size_t)");

// GBP node
export const gbp_node_create = lib.func("int32_t gbp_node_create(uint32_t, void *)");
export const gbp_node_destroy = lib.func("void gbp_node_destroy(int32_t)");
export const gbp_node_bootstrap_creator = lib.func("bool gbp_node_bootstrap_creator(int32_t, uint64_t)");
export const gbp_node_bootstrap_joiner = lib.func("bool gbp_node_bootstrap_joiner(int32_t, uint64_t, uint32_t)");
export const gbp_node_state = lib.func("uint32_t gbp_node_state(int32_t)");
export const gbp_node_epoch = lib.func("uint64_t gbp_node_epoch(int32_t)");
export const gbp_node_last_transition_id = lib.func("uint32_t gbp_node_last_transition_id(int32_t)");
export const gbp_node_set_epoch = lib.func("bool gbp_node_set_epoch(int32_t, uint64_t)");
export const gbp_node_apply_transition = lib.func("bool gbp_node_apply_transition(int32_t, uint32_t)");
export const gbp_node_send_control = lib.func(
    "GbpBuffer gbp_node_send_control(int32_t, int32_t, uint32_t, uint16_t, uint32_t, uint32_t, void *, size_t)"
);
export const gbp_node_on_wire = lib.func(
    "void *gbp_node_on_wire(int32_t, int32_t, void *, size_t)"
);
export const gbp_node_drain_events = lib.func("void *gbp_node_drain_events(int32_t)");

// GTP client
export const gtp_client_create = lib.func("int32_t gtp_client_create()");
export const gtp_client_destroy = lib.func("void gtp_client_destroy(int32_t)");
export const gtp_client_reset = lib.func("void gtp_client_reset(int32_t)");
export const gtp_client_send = lib.func(
    "GbpBuffer gtp_client_send(int32_t, int32_t, int32_t, uint32_t, uint64_t, void *, size_t)"
);
export const gtp_client_accept = lib.func("void *gtp_client_accept(int32_t, uint64_t, void *, size_t)");

// GAP client
export const gap_client_create = lib.func("int32_t gap_client_create()");
export const gap_client_destroy = lib.func("void gap_client_destroy(int32_t)");
export const gap_client_reset = lib.func("void gap_client_reset(int32_t)");
export const gap_client_send = lib.func(
    "GbpBuffer gap_client_send(int32_t, int32_t, int32_t, uint32_t, uint32_t, uint64_t, void *, size_t)"
);
export const gap_client_accept = lib.func("void *gap_client_accept(int32_t, uint64_t, void *, size_t)");

// GSP client
export const gsp_client_create = lib.func("int32_t gsp_client_create()");
export const gsp_client_destroy = lib.func("void gsp_client_destroy(int32_t)");
export const gsp_client_reset = lib.func("void gsp_client_reset(int32_t)");
export const gsp_client_send = lib.func(
    "GbpBuffer gsp_client_send(int32_t, int32_t, int32_t, uint32_t, uint32_t, uint32_t, uint32_t)"
);
export const gsp_client_accept = lib.func("void *gsp_client_accept(int32_t, uint64_t, void *, size_t)");

// SFrame session / encryptor
export const gbp_sframe_session_create = lib.func(
    "int32_t gbp_sframe_session_create(int32_t, uint8_t, void *, size_t)"
);
export const gbp_sframe_session_free = lib.func("void gbp_sframe_session_free(int32_t)");
export const gbp_sframe_encryptor_create = lib.func(
    "int32_t gbp_sframe_encryptor_create(int32_t, int32_t, uint32_t, uint8_t, void *, size_t)"
);
export const gbp_sframe_encryptor_free = lib.func("void gbp_sframe_encryptor_free(int32_t)");
export const gbp_sframe_encrypt = lib.func(
    "GbpBuffer gbp_sframe_encrypt(int32_t, void *, size_t, void *, size_t)"
);
export const gbp_sframe_decrypt = lib.func(
    "GbpBuffer gbp_sframe_decrypt(int32_t, void *, size_t, void *, size_t, _Out_ uint32_t *)"
);

/** Copy a returned ``GbpBuffer`` into a ``Buffer`` and free it. */
export function takeBuffer(buf: GbpBuffer): Buffer {
    if (!buf.ptr || buf.len === 0) {
        gbp_buffer_free(buf);
        return Buffer.alloc(0);
    }
    const out = Buffer.from(koffi.decode(buf.ptr, "uint8_t", buf.len) as Uint8Array);
    gbp_buffer_free(buf);
    return out;
}

/** Copy a returned C-string into a ``string`` and free it. */
export function takeCString(ptr: number | null): string {
    if (!ptr) return "";
    try {
        return koffi.decode(ptr, "string");
    } finally {
        gbp_string_free(ptr);
    }
}

/** Last FFI error on this thread. */
export function lastError(): string {
    return takeCString(gbp_last_error());
}

/** Native library version. */
export function version(): string {
    return takeCString(gbp_version());
}
