namespace GBPStack;

/// <summary>Outcome of <see cref="MlsContext.InviteFull"/>.</summary>
/// <param name="Commit">The MLS Commit message to broadcast to existing members.</param>
/// <param name="Welcome">The MLS Welcome message to unicast to the new joiner.</param>
public sealed record InviteResult(byte[] Commit, byte[] Welcome);
