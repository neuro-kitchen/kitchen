//! The neuro-kitchen network layer: who may connect to a server, and how a computer or a browser
//! gets that right with a short code instead of a file handed over.
//!
//! - [`service`]: the project's name on the wire (ALPN, pairing identities, paths, the CA's
//!   name), set once at start-up with [`set_service`]. Existing clients keep working because
//!   each project keeps its own names.
//! - [`pair`]: pairing as every side shares it: the code, the messages, a browser's side (ending
//!   in a token), and the page's handshake types. No transport and no files, so a page built for
//!   WebAssembly uses it as the server does.
//!
//! Not in a browser:
//!
//! - [`auth`]: a server's own certificate authority, the registry of enrolled clients
//!   (certificates and tokens), revocation.
//! - [`tls`]: QUIC configurations: the server's certificate, mutual TLS against the authority,
//!   what a client trusts.
//! - [`pairing`]: the server's side of pairing (codes, the exchange), a computer's side (ending in
//!   a certificate), and both over QUIC.
//!
//! A project keeps its messages, the engine that answers them, and its server's request handling.

mod error;
pub mod pair;
pub mod service;

pub use error::{NetError, NetResult};
pub use service::{Service, service, set_service};
