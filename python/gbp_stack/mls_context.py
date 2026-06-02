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
        """Invite ``key_package`` into the local group; returns the Welcome
        only. Use :meth:`invite_full` to also obtain the Commit message that
        must be broadcast to existing members (RFC 9420 §11/§12.4)."""
        def call(ptr, length):
            return _n.gbp_mls_invite(self._handle, ptr, length)
        buf = _n.call_with_bytes(key_package, call)
        out = _n.take_buffer(buf)
        if not out:
            raise OSError(f"invite: {_n.last_error()}")
        return out

    def invite_full(self, key_package: bytes) -> tuple[bytes, bytes]:
        """Invite ``key_package`` and return ``(commit_bytes, welcome_bytes)``.

        The Commit MUST be broadcast to existing members (embedded in
        ``PREPARE_TRANSITION`` args). The Welcome MUST be unicast to the
        new joiner.
        """
        def call(ptr, length):
            return _n.gbp_mls_invite_full(self._handle, ptr, length)
        buf = _n.call_with_bytes(key_package, call)
        out = _n.take_buffer(buf)
        if len(out) < 4:
            raise OSError(f"invite_full: {_n.last_error() or 'truncated'}")
        commit_len = int.from_bytes(out[:4], "little")
        if commit_len < 0 or 4 + commit_len > len(out):
            raise OSError("invite_full: bad commit_len")
        return bytes(out[4 : 4 + commit_len]), bytes(out[4 + commit_len :])

    def remove_member(self, leaf_index: int) -> bytes:
        """Remove the member at ``leaf_index`` and return the Commit bytes."""
        buf = _n.gbp_mls_remove(self._handle, leaf_index & 0xFFFFFFFF)
        out = _n.take_buffer(buf)
        if not out:
            raise OSError(f"remove: {_n.last_error()}")
        return out

    def process_message(self, message: bytes) -> str:
        """Apply a Commit (or staged Proposal) to the local MLS group.

        Returns one of ``"commit"``, ``"application"``, ``"proposal"``,
        ``"external"``.
        """
        def call(ptr, length):
            return _n.gbp_mls_process_message(self._handle, ptr, length)
        code = int(_n.call_with_bytes(message, call))
        kinds = {1: "commit", 2: "application", 3: "proposal", 4: "external"}
        if code not in kinds:
            raise OSError(f"process_message: {_n.last_error()}")
        return kinds[code]

    def finalize_commit(self) -> None:
        """Merge any pending commit produced by :meth:`invite_full` or
        :meth:`remove_member`. Idempotent."""
        if not _n.gbp_mls_finalize_commit(self._handle):
            raise OSError(f"finalize_commit: {_n.last_error()}")

    def clear_pending_commit(self) -> None:
        """Discard any pending commit without applying it (used on ABORT)."""
        if not _n.gbp_mls_clear_pending_commit(self._handle):
            raise OSError(f"clear_pending_commit: {_n.last_error()}")

    def accept_welcome(self, welcome: bytes) -> None:
        """Replace the local group with the one described by ``welcome``."""
        def call(ptr, length):
            return _n.gbp_mls_accept_welcome(self._handle, ptr, length)
        ok = _n.call_with_bytes(welcome, call)
        if not ok:
            raise OSError(f"accept_welcome: {_n.last_error()}")

    def export_state(self) -> bytes:
        """Serialise the full MLS state into an opaque blob that
        :meth:`restore_state` can reconstruct, so a consumer can persist the
        context across restarts. The blob contains **private key material** —
        store it encrypted at rest."""
        buf = _n.gbp_mls_export_state(self._handle)
        out = _n.take_buffer(buf)
        if not out:
            raise OSError(f"export_state: {_n.last_error()}")
        return out

    @classmethod
    def restore_state(cls, state: bytes, identity: str = "") -> "MlsContext":
        """Reconstruct a context from a blob produced by :meth:`export_state`.

        The restored context is at the same epoch / group state and can send
        and receive again. ``identity`` is informational (the real identity is
        inside the blob).
        """
        handle = _n.call_with_bytes(state, _n.gbp_mls_restore_state)
        if handle <= 0:
            raise OSError(f"restore_state: {_n.last_error()}")
        return cls(handle, identity)

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
