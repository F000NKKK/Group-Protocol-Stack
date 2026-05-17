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

## Per-signal `args` schemas

Signals without required args (JOIN, LEAVE) MAY be sent with empty `args`.
All other signals MUST include a CBOR map in `args`; missing or invalid `args`
causes the receiver to return `status = "error"`.

| Signal | `args` CBOR | Description |
|--------|------------|-------------|
| JOIN (100) | *(empty)* | Member announces join |
| LEAVE (101) | *(empty)* | Member announces leave |
| ROLE_CHANGE (102) | `{0: target_id, 1: new_role}` | Assign role to member |
| MUTE (200) | `{0: target_id}` | Mute a member |
| UNMUTE (201) | `{0: target_id}` | Unmute a member |
| STREAM_START (300) | `{0: stream_type}` | Start a media stream |
| STREAM_STOP (301) | `{0: stream_type}` | Stop a media stream |
| CODEC_UPDATE (400) | `{0: codec_id}` | Switch codec |

All map keys and values are CBOR unsigned integers (major type 0).

## License

Licensed under [Apache License, Version 2.0](../../LICENSE).
