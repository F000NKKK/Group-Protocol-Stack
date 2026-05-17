# gbp-stack — Python bindings for the Group Protocol Stack

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE)

Python bindings for the [Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack):
a layered, end-to-end encrypted group-messaging protocol family built on top
of [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420).

This package wraps the native `gbp_stack` shared library through `ctypes`.
The wheel for each supported platform bundles the appropriate native binary
under `gbp_stack/_native/<rid>/`.

## Layers

```
┌── application ──────────────────────────────────────────────────────┐
│   GtpClient · GapClient · GspClient   (TCP / UDP / SCTP-like)       │
├─────────────────────────────────────────────────────────────────────┤
│   GroupNode (GBP — IP-like base)                                    │
├─────────────────────────────────────────────────────────────────────┤
│   MlsContext (RFC 9420)                                             │
└─────────────────────────────────────────────────────────────────────┘
```

## Sub-protocol toolkits

Beyond the protocol clients, the package ships ready-made helpers:

* `MessageHistory` + `Watermark` — bounded GTP message log + per-sender
  high-water mark for serving and consuming resync requests.
* `JitterBuffer` — bounded GAP reorder window keyed by `media_source_id`,
  with `push`, `pop_in_order`, `pop_force` and late-frame detection.
* `RoleRegistry` + `Permissions` — bind numeric role ids to permission
  bit-masks and check them with `require` / `has`.
* `CapabilitiesNegotiator` — track per-member advertisements and query the
  `intersection`, `union`, `group_supports` and `missing` views.
* `SFrameSession` + `SFrameEncryptor` — SFrame (draft-ietf-sframe-enc) E2EE
  for GAP audio frames; per-sender AES-GCM keys derived from MLS exporter,
  1024-entry sliding-window replay protection.
* `encode_gbp_frame` — low-level helper to construct a raw CBOR GBP frame.
* `lookup_error` — return the CBOR `ErrorObject` for a known error code.

### Coordinator events

`NodeEvent` surfaces three new event kinds for coordinator election:

| `kind` | Extra fields | Meaning |
|--------|-------------|---------|
| `coordinator_election_needed` | — | The local node should initiate GSP `COORDINATOR_CLAIM` |
| `became_coordinator` | — | This node won the election |
| `coordinator_claim` | `claimant` | A peer sent `COORDINATOR_CLAIM` with this member id |

## Install

```sh
pip install gbp-stack==1.3.0
```

## Quick start

```python
from gbp_stack import MlsContext, GroupNode, GtpClient

with MlsContext.create("alice") as alice_mls, \
     MlsContext.create("bob")   as bob_mls:

    bob_kp  = bob_mls.export_key_package()
    welcome = alice_mls.invite(bob_kp)
    bob_mls.accept_welcome(welcome)

    group_id = alice_mls.group_id
    with GroupNode.create(member_id=1, group_id=group_id) as alice, \
         GroupNode.create(member_id=2, group_id=group_id) as bob, \
         GtpClient.create() as gtp_alice, \
         GtpClient.create() as gtp_bob:

        alice.bootstrap_as_creator(alice_mls.epoch)
        bob.bootstrap_as_joiner(bob_mls.epoch)

        frame = gtp_alice.send(alice, alice_mls, target=2,
                                message_id=0xCAFE_F00D, text="hello")
        for ev in bob.on_wire(bob_mls, frame.wire):
            if ev.kind == "payload_received" and ev.stream_type == 2:
                print(gtp_bob.accept(ev.plaintext).text)
```

## License

Licensed under [Apache License, Version 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE).
