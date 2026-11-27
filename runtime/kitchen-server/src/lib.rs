//! The runtime API served ([standard §6](../../../standard/spec/06-runtime-api.md), decision 0017):
//! WebTransport over HTTP/3, as lucem-stream serves its pages.
//!
//! - **The session:** a page or a computer opens a WebTransport session at the service's path
//!   ([`kitchen_network::Service::webtransport_path`]). The server's certificate is its own,
//!   self-signed, ECDSA and valid 14 days: a browser accepts it by its SHA-256, which the server
//!   tells in its [`ServeInfo`]; a native client is given the same hash.
//! - **Who it is:** the session's first stream says it, in JSON frames: a token from an earlier
//!   pairing, or a pairing code (SPAKE2, then a token), as [`kitchen_network::pair`] defines them.
//!   The server answers with a [`WebWelcome`]; a caller it does not know is closed out.
//! - **The calls:** one bidirectional stream per call, its request and its answer one frame each
//!   ([`kitchen_wire`]'s protobuf, a 4-byte big-endian length before each). Calls are answered
//!   concurrently, on a blocking pool, never on the async threads; a stream the caller closes is
//!   a call it no longer wants. A call's `by` is the peer the server let in, whatever it says;
//!   running and exporting need a peer that may control.
//!
//! The same calls are served on TCP too ([`http`]: Connect, gRPC and gRPC-Web), for networks
//! that block UDP and for clients that speak gRPC.
//!
//! [`Remote`] is the other side, for a program: the [`kitchen_client::Runtime`] trait over a
//! session, so a desktop app, a test or a notebook calls a server as it calls its own runtime.

mod frames;
pub mod http;
mod remote;
mod server;

pub use kitchen_network::pair::{ServeInfo, WebHello, WebWelcome};
pub use remote::{Remote, RemoteOptions};
pub use http::{CALL_PATH, PAIR_FINISH_PATH, PAIR_START_PATH};
pub use server::{Http, Server, ServerOptions};

/// The largest request frame a server reads (a pipeline, a batch's fillings).
pub const MAX_REQUEST: usize = 16 << 20;

/// The largest frame while a caller says who it is.
pub const MAX_HELLO: usize = 64 << 10;

/// How long a caller has to say who it is.
pub const HELLO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// What went wrong serving or calling.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// A socket or a file.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The certificate or TLS.
    #[error("TLS: {0}")]
    Tls(String),
    /// The other side said something unexpected, or went away.
    #[error("{0}")]
    Protocol(String),
    /// Not let in (an unknown or revoked token, a wrong code).
    #[error("not let in: {0}")]
    Refused(String),
    /// A frame larger than allowed.
    #[error("a frame of {len} bytes, more than {max}")]
    TooLarge {
        /// Its length.
        len: usize,
        /// The limit.
        max: usize,
    },
}

impl ServerError {
    pub(crate) fn protocol(e: impl std::fmt::Display) -> Self {
        Self::Protocol(e.to_string())
    }
}

/// What this crate returns.
pub type ServerResult<T> = Result<T, ServerError>;
