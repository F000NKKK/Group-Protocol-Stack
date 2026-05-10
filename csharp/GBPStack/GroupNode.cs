using System;
using System.Collections.Generic;
using System.Text.Json;

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

/// <summary>Node FSM state.</summary>
public enum NodeState : uint
{
    Idle = 0, Connecting, EstablishingGroup, Active, Resyncing, Failed, Closed
}

/// <summary>Stream class.</summary>
public enum StreamType : uint { Control = 0, Audio = 1, Text = 2, Signal = 3 }

/// <summary>An outbound wire frame: target member and the serialised bytes.</summary>
public sealed record OutboundFrame(uint Target, byte[] Wire);

/// <summary>
/// Managed wrapper around the GBP-layer node. Owns the framing, AEAD, replay
/// window, FSM and control plane. Sub-protocol semantics live in
/// <see cref="GtpClient"/>, <see cref="GapClient"/> and <see cref="GspClient"/>.
/// </summary>
public sealed class GroupNode : IDisposable
{
    /// <summary>Native handle.</summary>
    public int Handle { get; private set; }

    /// <summary>This node's member id.</summary>
    public uint MemberId { get; }

    private readonly byte[] _groupId;

    private GroupNode(int handle, uint memberId, byte[] groupId)
    {
        Handle = handle;
        MemberId = memberId;
        _groupId = groupId;
    }

    /// <summary>Creates a node with the given member id and 16-byte group id.</summary>
    public static GroupNode Create(uint memberId, byte[] groupId16)
    {
        if (groupId16.Length != 16) throw new ArgumentException("group_id must be 16 bytes");
        int h;
        unsafe
        {
            fixed (byte* p = groupId16) h = Native.gbp_node_create(memberId, (IntPtr)p);
        }
        if (h <= 0) throw new InvalidOperationException($"node_create: {Native.LastError()}");
        return new GroupNode(h, memberId, (byte[])groupId16.Clone());
    }

    /// <summary>Current node FSM state.</summary>
    public NodeState State => (NodeState)Native.gbp_node_state(Handle);

    /// <summary>Current node epoch.</summary>
    public ulong Epoch => Native.gbp_node_epoch(Handle);

    /// <summary>Last applied <c>transition_id</c>.</summary>
    public uint LastTransitionId => Native.gbp_node_last_transition_id(Handle);

    /// <summary>Group identifier (a copy of the 16-byte array).</summary>
    public byte[] GroupId => (byte[])_groupId.Clone();

    /// <summary>Drives the node from <c>IDLE</c> to <c>ACTIVE</c> as a creator.</summary>
    public void BootstrapAsCreator(ulong epoch)
    {
        if (!Native.gbp_node_bootstrap_creator(Handle, epoch))
            throw new InvalidOperationException(Native.LastError());
    }

    /// <summary>Drives the node from <c>IDLE</c> to <c>ACTIVE</c> as a joiner.</summary>
    public void BootstrapAsJoiner(ulong epoch)
    {
        if (!Native.gbp_node_bootstrap_joiner(Handle, epoch))
            throw new InvalidOperationException(Native.LastError());
    }

    /// <summary>Forces the node's <c>current_epoch</c> (intended for tests of late peers).</summary>
    public void SetEpochForTesting(ulong epoch)
    {
        if (!Native.gbp_node_set_epoch(Handle, epoch))
            throw new InvalidOperationException(Native.LastError());
    }

    /// <summary>Applies an epoch transition locally.</summary>
    public void ApplyTransition(uint tid)
    {
        if (!Native.gbp_node_apply_transition(Handle, tid))
            throw new InvalidOperationException(Native.LastError());
    }

    /// <summary>Sends a control plane message on Stream 0.</summary>
    public OutboundFrame SendControl(MlsContext mls, uint target, ControlOpcode op, uint tid, uint requestId, byte[]? args = null)
    {
        args ??= Array.Empty<byte>();
        var buf = Native.WithBytes(args, (p, l) =>
            Native.gbp_node_send_control(Handle, mls.Handle, target, (ushort)op, tid, requestId, p, l));
        return Unpack(buf, "send_control");
    }

