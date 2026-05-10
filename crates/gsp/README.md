# gsp-protocol

**Group Signaling Protocol** — signalling sub-protocol of the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

GSP carries membership, role and stream-state events on top of the GBP base
layer with reliable delivery and `request_id` deduplication.

## Receive-side pipeline

Per GSP §7:

1. Decrypt (handled by GBP).
2. Check sender authorisation.
3. Validate the `args` schema.
4. Apply the effect atomically.
5. Emit ACK / NACK.

## What this crate provides

* `GspSignal` — the CBOR-encoded signal envelope.
* `GspClient` — stateful client that maintains `request_id` deduplication,
  the mute-list, the current membership set and `signal_type` decoding.

## License

Licensed under [Apache License, Version 2.0](../../LICENSE).
