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

## Payload codec

Each sub-protocol payload can be encoded as **CBOR** (default), **Protobuf**,
or **FlatBuffers**. Pass `PayloadCodec` to `send` and `accept`; the chosen
codec is surfaced in `ev.codec` on `payload_received` events.

```python
from gbp_stack import GtpClient, PayloadCodec

frame = gtp_alice.send(alice, alice_mls, target=2, message_id=1,
                       text="hello", codec=PayloadCodec.FLATBUFFERS)
for ev in bob.on_wire(bob_mls, frame.wire):
    if ev.kind == "payload_received":
        codec = ev.codec or PayloadCodec.CBOR
        result = gtp_bob.accept(ev.plaintext, bob_mls.epoch, codec=codec)
        print(result.text)
```

| Value | Name | Description |
|-------|------|-------------|
| `0`   | `PayloadCodec.CBOR`         | Default; `pf` field omitted from wire |
| `1`   | `PayloadCodec.PROTOBUF`     | Protobuf via `gbp-proto` |
| `2`   | `PayloadCodec.FLATBUFFERS`  | FlatBuffers via `gbp-flat`; lowest latency |

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
pip install gbp-stack==1.8.1
```

## Quick start

```python
from gbp_stack import MlsContext, GroupNode, GtpClient

with MlsContext.create("alice") as alice_mls, \
     MlsContext.create("bob")   as bob_mls:

    bob_kp  = bob_mls.export_key_package()
    welcome = alice_mls.invite(bob_kp)       # alice auto-finalizes; epoch advances to 1
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
            if ev.kind == "payload_received" and ev.stream_type == 2:  # StreamType.Text
                result = gtp_bob.accept(ev.plaintext, bob_mls.epoch)
                print(result.text)   # → "hello"
                # result.status is "new" (first message from this sender)
                # subsequent messages → "new"; duplicates → "duplicate"
```

## GSP signals with per-signal arguments

Signals that target a specific member or resource require CBOR-encoded `args`.
The `send` method accepts an optional `args: bytes` keyword argument.

```python
import struct
from gbp_stack import GspClient, SignalType

# Minimal CBOR helpers
def cbor_uint(n: int) -> bytes:
    if n <= 23:     return bytes([n])
    if n <= 0xFF:   return bytes([0x18, n])
    if n <= 0xFFFF: return bytes([0x19, n >> 8, n & 0xFF])
    return bytes([0x1A, (n>>24)&0xFF, (n>>16)&0xFF, (n>>8)&0xFF, n&0xFF])

def cbor_map1(k: int, v: int) -> bytes:
    return bytes([0xA1]) + cbor_uint(k) + cbor_uint(v)

def cbor_map2(k0: int, v0: int, k1: int, v1: int) -> bytes:
    return bytes([0xA2]) + cbor_uint(k0) + cbor_uint(v0) + cbor_uint(k1) + cbor_uint(v1)

# Signal-specific args schemas:
#   MUTE / UNMUTE  → {0: target_member_id}
#   ROLE_CHANGE    → {0: target_member_id, 1: new_role_id}
#   STREAM_START / STREAM_STOP → {0: stream_type}
#   CODEC_UPDATE   → {0: codec_id}
#   JOIN / LEAVE   → no args required

with GspClient.create() as gsp_alice:
    # Mute member 3 (no role_claim needed for self-moderation)
    frame = gsp_alice.send(
        alice_node, alice_mls,
        target=0,  # 0 = broadcast
        signal=SignalType.MUTE,
        role_claim=0,
        request_id=1,
        args=cbor_map1(0, 3),  # {0: target_member_id=3}
    )
```

## MLS multi-member group pattern

When inviting a member to an **existing** group (not the first invite), use
`invite_full` so that existing members can process the commit:

```python
# Alice adds Carol to an alice+bob group
commit, welcome = alice_mls.invite_full(carol_mls.export_key_package())
alice_mls.finalize_commit()          # alice's epoch advances
bob_mls.process_message(commit)      # bob stages the commit
bob_mls.finalize_commit()            # bob's epoch advances to match alice
carol_mls.accept_welcome(welcome)    # carol joins
assert alice_mls.epoch == bob_mls.epoch == carol_mls.epoch
```

## Persisting MLS state

Serialise a context so it survives a restart, then restore it later — the
restored context is at the same epoch and can send / receive again. The blob
holds **private key material**, so store it encrypted at rest.

```python
blob = mls.export_state()                   # persist (encrypted) to disk
# ... later / after restart ...
with MlsContext.restore_state(blob, "alice") as restored:
    assert restored.epoch == mls.epoch
    assert restored.group_id == mls.group_id
```

## License

Licensed under [Apache License, Version 2.0](https://github.com/F000NKKK/Group-Protocol-Stack/blob/main/LICENSE).
