//! How the orchestrator drives a neuro-kitchen tool without its window. A tool (lucem, dspk,
//! neuro-convert, garnish, any tool from kitchen-template) is a plugin program
//! (`kitchen-plugin`: JSON lines over stdin and stdout, large data as files) that answers three
//! more requests:
//!
//! | Request | Answer |
//! |---|---|
//! | `describe` | [`ToolDescription`]: its operations, what each reads and writes (artifact kinds), its recipe's JSON Schema |
//! | `validate` | whether a recipe is one an operation can run, and if not, why |
//! | `run` | runs an operation on input artifacts into an output folder: [`RunReport`] |
//!
//! The orchestrator knows nothing else about a tool. It connects tools by the kinds they declare,
//! so a new tool joins the soup by answering these three requests.
//!
//! - [`description`], [`protocol`], [`report`]: the data.
//! - [`serve`]: the tool's side ([`HeadlessTool`], [`serve_stdio`]).

pub mod description;
pub mod protocol;
pub mod report;
pub mod serve;

pub use description::{InputPort, Operation, OutputPort, Resources, ToolDescription};
pub use protocol::{RunInput, RunRequest, TOOL_PROTOCOL, ToolRequest, Validation};
pub use report::{ReviewFlag, RunOutput, RunReport};
pub use serve::{HeadlessTool, plugin_info, serve_stdio};
