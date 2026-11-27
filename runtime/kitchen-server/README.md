# kitchen-server

The runtime API served ([standard §6](../../standard/spec/06-runtime-api.md), decision 0017):

- `Server`: a WebTransport endpoint (HTTP/3 over QUIC) answering the calls with any
  `kitchen_client::Runtime`. A computer holding a client certificate from the server's authority
  comes in by it (mutual TLS, `Remote::connect_with_certificate`). Its certificate is its own, self-signed and valid 14 days, told to
  pages by its SHA-256 (`serve_info`), as lucem-stream does. The session's first stream says who
  the caller is: a token, or a pairing code (`kitchen-network`); then one stream per call, its
  request and answer one `kitchen-wire` frame each, answered concurrently on a blocking pool. The
  server records the person it let in as the call's `by`; running, cancelling and exporting need
  control.
- `http`: the same calls on TCP, on the same port number: Connect (`application/proto`, or
  `application/json` by protobuf's own mapping, so `curl` works), gRPC over HTTP/2 and gRPC-Web, all
  as one unary method `POST /kitchen.runtime.v1.Runtime/Call` answered by hand on hyper; the token
  as `authorization: Bearer`, or a client certificate. Pairing with a code works here too
  (`POST /kitchen.runtime.v1.Pairing/Start`, then `/Finish`). Plain HTTP on loopback only, otherwise TLS from the server's
  authority.
- `Remote`: the same calls from a program (a desktop app, a test, a notebook's bindings), as a
  `Runtime`: pair once with a code, then connect with the token.

Try it: `cargo run -p kitchen-server --example serve` prints the URLs, a token and a pairing code;
`examples/python/call_grpc.py` runs an op from Python's grpcio.

Stored results kept as Zarr folders are read by region (`result.array`, `result.range`), on every
transport, as one data chunk each (raw, lz4 or zstd): a viewer's tiles and windows.

Phase 2, steps 4c–4e ([runbook](../../docs/plans/0.0.10-standard.md)). Next: MCP (4f).
