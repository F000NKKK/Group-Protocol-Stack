namespace GBPStack;

/// <summary>
/// Payload encoding used by GTP / GAP / GSP sub-protocols.
/// Passed as the <c>codec</c> byte in all send/accept FFI calls.
/// </summary>
public enum PayloadCodec : byte
{
    /// <summary>CBOR — default, backward-compatible. Suitable for all sub-protocols.</summary>
    Cbor = 0,
    /// <summary>Protocol Buffers — compact binary, best for cross-language gRPC.</summary>
    Protobuf = 1,
    /// <summary>FlatBuffers — zero-copy decode, lowest latency for audio (GAP).</summary>
    FlatBuffers = 2,
}
