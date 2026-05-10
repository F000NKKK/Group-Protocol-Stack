"""GBP-layer group node wrapper (the IP-like base)."""

from __future__ import annotations

import base64
import ctypes
import enum
import json
from dataclasses import dataclass
from typing import List, Optional

from . import _native as _n
from .mls_context import MlsContext


class StreamType(enum.IntEnum):
    """Stream class."""

    CONTROL = 0
    AUDIO = 1
    TEXT = 2
    SIGNAL = 3


class NodeState(enum.IntEnum):
    """Node FSM state."""

    IDLE = 0
    CONNECTING = 1
    ESTABLISHING_GROUP = 2
    ACTIVE = 3
    RESYNCING = 4
    FAILED = 5
    CLOSED = 6


class ControlOpcode(enum.IntEnum):
    """Control plane opcode."""

    PREPARE_TRANSITION = 0x0001
    READY_FOR_TRANSITION = 0x0002
    EXECUTE_TRANSITION = 0x0003
    ABORT_TRANSITION = 0x0004
    GROUP_STATE_DIGEST_REQUEST = 0x0005
    GROUP_STATE_DIGEST_RESPONSE = 0x0006
    REPORT_INVALID_COMMIT = 0x0007
    CAPABILITIES_ADVERTISE = 0x0008
    ACK = 0x0009
    NACK = 0x000A


@dataclass(frozen=True)
class OutboundFrame:
    """Wire frame produced by the native library."""

    target: int
    wire: bytes


@dataclass
class NodeEvent:
    """Event surfaced by the GBP layer.

    ``kind`` tells which fields are populated; common kinds are:

    * ``state_changed`` — populates ``from_state`` and ``to_state``;
    * ``payload_received`` — populates ``stream_type``, ``stream_id``,
      ``sequence_no``, ``flags`` and ``plaintext``;
    * ``control`` — populates ``sender``, ``opcode`` and ``transition_id``;
    * ``error`` — populates ``code``, ``code_hex``, ``class_``, ``retryable``,
      ``fatal`` and ``reason``;
    * ``epoch_advanced`` — populates ``epoch`` and ``transition_id``.
    """

    kind: str
    from_state: Optional[str] = None
    to_state: Optional[str] = None
    stream_type_name: Optional[str] = None
    stream_type: Optional[StreamType] = None
    stream_id: Optional[int] = None
    sequence_no: Optional[int] = None
    flags: Optional[int] = None
    plaintext: Optional[bytes] = None
    sender: Optional[int] = None
    opcode: Optional[str] = None
    opcode_code: Optional[int] = None
    transition_id: Optional[int] = None
    code: Optional[int] = None
    code_hex: Optional[str] = None
    class_: Optional[int] = None
    retryable: Optional[bool] = None
    fatal: Optional[bool] = None
    reason: Optional[str] = None
    epoch: Optional[int] = None

    @classmethod
    def _from_dict(cls, d: dict) -> "NodeEvent":
        st_code = d.get("stream_type_code")
        plaintext_b64 = d.get("plaintext_b64")
        return cls(
            kind=d["kind"],
            from_state=d.get("from"),
            to_state=d.get("to"),
            stream_type_name=d.get("stream_type"),
            stream_type=StreamType(st_code) if st_code is not None else None,
            stream_id=d.get("stream_id"),
            sequence_no=d.get("sequence_no"),
            flags=d.get("flags"),
            plaintext=base64.b64decode(plaintext_b64) if plaintext_b64 else None,
            sender=d.get("from") if isinstance(d.get("from"), int) else None,
            opcode=d.get("opcode"),
            opcode_code=d.get("opcode_code"),
            transition_id=d.get("transition_id"),
            code=d.get("code"),
            code_hex=d.get("code_hex"),
            class_=d.get("class"),
            retryable=d.get("retryable"),
            fatal=d.get("fatal"),
            reason=d.get("reason"),
            epoch=d.get("epoch"),
        )


def _parse_events(json_text: str) -> List[NodeEvent]:
    if not json_text or json_text == "[]":
        return []
    return [NodeEvent._from_dict(d) for d in json.loads(json_text)]


def _unpack(buf: _n.GbpBuffer, what: str) -> OutboundFrame:
    raw = _n.take_buffer(buf)
    if not raw:
        raise OSError(f"{what}: {_n.last_error()}")
    if len(raw) < 4:
        raise OSError(f"{what}: buffer too short")
    target = int.from_bytes(raw[:4], byteorder="little", signed=False)
    return OutboundFrame(target=target, wire=raw[4:])


