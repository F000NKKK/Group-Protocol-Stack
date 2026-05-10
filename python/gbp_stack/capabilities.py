"""Capability negotiation helper for GSP."""

from __future__ import annotations

from typing import Dict, Iterable, List, Optional, Set


class CapabilitiesNegotiator:
    """Per-member capability advertisement plus intersection / union queries."""

    __slots__ = ("_advertised",)

    def __init__(self) -> None:
        self._advertised: Dict[int, Set[str]] = {}

    def advertise(self, member_id: int, capabilities: Iterable[str]) -> None:
        """Record an advertisement (replaces any prior one)."""
        self._advertised[member_id] = set(capabilities)

    def forget(self, member_id: int) -> None:
        """Remove a member's advertisement."""
        self._advertised.pop(member_id, None)

    def capabilities_of(self, member_id: int) -> Optional[Set[str]]:
        """Current advertisement for ``member_id``."""
        s = self._advertised.get(member_id)
        return set(s) if s is not None else None

    def group_supports(self, capability: str) -> bool:
        """True iff every advertised member supports ``capability``."""
        if not self._advertised:
            return False
        return all(capability in s for s in self._advertised.values())

    def intersection(self) -> Set[str]:
        """Capabilities every member advertises (safe-to-use set)."""
        if not self._advertised:
            return set()
        sets = iter(self._advertised.values())
        acc = set(next(sets))
        for s in sets:
            acc &= s
        return acc

    def union(self) -> Set[str]:
        """Every capability advertised by any member."""
        acc: Set[str] = set()
        for s in self._advertised.values():
            acc |= s
        return acc

    def missing(self, capability: str) -> List[int]:
        """Members that did not advertise ``capability``."""
        return [m for m, s in self._advertised.items() if capability not in s]

    def __len__(self) -> int:
        return len(self._advertised)
