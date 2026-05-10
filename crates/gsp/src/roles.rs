//! Role registry and permission checks (GSP §5 — `ROLE_CHANGE`).
//!
//! GSP defines `ROLE_CHANGE` but leaves the role hierarchy and the
//! permission semantics up to the application. [`RoleRegistry`] captures
//! both pieces in a small, allocation-light data structure:
//!
//! * a numeric **role id** is bound to a stable role name (for logs);
//! * each role carries a [`Permissions`] bitset that the application can
//!   query before applying side-effecting signals.

use gbp_core::MemberId;
use std::collections::HashMap;

/// Application-defined permission bits. The first eight slots are
/// pre-named to cover the common audio/text/role permissions; bits 8..32
/// are free for application use.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Permissions(pub u32);

impl Permissions {
    /// Bit 0: may publish to text streams (GTP).
    pub const SEND_TEXT: u32 = 1 << 0;
    /// Bit 1: may publish to audio streams (GAP).
    pub const SEND_AUDIO: u32 = 1 << 1;
    /// Bit 2: may emit signals (GSP).
    pub const SEND_SIGNAL: u32 = 1 << 2;
    /// Bit 3: may mute / unmute other members.
    pub const MUTE_OTHERS: u32 = 1 << 3;
    /// Bit 4: may approve `ROLE_CHANGE` requests.
    pub const ASSIGN_ROLES: u32 = 1 << 4;
    /// Bit 5: may invite new members (drives MLS adds).
    pub const INVITE: u32 = 1 << 5;
    /// Bit 6: may remove members from the group.
    pub const REMOVE_MEMBERS: u32 = 1 << 6;
    /// Bit 7: may close / archive the group.
    pub const CLOSE_GROUP: u32 = 1 << 7;

    /// `true` if every bit in `mask` is set.
    pub const fn has(self, mask: u32) -> bool {
        self.0 & mask == mask
    }

    /// Bitwise OR.
    pub const fn with(self, mask: u32) -> Self {
        Self(self.0 | mask)
    }

    /// Bitwise AND-NOT.
    pub const fn without(self, mask: u32) -> Self {
        Self(self.0 & !mask)
    }
}

impl core::ops::BitOr<u32> for Permissions {
    type Output = Self;
    fn bitor(self, rhs: u32) -> Self::Output {
        Self(self.0 | rhs)
    }
}

/// Role definition in [`RoleRegistry`].
#[derive(Clone, Debug)]
pub struct RoleSpec {
    /// Numeric role id (matches the `role_claim` field on the wire).
    pub id: u32,
    /// Stable human-readable name for logs.
    pub name: String,
    /// Permissions granted by the role.
    pub permissions: Permissions,
}

/// Errors returned by [`RoleRegistry`].
#[derive(Debug, thiserror::Error)]
pub enum RoleError {
    /// The role id is not registered.
    #[error("unknown role: {0}")]
    UnknownRole(u32),
    /// The acting member is not authorised for this operation.
    #[error("member {member} lacks permission 0x{permission:08X}")]
    Unauthorised {
        /// Acting member.
        member: MemberId,
        /// Required permission mask.
        permission: u32,
    },
}

/// Bidirectional mapping of role ids to [`RoleSpec`]s plus an assignment
/// table tracking each member's current role.
#[derive(Default)]
pub struct RoleRegistry {
    roles: HashMap<u32, RoleSpec>,
    assignments: HashMap<MemberId, u32>,
}

impl RoleRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a role. Replaces any existing role with the same id.
    pub fn define(&mut self, spec: RoleSpec) {
        self.roles.insert(spec.id, spec);
    }

    /// Convenience: defines a role from primitive components.
    pub fn define_role(&mut self, id: u32, name: impl Into<String>, permissions: Permissions) {
        self.define(RoleSpec {
            id,
            name: name.into(),
            permissions,
        });
    }

    /// Looks up a role by id.
    pub fn role(&self, id: u32) -> Option<&RoleSpec> {
        self.roles.get(&id)
    }

    /// Iterates every defined role.
    pub fn roles(&self) -> impl Iterator<Item = &RoleSpec> {
        self.roles.values()
    }

    /// Assigns a role to a member.
    pub fn assign(&mut self, member: MemberId, role_id: u32) -> Result<(), RoleError> {
        if !self.roles.contains_key(&role_id) {
            return Err(RoleError::UnknownRole(role_id));
        }
        self.assignments.insert(member, role_id);
        Ok(())
    }

    /// Returns the role currently assigned to `member`, if any.
    pub fn role_of(&self, member: MemberId) -> Option<&RoleSpec> {
        let id = self.assignments.get(&member)?;
        self.roles.get(id)
    }

    /// Returns the effective permissions of `member` (zero if no role is
    /// assigned).
    pub fn permissions_of(&self, member: MemberId) -> Permissions {
        self.role_of(member)
            .map(|r| r.permissions)
            .unwrap_or_default()
    }

    /// `Ok(())` if `member` carries every bit in `mask`; otherwise
    /// [`RoleError::Unauthorised`].
    pub fn require(&self, member: MemberId, mask: u32) -> Result<(), RoleError> {
        if self.permissions_of(member).has(mask) {
            Ok(())
        } else {
            Err(RoleError::Unauthorised {
                member,
                permission: mask,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_and_require() {
        let mut r = RoleRegistry::new();
        r.define_role(1, "viewer", Permissions::default());
        r.define_role(
            10,
            "speaker",
            Permissions::default() | Permissions::SEND_TEXT | Permissions::SEND_AUDIO,
        );
        r.define_role(
            100,
            "admin",
            Permissions::default()
                | Permissions::SEND_TEXT
                | Permissions::SEND_AUDIO
                | Permissions::MUTE_OTHERS
                | Permissions::ASSIGN_ROLES,
        );

        r.assign(2, 10).unwrap();
        r.assign(3, 100).unwrap();

        assert!(r.require(2, Permissions::SEND_TEXT).is_ok());
        assert!(r.require(2, Permissions::MUTE_OTHERS).is_err());
        assert!(r.require(3, Permissions::MUTE_OTHERS).is_ok());
    }

    #[test]
    fn unknown_role_rejected() {
        let mut r = RoleRegistry::new();
        let err = r.assign(1, 42).unwrap_err();
        assert!(matches!(err, RoleError::UnknownRole(42)));
    }
}
