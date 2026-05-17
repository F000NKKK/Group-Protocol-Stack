# gbp-stack

Top-level facade for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

This crate is a thin re-export of every layer in the stack:

* `gbp-core` — type vocabulary (`StreamType`, `PayloadCodec`, error codes)
* `gbp-protocol` — base GBP layer (`GbpFrame` with `pf` payload-codec field)
* `gtp-protocol`, `gap-protocol`, `gsp-protocol` — sub-protocols
* `gbp-mls` — MLS / AEAD adapter
* `gbp-sframe` — SFrame E2EE for GAP audio streams
* `gbp-transport` — TCP framing helper
* `gbp-node` — GBP-layer group node
* `gbp-proto` — Protobuf wire-format codec (`PayloadCodec::Protobuf`)
* `gbp-flat` — FlatBuffers wire-format codec (`PayloadCodec::FlatBuffers`)

Most users should depend on `gbp-stack` only; the per-layer crates are
available for users who need a smaller surface.

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
