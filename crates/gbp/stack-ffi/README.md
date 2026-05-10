# gbp-stack-ffi

C ABI surface (cdylib + rlib) for the
[Group Protocol Stack](https://github.com/F000NKKK/Group-Protocol-Stack).

This crate exposes a handle-based C ABI suitable for consumption from `.NET`
(via P/Invoke) or any other FFI-capable runtime. The shared library is named
`gbp_stack` (`gbp_stack.dll` on Windows, `libgbp_stack.so` / `libgbp_stack.dylib`
on Unix).

## Function families

| Prefix             | Purpose                                                  |
| ------------------ | -------------------------------------------------------- |
| `gbp_buffer_*`     | Buffer memory protocol                                    |
| `gbp_string_*`     | C-string memory protocol                                  |
| `gbp_last_error`   | Thread-local last-error reporter                          |
| `gbp_version`      | Library version                                           |
| `gbp_mls_*`        | RFC 9420 MLS context (handshake, exporter, AEAD)          |
| `gbp_node_*`       | GBP-layer group node (framing, AEAD, replay, control)     |
| `gtp_client_*`     | GTP (text) sub-protocol client                            |
| `gap_client_*`     | GAP (audio) sub-protocol client                           |
| `gsp_client_*`     | GSP (signalling) sub-protocol client                      |
| `gbp_frame_*`      | Frame codec helpers                                       |
| `gbp_error_*`      | Error registry helpers                                    |

## Memory protocol

* Binary blobs are returned as `GbpBuffer { ptr, len, cap }` and MUST be
  released via `gbp_buffer_free`.
* Owned strings are returned as `*mut c_char` and MUST be released via
  `gbp_string_free`.

## License

Licensed under [Apache License, Version 2.0](../../../LICENSE).
