/** Role registry and permission checks for GSP. */

/** Application-defined permission bits. */
export const Permissions = {
    None:           0,
    SendText:       1 << 0,
    SendAudio:      1 << 1,
    SendSignal:     1 << 2,
    MuteOthers:     1 << 3,
    AssignRoles:    1 << 4,
    Invite:         1 << 5,
    RemoveMembers:  1 << 6,
    CloseGroup:     1 << 7,
} as const;
export type Permissions = number;

/** Role definition in {@link RoleRegistry}. */
export interface RoleSpec {
    id: number;
    name: string;
    permissions: Permissions;
}

/** Error thrown by {@link RoleRegistry}. */
export class RoleError extends Error {
    constructor(message: string) {
        super(message);
        this.name = "RoleError";
    }
}

/**
 * Mapping of role ids to {@link RoleSpec}s plus per-member assignments.
 */
export class RoleRegistry {
    private readonly roles = new Map<number, RoleSpec>();
    private readonly assignments = new Map<number, number>();

    /** Register (or replace) a role. */
    define(spec: RoleSpec): void { this.roles.set(spec.id, spec); }

    /** Convenience: define a role from primitive components. */
    defineRole(id: number, name: string, permissions: Permissions): void {
        this.define({ id, name, permissions });
    }

    /** Look up a role by id. */
    role(id: number): RoleSpec | undefined { return this.roles.get(id); }

    /** Iterate every defined role. */
    *allRoles(): Iterable<RoleSpec> { yield* this.roles.values(); }

    /** Assign a role to a member. */
    assign(memberId: number, roleId: number): void {
        if (!this.roles.has(roleId)) throw new RoleError(`unknown role: ${roleId}`);
        this.assignments.set(memberId, roleId);
    }

    /** Role currently assigned to `memberId`, if any. */
    roleOf(memberId: number): RoleSpec | undefined {
        const rid = this.assignments.get(memberId);
        return rid !== undefined ? this.roles.get(rid) : undefined;
    }

    /** Effective permissions of `memberId` (`None` if no role). */
    permissionsOf(memberId: number): Permissions {
        return this.roleOf(memberId)?.permissions ?? Permissions.None;
    }

    /** Throws {@link RoleError} when the member lacks any bit in `mask`. */
    require(memberId: number, mask: Permissions): void {
        if ((this.permissionsOf(memberId) & mask) !== mask) {
            throw new RoleError(`member ${memberId} lacks permission 0x${mask.toString(16).padStart(8, "0").toUpperCase()}`);
        }
    }

    /** `true` when the member carries every bit in `mask`. */
    has(memberId: number, mask: Permissions): boolean {
        return (this.permissionsOf(memberId) & mask) === mask;
    }
}
