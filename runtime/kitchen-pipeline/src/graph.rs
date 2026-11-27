//! The contract's pipeline format and its check (`kitchen_contract::pipelines`), under the names
//! kitchen-pipeline used before the standard: [`Graph`] is [`kitchen_contract::Pipeline`].

pub use kitchen_contract::pipelines::{Edge, Endpoint, Node, Plan, Problem};
pub use kitchen_contract::{PIPELINE_VERSION as GRAPH_VERSION, Pipeline as Graph};
