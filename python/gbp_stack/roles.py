"""Role registry and permission checks for GSP."""

from __future__ import annotations

from dataclasses import dataclass
from enum import IntFlag
from typing import Dict, Iterable, Optional


class Permissions(IntFlag):
    """Application-defined permission bits."""

    NONE             = 0
    SEND_TEXT        = 1 << 0
    SEND_AUDIO       = 1 << 1
    SEND_SIGNAL      = 1 << 2
    MUTE_OTHERS      = 1 << 3
    ASSIGN_ROLES     = 1 << 4
    INVITE           = 1 << 5
    REMOVE_MEMBERS   = 1 << 6
    CLOSE_GROUP      = 1 << 7


@dataclass(frozen=True)
class RoleSpec:
    """Role definition in :class:`RoleRegistry`."""

    id: int
    name: str
    permissions: Permissions


class RoleError(Exception):
    """Raised by :class:`RoleRegistry` on unknown role / unauthorised member."""


class RoleRegistry:
    """Mapping of role ids to :class:`RoleSpec` plus per-member assignments."""

    __slots__ = ("_roles", "_assignments")

    def __init__(self) -> None:
        self._roles: Dict[int, RoleSpec] = {}
        self._assignments: Dict[int, int] = {}

    def define(self, spec: RoleSpec) -> None:
        """Register (or replace) a role."""
        self._roles[spec.id] = spec

    def define_role(self, role_id: int, name: str, permissions: Permissions) -> None:
        """Convenience: define a role from primitive components."""
        self.define(RoleSpec(role_id, name, permissions))

    def role(self, role_id: int) -> Optional[RoleSpec]:
        """Look up a role by id."""
        return self._roles.get(role_id)

    def roles(self) -> Iterable[RoleSpec]:
        """Iterate every defined role."""
        return self._roles.values()

    def assign(self, member_id: int, role_id: int) -> None:
        """Assign a role to a member."""
        if role_id not in self._roles:
            raise RoleError(f"unknown role: {role_id}")
        self._assignments[member_id] = role_id

    def role_of(self, member_id: int) -> Optional[RoleSpec]:
        """Role currently assigned to ``member_id``, if any."""
        rid = self._assignments.get(member_id)
        return self._roles.get(rid) if rid is not None else None

    def permissions_of(self, member_id: int) -> Permissions:
        """Effective permissions of ``member_id`` (NONE if no role)."""
        spec = self.role_of(member_id)
        return spec.permissions if spec else Permissions.NONE

    def require(self, member_id: int, mask: Permissions) -> None:
        """Raise :class:`RoleError` if ``member_id`` lacks any bit in ``mask``."""
        if (self.permissions_of(member_id) & mask) != mask:
            raise RoleError(f"member {member_id} lacks permission 0x{int(mask):08X}")

    def has(self, member_id: int, mask: Permissions) -> bool:
        """``True`` when the member carries every bit in ``mask``."""
        return (self.permissions_of(member_id) & mask) == mask
