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
use gbp_node::GroupNode;
use gtp::GtpClient;
use gbp_core::PayloadCodec;

// After MLS handshake:
let mut alice = GroupNode::new(1, group_id);
let mut bob   = GroupNode::new(2, group_id);
alice.bootstrap_as_creator(alice_mls.epoch())?;
bob.bootstrap_as_joiner(bob_mls.epoch())?;

let mut gtp_alice = GtpClient::new();
let mut gtp_bob   = GtpClient::new();

// Send
let frame = gtp_alice.send(&mut alice, &mut alice_mls,
                           /*target*/ 2, /*msg_id*/ 1,
                           "hello", PayloadCodec::Cbor)?;

// Receive
for event in bob.on_wire(&mut bob_mls, &frame.wire)? {
    if let gbp_node::Event::PayloadReceived(p) = event {
        let r = gtp_bob.accept(&p.plaintext, bob_mls.epoch(), p.codec)?;
        println!("{}", r.text); // → "hello"
    }
}
```

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
