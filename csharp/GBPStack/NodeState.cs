namespace GBPStack;

/// <summary>Node FSM state.</summary>
public enum NodeState : uint
{
    Idle = 0, Connecting, EstablishingGroup, Active, Resyncing, Failed, Closed
}
