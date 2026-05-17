"""Group Audio Protocol client wrapper."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Optional

from . import _native as _n
from ._native import PayloadCodec
from .gbp_node import GroupNode, OutboundFrame, _unpack
from .mls_context import MlsContext


@dataclass(frozen=True)
class GapAcceptResult:
    """Outcome of :meth:`GapClient.accept`."""

    status: str
    source: Optional[int] = None
    seq: Optional[int] = None
    bytes_: Optional[int] = None
    reason: Optional[str] = None

    @classmethod
    def _parse(cls, json_text: str) -> "GapAcceptResult":
        d = json.loads(json_text) if json_text else {}
        return cls(
            status=d.get("status", "?"),
            source=d.get("source"),
            seq=d.get("seq"),
            bytes_=d.get("bytes"),
            reason=d.get("reason"),
        )


class GapClient:
    """Group Audio Protocol client.

    Maintains a per-source ``rtp_sequence`` window and validates ``key_phase``
    against the current group epoch.
    """

    __slots__ = ("_handle",)

    def __init__(self, handle: int) -> None:
        self._handle = handle

    @classmethod
    def create(cls) -> "GapClient":
        """Create a fresh GAP client."""
        h = _n.gap_client_create()
        if h <= 0:
            raise OSError("gap_client_create")
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
        media_source_id: int,
        rtp_timestamp: int,
        opus: bytes,
        codec: PayloadCodec = PayloadCodec.CBOR,
    ) -> OutboundFrame:
        """Send an Opus audio frame.

        ``codec`` selects the payload encoding; use ``PayloadCodec.FLATBUFFERS``
        for lowest decode latency.
        """
        def call(ptr, length):
            return _n.gap_client_send(
                self._handle, node.handle, mls.handle, target,
                media_source_id, rtp_timestamp, ptr, length, int(codec),
            )
        buf = _n.call_with_bytes(opus, call)
        return _unpack(buf, "gap_client_send")

    def accept(
        self, plaintext: bytes, current_epoch: int,
        codec: PayloadCodec = PayloadCodec.CBOR,
    ) -> GapAcceptResult:
        """Accept a plaintext payload delivered by the GBP layer.

        ``codec`` must match the ``codec`` field of the ``payload_received`` event.
        """
        def call(ptr, length):
            return _n.gap_client_accept(self._handle, current_epoch, ptr, length, int(codec))
        ptr = _n.call_with_bytes(plaintext, call)
        return GapAcceptResult._parse(_n.take_cstring(ptr))

    def reset(self) -> None:
        """Clear the replay window. Intended for use after an epoch change."""
        _n.gap_client_reset(self._handle)

    def close(self) -> None:
        """Release the native handle. Idempotent."""
        if self._handle:
            _n.gap_client_destroy(self._handle)
            self._handle = 0

    def __enter__(self) -> "GapClient":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass
