# gbp-proto

Protobuf codec for the [Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack) (GBP/GTP/GAP/GSP).

Alternative wire format to CBOR per gbp_rfc §12.2. All message types derive
[`prost::Message`](https://docs.rs/prost) — no `protoc` compiler required.

## Messages

| Module | Types |
|--------|-------|
| `gbp` | `GbpFrame`, `ControlMessage`, `ErrorObject` |
| `gtp` | `GtpMessage`, `AttachmentManifest`, `AttachmentChunk` |
| `gap` | `GapPayload` |
| `gsp` | `GspSignal` |

## Usage

```rust
use gbp_proto::gbp::GbpFrame;
use prost::Message;

let frame = GbpFrame { version: 1, sequence_no: 1, ..Default::default() };
let bytes = frame.encode_to_vec();
let decoded = GbpFrame::decode(bytes.as_slice()).unwrap();
```

## License

Apache-2.0
