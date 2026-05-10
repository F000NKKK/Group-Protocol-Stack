# gbp-cli

Reference CLI binary for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

The binary is named `gbp-node` and supports two modes:

```sh
# listener: accept one peer, do MLS handshake, read one GTP message
gbp-node listen 127.0.0.1:7878

# connector: handshake and send one GTP message
gbp-node connect 127.0.0.1:7878
```

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
