using System.Text;
using Xunit;

namespace GBPStack.Tests;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

internal static class H
{
    public static (MlsContext AliceMls, GroupNode AliceNode, MlsContext BobMls, GroupNode BobNode)
        TwoMemberGroup()
    {
        var aliceMls = MlsContext.Create("alice");
        var bobMls = MlsContext.Create("bob");
        bobMls.AcceptWelcome(aliceMls.Invite(bobMls.ExportKeyPackage()));
        var gid = aliceMls.GroupId;
        var aliceNode = GroupNode.Create(1, gid);
        var bobNode = GroupNode.Create(2, gid);
        aliceNode.BootstrapAsCreator(aliceMls.Epoch);
        bobNode.BootstrapAsJoiner(bobMls.Epoch);
        return (aliceMls, aliceNode, bobMls, bobNode);
    }

    public static NodeEvent[] TextEvents(NodeEvent[] evs) =>
        evs.Where(e => e.Kind == "payload_received" && e.StreamType == (StreamType)2).ToArray();

    public static NodeEvent[] AudioEvents(NodeEvent[] evs) =>
        evs.Where(e => e.Kind == "payload_received" && e.StreamType == (StreamType)1).ToArray();

    public static NodeEvent[] SignalEvents(NodeEvent[] evs) =>
        evs.Where(e => e.Kind == "payload_received" && e.StreamType == (StreamType)3).ToArray();
}

// ---------------------------------------------------------------------------
// CBOR encoding helpers for GSP signal args
// ---------------------------------------------------------------------------

internal static class Cbor
{
    public static byte[] Uint(uint n)
    {
        if (n <= 23) return new[] { (byte)n };
        if (n <= 0xFF) return new[] { (byte)0x18, (byte)n };
        if (n <= 0xFFFF) return new[] { (byte)0x19, (byte)(n >> 8), (byte)n };
        return new[] { (byte)0x1A, (byte)(n >> 24), (byte)(n >> 16), (byte)(n >> 8), (byte)n };
    }

    public static byte[] Map1(uint k, uint v) =>
        new[] { (byte)0xA1 }.Concat(Uint(k)).Concat(Uint(v)).ToArray();

    public static byte[] Map2(uint k0, uint v0, uint k1, uint v1) =>
        new[] { (byte)0xA2 }
            .Concat(Uint(k0)).Concat(Uint(v0))
            .Concat(Uint(k1)).Concat(Uint(v1))
            .ToArray();
}

// ---------------------------------------------------------------------------
// MLS context
// ---------------------------------------------------------------------------

public class MlsContextTests
{
    [Fact]
    public void Create_Epoch_Is_Zero()
    {
        using var ctx = MlsContext.Create("alice");
        Assert.Equal(0UL, ctx.Epoch);
        Assert.Equal("alice", ctx.Identity);
    }

    [Fact]
    public void GroupId_Is_16_Bytes()
    {
        using var ctx = MlsContext.Create("alice");
        Assert.Equal(16, ctx.GroupId.Length);
    }

    [Fact]
    public void ExportKeyPackage_Returns_Bytes()
    {
        using var ctx = MlsContext.Create("alice");
        var kp = ctx.ExportKeyPackage();
        Assert.True(kp.Length > 0);
    }

    [Fact]
    public void Invite_And_AcceptWelcome_Syncs_Epoch()
    {
        using var alice = MlsContext.Create("alice");
        using var bob = MlsContext.Create("bob");
        bob.AcceptWelcome(alice.Invite(bob.ExportKeyPackage()));
        Assert.Equal(alice.Epoch, bob.Epoch);
    }

    [Fact]
    public void ExportState_RestoreState_Preserves_Epoch_And_GroupId()
    {
        using var alice = MlsContext.Create("alice");
        using var bob = MlsContext.Create("bob");
        bob.AcceptWelcome(alice.Invite(bob.ExportKeyPackage()));
        Assert.Equal(1UL, alice.Epoch);

        var blob = alice.ExportState();
        Assert.True(blob.Length > 0);
        using var restored = MlsContext.RestoreState(blob, "alice");
        Assert.Equal(alice.Epoch, restored.Epoch);
        Assert.Equal(alice.GroupId, restored.GroupId);
    }

    [Fact]
    public void RestoreState_Rejects_Truncated_Blob()
    {
        using var alice = MlsContext.Create("alice");
        var blob = alice.ExportState();
        Assert.Throws<InvalidOperationException>(
            () => MlsContext.RestoreState(blob[..(blob.Length / 2)]));
    }

    [Fact]
    public void InviteFull_Returns_Commit_And_Welcome()
    {
        using var alice = MlsContext.Create("alice");
        using var bob = MlsContext.Create("bob");
        var res = alice.InviteFull(bob.ExportKeyPackage());
        Assert.True(res.Commit.Length > 0);
        Assert.True(res.Welcome.Length > 0);
        var epochBefore = alice.Epoch;
        alice.FinalizeCommit();
        Assert.True(alice.Epoch > epochBefore);
        bob.AcceptWelcome(res.Welcome);
        Assert.Equal(alice.Epoch, bob.Epoch);
    }

