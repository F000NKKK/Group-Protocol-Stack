"""Bounded reorder buffer for GAP audio frames."""

from __future__ import annotations

import enum
from dataclasses import dataclass, field
from typing import Dict, List, Optional


@dataclass(frozen=True)
class AudioFrame:
    """One frame held by :class:`JitterBuffer`."""

    media_source_id: int
    rtp_sequence: int
    plaintext: bytes


class JitterPushOutcome(enum.Enum):
    """Outcome of :meth:`JitterBuffer.push`."""

    ACCEPTED = "accepted"
    LATE = "late"
    EVICTED = "evicted"


@dataclass(frozen=True)
class JitterPushResult:
    """Result returned by :meth:`JitterBuffer.push`."""

    outcome: JitterPushOutcome
    evicted: Optional[AudioFrame] = None


@dataclass
class _SourceState:
    waiting: List[AudioFrame] = field(default_factory=list)
    next_seq: Optional[int] = None


class JitterBuffer:
    """Bounded reorder window keyed by ``media_source_id``.

    Holds incoming GAP frames briefly so the decoder consumes them in
    ``rtp_sequence`` order; drops anything older than the next-expected
    sequence as late.
    """

    __slots__ = ("_capacity_per_source", "_sources")

    def __init__(self, capacity_per_source: int) -> None:
        if capacity_per_source <= 0:
            raise ValueError("capacity_per_source must be > 0")
        self._capacity_per_source = capacity_per_source
        self._sources: Dict[int, _SourceState] = {}

    def push(self, frame: AudioFrame) -> JitterPushResult:
        """Insert a frame into the buffer."""
        state = self._sources.setdefault(frame.media_source_id, _SourceState())
        if state.next_seq is not None and frame.rtp_sequence < state.next_seq:
            return JitterPushResult(JitterPushOutcome.LATE)
        # Dedupe.
        if any(f.rtp_sequence == frame.rtp_sequence for f in state.waiting):
            return JitterPushResult(JitterPushOutcome.ACCEPTED)
        # Insert in sorted order.
        idx = next(
            (i for i, f in enumerate(state.waiting) if f.rtp_sequence > frame.rtp_sequence),
            len(state.waiting),
        )
        state.waiting.insert(idx, frame)
        if len(state.waiting) > self._capacity_per_source:
            evicted = state.waiting.pop(0)
            return JitterPushResult(JitterPushOutcome.EVICTED, evicted)
        return JitterPushResult(JitterPushOutcome.ACCEPTED)

    def pop_in_order(self, media_source_id: int) -> Optional[AudioFrame]:
        """Pop the next frame if its ``rtp_sequence`` is contiguous."""
        state = self._sources.get(media_source_id)
        if not state or not state.waiting:
            return None
        head = state.waiting[0]
        if state.next_seq is not None and head.rtp_sequence != state.next_seq:
            return None
        state.waiting.pop(0)
        state.next_seq = (head.rtp_sequence + 1) & 0xFFFFFFFF
        return head

    def pop_force(self, media_source_id: int) -> Optional[AudioFrame]:
        """Pop the next frame regardless of contiguity (skip gaps)."""
        state = self._sources.get(media_source_id)
        if not state or not state.waiting:
            return None
        head = state.waiting.pop(0)
        state.next_seq = (head.rtp_sequence + 1) & 0xFFFFFFFF
        return head

    def length_for(self, media_source_id: int) -> int:
        """Number of frames buffered for the given source."""
        state = self._sources.get(media_source_id)
        return len(state.waiting) if state else 0

    def clear(self) -> None:
        """Drop every queued frame."""
        self._sources.clear()
