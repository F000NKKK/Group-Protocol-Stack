# gbp-node

GBP-layer group node for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

This is the IP-like substrate that the three sub-protocol crates
(`gtp-protocol`, `gap-protocol`, `gsp-protocol`) build on. It owns the
framing, AEAD, replay window, FSM and control plane.

## What this crate provides

* `GroupNode` — the core node type. Call `bootstrap_as_creator` or
  `bootstrap_as_joiner` once after MLS welcome, then pipe every inbound wire
  frame through `on_wire` and route the resulting `Event`s to sub-protocol
  clients.
* `OutboundFrame` — a wire-ready `(to, wire)` pair returned by every send
  helper.
* `DeliveredPayload` — decrypted plaintext + `stream_type`, `stream_id`,
  `sequence_no`, `flags`, and `codec` surfaced to sub-protocols via
  `Event::PayloadReceived`.
* `Event` — all events emitted by the node:
  `StateChanged`, `PayloadReceived`, `Control`, `Error`, `EpochAdvanced`,
  `CoordinatorElectionNeeded`, `BecameCoordinator`, `CoordinatorClaim`.

## Example

```rust,ignore
use gbp_stack::{
    ControlOpcode, Event, GtpAccept, GtpClient, GroupNode, MlsContext,
    PayloadCodec, StreamType,
};

// After MLS two-party handshake (alice invites bob):
let gid = alice_mls.group_id_16();
let mut alice = GroupNode::new(1, gid);
let mut bob   = GroupNode::new(2, gid);
alice.bootstrap_as_creator(0);          // epoch 0
bob.bootstrap_as_joiner(0, 1);          // epoch 0, expect tid=1

// Apply epoch-1 transition.
let exec = alice.send_control(
    &mut alice_mls, 0, ControlOpcode::ExecuteTransition, 1, 7, vec![],
)?;
alice.apply_transition(1);
bob.on_wire(&mut bob_mls, &exec.wire)?;

// Send a text message via GTP.
let mut gtp_alice = GtpClient::new();
let mut gtp_bob   = GtpClient::new();

let frame = gtp_alice.send(
    &mut alice, &mut alice_mls, 2, 1, "hello", PayloadCodec::Cbor,
)?;

for event in bob.on_wire(&mut bob_mls, &frame.wire)? {
    if let Event::PayloadReceived(p) = event {
        if p.stream_type == StreamType::Text {
            match gtp_bob.accept(&p.plaintext, bob_mls.epoch(), p.codec)? {
                GtpAccept::New(msg)       => println!("{:?}", msg.text()),
                GtpAccept::Duplicate(msg) => println!("dup: {:?}", msg.text()),
            }
        }
    }
}
```

See [`crates/gbp/stack/examples/`](../stack/examples/) for fully-runnable examples
covering GTP, GAP and GSP.

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
