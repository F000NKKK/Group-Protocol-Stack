"""Low-level ctypes bindings to the native ``gbp_stack`` shared library.

The library is loaded from a platform-specific subdirectory of
``gbp_stack/_native/`` (so it can be packaged inside the wheel), with a
fallback to the OS loader path. Call sites should not depend on the symbols
in this module directly — use the high-level wrappers in :mod:`gbp_stack`.
"""

from __future__ import annotations

import ctypes
import os
import platform
from ctypes import (
    Structure,
    c_bool,
    c_char_p,
    c_int32,
    c_size_t,
    c_uint8,
    c_uint16,
    c_uint32,
    c_uint64,
    c_void_p,
)
from typing import Iterable


def _candidate_paths() -> Iterable[str]:
    here = os.path.dirname(os.path.abspath(__file__))
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "windows":
        rid = "win-x64" if machine in ("amd64", "x86_64") else f"win-{machine}"
        name = "gbp_stack.dll"
    elif system == "darwin":
        rid = "osx-arm64" if machine == "arm64" else "osx-x64"
        name = "libgbp_stack.dylib"
    else:
        rid = "linux-x64" if machine in ("x86_64", "amd64") else f"linux-{machine}"
        name = "libgbp_stack.so"
    yield os.path.join(here, "_native", rid, name)
    yield os.path.join(here, "_native", name)
    yield name


def _load() -> ctypes.CDLL:
    last_err: OSError | None = None
    for path in _candidate_paths():
        try:
            return ctypes.CDLL(path)
        except OSError as e:
            last_err = e
    raise OSError(
        "failed to load native gbp_stack library; tried: "
        + ", ".join(_candidate_paths())
        + (f"; last error: {last_err}" if last_err else "")
    )


_lib = _load()


class GbpBuffer(Structure):
    """``(ptr, len, cap)`` triple matching the FFI ``GbpBuffer`` struct."""

    _fields_ = [
        ("ptr", c_void_p),
        ("len", c_size_t),
        ("cap", c_size_t),
    ]


def _bind(name: str, restype, argtypes):
    fn = getattr(_lib, name)
    fn.restype = restype
    fn.argtypes = argtypes
    return fn


gbp_buffer_free = _bind("gbp_buffer_free", None, [GbpBuffer])
gbp_string_free = _bind("gbp_string_free", None, [c_void_p])
_gbp_last_error = _bind("gbp_last_error", c_void_p, [])
_gbp_version = _bind("gbp_version", c_void_p, [])

gbp_mls_create = _bind("gbp_mls_create", c_int32, [c_void_p, c_size_t])
gbp_mls_destroy = _bind("gbp_mls_destroy", None, [c_int32])
gbp_mls_epoch = _bind("gbp_mls_epoch", c_uint64, [c_int32])
gbp_mls_group_id = _bind("gbp_mls_group_id", c_bool, [c_int32, c_void_p])
gbp_mls_export_key_package = _bind("gbp_mls_export_key_package", GbpBuffer, [c_int32])
gbp_mls_invite = _bind("gbp_mls_invite", GbpBuffer, [c_int32, c_void_p, c_size_t])
gbp_mls_accept_welcome = _bind(
    "gbp_mls_accept_welcome", c_bool, [c_int32, c_void_p, c_size_t]
)

gbp_node_create = _bind("gbp_node_create", c_int32, [c_uint32, c_void_p])
gbp_node_destroy = _bind("gbp_node_destroy", None, [c_int32])
gbp_node_bootstrap_creator = _bind("gbp_node_bootstrap_creator", c_bool, [c_int32, c_uint64])
gbp_node_bootstrap_joiner = _bind("gbp_node_bootstrap_joiner", c_bool, [c_int32, c_uint64])
gbp_node_state = _bind("gbp_node_state", c_uint32, [c_int32])
gbp_node_epoch = _bind("gbp_node_epoch", c_uint64, [c_int32])
gbp_node_last_transition_id = _bind("gbp_node_last_transition_id", c_uint32, [c_int32])
gbp_node_set_epoch = _bind("gbp_node_set_epoch", c_bool, [c_int32, c_uint64])
gbp_node_apply_transition = _bind("gbp_node_apply_transition", c_bool, [c_int32, c_uint32])
gbp_node_send_control = _bind(
    "gbp_node_send_control",
    GbpBuffer,
    [c_int32, c_int32, c_uint32, c_uint16, c_uint32, c_uint32, c_void_p, c_size_t],
)
gbp_node_on_wire = _bind(
    "gbp_node_on_wire", c_void_p, [c_int32, c_int32, c_void_p, c_size_t]
)
gbp_node_drain_events = _bind("gbp_node_drain_events", c_void_p, [c_int32])

gtp_client_create = _bind("gtp_client_create", c_int32, [])
gtp_client_destroy = _bind("gtp_client_destroy", None, [c_int32])
gtp_client_reset = _bind("gtp_client_reset", None, [c_int32])
gtp_client_send = _bind(
    "gtp_client_send",
    GbpBuffer,
    [c_int32, c_int32, c_int32, c_uint32, c_uint64, c_void_p, c_size_t],
)
gtp_client_accept = _bind("gtp_client_accept", c_void_p, [c_int32, c_void_p, c_size_t])

gap_client_create = _bind("gap_client_create", c_int32, [])
gap_client_destroy = _bind("gap_client_destroy", None, [c_int32])
gap_client_reset = _bind("gap_client_reset", None, [c_int32])
gap_client_send = _bind(
    "gap_client_send",
    GbpBuffer,
    [c_int32, c_int32, c_int32, c_uint32, c_uint32, c_uint64, c_void_p, c_size_t],
)
gap_client_accept = _bind(
    "gap_client_accept", c_void_p, [c_int32, c_uint64, c_void_p, c_size_t]
)

gsp_client_create = _bind("gsp_client_create", c_int32, [])
gsp_client_destroy = _bind("gsp_client_destroy", None, [c_int32])
gsp_client_reset = _bind("gsp_client_reset", None, [c_int32])
gsp_client_send = _bind(
    "gsp_client_send",
    GbpBuffer,
    [c_int32, c_int32, c_int32, c_uint32, c_uint32, c_uint32, c_uint32],
)
gsp_client_accept = _bind("gsp_client_accept", c_void_p, [c_int32, c_void_p, c_size_t])


def take_buffer(buf: GbpBuffer) -> bytes:
    """Copy a returned :class:`GbpBuffer` into a ``bytes`` object and free it."""
    if not buf.ptr or buf.len == 0:
        gbp_buffer_free(buf)
        return b""
    raw = ctypes.string_at(buf.ptr, buf.len)
    gbp_buffer_free(buf)
    return raw


def take_cstring(ptr: int) -> str:
    """Copy a returned C string into a ``str`` and free it."""
    if not ptr:
        return ""
    try:
        raw = ctypes.cast(ptr, c_char_p).value
        return raw.decode("utf-8") if raw is not None else ""
    finally:
        gbp_string_free(ptr)


def last_error() -> str:
    """Return the text of the last FFI error on this thread."""
    return take_cstring(_gbp_last_error())


def version() -> str:
    """Return the native library version string."""
    return take_cstring(_gbp_version())


def call_with_bytes(data: bytes, fn):
    """Invoke ``fn(ptr, length)`` with a temporary ctypes buffer.

    The buffer's lifetime is bounded to this call; ``fn`` MUST NOT retain
    ``ptr`` past its return.
    """
    length = len(data)
    if length == 0:
        return fn(None, 0)
    arr = (c_uint8 * length).from_buffer_copy(data)
    return fn(ctypes.cast(arr, c_void_p), length)
