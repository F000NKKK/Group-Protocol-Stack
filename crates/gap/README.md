# gap-protocol

**Group Audio Protocol** — audio sub-protocol of the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

GAP carries Opus media frames on top of the GBP base layer with per-source
replay protection and epoch-bound key material.

## Profile

* Opus at 48 kHz **REQUIRED**.
* 20 ms packetisation **RECOMMENDED**.
* FEC **RECOMMENDED**.
* Reliable delivery **NOT RECOMMENDED** (use the `O` flag profile only).

## What this crate provides

* `GapPayload` — the audio frame envelope; can be encoded as CBOR,
  Protobuf, or FlatBuffers (selected by `PayloadCodec`).
  Using `PayloadCodec::FlatBuffers` (`pf=2`) minimises decode latency for
  real-time audio paths.
* `GapClient` — stateful client that maintains a per-source `rtp_sequence`
  window and validates `key_phase` against the current group epoch; accepts
  any `PayloadCodec` echoed from the `payload_received` event.

## License

Licensed under [Apache License, Version 2.0](../../LICENSE).
