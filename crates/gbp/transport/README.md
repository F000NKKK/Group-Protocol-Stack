# gbp-transport

Transport adapters for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

Both adapters use the same on-the-wire framing: `u32-LE length || bytes`.

## TCP adapter

`TcpStream`-based length-prefixed adapter (`tcp` module).

```rust
use gbp_transport::tcp::{TcpListener, connect};

let listener = TcpListener::bind("127.0.0.1:0").await?;
let stream = connect(listener.local_addr()?).await?;
```

## QUIC adapter

Async QUIC transport built on [quinn](https://crates.io/crates/quinn)
and [rustls](https://crates.io/crates/rustls) (`quic` module).

```rust
use gbp_transport::quic::{make_server_endpoint, make_client_endpoint, QuicStream};

let server = make_server_endpoint(addr, &cert_der, &key_der)?;
let client = make_client_endpoint("0.0.0.0:0".parse()?, None)?; // None = skip-verify
```

`QuicStream` wraps a single QUIC bidirectional stream and provides the same
`write_frame` / `read_frame` API as the TCP adapter.

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
