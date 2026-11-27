# 0017. The runtime API on the wire: protobuf over WebTransport, Connect as the fallback

**Context.** The runtime API ([§6](../spec/06-runtime-api.md)) was drafted as HTTP with JSON. The
apps have since shown what the wire must do: dsp-kitchen streams recordings and envelopes as
length-prefixed protobuf on QUIC streams (one request per stream, so nothing waits behind data),
carrying documents (pipeline nodes, plugin calls) as JSON inside protobuf fields; lucem serves
tiles over WebTransport with a WebSocket fallback; both keep a browser page's main thread from
ever waiting, run their engine in a worker that owns the GPU, and pair pages with a code. Most of
the traffic is data, not calls.

**Decision.**

- **One definition**: the API is a protobuf `service` with its messages
  (`contract/proto/kitchen/runtime/v1/runtime.proto`, package `kitchen.runtime.v1`), versioned by
  package; field numbers are never reused, and a breaking-change check runs in CI.
- **Primary transport: WebTransport over HTTP/3 (QUIC)**, for every client: desktop apps, the
  orchestrator's workers and browser pages alike (WebTransport is Baseline in every browser engine
  since 2026). Inside a session, dsp-kitchen's framing: each call on its own bidirectional stream
  (a 4-byte big-endian length, then the protobuf message; lengths checked before allocating), bulk
  data on streams of its own, so a call never waits behind data.
- **Fallback over TCP: the Connect protocol**, from the same `.proto`: one server answers Connect,
  gRPC and gRPC-Web over HTTP/1.1 and HTTP/2, so a browser whose network blocks UDP, a Python
  notebook (any gRPC client) and `curl` (Connect's JSON) all reach it without a proxy. gRPC over
  HTTP/3 is not used: no mature implementation yet.
- **In-process** for an app's own view-models (no encoding); **stdio** for tools and tests (the same
  framing); **MCP** for agents (JSON, as MCP requires), a thin layer over the same calls.
- **Payloads**:
  - documents (pipelines, recipes, op descriptions, run records) travel as their canonical JSON
    inside protobuf `bytes` fields (dsp-kitchen's convention): one definition (the contract crate
    and its JSON Schemas), and content ids stay hashes of canonical JSON (decision 0003);
  - tables travel as Arrow IPC record batches (what Polars and Python read without converting);
  - arrays (tiles, windows, envelopes) as a small protobuf header (type, shape, region, level,
    encoding) and raw little-endian bytes, compressed per chunk (zstd or lz4) when it helps.
- **Stored large results** are served as files, read with HTTP range requests (Zarr v3 shards: the
  shard index, then the chunk), so any Zarr reader reaches them; the runtime's streams carry what is
  computed (previews, processed views, envelopes).
- **Who may connect** is kitchen-network's: TLS 1.3 always; computers with client certificates
  (mutual TLS), pages with a token from a pairing code; plain HTTP only on loopback.
- **Concurrency**: network on an async runtime; compute on a worker pool apart from it; GPU work
  queued per device; requests prioritized (what is on screen, then prefetch, then batch), answered
  coarse first where the data has levels; flow control and bounded queues; resetting a stream
  cancels its work.
- **Documents on disk** are JSON, or TOML for any app (read into the same document and hashed as
  canonical JSON, so the format never changes an id).

**Consequences.** One protocol for the desktop and the browser instead of two; a fallback that
needs no proxy; tables and arrays move without per-value encoding. The Connect implementation in
Rust is young (0.x): it is tried first in step 4d, with tonic (gRPC only) as plan B. Browsers still
differ in their WebTransport details and in WebGPU inside workers: both are tested per browser.

**Alternatives.** HTTP/JSON (simple, slow for data, no streams); gRPC over HTTP/2 only (no browser
without a proxy); gRPC over HTTP/3 (experimental in Rust); fully typed protobuf documents (faster
by little for small documents, two definitions to keep in step).

**Sources** (2026-11-27): WebTransport Baseline and Safari 26.4
(<https://developer.mozilla.org/docs/Web/API/WebTransport_API>, <https://caniuse.com/webtransport>);
Connect in Rust (<https://buf.build/blog/connect-rust-joins-the-connect-project>,
<https://docs.rs/crate/connectrpc/latest>); gRPC over HTTP/3 in Rust
(<https://docs.rs/crate/tonic-h3/latest/source/README.md>); Arrow Flight and gRPC copies
(<https://arrow.apache.org/blog/2019/10/13/introducing-arrow-flight/>); Zarr sharding and range
reads (<https://zarr.dev/zeps/accepted/ZEP0002.html>); WebAssembly threads
(<https://web.dev/articles/webassembly-threads>).
