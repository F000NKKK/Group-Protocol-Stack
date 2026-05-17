# GBPStack — .NET bindings for the Group Protocol Stack

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE)

Managed (.NET) bindings for the [Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack):
a layered, end-to-end encrypted group-messaging protocol family built on top
of [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420).

This NuGet package bundles the managed wrapper plus the native runtime
library `gbp_stack` for every supported runtime identifier:
`win-x64`, `win-arm64`, `linux-x64`, `linux-arm64`, `osx-x64`, `osx-arm64`.
.NET picks the right binary at runtime — no extra setup is required.

## Layers

```
┌── application ──────────────────────────────────────────────────────┐
│   GtpClient · GapClient · GspClient   (TCP/UDP/SCTP-like)           │
├─────────────────────────────────────────────────────────────────────┤
│   GroupNode (GBP — IP-like base)                                    │
├─────────────────────────────────────────────────────────────────────┤
│   MlsContext (RFC 9420)                                             │
└─────────────────────────────────────────────────────────────────────┘
```

## Payload codec

Each sub-protocol payload can be encoded as **CBOR** (default), **Protobuf**,
or **FlatBuffers**. Pass `PayloadCodec` to `Send` and `Accept`; the chosen
codec is surfaced in `ev.Codec` on `payload_received` events.

```csharp
using GBPStack;

var frame = aliceGtp.Send(alice, aliceMls, target: 2, messageId: 1,
                          text: "hi", codec: PayloadCodec.FlatBuffers);
foreach (var ev in bob.OnWire(bobMls, frame.Wire))
    if (ev.Kind == "payload_received")
    {
        var codec = ev.Codec ?? PayloadCodec.Cbor;
        var r = bobGtp.Accept(ev.Plaintext!, bobMls.Epoch, codec);
        Console.WriteLine(r.Text);
    }
```

| Value | Name | Description |
|-------|------|-------------|
| `0`   | `PayloadCodec.Cbor`         | Default; `pf` field omitted from wire |
| `1`   | `PayloadCodec.Protobuf`     | Protobuf |
| `2`   | `PayloadCodec.FlatBuffers`  | FlatBuffers; lowest latency |

## Sub-protocol toolkits

Beyond the protocol clients, the package ships ready-made helpers:

* `MessageHistory` + `Watermark` — bounded GTP message log + per-sender
  high-water mark for serving and consuming resync requests.
* `JitterBuffer` — bounded GAP reorder window keyed by `MediaSourceId`,
  with `Push`, `PopInOrder`, `PopForce` and late-frame detection.
* `RoleRegistry` + `Permissions` — bind numeric role ids to permission
  bit-masks and check them with `Require` / `Has`.
* `CapabilitiesNegotiator` — track per-member advertisements and query the
  `Intersection`, `Union`, `GroupSupports` and `Missing` views.
* `SFrameSession` + `SFrameEncryptor` — SFrame (draft-ietf-sframe-enc) E2EE
  for GAP audio frames; per-sender AES-GCM keys derived from MLS exporter,
  1024-entry sliding-window replay protection.
* `GbpHelpers` — low-level utilities: `EncodeFrame` (raw GBP frame → CBOR)
  and `LookupError` (error code → CBOR `ErrorObject`).

### Coordinator events

`NodeEvent` surfaces three new event kinds for coordinator election:

| `Kind` | Extra fields | Meaning |
|--------|-------------|---------|
| `coordinator_election_needed` | — | The local node should initiate GSP `COORDINATOR_CLAIM` |
| `became_coordinator` | — | This node won the election |
| `coordinator_claim` | `Claimant` | A peer sent `COORDINATOR_CLAIM` with this member id |

## Install

```sh
dotnet add package GBPStack --version 1.5.0
```

## Quick start

```csharp
using GBPStack;

using var aliceMls = MlsContext.Create("alice");
using var bobMls   = MlsContext.Create("bob");

var bobKp   = bobMls.ExportKeyPackage();
var welcome = aliceMls.Invite(bobKp);   // alice auto-finalizes; epoch advances to 1
bobMls.AcceptWelcome(welcome);

var groupId = aliceMls.GroupId;
using var alice = GroupNode.Create(memberId: 1, groupId);
using var bob   = GroupNode.Create(memberId: 2, groupId);
alice.BootstrapAsCreator(aliceMls.Epoch);
bob.BootstrapAsJoiner(bobMls.Epoch);

using var aliceGtp = GtpClient.Create();
using var bobGtp   = GtpClient.Create();

var frame = aliceGtp.Send(alice, aliceMls, target: 2, messageId: 0xCAFE_F00D, "hello");
foreach (var ev in bob.OnWire(bobMls, frame.Wire))
    if (ev.Kind == "payload_received" && ev.StreamType == StreamType.Text)
    {
        var r = bobGtp.Accept(ev.Plaintext!, bobMls.Epoch);
        Console.WriteLine(r.Text);   // → "hello"
        // r.Status is "new" (first message from this sender)
        // subsequent messages → "new"; duplicates → "duplicate"
    }
```

## GSP signals with per-signal arguments

Signals that target a specific member or resource require CBOR-encoded arguments.
Use `GspClient.SendWithArgs` for these signals:

```csharp
// Minimal CBOR helpers
static byte[] CborUint(uint n) => n <= 23 ? new[] { (byte)n }
    : n <= 0xFF   ? new byte[] { 0x18, (byte)n }
    : n <= 0xFFFF ? new byte[] { 0x19, (byte)(n >> 8), (byte)n }
    : new byte[] { 0x1A, (byte)(n>>24), (byte)(n>>16), (byte)(n>>8), (byte)n };

static byte[] CborMap1(uint k, uint v) =>
    new byte[] { 0xA1 }.Concat(CborUint(k)).Concat(CborUint(v)).ToArray();

// Signal-specific args schemas:
//   MUTE / UNMUTE  → {0: target_member_id}
//   ROLE_CHANGE    → {0: target_member_id, 1: new_role_id}
//   STREAM_START / STREAM_STOP → {0: stream_type}
//   CODEC_UPDATE   → {0: codec_id}
//   JOIN / LEAVE   → no args; use GspClient.Send

using var gsp = GspClient.Create();

// Mute member 3
var frame = gsp.SendWithArgs(
    aliceNode, aliceMls,
    target: 0,             // 0 = broadcast
    signal: SignalType.Mute,
    roleClaim: 0,
    requestId: 1,
    args: CborMap1(0, 3)   // {0: target_member_id=3}
);
```

## MLS multi-member group pattern

When inviting a member to an **existing** group (not the first invite), use
`InviteFull` so that existing members can process the commit:

```csharp
// Alice adds Carol to an alice+bob group
var (commit, welcome) = aliceMls.InviteFull(carolMls.ExportKeyPackage());
aliceMls.FinalizeCommit();         // alice's epoch advances
bobMls.ProcessMessage(commit);     // bob stages the commit
bobMls.FinalizeCommit();           // bob's epoch advances to match alice
carolMls.AcceptWelcome(welcome);   // carol joins
Debug.Assert(aliceMls.Epoch == bobMls.Epoch && bobMls.Epoch == carolMls.Epoch);
```

## License

Licensed under [Apache License, Version 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE).
