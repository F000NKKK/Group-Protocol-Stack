"""Group Text Protocol client wrapper."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Optional

from . import _native as _n
from ._native import PayloadCodec
from .gbp_node import GroupNode, OutboundFrame, _unpack
from .mls_context import MlsContext


@dataclass(frozen=True)
class GtpAcceptResult:
    """Outcome of :meth:`GtpClient.accept`."""

    status: str
    sender: Optional[int] = None
    message_id: Optional[int] = None
    text: Optional[str] = None
    reason: Optional[str] = None

    @classmethod
    def _parse(cls, json_text: str) -> "GtpAcceptResult":
        d = json.loads(json_text) if json_text else {}
        return cls(
            status=d.get("status", "?"),
            sender=d.get("sender"),
            message_id=d.get("message_id"),
            text=d.get("text"),
            reason=d.get("reason"),
        )


class GtpClient:
    """Group Text Protocol client.

    Tracks idempotency by ``(sender_id, message_id)``.
    """

    __slots__ = ("_handle",)

    def __init__(self, handle: int) -> None:
        self._handle = handle

    @classmethod
    def create(cls) -> "GtpClient":
        """Create a fresh GTP client."""
        h = _n.gtp_client_create()
        if h <= 0:
            raise OSError("gtp_client_create")
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
        message_id: int,
        text: str,
        codec: PayloadCodec = PayloadCodec.CBOR,
    ) -> OutboundFrame:
        """Send a text message.

        ``codec`` selects the payload encoding (default: CBOR).
        """
        data = text.encode("utf-8")
        def call(ptr, length):
            return _n.gtp_client_send(
                self._handle, node.handle, mls.handle, target, message_id, ptr, length, int(codec)
            )
        buf = _n.call_with_bytes(data, call)
        return _unpack(buf, "gtp_client_send")

    def accept(
        self, plaintext: bytes, current_epoch: int,
        codec: PayloadCodec = PayloadCodec.CBOR,
    ) -> GtpAcceptResult:
        """Accept a plaintext payload delivered by the GBP layer.

        ``current_epoch`` lets the client auto-reset its idempotency set
        when the epoch advances. ``codec`` must match the ``codec`` field
        of the ``payload_received`` event.
        """
        def call(ptr, length):
            return _n.gtp_client_accept(self._handle, current_epoch, ptr, length, int(codec))
        ptr = _n.call_with_bytes(plaintext, call)
        return GtpAcceptResult._parse(_n.take_cstring(ptr))

    def reset(self) -> None:
        """Clear the idempotency state. Intended for use after an epoch change."""
        _n.gtp_client_reset(self._handle)

    def close(self) -> None:
        """Release the native handle. Idempotent."""
        if self._handle:
            _n.gtp_client_destroy(self._handle)
            self._handle = 0

    def __enter__(self) -> "GtpClient":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass
