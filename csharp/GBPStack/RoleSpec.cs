namespace GBPStack;

/// <summary>Role definition in <see cref="RoleRegistry"/>.</summary>
public sealed record RoleSpec(uint Id, string Name, Permissions Permissions);
