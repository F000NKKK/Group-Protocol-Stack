# gtp-protocol

**Group Text Protocol** — text-messaging sub-protocol of the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

GTP is to GBP what TCP is to IP: it adds idempotent message-level semantics
on top of the GBP base layer's framing and AEAD.

## What this crate provides

* `GtpMessage` — the CBOR-encoded text message envelope.
* `GtpClient` — stateful client that:
  * sends text messages through a `gbp_node::GroupNode`;
  * accepts incoming plaintext payloads delivered by GBP and rejects
    duplicates by `(sender_id, message_id)`.

## Example

```rust,ignore
use gtp::GtpClient;

let mut client = GtpClient::new();
let frame = client.send(&mut node, &mut sealer, target, 0xCAFE_F00D, "hello")?;
// hand `frame.wire` to your transport...
```

## License

Licensed under [Apache License, Version 2.0](../../LICENSE).
