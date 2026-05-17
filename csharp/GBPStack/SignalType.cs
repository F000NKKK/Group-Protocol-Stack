namespace GBPStack;

/// <summary>GSP signal type registry.</summary>
public enum SignalType : uint
{
    Join = 100,
    Leave = 101,
    RoleChange = 102,
    Mute = 200,
    Unmute = 201,
    StreamStart = 300,
    StreamStop = 301,
    CodecUpdate = 400
}
