# kitchen-net

The neuro-kitchen network layer: who may connect to a server, and how a computer or a browser
gets that right with a short code (`42-8317-2956`) instead of a file handed over. Merged from
dsp-kitchen's `dspk-stream` 0.0.13 and lucem's `lucem-stream` 0.0.6, where the same code lived
twice.

**Status: written, not built yet.** It's merged from code that compiles in both projects; expect
small fixes on the first build.

| Module | From | What |
|---|---|---|
| `service` | new | the project's names on the wire, set once with `set_service` |
| `pair` | dsp-kitchen `pair` (lucem's is a port) | the code, the exchange's messages, a browser's side, the page's handshake types; also in WebAssembly |
| `auth` | dsp-kitchen `transport::auth` + lucem's `issue`, `Peer::pairing_only` | a server's certificate authority, enrolled clients and tokens, revocation |
| `tls` | dsp-kitchen `transport::tls` (a superset of lucem's) | QUIC configurations: mutual TLS, pairing connections, a project's own protocols |
| `pairing` | dsp-kitchen `transport::pairing` | codes, the server's and a computer's side, both over QUIC |

These stay in the projects:
- their messages (protobuf) and the engines that answer them
- their servers' request handling
- the browser endpoint (WebTransport and the WebSocket)
- dsp-kitchen's live device protocol

## The service

Each project sets its names once, before anything connects:

```rust
// dsp-kitchen: its names today, so clients already paired keep working
kitchen_net::set_service(kitchen_net::Service { name: "dspk", alpn: b"dspk-stream/3", pair_alpn: b"dspk-pair/1" });
// lucem
kitchen_net::set_service(kitchen_net::Service { name: "lucem", alpn: b"lucem-stream/1", pair_alpn: b"lucem-pair/1" });
```

From the name come:
- the SPAKE2 identities (`dspk-pair-client`, `dspk-pair-server`)
- the pairing server name (`dspk-pairing`)
- the authority's common name (`dspk serve authority`, which must match an existing authority's)
- the WebTransport path (`/dspk`) and the page's serve-info path (`/dspk-serve.json`)

## Adopting it in a project

Do one pull request per project, in a session that can build and test.

1. **Depend on it:** add `kitchen-net` in `<px>-stream`, and call `set_service` in the project's
   `init()`.
2. **Delete the merged modules:** remove dsp-kitchen's `transport::{auth, tls, pairing}` and
   `pair`, or lucem's `auth`, `tls`, `pairing` and `pair`. Re-export from `kitchen_net` under the
   old paths where callers use them.
3. **Errors:** add `#[error(transparent)] Net(#[from] kitchen_net::NetError)` to `StreamError`.
4. **dsp-kitchen's device protocol:** rebuild `server_config_live` and `live_client_config` on
   `tls::server_config_for_protocols` and `tls::client_config_for_protocol`, with
   `crate::live::ALPN`.
5. **lucem's pairing moves to its own connection.** lucem pairs inside its session protocol
   (`Request::Pair`); kitchen-net pairs over a connection with the pairing ALPN, as dsp-kitchen
   does. The server uses `tls::server_config_with_pairing`, checks `pairing::is_pairing` on each
   connection, and calls `pairing::serve_pairing`. The CLI uses `pairing::pair`. Already-paired
   computers keep working (they connect with their certificate). An old lucem client can't pair
   with a new server: say so in lucem's changelog.
6. **`WebWelcome`:** it carries both projects' fields (`has_files`, `plugins`), each with a
   default, so a page of either version reads it.
7. **Test:** run both projects' pairing and server tests: pairing with a code, a wrong code, an
   impostor, a browser's token, revocation.
