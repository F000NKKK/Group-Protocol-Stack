# gbp-transport

Length-prefixed TCP transport adapter for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

This crate is intentionally minimal: a pragmatic stand-in for a future QUIC
binding. The on-the-wire framing is `u32-LE length || bytes`.

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
