# gbp-flat

FlatBuffers codec for the [Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack) (GBP/GTP/GAP/GSP).

Alternative wire format to CBOR per gbp_rfc §12.2. Schemas are compiled from
`.fbs` files at build time using [planus](https://crates.io/crates/planus) —
no `flatc` binary required.

## Messages

| Module | Types |
|--------|-------|
| `gbp` | `GbpFrame`, `ControlMessage`, `ErrorObject` |
| `gtp` | `GtpMessage`, `AttachmentManifest`, `AttachmentChunk` |
| `gap` | `GapPayload` |
| `gsp` | `GspSignal` |

## Usage

```rust
use gbp_flat::gbp::{GbpFrame, GbpFrameRef};
use planus::{Builder, ReadAsRoot};

// Serialize
let frame = GbpFrame { version: 1, epoch: 42, ..Default::default() };
let mut builder = Builder::new();
let bytes = builder.finish(frame, None).to_vec();

// Deserialize (zero-copy view)
let view = GbpFrameRef::read_as_root(&bytes).unwrap();
assert_eq!(view.version().unwrap(), 1);

// Convert to owned struct
let owned: GbpFrame = view.try_into().unwrap();
```

## License

Apache-2.0
