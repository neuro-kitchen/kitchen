# kitchen-server

The runtime API served ([standard §6](../../standard/spec/06-runtime-api.md), decision 0017):

- `Server`: a WebTransport endpoint (HTTP/3 over QUIC) answering the calls with any
  `kitchen_client::Runtime`. Its certificate is its own, self-signed and valid 14 days, told to
  pages by its SHA-256 (`serve_info`), as lucem-stream does. The session's first stream says who
  the caller is: a token, or a pairing code (`kitchen-network`); then one stream per call, its
  request and answer one `kitchen-wire` frame each, answered concurrently on a blocking pool. The
  server records the person it let in as the call's `by`; running, cancelling and exporting need
  control.
- `Remote`: the same calls from a program (a desktop app, a test, a notebook's bindings), as a
  `Runtime`: pair once with a code, then connect with the token.

Phase 2, step 4c ([runbook](../../docs/plans/0.0.10-standard.md)). Next: Connect, gRPC and gRPC-Web
on TCP (4d), stored results read by range (4e), MCP (4f).