    [Fact]
    public void InviteFull_Three_Members()
    {
        using var alice = MlsContext.Create("alice");
        using var bob = MlsContext.Create("bob");
        using var carol = MlsContext.Create("carol");
        bob.AcceptWelcome(alice.Invite(bob.ExportKeyPackage()));
        var res = alice.InviteFull(carol.ExportKeyPackage());
        alice.FinalizeCommit();
        bob.ProcessMessage(res.Commit);
        bob.FinalizeCommit();
        carol.AcceptWelcome(res.Welcome);
        Assert.Equal(alice.Epoch, bob.Epoch);
        Assert.Equal(alice.Epoch, carol.Epoch);
    }
}

// ---------------------------------------------------------------------------
// GTP — text messaging
// ---------------------------------------------------------------------------

public class GtpClientTests
{
    [Fact]
    public void Basic_Send_Receive()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gtpA = GtpClient.Create())
        using (var gtpB = GtpClient.Create())
        {
            var frame = gtpA.Send(an, am, 2, 1, "hello");
            var evs = H.TextEvents(bn.OnWire(bm, frame.Wire));
            Assert.Single(evs);
            var r = gtpB.Accept(evs[0].Plaintext!, bm.Epoch);
            Assert.Equal("new", r.Status);
            Assert.Equal("hello", r.Text);
        }
    }

    [Fact]
    public void Bidirectional_Messaging()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gtpA = GtpClient.Create())
        using (var gtpB = GtpClient.Create())
        {
            var frame = gtpB.Send(bn, bm, 1, 10, "hi alice");
            var evs = H.TextEvents(an.OnWire(am, frame.Wire));
            Assert.Single(evs);
            var r = gtpA.Accept(evs[0].Plaintext!, am.Epoch);
            Assert.Equal("hi alice", r.Text);
            Assert.Equal(2U, r.Sender);
        }
    }

    [Fact]
    public void Multiple_Messages()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gtpA = GtpClient.Create())
        using (var gtpB = GtpClient.Create())
        {
            var msgs = new[] { "first", "second", "third" };
            for (uint i = 0; i < msgs.Length; i++)
            {
                var frame = gtpA.Send(an, am, 2, i + 1, msgs[i]);
                var evs = H.TextEvents(bn.OnWire(bm, frame.Wire));
                Assert.Equal(msgs[i], gtpB.Accept(evs[0].Plaintext!, bm.Epoch).Text);
            }
        }
    }

    [Fact]
    public void Duplicate_Rejection()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gtpA = GtpClient.Create())
        using (var gtpB = GtpClient.Create())
        {
            var frame = gtpA.Send(an, am, 2, 99, "once");
            var pt = H.TextEvents(bn.OnWire(bm, frame.Wire))[0].Plaintext!;
            Assert.Equal("new", gtpB.Accept(pt, bm.Epoch).Status);
            Assert.Equal("duplicate", gtpB.Accept(pt, bm.Epoch).Status);
        }
    }

    [Fact]
    public void Unicode_Text()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gtpA = GtpClient.Create())
        using (var gtpB = GtpClient.Create())
        {
            const string text = "Привет мир 🌍";
            var frame = gtpA.Send(an, am, 2, 5, text);
            var evs = H.TextEvents(bn.OnWire(bm, frame.Wire));
            Assert.Equal(text, gtpB.Accept(evs[0].Plaintext!, bm.Epoch).Text);
        }
    }

    [Fact]
    public void Reset_Clears_Dedup()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gtpA = GtpClient.Create())
        using (var gtpB = GtpClient.Create())
        {
            var frame = gtpA.Send(an, am, 2, 7, "test");
            var pt = H.TextEvents(bn.OnWire(bm, frame.Wire))[0].Plaintext!;
            Assert.Equal("new", gtpB.Accept(pt, bm.Epoch).Status);
            gtpB.Reset();
            Assert.Equal("new", gtpB.Accept(pt, bm.Epoch).Status);
        }
    }
}

// ---------------------------------------------------------------------------
// GAP — audio
// ---------------------------------------------------------------------------

public class GapClientTests
{
    [Fact]
    public void Basic_Audio_Frame()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gapA = GapClient.Create())
        using (var gapB = GapClient.Create())
        {
            var frame = gapA.Send(an, am, 2, 1, 0, new byte[60]);
            var evs = H.AudioEvents(bn.OnWire(bm, frame.Wire));
            Assert.Single(evs);
            var r = gapB.Accept(evs[0].Plaintext!, bm.Epoch);
            Assert.Equal("new", r.Status);
            Assert.Equal(1U, r.Source);
        }
    }

    [Fact]
    public void Multiple_Frames_In_Order()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gapA = GapClient.Create())
        using (var gapB = GapClient.Create())
        {
            for (uint i = 0; i < 5; i++)
            {
                var frame = gapA.Send(an, am, 2, 1, (ulong)(i * 960), new[] { (byte)i });
                var evs = H.AudioEvents(bn.OnWire(bm, frame.Wire));
                var r = gapB.Accept(evs[0].Plaintext!, bm.Epoch);
                Assert.Equal("new", r.Status);
                Assert.Equal(i + 1, r.Seq);
            }
        }
    }

    [Fact]
    public void Multiple_Sources()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gapA = GapClient.Create())
        using (var gapB = GapClient.Create())
        {
            foreach (uint src in new[] { 10U, 20U })
            {
                var frame = gapA.Send(an, am, 2, src, 0, new[] { (byte)src });
                var evs = H.AudioEvents(bn.OnWire(bm, frame.Wire));
                var r = gapB.Accept(evs[0].Plaintext!, bm.Epoch);
                Assert.Equal("new", r.Status);
                Assert.Equal(src, r.Source);
            }
        }
    }

    [Fact]
    public void Reset_Does_Not_Throw()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gapA = GapClient.Create())
        using (var gapB = GapClient.Create())
        {
            var frame = gapA.Send(an, am, 2, 1, 0, new byte[5]);
            var evs = H.AudioEvents(bn.OnWire(bm, frame.Wire));
            gapB.Accept(evs[0].Plaintext!, bm.Epoch);
            gapB.Reset();
        }
    }
}

// ---------------------------------------------------------------------------
// GSP — signals
// ---------------------------------------------------------------------------

public class GspClientTests
{
    private static GspAcceptResult SendSignal(
        SignalType signal, uint requestId,
        MlsContext am, GroupNode an, MlsContext bm, GroupNode bn,
        uint roleClaim = 0, byte[]? args = null)
    {
        using var gspA = GspClient.Create();
        using var gspB = GspClient.Create();
        OutboundFrame frame;
        if (args is null)
            frame = gspA.Send(an, am, 2, signal, roleClaim, requestId);
        else
            frame = gspA.SendWithArgs(an, am, 2, signal, roleClaim, requestId, args);
        var evs = H.SignalEvents(bn.OnWire(bm, frame.Wire));
        Assert.Single(evs);
        return gspB.Accept(evs[0].Plaintext!, bm.Epoch);
    }

    [Fact]
    public void Join_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.Join, 1, am, an, bm, bn); Assert.Equal("new", r.Status); Assert.Equal(SignalType.Join, r.SignalCode); }
    }

    [Fact]
    public void Leave_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.Leave, 2, am, an, bm, bn); Assert.Equal("new", r.Status); Assert.Equal(SignalType.Leave, r.SignalCode); }
    }

    [Fact]
    public void Mute_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.Mute, 3, am, an, bm, bn, args: Cbor.Map1(0, 2)); Assert.Equal("new", r.Status); Assert.Equal(SignalType.Mute, r.SignalCode); }
    }

    [Fact]
    public void Unmute_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.Unmute, 4, am, an, bm, bn, args: Cbor.Map1(0, 2)); Assert.Equal("new", r.Status); Assert.Equal(SignalType.Unmute, r.SignalCode); }
    }

    [Fact]
    public void RoleChange_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.RoleChange, 5, am, an, bm, bn, 7, args: Cbor.Map2(0, 2, 1, 7)); Assert.Equal("new", r.Status); Assert.Equal(SignalType.RoleChange, r.SignalCode); }
    }

    [Fact]
    public void StreamStart_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.StreamStart, 6, am, an, bm, bn, args: Cbor.Map1(0, 1)); Assert.Equal("new", r.Status); Assert.Equal(SignalType.StreamStart, r.SignalCode); }
    }

    [Fact]
    public void StreamStop_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.StreamStop, 7, am, an, bm, bn, args: Cbor.Map1(0, 1)); Assert.Equal("new", r.Status); Assert.Equal(SignalType.StreamStop, r.SignalCode); }
    }

    [Fact]
    public void CodecUpdate_Signal()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        { var r = SendSignal(SignalType.CodecUpdate, 8, am, an, bm, bn, args: Cbor.Map1(0, 1)); Assert.Equal("new", r.Status); Assert.Equal(SignalType.CodecUpdate, r.SignalCode); }
    }

    [Fact]
    public void Duplicate_Signal_Rejected()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gspA = GspClient.Create())
        using (var gspB = GspClient.Create())
        {
            var frame = gspA.SendWithArgs(an, am, 2, SignalType.Mute, 0, 50, Cbor.Map1(0, 2));
            var pt = H.SignalEvents(bn.OnWire(bm, frame.Wire))[0].Plaintext!;
            Assert.Equal("new", gspB.Accept(pt, bm.Epoch).Status);
            Assert.Equal("duplicate", gspB.Accept(pt, bm.Epoch).Status);
        }
    }

    [Fact]
    public void Sender_Id()
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        {
            var r = SendSignal(SignalType.Join, 9, am, an, bm, bn);
            Assert.Equal(1U, r.Sender);
        }
    }
}

