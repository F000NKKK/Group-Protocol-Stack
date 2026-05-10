"""Bounded message log + per-sender resync watermark for GTP."""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass
from typing import Dict, Iterator, Optional


@dataclass(frozen=True)
class MessageEntry:
    """One entry in :class:`MessageHistory`."""

    sender_id: int
    message_id: int
    text: str


class MessageHistory:
    """Bounded ring buffer of recent GTP messages.

    Used to serve resync requests from re-joining peers — keep a few
    thousand recent messages, then `since(watermark)` returns everything
    above the caller's high-water mark.
    """

    __slots__ = ("_capacity", "_buffer")

    def __init__(self, capacity: int) -> None:
        if capacity <= 0:
            raise ValueError("capacity must be > 0")
        self._capacity = capacity
        self._buffer: "deque[MessageEntry]" = deque(maxlen=capacity)

    def __len__(self) -> int:
        return len(self._buffer)

    def push(self, entry: MessageEntry) -> bool:
        """Record a message. Returns ``True`` if newly added."""
        if self.contains(entry.sender_id, entry.message_id):
            return False
        self._buffer.append(entry)
        return True

    def contains(self, sender_id: int, message_id: int) -> bool:
        """``True`` if ``(sender_id, message_id)`` is buffered."""
        return any(
            m.sender_id == sender_id and m.message_id == message_id
            for m in self._buffer
        )

    def since(self, watermark: "Watermark") -> Iterator[MessageEntry]:
        """Yields every message produced after ``watermark`` in insertion order."""
        for m in self._buffer:
            hw = watermark.last_seen(m.sender_id)
            if hw is None or m.message_id > hw:
                yield m

    def since_for_sender(self, sender_id: int, since_message_id: int) -> Iterator[MessageEntry]:
        """Yields messages from a single sender newer than ``since_message_id``."""
        for m in self._buffer:
            if m.sender_id == sender_id and m.message_id > since_message_id:
                yield m

    def clear(self) -> None:
        """Drops every message in the buffer."""
        self._buffer.clear()


class Watermark:
    """Per-sender high-water mark of accepted GTP ``message_id`` s."""

    __slots__ = ("_last_seen",)

    def __init__(self) -> None:
        self._last_seen: Dict[int, int] = {}

    def observe(self, sender_id: int, message_id: int) -> None:
        """Record that ``message_id`` from ``sender_id`` has been observed."""
        prev = self._last_seen.get(sender_id, 0)
        if message_id > prev:
            self._last_seen[sender_id] = message_id

    def last_seen(self, sender_id: int) -> Optional[int]:
        """Last observed ``message_id`` for ``sender_id`` (``None`` if unseen)."""
        return self._last_seen.get(sender_id)

    def snapshot(self) -> Dict[int, int]:
        """Returns a copy of the underlying mapping."""
        return dict(self._last_seen)

    def __len__(self) -> int:
        return len(self._last_seen)

    def clear(self) -> None:
        """Drops every entry."""
        self._last_seen.clear()
