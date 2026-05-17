// Two-party GAP (audio) frame exchange — minimal in-memory end-to-end example.
//
// Demonstrates:
//   - Sending synthetic Opus frames with FlatBuffers codec (recommended for audio)
//   - Sequential frames: rtp_sequence advances automatically inside GapClient
//
// Run from repo root:
//   dotnet script csharp/examples/GapAudio.cs

using GBPStack;

using var aliceMls = MlsContext.Create("alice");
using var bobMls   = MlsContext.Create("bob");
bobMls.AcceptWelcome(aliceMls.Invite(bobMls.ExportKeyPackage()));

var gid = aliceMls.GroupId;
using var alice = GroupNode.Create(1, gid);
using var bob   = GroupNode.Create(2, gid);
alice.BootstrapAsCreator(aliceMls.Epoch);
bob.BootstrapAsJoiner(bobMls.Epoch);

using var gapAlice = GapClient.Create();
using var gapBob   = GapClient.Create();

// Synthetic 20 ms Opus frame (zeroed; real usage: encode from PCM).
var opus = new byte[40];

for (ulong i = 0; i < 3; i++)
{
    // FlatBuffers minimises decode latency on real-time audio paths.
    var frame = gapAlice.Send(alice, aliceMls,
        target: 2, mediaSourceId: 1, rtpTimestamp: i * 960, opus,
        codec: PayloadCodec.FlatBuffers);

    foreach (var ev in bob.OnWire(bobMls, frame.Wire))
    {
        if (ev.Kind == "payload_received" && ev.StreamType == StreamType.Audio)
        {
            var r = gapBob.Accept(ev.Plaintext!, bobMls.Epoch, ev.Codec ?? PayloadCodec.Cbor);
            Console.WriteLine($"frame {i + 1}: status={r.Status}  seq={r.Seq}  codec={ev.Codec}");
        }
    }
}
