# gbp-protocol

The **Group Broadcast Protocol** base layer: the framing, control plane and
error object that the three sub-protocols (`gtp-protocol`, `gap-protocol`,
`gsp-protocol`) build on top of, the same way TCP and UDP build on top of IP.

This crate provides:

* [`GbpFrame`] — the CBOR-encoded transport frame with `version`, `group_id`,
  `epoch`, `transition_id`, `stream_type`, `stream_id`, `flags`, `sequence_no`
  and the encrypted payload.
* [`ControlMessage`] — the CBOR-encoded control plane message format.
* [`ErrorObject`] — the wire-serialisable error object referenced by the
  registry in [`gbp-core`](https://crates.io/crates/gbp-core).
* [`CodecError`] — a unified codec error type used by every codec in the
  stack.

## Wire format

Frames are encoded with deterministic CBOR (RFC 8949). Every frame is
self-describing and validated up-front against the constraints in §6.2 of the
GBP specification (version, group_id length, epoch, payload_size).

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
