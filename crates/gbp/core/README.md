# gbp-core

Core type vocabulary for the **Group Protocol Stack** — `StreamType`, frame
flags, FSM states, control opcodes, signal opcodes and the canonical error
code registry.

This crate is dependency-light (only `core` / `alloc`) and is the foundation
every other crate in the stack builds on.

Key types:
* `StreamType` — `Audio=1`, `Text=2`, `Signaling=3`
* `PayloadCodec` — `Cbor=0`, `Protobuf=1`, `FlatBuffers=2`; controls which
  wire format is used for sub-protocol payloads (signalled via GBP frame `pf`)
* `ControlOpcode`, `SignalOpcode` — numeric registries
* `GbpError` / error code constants

## Stack overview

```
┌── application ──────────────────────────────────────────────────────┐
│  gtp-protocol · gap-protocol · gsp-protocol  (TCP/UDP/SCTP-like)    │
├──────────────────────────────────────────────────────────────────────┤
│                  gbp-protocol  (the IP-like base)                   │
├──────────────────────────────────────────────────────────────────────┤
│                  gbp-core   ← you are here                          │
└──────────────────────────────────────────────────────────────────────┘
```

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
