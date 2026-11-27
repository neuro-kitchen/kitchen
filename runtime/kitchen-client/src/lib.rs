//! The runtime API's client side ([standard §6](../../../standard/spec/06-runtime-api.md)): the calls
//! as data ([`Request`], [`Answer`], made by a [`Caller`]), anything that answers them
//! ([`Runtime`]), and the in-process runtime ([`Local`]) an app's view-models use with no
//! encoding and no network.
//!
//! A view-model, the Python package, a generated page and an agent all reach the runtime through a
//! [`Runtime`]: in the app it is [`Local`]; across the network it is a connection to a server
//! (WebTransport, or Connect as the fallback: step 4c and 4d), which carries the same messages.

pub mod calls;
pub mod local;
pub mod runtime;

pub use calls::{Answer, Caller, OpsFilter, Request, ResultsFilter, RunState, RuntimeError, Source, Target};
pub use local::{Cancel, Job, Local, Produced, Runner};
pub use runtime::{Reply, Runtime};
