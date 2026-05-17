"""Group Signaling Protocol client wrapper."""

from __future__ import annotations

import enum
import json
from dataclasses import dataclass
from typing import Optional

from . import _native as _n
from ._native import PayloadCodec
from .gbp_node import GroupNode, OutboundFrame, _unpack
from .mls_context import MlsContext


class SignalType(enum.IntEnum):
    """Signal opcode registry."""

    JOIN = 100
    LEAVE = 101
    ROLE_CHANGE = 102
    MUTE = 200
    UNMUTE = 201
    STREAM_START = 300
    STREAM_STOP = 301
    CODEC_UPDATE = 400


@dataclass(frozen=True)
class GspAcceptResult:
    """Outcome of :meth:`GspClient.accept`."""

    status: str
    signal: Optional[str] = None
    signal_code: Optional[SignalType] = None
    sender: Optional[int] = None
    role_claim: Optional[int] = None
    request_id: Optional[int] = None
    reason: Optional[str] = None

    @classmethod
    def _parse(cls, json_text: str) -> "GspAcceptResult":
        d = json.loads(json_text) if json_text else {}
        sc = d.get("signal_code")
        return cls(
            status=d.get("status", "?"),
            signal=d.get("signal"),
            signal_code=SignalType(sc) if sc is not None else None,
            sender=d.get("sender"),
            role_claim=d.get("role_claim"),
            request_id=d.get("request_id"),
            reason=d.get("reason"),
        )


class GspClient:
    """Group Signaling Protocol client.

    Tracks ``request_id`` deduplication and maintains the local membership
    and mute-list state.
    """

    __slots__ = ("_handle",)

    def __init__(self, handle: int) -> None:
        self._handle = handle

    @classmethod
    def create(cls) -> "GspClient":
        """Create a fresh GSP client."""
        h = _n.gsp_client_create()
        if h <= 0:
            raise OSError("gsp_client_create")
        return cls(h)

    @property
    def handle(self) -> int:
        """Native handle (i32)."""
        return self._handle

    def send(
        self,
        node: GroupNode,
        mls: MlsContext,
        target: int,
        signal: SignalType,
        role_claim: int,
        request_id: int,
        args: bytes = b"",
        codec: PayloadCodec = PayloadCodec.CBOR,
    ) -> OutboundFrame:
        """Send a signal.

        ``args`` carries opcode-specific CBOR-encoded arguments required by
        signals such as MUTE/UNMUTE (``{0: target_member_id}``),
        ROLE_CHANGE (``{0: target_member_id, 1: new_role_id}``), etc.
        ``codec`` selects the payload encoding (default: CBOR).
        """
        def do_send(ptr, length):
            return _n.gsp_client_send_with_args(
                self._handle, node.handle, mls.handle, target,
                int(signal), role_claim, request_id, ptr, length, int(codec),
            )
        buf = _n.call_with_bytes(args, do_send)
        return _unpack(buf, "gsp_client_send")

    def accept(
        self, plaintext: bytes, current_epoch: int,
        codec: PayloadCodec = PayloadCodec.CBOR,
    ) -> GspAcceptResult:
        """Accept a plaintext payload delivered by the GBP layer.

        ``current_epoch`` lets the client auto-reset its dedup state
        when the epoch advances. ``codec`` must match the ``codec`` field
        of the ``payload_received`` event.
        """
        def call(ptr, length):
            return _n.gsp_client_accept(self._handle, current_epoch, ptr, length, int(codec))
        ptr = _n.call_with_bytes(plaintext, call)
        return GspAcceptResult._parse(_n.take_cstring(ptr))

    def reset(self) -> None:
        """Clear the request-id deduplication set. Intended for use after an epoch change."""
        _n.gsp_client_reset(self._handle)

    def close(self) -> None:
        """Release the native handle. Idempotent."""
        if self._handle:
            _n.gsp_client_destroy(self._handle)
            self._handle = 0

    def __enter__(self) -> "GspClient":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass
