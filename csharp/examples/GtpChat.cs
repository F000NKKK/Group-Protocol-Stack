// Two-party GTP (text) chat — minimal in-memory end-to-end example.
//
// Demonstrates:
//   - MLS two-party handshake
//   - GBP node bootstrap
//   - Sending text messages with CBOR and FlatBuffers codecs
//   - Idempotency: duplicate (sender, messageId) returns status "duplicate"
//
// Run from repo root:
//   dotnet script csharp/examples/GtpChat.cs
// Or add to a console project and run with `dotnet run`.

using GBPStack;

// --- MLS handshake -----------------------------------------------------------
using var aliceMls = MlsContext.Create("alice");
using var bobMls   = MlsContext.Create("bob");

bobMls.AcceptWelcome(aliceMls.Invite(bobMls.ExportKeyPackage()));
Console.WriteLine($"MLS epoch after invite: alice={aliceMls.Epoch}  bob={bobMls.Epoch}");

// --- GBP nodes ---------------------------------------------------------------
var gid = aliceMls.GroupId;
using var alice = GroupNode.Create(memberId: 1, gid);
using var bob   = GroupNode.Create(memberId: 2, gid);
alice.BootstrapAsCreator(aliceMls.Epoch);
bob.BootstrapAsJoiner(bobMls.Epoch);

// --- GTP clients -------------------------------------------------------------
using var gtpAlice = GtpClient.Create();
using var gtpBob   = GtpClient.Create();

// Send "hello" with default CBOR codec.
var frame = gtpAlice.Send(alice, aliceMls, target: 2, messageId: 1, "hello");
foreach (var ev in bob.OnWire(bobMls, frame.Wire))
{
    if (ev.Kind == "payload_received" && ev.StreamType == StreamType.Text)
    {
        var r = gtpBob.Accept(ev.Plaintext!, bobMls.Epoch, ev.Codec ?? PayloadCodec.Cbor);
        Console.WriteLine($"new (cbor):   text={r.Text}  status={r.Status}");
    }
}

// Send with FlatBuffers codec.
var frame2 = gtpAlice.Send(alice, aliceMls, target: 2, messageId: 2, "hello flatbuffers",
    codec: PayloadCodec.FlatBuffers);
foreach (var ev in bob.OnWire(bobMls, frame2.Wire))
{
    if (ev.Kind == "payload_received" && ev.StreamType == StreamType.Text)
    {
        var r = gtpBob.Accept(ev.Plaintext!, bobMls.Epoch, ev.Codec ?? PayloadCodec.Cbor);
        Console.WriteLine($"new (fbs):    text={r.Text}  codec={ev.Codec}");
    }
}

// Replay: same messageId=1 must come back as "duplicate".
var dup = gtpAlice.Send(alice, aliceMls, target: 2, messageId: 1, "hello");
foreach (var ev in bob.OnWire(bobMls, dup.Wire))
{
    if (ev.Kind == "payload_received" && ev.StreamType == StreamType.Text)
    {
        var r = gtpBob.Accept(ev.Plaintext!, bobMls.Epoch);
        Console.WriteLine($"replay:       status={r.Status}");  // → duplicate
    }
}