class GroupNode:
    """GBP-layer group node.

    Owns the framing, AEAD, replay window, FSM and control plane.
    Sub-protocol semantics live in :class:`gbp_stack.GtpClient`,
    :class:`gbp_stack.GapClient` and :class:`gbp_stack.GspClient`.
    """

    __slots__ = ("_handle", "member_id", "_group_id")

    def __init__(self, handle: int, member_id: int, group_id: bytes) -> None:
        self._handle = handle
        self.member_id = member_id
        self._group_id = bytes(group_id)

    @classmethod
    def create(cls, member_id: int, group_id: bytes) -> "GroupNode":
        """Create a node bound to ``group_id`` (which MUST be 16 bytes)."""
        if len(group_id) != 16:
            raise ValueError("group_id must be 16 bytes")
        gid = (ctypes.c_uint8 * 16).from_buffer_copy(group_id)
        handle = _n.gbp_node_create(member_id, ctypes.cast(gid, ctypes.c_void_p))
        if handle <= 0:
            raise OSError(f"node_create: {_n.last_error()}")
        return cls(handle, member_id, group_id)

    @property
    def handle(self) -> int:
        """Native handle (i32)."""
        return self._handle

    @property
    def state(self) -> NodeState:
        """Current FSM state."""
        return NodeState(_n.gbp_node_state(self._handle))

    @property
    def epoch(self) -> int:
        """Current node epoch."""
        return int(_n.gbp_node_epoch(self._handle))

    @property
    def last_transition_id(self) -> int:
        """Last applied ``transition_id``."""
        return int(_n.gbp_node_last_transition_id(self._handle))

    @property
    def group_id(self) -> bytes:
        """16-byte group identifier."""
        return self._group_id

    def bootstrap_as_creator(self, epoch: int) -> None:
        """Drive the node from ``IDLE`` to ``ACTIVE`` as a creator."""
        if not _n.gbp_node_bootstrap_creator(self._handle, epoch):
            raise OSError(_n.last_error())

    def bootstrap_as_joiner(self, epoch: int, expected_first_tid: int = 0) -> None:
        """Drive the node from ``IDLE`` to ``ACTIVE`` as a joiner.

        ``expected_first_tid`` pre-arms ``pending_transition_id`` so the next
        ``EXECUTE_TRANSITION`` is accepted by the per-opcode tid validation
        matrix. The matching ``PREPARE_TRANSITION`` was sealed under the
        pre-Welcome MLS epoch and is therefore undecryptable to the joiner;
        the joiner is brought into the group when ``EXECUTE`` arrives on the
        new shared epoch. Pass ``0`` if the joiner recovered out-of-band and
        is already current.
        """
        if not _n.gbp_node_bootstrap_joiner(
            self._handle, epoch, expected_first_tid & 0xFFFFFFFF
        ):
            raise OSError(_n.last_error())

    def set_epoch_for_testing(self, epoch: int) -> None:
        """Forcibly override ``current_epoch`` (intended for tests of late peers)."""
        if not _n.gbp_node_set_epoch(self._handle, epoch):
            raise OSError(_n.last_error())

    def apply_transition(self, transition_id: int) -> None:
        """Apply an epoch transition locally."""
        if not _n.gbp_node_apply_transition(self._handle, transition_id):
            raise OSError(_n.last_error())

    def send_control(
        self,
        mls: MlsContext,
        target: int,
        opcode: ControlOpcode,
        transition_id: int,
        request_id: int,
        args: bytes = b"",
    ) -> OutboundFrame:
        """Send a control plane message on Stream 0."""
        def call(ptr, length):
            return _n.gbp_node_send_control(
                self._handle, mls.handle, target, int(opcode),
                transition_id, request_id, ptr, length,
            )
        buf = _n.call_with_bytes(args, call)
        return _unpack(buf, "send_control")

    def on_wire(self, mls: MlsContext, wire: bytes) -> List[NodeEvent]:
        """Feed wire bytes to the node and return the resulting events."""
        def call(ptr, length):
            return _n.gbp_node_on_wire(self._handle, mls.handle, ptr, length)
        ptr = _n.call_with_bytes(wire, call)
        return _parse_events(_n.take_cstring(ptr))

    def drain_events(self) -> List[NodeEvent]:
        """Drain queued events without consuming any wire bytes."""
        return _parse_events(_n.take_cstring(_n.gbp_node_drain_events(self._handle)))

    def close(self) -> None:
        """Release the native handle. Idempotent."""
        if self._handle:
            _n.gbp_node_destroy(self._handle)
            self._handle = 0

    def __enter__(self) -> "GroupNode":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass
