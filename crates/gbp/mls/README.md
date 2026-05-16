# gbp-mls

MLS (RFC 9420) integration for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

This crate wraps `openmls 0.8` and adds a ChaCha20-Poly1305 AEAD layer driven
by MLS labelled exporters (`gbp/control`, `gbp/audio`, `gbp/text`,
`gbp/signal`).

The `MlsContext::export_raw(label, context, len)` method exposes the raw MLS
exporter for custom labels — used by `gbp-sframe` to derive
`sframe_base_key = MLS.ExportSecret("gbp/sframe v1", epoch_be8, 32)`.

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