// ---------------------------------------------------------------------------
// User lifecycle
// ---------------------------------------------------------------------------

public class UserLifecycleTests
{
    [Fact]
    public void Three_Member_Group_Broadcast()
    {
        using var aliceMls = MlsContext.Create("alice");
        using var bobMls = MlsContext.Create("bob");
        using var carolMls = MlsContext.Create("carol");

        bobMls.AcceptWelcome(aliceMls.Invite(bobMls.ExportKeyPackage()));
        var res = aliceMls.InviteFull(carolMls.ExportKeyPackage());
        aliceMls.FinalizeCommit();
        bobMls.ProcessMessage(res.Commit);
        bobMls.FinalizeCommit();
        carolMls.AcceptWelcome(res.Welcome);

        Assert.Equal(aliceMls.Epoch, bobMls.Epoch);
        Assert.Equal(aliceMls.Epoch, carolMls.Epoch);

        var gid = aliceMls.GroupId;
        using var aliceNode = GroupNode.Create(1, gid);
        using var bobNode = GroupNode.Create(2, gid);
        using var carolNode = GroupNode.Create(3, gid);
        aliceNode.BootstrapAsCreator(aliceMls.Epoch);
        bobNode.BootstrapAsJoiner(bobMls.Epoch);
        carolNode.BootstrapAsJoiner(carolMls.Epoch);

        using var gtpAlice = GtpClient.Create();
        using var gtpBob = GtpClient.Create();
        using var gtpCarol = GtpClient.Create();

        var frame = gtpAlice.Send(aliceNode, aliceMls, 0, 1, "hi all");
        foreach (var (node, mls, client) in new (GroupNode, MlsContext, GtpClient)[] {
            (bobNode, bobMls, gtpBob), (carolNode, carolMls, gtpCarol)
        })
        {
            var r = client.Accept(H.TextEvents(node.OnWire(mls, frame.Wire))[0].Plaintext!, mls.Epoch);
            Assert.Equal("hi all", r.Text);
        }
    }

    [Fact]
    public void Member_Rejoin()
    {
        using var aliceMls = MlsContext.Create("alice");
        using var bobMls = MlsContext.Create("bob");
        bobMls.AcceptWelcome(aliceMls.Invite(bobMls.ExportKeyPackage()));
        var gid = aliceMls.GroupId;

        // Temporary bob node — disposed before remove
        { using var bobNode = GroupNode.Create(2, gid); bobNode.BootstrapAsJoiner(bobMls.Epoch); }

        aliceMls.RemoveMember(1);
        aliceMls.FinalizeCommit();

        using var bobMls2 = MlsContext.Create("bob2");
        bobMls2.AcceptWelcome(aliceMls.Invite(bobMls2.ExportKeyPackage()));

        // Bootstrap alice AFTER all epoch advances so node epoch matches MLS epoch
        using var aliceNode = GroupNode.Create(1, gid);
        aliceNode.BootstrapAsCreator(aliceMls.Epoch);
        using var bobNode2 = GroupNode.Create(3, gid);
        bobNode2.BootstrapAsJoiner(bobMls2.Epoch);

        using var gtpAlice = GtpClient.Create();
        using var gtpBob2 = GtpClient.Create();

        var frame = gtpAlice.Send(aliceNode, aliceMls, 3, 1, "welcome back");
        var r = gtpBob2.Accept(H.TextEvents(bobNode2.OnWire(bobMls2, frame.Wire))[0].Plaintext!, bobMls2.Epoch);
        Assert.Equal("welcome back", r.Text);
    }
}

// ---------------------------------------------------------------------------
// Coordinator events
// ---------------------------------------------------------------------------

public class CoordinatorEventTests
{
    [Fact]
    public void NodeEvent_Has_Claimant_Field_Null_By_Default()
    {
        var ev = new NodeEvent("coordinator_election_needed");
        Assert.Null(ev.Claimant);
    }

    [Fact]
    public void NodeEvent_Became_Coordinator_No_Claimant()
    {
        var ev = new NodeEvent("became_coordinator");
        Assert.Null(ev.Claimant);
        Assert.Equal("became_coordinator", ev.Kind);
    }

    [Fact]
    public void NodeEvent_Coordinator_Claim_With_Claimant()
    {
        var ev = new NodeEvent("coordinator_claim", Claimant: 42U);
        Assert.Equal(42U, ev.Claimant);
        Assert.Equal("coordinator_claim", ev.Kind);
    }

    [Fact]
    public void NodeEvent_Election_Needed_Kind()
    {
        var ev = new NodeEvent("coordinator_election_needed");
        Assert.Equal("coordinator_election_needed", ev.Kind);
    }
}

// ---------------------------------------------------------------------------
// MessageHistory + Watermark
// ---------------------------------------------------------------------------

public class MessageHistoryTests
{
    [Fact]
    public void Push_And_Contains()
    {
        var h = new MessageHistory(10);
        Assert.True(h.Push(new MessageEntry(1, 100, "hello")));
        Assert.True(h.Contains(1, 100));
    }

