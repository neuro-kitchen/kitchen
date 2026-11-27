# kitchen-web

The runtime API from a browser page ([standard §6](../../standard/spec/06-runtime-api.md), decision
0017), compiled for WebAssembly. Built on lucem's `web_client` and dsp-kitchen's `dspk-engine-web`.

- `page`: the page's link, a `kitchen_client::Runtime`.
  - It connects to the server that served the page (`serve_info`, then `connect`): WebTransport
    (`web/kitchen-transport.js`, a stream per call, the server's certificate accepted by its hash),
    else Connect over `fetch` where UDP is blocked.
  - It pairs with a code (the page keeps `new_token()` for next time) or comes back with a token.
  - On the page's main thread, calls are awaited with `call_async`. From a worker of a build with
    threads, `Runtime::call` waits for the answer.
- `page::start_worker`: the page's own engine worker (`web/engine-worker.js?module=…`) for a page no
  server served. The app's module exports an `Engine` whose `answer(bytes)` is `worker::answer`
  over the app's runtime. The worker owns the GPU and takes the files the user picked
  (`set_files`) and an `OffscreenCanvas` (`set_canvas`).
- `worker`: the engine's side, plain Rust (tested natively too).
- `web/isolate-sw.js`: cross-origin isolation where a host cannot send the headers (GitHub Pages).
  A `kitchen-server` web root sends them itself.

**Two builds.** Without threads, the default: `cargo build --target wasm32-unknown-unknown`, then
`wasm-bindgen --target web`. With threads, the page must be cross-origin isolated, and the module
is built with a nightly toolchain:

```
RUSTFLAGS="-C target-feature=+atomics,+bulk-memory" cargo +nightly build -Z build-std=std,panic_abort --target wasm32-unknown-unknown
```

The threads build is not yet tested here.

**Tested in a browser:** `tests/browser/run.sh` (and the `browser` job in CI) runs the page in headless
Chromium against `kitchen-server`'s `web_check` example:
- over WebTransport and over Connect, the page pairs, lists, runs an op (recorded as the page),
  reads a region of the Zarr result, comes back with its token, and a stranger's token is refused;
- the engine worker gets an OffscreenCanvas and answers.
