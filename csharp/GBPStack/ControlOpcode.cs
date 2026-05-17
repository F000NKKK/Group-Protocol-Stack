namespace GBPStack;

/// <summary>Control plane opcode registry.</summary>
public enum ControlOpcode : ushort
{
    PrepareTransition = 0x0001,
    ReadyForTransition = 0x0002,
    ExecuteTransition = 0x0003,
    AbortTransition = 0x0004,
    GroupStateDigestRequest = 0x0005,
    GroupStateDigestResponse = 0x0006,
    ReportInvalidCommit = 0x0007,
    CapabilitiesAdvertise = 0x0008,
    Ack = 0x0009,
    Nack = 0x000A
}
