// GSP (signaling) examples: JOIN, MUTE with CBOR args, ROLE_CHANGE.
//
// Demonstrates:
//   - Signals without args via GspClient.Send
//   - Signals with per-signal CBOR args via GspClient.SendWithArgs
//
// Run from repo root:
//   dotnet script csharp/examples/GspSignals.cs

using GBPStack;

// Minimal CBOR helpers.
static byte[] CborUint(uint n) => n <= 23
    ? new[] { (byte)n }
    : n <= 0xFF   ? new byte[] { 0x18, (byte)n }
    : n <= 0xFFFF ? new byte[] { 0x19, (byte)(n >> 8), (byte)n }
    : new byte[] { 0x1A, (byte)(n >> 24), (byte)(n >> 16), (byte)(n >> 8), (byte)n };

static byte[] CborMap1(uint k, uint v) =>
    new byte[] { 0xA1 }.Concat(CborUint(k)).Concat(CborUint(v)).ToArray();

static byte[] CborMap2(uint k0, uint v0, uint k1, uint v1) =>
    new byte[] { 0xA2 }
        .Concat(CborUint(k0)).Concat(CborUint(v0))
        .Concat(CborUint(k1)).Concat(CborUint(v1)).ToArray();

using var aliceMls = MlsContext.Create("alice");
using var bobMls   = MlsContext.Create("bob");
bobMls.AcceptWelcome(aliceMls.Invite(bobMls.ExportKeyPackage()));

var gid = aliceMls.GroupId;
using var alice = GroupNode.Create(1, gid);
using var bob   = GroupNode.Create(2, gid);
alice.BootstrapAsCreator(aliceMls.Epoch);
bob.BootstrapAsJoiner(bobMls.Epoch);

using var gspAlice = GspClient.Create();
using var gspBob   = GspClient.Create();

void Recv(byte[] wire, string label)
{
    foreach (var ev in bob.OnWire(bobMls, wire))
    {
        if (ev.Kind == "payload_received" && ev.StreamType == StreamType.Signal)
        {
            var r = gspBob.Accept(ev.Plaintext!, bobMls.Epoch, ev.Codec ?? PayloadCodec.Cbor);
            Console.WriteLine($"{label}: signal={r.Signal}  sender={r.Sender}  requestId={r.RequestId}");
        }
    }
}

// 1. JOIN — no args.
Recv(gspAlice.Send(alice, aliceMls, 0, SignalType.Join, roleClaim: 0, requestId: 1).Wire, "JOIN");

// 2. MUTE member 2 — args: {0: target_member_id=2}.
Recv(gspAlice.SendWithArgs(alice, aliceMls, 0, SignalType.Mute, 0, 2, CborMap1(0, 2)).Wire, "MUTE");

// 3. ROLE_CHANGE member 2 → role 1 — args: {0: target=2, 1: new_role=1}.
Recv(gspAlice.SendWithArgs(alice, aliceMls, 0, SignalType.RoleChange, roleClaim: 1, requestId: 3,
    CborMap2(0, 2, 1, 1)).Wire, "ROLE_CHANGE");
