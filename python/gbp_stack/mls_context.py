"""MLS (RFC 9420) context wrapper."""

from __future__ import annotations

import ctypes

from . import _native as _n


class MlsContext:
    """Managed wrapper around an MLS context owned by the native library.

    Owns a single-member group plus a published ``KeyPackage`` that can be
    used to invite this member into another group. Always use as a context
    manager so the native handle is released.
    """

    __slots__ = ("_handle", "identity")

    def __init__(self, handle: int, identity: str) -> None:
        self._handle = handle
        self.identity = identity

    @classmethod
    def create(cls, identity: str) -> "MlsContext":
        """Create a fresh MLS context."""
        data = identity.encode("utf-8")
        handle = _n.call_with_bytes(data, _n.gbp_mls_create)
        if handle <= 0:
            raise OSError(f"gbp_mls_create: {_n.last_error()}")
        return cls(handle, identity)

    @property
    def handle(self) -> int:
        """Native handle (i32)."""
        return self._handle

    @property
    def epoch(self) -> int:
        """Current group epoch."""
        return int(_n.gbp_mls_epoch(self._handle))

    @property
    def group_id(self) -> bytes:
        """16-byte group identifier."""
        buf = (ctypes.c_uint8 * 16)()
        if not _n.gbp_mls_group_id(self._handle, ctypes.cast(buf, ctypes.c_void_p)):
            raise OSError(f"group_id: {_n.last_error()}")
        return bytes(buf)

    def export_key_package(self) -> bytes:
        """Export this member's TLS-serialised KeyPackage."""
        buf = _n.gbp_mls_export_key_package(self._handle)
        out = _n.take_buffer(buf)
        if not out:
            raise OSError(f"export_key_package: {_n.last_error()}")
        return out

    def invite(self, key_package: bytes) -> bytes:
        """Invite ``key_package`` into the local group; returns the Welcome."""
        def call(ptr, length):
            return _n.gbp_mls_invite(self._handle, ptr, length)
        buf = _n.call_with_bytes(key_package, call)
        out = _n.take_buffer(buf)
        if not out:
            raise OSError(f"invite: {_n.last_error()}")
        return out

    def accept_welcome(self, welcome: bytes) -> None:
        """Replace the local group with the one described by ``welcome``."""
        def call(ptr, length):
            return _n.gbp_mls_accept_welcome(self._handle, ptr, length)
        ok = _n.call_with_bytes(welcome, call)
        if not ok:
            raise OSError(f"accept_welcome: {_n.last_error()}")

    def close(self) -> None:
        """Release the native handle. Idempotent."""
        if self._handle:
            _n.gbp_mls_destroy(self._handle)
            self._handle = 0

    def __enter__(self) -> "MlsContext":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass
