# gbp-core

Core type vocabulary for the **Group Protocol Stack** — `StreamType`, frame
flags, FSM states, control opcodes, signal opcodes and the canonical error
code registry.

This crate is dependency-light (only `core` / `alloc`) and is the foundation
every other crate in the stack builds on.

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
