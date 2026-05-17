namespace GBPStack;

/// <summary>Application-defined permission bits for GSP roles.</summary>
[Flags]
public enum Permissions : uint
{
    None = 0,
    SendText = 1u << 0,
    SendAudio = 1u << 1,
    SendSignal = 1u << 2,
    MuteOthers = 1u << 3,
    AssignRoles = 1u << 4,
    Invite = 1u << 5,
    RemoveMembers = 1u << 6,
    CloseGroup = 1u << 7
}