    [Fact]
    public void Duplicate_Push_Returns_False()
    {
        var h = new MessageHistory(10);
        var e = new MessageEntry(1, 1, "x");
        h.Push(e);
        Assert.False(h.Push(e));
    }

    [Fact]
    public void Capacity_Limit()
    {
        var h = new MessageHistory(3);
        for (int i = 0; i < 5; i++) h.Push(new MessageEntry(1, (ulong)i, i.ToString()));
        Assert.Equal(3, h.Count);
    }

    [Fact]
    public void Since_Watermark()
    {
        var h = new MessageHistory(100);
        for (ulong i = 0; i < 5; i++) h.Push(new MessageEntry(1, i, i.ToString()));
        var w = new Watermark();
        w.Observe(1, 2);
        var ids = h.Since(w).Select(m => m.MessageId).ToArray();
        Assert.Equal(new[] { 3UL, 4UL }, ids);
    }

    [Fact]
    public void Since_For_Sender()
    {
        var h = new MessageHistory(100);
        h.Push(new MessageEntry(1, 10, "a"));
        h.Push(new MessageEntry(2, 20, "b"));
        h.Push(new MessageEntry(1, 30, "c"));
        var msgs = h.SinceForSender(1, 10).ToList();
        Assert.Single(msgs);
        Assert.Equal(30UL, msgs[0].MessageId);
    }

    [Fact]
    public void Clear()
    {
        var h = new MessageHistory(10);
        h.Push(new MessageEntry(1, 1, "hi"));
        h.Clear();
        Assert.Equal(0, h.Count);
    }

    [Fact]
    public void Zero_Capacity_Throws()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new MessageHistory(0));
    }
}

public class WatermarkTests
{
    [Fact]
    public void Observe_And_LastSeen()
    {
        var w = new Watermark();
        w.Observe(1, 5); w.Observe(1, 3); w.Observe(2, 10);
        Assert.Equal(5UL, w.LastSeen(1));
        Assert.Equal(10UL, w.LastSeen(2));
        Assert.Null(w.LastSeen(3));
    }

    [Fact]
    public void Snapshot()
    {
        var w = new Watermark();
        w.Observe(1, 5); w.Observe(2, 10);
        var snap = w.Snapshot();
        Assert.Equal(5UL, snap[1]);
        Assert.Equal(10UL, snap[2]);
    }

    [Fact]
    public void Clear_Removes_Entries()
    {
        var w = new Watermark();
        w.Observe(1, 1);
        w.Clear();
        Assert.Null(w.LastSeen(1));
        Assert.Empty(w.Snapshot());
    }
}

// ---------------------------------------------------------------------------
// JitterBuffer
// ---------------------------------------------------------------------------

public class JitterBufferTests
{
    [Fact]
    public void Accepted_Frame()
    {
        var buf = new JitterBuffer(10);
        var r = buf.Push(new AudioFrame(1, 0, new byte[] { 1 }));
        Assert.Equal(JitterPushOutcome.Accepted, r.Outcome);
        Assert.Null(r.Evicted);
    }

    [Fact]
    public void Pop_In_Order()
    {
        var buf = new JitterBuffer(10);
        buf.Push(new AudioFrame(1, 0, new byte[] { 0 }));
        buf.Push(new AudioFrame(1, 1, new byte[] { 1 }));
        var f0 = buf.PopInOrder(1);
        Assert.NotNull(f0); Assert.Equal(0U, f0!.RtpSequence);
        var f1 = buf.PopInOrder(1);
        Assert.NotNull(f1); Assert.Equal(1U, f1!.RtpSequence);
    }

    [Fact]
    public void Pop_Force_Skips_Gap()
    {
        var buf = new JitterBuffer(10);
        buf.Push(new AudioFrame(1, 0, Array.Empty<byte>()));
        buf.PopInOrder(1);
        buf.Push(new AudioFrame(1, 5, Array.Empty<byte>()));
        var f = buf.PopForce(1);
        Assert.NotNull(f); Assert.Equal(5U, f!.RtpSequence);
    }

    [Fact]
    public void Late_Frame()
    {
        var buf = new JitterBuffer(10);
        buf.Push(new AudioFrame(1, 5, Array.Empty<byte>()));
        buf.PopForce(1);
        Assert.Equal(JitterPushOutcome.Late, buf.Push(new AudioFrame(1, 3, Array.Empty<byte>())).Outcome);
    }

    [Fact]
    public void Eviction_On_Overflow()
    {
        var buf = new JitterBuffer(2);
        buf.Push(new AudioFrame(1, 0, Array.Empty<byte>()));
        buf.Push(new AudioFrame(1, 1, Array.Empty<byte>()));
        var r = buf.Push(new AudioFrame(1, 2, Array.Empty<byte>()));
        Assert.Equal(JitterPushOutcome.Evicted, r.Outcome);
        Assert.NotNull(r.Evicted);
    }

