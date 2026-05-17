namespace GBPStack;

/// <summary>An outbound wire frame: target member and the serialised bytes.</summary>
public sealed record OutboundFrame(uint Target, byte[] Wire);
