//! The runtime API from a browser page ([standard §6](../../../standard/spec/06-runtime-api.md),
//! decision 0017), compiled for WebAssembly:
//!
//! - [`page`]: the page's link to a runtime, a [`kitchen_client::Runtime`]: the server that
//!   served the page over WebTransport (Connect over `fetch` where UDP is blocked), pairing with a
//!   code or coming back with a token; or the page's own engine worker.
//! - [`worker`]: the engine worker's side, an app's own runtime answering the same calls (plain
//!   Rust, so it is tested natively too).
//! - `web/`: the JavaScript a page ships with it: `kitchen-transport.js` (WebTransport for the
//!   link), `engine-worker.js` (the module worker that loads an app's engine), `isolate-sw.js`
//!   (cross-origin isolation where a host cannot send the headers, for a build with threads).
//!
//! On other targets only [`worker`] is here.

pub mod worker;

#[cfg(target_family = "wasm")]
pub mod page;

#[cfg(all(target_family = "wasm", feature = "check"))]
mod check;
