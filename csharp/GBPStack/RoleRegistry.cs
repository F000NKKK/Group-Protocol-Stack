using System;
using System.Collections.Generic;
using System.Linq;

namespace GBPStack;

/// <summary>
/// Application-defined permission bits for GSP roles.
/// </summary>
[Flags]
public enum Permissions : uint
{
    None             = 0,
    SendText         = 1u << 0,
    SendAudio        = 1u << 1,
    SendSignal       = 1u << 2,
    MuteOthers       = 1u << 3,
    AssignRoles      = 1u << 4,
    Invite           = 1u << 5,
    RemoveMembers    = 1u << 6,
    CloseGroup       = 1u << 7
}

/// <summary>Role definition in <see cref="RoleRegistry"/>.</summary>
public sealed record RoleSpec(uint Id, string Name, Permissions Permissions);

/// <summary>Errors thrown by <see cref="RoleRegistry"/>.</summary>
public sealed class RoleException : Exception
{
    public RoleException(string message) : base(message) { }
}

/// <summary>
/// Bidirectional mapping of role ids to <see cref="RoleSpec"/>s plus an
/// assignment table tracking each member's current role.
/// </summary>
public sealed class RoleRegistry
{
    private readonly Dictionary<uint, RoleSpec> _roles = new();
    private readonly Dictionary<uint, uint> _assignments = new();

    /// <summary>Registers a role (replaces any existing one with the same id).</summary>
    public void Define(RoleSpec spec) => _roles[spec.Id] = spec;

    /// <summary>Convenience: defines a role from primitive components.</summary>
    public void DefineRole(uint id, string name, Permissions permissions) =>
        Define(new RoleSpec(id, name, permissions));

    /// <summary>Looks up a role by id.</summary>
    public RoleSpec? Role(uint id) => _roles.TryGetValue(id, out var s) ? s : null;

    /// <summary>Iterates every defined role.</summary>
    public IEnumerable<RoleSpec> Roles => _roles.Values;

    /// <summary>Assigns a role to a member.</summary>
    public void Assign(uint memberId, uint roleId)
    {
        if (!_roles.ContainsKey(roleId))
            throw new RoleException($"unknown role: {roleId}");
        _assignments[memberId] = roleId;
    }

    /// <summary>Returns the role currently assigned to <paramref name="memberId"/>, if any.</summary>
    public RoleSpec? RoleOf(uint memberId) =>
        _assignments.TryGetValue(memberId, out var rid) ? Role(rid) : null;

    /// <summary>Effective permissions of <paramref name="memberId"/> (None if unassigned).</summary>
    public Permissions PermissionsOf(uint memberId) =>
        RoleOf(memberId)?.Permissions ?? Permissions.None;

    /// <summary>Throws <see cref="RoleException"/> if the member lacks the required permissions.</summary>
    public void Require(uint memberId, Permissions mask)
    {
        if ((PermissionsOf(memberId) & mask) != mask)
            throw new RoleException($"member {memberId} lacks permission 0x{(uint)mask:X8}");
    }

    /// <summary>Returns <c>true</c> when the member has every bit in <paramref name="mask"/>.</summary>
    public bool Has(uint memberId, Permissions mask) =>
        (PermissionsOf(memberId) & mask) == mask;
}
