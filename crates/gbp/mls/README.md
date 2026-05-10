# gbp-mls

MLS (RFC 9420) integration for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

This crate wraps `openmls 0.8` and adds a ChaCha20-Poly1305 AEAD layer driven
by MLS labelled exporters (`gbp/control`, `gbp/audio`, `gbp/text`,
`gbp/signal`).

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
