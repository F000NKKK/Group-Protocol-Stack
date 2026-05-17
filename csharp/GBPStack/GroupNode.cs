using System;
using System.Collections.Generic;
using System.Text.Json;

namespace GBPStack;

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

    /// <summary>Drives the node from <c>IDLE</c> to <c>ACTIVE</c> as a joiner.
    /// <paramref name="expectedFirstTid"/> pre-arms pending transition state so the
    /// next <c>EXECUTE_TRANSITION</c> is accepted; pass <c>0</c> if recovered
    /// out-of-band.</summary>
    public void BootstrapAsJoiner(ulong epoch, uint expectedFirstTid = 0)
    {
        if (!Native.gbp_node_bootstrap_joiner(Handle, epoch, expectedFirstTid))
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