    [Fact]
    public void Multiple_Sources_Independent()
    {
        var buf = new JitterBuffer(5);
        buf.Push(new AudioFrame(1, 0, new byte[] { 0xAA }));
        buf.Push(new AudioFrame(2, 0, new byte[] { 0xBB }));
        var f1 = buf.PopInOrder(1);
        var f2 = buf.PopInOrder(2);
        Assert.NotNull(f1); Assert.Equal(0xAA, f1!.Plaintext[0]);
        Assert.NotNull(f2); Assert.Equal(0xBB, f2!.Plaintext[0]);
    }

    [Fact]
    public void Length_For()
    {
        var buf = new JitterBuffer(10);
        Assert.Equal(0, buf.LengthFor(1));
        buf.Push(new AudioFrame(1, 0, Array.Empty<byte>()));
        Assert.Equal(1, buf.LengthFor(1));
    }

    [Fact]
    public void Clear()
    {
        var buf = new JitterBuffer(10);
        buf.Push(new AudioFrame(1, 0, Array.Empty<byte>()));
        buf.Clear();
        Assert.Equal(0, buf.LengthFor(1));
    }

    [Fact]
    public void Out_Of_Order_Reordering()
    {
        var buf = new JitterBuffer(10);
        buf.Push(new AudioFrame(1, 2, Array.Empty<byte>()));
        buf.Push(new AudioFrame(1, 0, Array.Empty<byte>()));
        buf.Push(new AudioFrame(1, 1, Array.Empty<byte>()));
        var seqs = new List<uint>();
        AudioFrame? f;
        while ((f = buf.PopInOrder(1)) != null) seqs.Add(f.RtpSequence);
        Assert.Equal(new[] { 0U, 1U, 2U }, seqs.ToArray());
    }

    [Fact]
    public void Invalid_Capacity_Throws()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new JitterBuffer(0));
    }
}

// ---------------------------------------------------------------------------
// RoleRegistry + CapabilitiesNegotiator
// ---------------------------------------------------------------------------

public class RoleRegistryTests
{
    [Fact]
    public void Define_And_Assign()
    {
        var reg = new RoleRegistry();
        reg.DefineRole(1, "mod", Permissions.SendText | Permissions.MuteOthers);
        reg.Assign(42, 1);
        Assert.Equal(Permissions.SendText | Permissions.MuteOthers, reg.PermissionsOf(42));
    }

    [Fact]
    public void Has_Permission()
    {
        var reg = new RoleRegistry();
        reg.DefineRole(2, "viewer", Permissions.SendText);
        reg.Assign(1, 2);
        Assert.True(reg.Has(1, Permissions.SendText));
        Assert.False(reg.Has(1, Permissions.MuteOthers));
    }

    [Fact]
    public void Require_Throws_On_Missing()
    {
        var reg = new RoleRegistry();
        reg.DefineRole(1, "guest", Permissions.None);
        reg.Assign(5, 1);
        Assert.Throws<RoleException>(() => reg.Require(5, Permissions.SendText));
    }

    [Fact]
    public void Require_Passes_When_Has_Permission()
    {
        var reg = new RoleRegistry();
        reg.DefineRole(1, "admin", Permissions.CloseGroup | Permissions.AssignRoles);
        reg.Assign(5, 1);
        reg.Require(5, Permissions.CloseGroup);
    }

    [Fact]
    public void Unknown_Role_Throws()
    {
        Assert.Throws<RoleException>(() => new RoleRegistry().Assign(1, 999));
    }

    [Fact]
    public void No_Role_Gives_None_Permissions()
    {
        Assert.Equal(Permissions.None, new RoleRegistry().PermissionsOf(99));
    }

    [Fact]
    public void Role_Of()
    {
        var reg = new RoleRegistry();
        reg.DefineRole(3, "speaker", Permissions.SendAudio);
        reg.Assign(10, 3);
        Assert.Equal("speaker", reg.RoleOf(10)?.Name);
    }

    [Fact]
    public void All_Permission_Bits()
    {
        var all = Permissions.SendText | Permissions.SendAudio | Permissions.SendSignal
                | Permissions.MuteOthers | Permissions.AssignRoles | Permissions.Invite
                | Permissions.RemoveMembers | Permissions.CloseGroup;
        var reg = new RoleRegistry();
        reg.DefineRole(10, "superadmin", all);
        reg.Assign(1, 10);
        Assert.True(reg.Has(1, Permissions.SendText));
        Assert.True(reg.Has(1, Permissions.CloseGroup));
    }
}

public class CapabilitiesNegotiatorTests
{
    [Fact]
    public void Advertise_And_GroupSupports()
    {
        var neg = new CapabilitiesNegotiator();
        neg.Advertise(1, new[] { "audio", "video" });
        neg.Advertise(2, new[] { "audio" });
        Assert.True(neg.GroupSupports("audio"));
        Assert.False(neg.GroupSupports("video"));
    }

    [Fact]
    public void Intersection()
    {
        var neg = new CapabilitiesNegotiator();
        neg.Advertise(1, new[] { "a", "b", "c" });
        neg.Advertise(2, new[] { "b", "c" });
        Assert.Equal(new HashSet<string> { "b", "c" }, neg.Intersection().ToHashSet());
    }