    /// <summary>Feeds wire bytes to the node and returns the resulting events.</summary>
    public NodeEvent[] OnWire(MlsContext mls, byte[] wire)
    {
        IntPtr cstr;
        unsafe
        {
            fixed (byte* p = wire) cstr = Native.gbp_node_on_wire(Handle, mls.Handle, (IntPtr)p, (nuint)wire.Length);
        }
        return ParseEvents(Native.CopyAndFree(cstr));
    }

    /// <summary>Drains queued events without consuming any wire bytes.</summary>
    public NodeEvent[] DrainEvents() => ParseEvents(Native.CopyAndFree(Native.gbp_node_drain_events(Handle)));

    internal static OutboundFrame Unpack(Native.GbpBuffer buf, string what)
    {
        if (buf.IsEmpty) throw new InvalidOperationException($"{what}: {Native.LastError()}");
        var bytes = Native.CopyAndFree(buf);
        if (bytes.Length < 4) throw new InvalidOperationException($"{what}: buffer too short");
        var target = BitConverter.ToUInt32(bytes, 0);
        var wire = new byte[bytes.Length - 4];
        Buffer.BlockCopy(bytes, 4, wire, 0, wire.Length);
        return new OutboundFrame(target, wire);
    }

    private static NodeEvent[] ParseEvents(string json)
    {
        if (string.IsNullOrEmpty(json) || json == "[]") return Array.Empty<NodeEvent>();
        using var doc = JsonDocument.Parse(json);
        var list = new List<NodeEvent>();
        foreach (var el in doc.RootElement.EnumerateArray()) list.Add(NodeEvent.FromJson(el));
        return list.ToArray();
    }

    /// <inheritdoc />
    public void Dispose()
    {
        if (Handle != 0) { Native.gbp_node_destroy(Handle); Handle = 0; }
        GC.SuppressFinalize(this);
    }

    ~GroupNode() { Dispose(); }
}

/// <summary>Event surfaced by the GBP layer.</summary>
public sealed record NodeEvent(
    string Kind,
    string? FromState = null,
    string? ToState = null,
    string? StreamTypeName = null,
    StreamType? StreamType = null,
    uint? StreamId = null,
    uint? SequenceNo = null,
    ushort? Flags = null,
    byte[]? Plaintext = null,
    uint? Sender = null,
    string? Opcode = null,
    ushort? OpcodeCode = null,
    uint? TransitionId = null,
    ushort? Code = null,
    string? CodeHex = null,
    byte? Class = null,
    bool? Retryable = null,
    bool? Fatal = null,
    string? Reason = null,
    ulong? Epoch = null)
{
    internal static NodeEvent FromJson(JsonElement el)
    {
        var kind = el.GetProperty("kind").GetString() ?? "";
        string? S(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.String ? x.GetString() : null;
        uint? U32(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetUInt32() : null;
        ushort? U16(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetUInt16() : null;
        ulong? U64(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetUInt64() : null;
        byte? U8(string n) => el.TryGetProperty(n, out var x) && x.ValueKind == JsonValueKind.Number ? x.GetByte() : null;
        bool? B(string n) => el.TryGetProperty(n, out var x) && x.ValueKind is JsonValueKind.True or JsonValueKind.False ? x.GetBoolean() : null;

        byte[]? plaintext = null;
        if (el.TryGetProperty("plaintext_b64", out var pb) && pb.ValueKind == JsonValueKind.String)
            plaintext = Convert.FromBase64String(pb.GetString()!);

        StreamType? st = null;
        if (el.TryGetProperty("stream_type_code", out var stc) && stc.ValueKind == JsonValueKind.Number)
            st = (StreamType)stc.GetUInt32();

        return new NodeEvent(
            kind,
            FromState: S("from"),
            ToState: S("to"),
            StreamTypeName: S("stream_type"),
            StreamType: st,
            StreamId: U32("stream_id"),
            SequenceNo: U32("sequence_no"),
            Flags: U16("flags"),
            Plaintext: plaintext,
            Sender: U32("from"),
            Opcode: S("opcode"),
            OpcodeCode: U16("opcode_code"),
            TransitionId: U32("transition_id"),
            Code: U16("code"),
            CodeHex: S("code_hex"),
            Class: U8("class"),
            Retryable: B("retryable"),
            Fatal: B("fatal"),
            Reason: S("reason"),
            Epoch: U64("epoch"));
    }
}
