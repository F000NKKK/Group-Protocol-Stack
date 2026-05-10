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
├──────────────────────────────────────────────────────────────────────┤
│   GroupNode (GBP — IP-like base)                                    │
├──────────────────────────────────────────────────────────────────────┤
│   MlsContext (RFC 9420)                                             │
└──────────────────────────────────────────────────────────────────────┘
```

## Quick start

```csharp
using GBPStack;

using var aliceMls = MlsContext.Create("alice");
using var bobMls   = MlsContext.Create("bob");

var bobKp   = bobMls.ExportKeyPackage();
var welcome = aliceMls.Invite(bobKp);
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
        Console.WriteLine(bobGtp.Accept(ev.Plaintext!).Text);
```

## License

Licensed under [Apache License, Version 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE).