    [Fact]
    public void Union()
    {
        var neg = new CapabilitiesNegotiator();
        neg.Advertise(1, new[] { "a" });
        neg.Advertise(2, new[] { "b" });
        Assert.Equal(new HashSet<string> { "a", "b" }, neg.Union().ToHashSet());
    }

    [Fact]
    public void Missing()
    {
        var neg = new CapabilitiesNegotiator();
        neg.Advertise(1, new[] { "x" });
        neg.Advertise(2, Array.Empty<string>());
        var m = neg.Missing("x");
        Assert.Contains(2U, m);
        Assert.DoesNotContain(1U, m);
    }

    [Fact]
    public void Forget()
    {
        var neg = new CapabilitiesNegotiator();
        neg.Advertise(1, new[] { "a" });
        neg.Forget(1);
        Assert.Equal(0, neg.Count);
    }

    [Fact]
    public void Capabilities_Of()
    {
        var neg = new CapabilitiesNegotiator();
        neg.Advertise(5, new[] { "alpha", "beta" });
        var caps = neg.CapabilitiesOf(5);
        Assert.NotNull(caps);
        Assert.Contains("alpha", caps!);
    }

    [Fact]
    public void Empty_Intersection()
    {
        Assert.Empty(new CapabilitiesNegotiator().Intersection());
    }

    [Fact]
    public void GroupSupports_False_When_Empty()
    {
        Assert.False(new CapabilitiesNegotiator().GroupSupports("x"));
    }

    [Fact]
    public void Update_Advertisement()
    {
        var neg = new CapabilitiesNegotiator();
        neg.Advertise(1, new[] { "a" });
        neg.Advertise(1, new[] { "b" });
        var caps = neg.CapabilitiesOf(1);
        Assert.NotNull(caps);
        Assert.Contains("b", caps!);
        Assert.DoesNotContain("a", caps!);
    }
}

// ---------------------------------------------------------------------------
// SFrame E2EE
// ---------------------------------------------------------------------------

public class SFrameSessionTests
{
    private static (MlsContext Alice, MlsContext Bob) Pair()
    {
        var alice = MlsContext.Create("alice");
        var bob = MlsContext.Create("bob");
        bob.AcceptWelcome(alice.Invite(bob.ExportKeyPackage()));
        return (alice, bob);
    }

    [Fact]
    public void Aes128_Encrypt_Decrypt()
    {
        var (alice, bob) = Pair();
        using (alice) using (bob)
        using (var aliceSess = SFrameSession.Create(alice, suite: SFrameCipherSuite.Aes128Gcm))
        using (var bobSess = SFrameSession.Create(bob, suite: SFrameCipherSuite.Aes128Gcm))
        using (var enc = aliceSess.CreateEncryptor(alice, 0))
        {
            var plaintext = Encoding.UTF8.GetBytes("opus_data");
            var ct = enc.Encrypt(plaintext);
            Assert.NotEqual(plaintext, ct);
            var (pt, leaf) = bobSess.Decrypt(ct);
            Assert.Equal(plaintext, pt);
            Assert.Equal(0U, leaf);
        }
    }

    [Fact]
    public void Aes256_Encrypt_Decrypt()
    {
        var (alice, bob) = Pair();
        using (alice) using (bob)
        using (var aliceSess = SFrameSession.Create(alice, suite: SFrameCipherSuite.Aes256Gcm))
        using (var bobSess = SFrameSession.Create(bob, suite: SFrameCipherSuite.Aes256Gcm))
        using (var enc = aliceSess.CreateEncryptor(alice, 0, suite: SFrameCipherSuite.Aes256Gcm))
        {
            var pt = new byte[] { 1, 2, 3 };
            var (dec, _) = bobSess.Decrypt(enc.Encrypt(pt));
            Assert.Equal(pt, dec);
        }
    }

    [Fact]
    public void Extra_Aad()
    {
        var (alice, bob) = Pair();
        using (alice) using (bob)
        using (var aliceSess = SFrameSession.Create(alice))
        using (var bobSess = SFrameSession.Create(bob))
        using (var enc = aliceSess.CreateEncryptor(alice, 0))
        {
            ReadOnlySpan<byte> aad = Encoding.UTF8.GetBytes("stream-42");
            var pt = new byte[] { 0xFF };
            var ct = enc.Encrypt(pt, aad);
            var (dec, _) = bobSess.Decrypt(ct, aad);
            Assert.Equal(pt, dec);
        }
    }

    [Fact]
    public void Multiple_Frames()
    {
        var (alice, bob) = Pair();
        using (alice) using (bob)
        using (var aliceSess = SFrameSession.Create(alice))
        using (var bobSess = SFrameSession.Create(bob))
        using (var enc = aliceSess.CreateEncryptor(alice, 0))
        {
            for (int i = 0; i < 10; i++)
            {
                var pt = new[] { (byte)i };
                var (dec, _) = bobSess.Decrypt(enc.Encrypt(pt));
                Assert.Equal(pt, dec);
            }
        }
    }

    [Fact]
    public void Wrong_Aad_Fails()
    {
        var (alice, bob) = Pair();
        using (alice) using (bob)
        using (var aliceSess = SFrameSession.Create(alice))
        using (var bobSess = SFrameSession.Create(bob))
        using (var enc = aliceSess.CreateEncryptor(alice, 0))
        {
            var ct = enc.Encrypt(new byte[] { 1 }, Encoding.UTF8.GetBytes("correct"));
            Assert.Throws<InvalidOperationException>(() =>
            {
                _ = bobSess.Decrypt(ct, Encoding.UTF8.GetBytes("wrong"));
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Payload codec roundtrip tests
// ---------------------------------------------------------------------------

public class GtpCodecTests
{
    [Theory]
    [InlineData(PayloadCodec.Cbor)]
    [InlineData(PayloadCodec.Protobuf)]
    [InlineData(PayloadCodec.FlatBuffers)]
    public void Roundtrip(PayloadCodec codec)
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gtpA = GtpClient.Create())
        using (var gtpB = GtpClient.Create())
        {
            var frame = gtpA.Send(an, am, 2, 1, "hello codec", codec);
            var evs = H.TextEvents(bn.OnWire(bm, frame.Wire));
            Assert.Single(evs);
            Assert.Equal(codec, evs[0].Codec ?? PayloadCodec.Cbor);
            var r = gtpB.Accept(evs[0].Plaintext!, bm.Epoch, codec);
            Assert.Equal("new", r.Status);
            Assert.Equal("hello codec", r.Text);
        }
    }
}

public class GapCodecTests
{
    [Theory]
    [InlineData(PayloadCodec.Cbor)]
    [InlineData(PayloadCodec.Protobuf)]
    [InlineData(PayloadCodec.FlatBuffers)]
    public void Roundtrip(PayloadCodec codec)
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gapA = GapClient.Create())
        using (var gapB = GapClient.Create())
        {
            var frame = gapA.Send(an, am, 2, 7, 0, new byte[40], codec);
            var evs = H.AudioEvents(bn.OnWire(bm, frame.Wire));
            Assert.Single(evs);
            Assert.Equal(codec, evs[0].Codec ?? PayloadCodec.Cbor);
            var r = gapB.Accept(evs[0].Plaintext!, bm.Epoch, codec);
            Assert.Equal("new", r.Status);
            Assert.Equal(7U, r.Source);
        }
    }
}

public class GspCodecTests
{
    [Theory]
    [InlineData(PayloadCodec.Cbor)]
    [InlineData(PayloadCodec.Protobuf)]
    [InlineData(PayloadCodec.FlatBuffers)]
    public void Roundtrip(PayloadCodec codec)
    {
        var (am, an, bm, bn) = H.TwoMemberGroup();
        using (am) using (an) using (bm) using (bn)
        using (var gspA = GspClient.Create())
        using (var gspB = GspClient.Create())
        {
            var frame = gspA.Send(an, am, 2, SignalType.Join, 0, 1, codec);
            var evs = H.SignalEvents(bn.OnWire(bm, frame.Wire));
            Assert.Single(evs);
            Assert.Equal(codec, evs[0].Codec ?? PayloadCodec.Cbor);
            var r = gspB.Accept(evs[0].Plaintext!, bm.Epoch, codec);
            Assert.Equal("new", r.Status);
            Assert.Equal(SignalType.Join, r.SignalCode);
        }
    }
}

// ---------------------------------------------------------------------------
// GbpHelpers
// ---------------------------------------------------------------------------

public class GbpHelpersTests
{
    [Fact]
    public void EncodeFrame_Returns_Bytes()
    {
        using var mls = MlsContext.Create("alice");
        var r = GbpHelpers.EncodeFrame(1, mls.GroupId, 1, 0, 2, 0, 0, 1, new byte[] { 1, 2, 3 });
        Assert.True(r.Length > 0);
    }

    [Fact]
    public void EncodeFrame_Bad_GroupId_Throws()
    {
        Assert.Throws<ArgumentException>(() =>
            GbpHelpers.EncodeFrame(1, new byte[5], 1, 0, 2, 0, 0, 1, Array.Empty<byte>()));
    }

    [Fact]
    public void LookupError_Unknown_Returns_Null()
    {
        Assert.Null(GbpHelpers.LookupError(0xFFFF));
    }

    [Fact]
    public void LookupError_Known_Code()
    {
        var r = GbpHelpers.LookupError(0x0001);
        if (r != null) Assert.True(r.Length > 0);
    }

    [Fact]
    public void EncodeFrame_Empty_Payload()
    {
        using var mls = MlsContext.Create("alice");
        var r = GbpHelpers.EncodeFrame(1, mls.GroupId, 1, 0, 0, 0, 0, 0, Array.Empty<byte>());
        Assert.NotNull(r);
    }
}
