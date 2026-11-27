//! The contract's ports, parameters and margins, under the names kitchen-pipeline used before the
//! standard: [`PortType`] is [`kitchen_contract::Type`], [`Context`] is [`kitchen_contract::Margin`].

pub use kitchen_contract::{Margin as Context, ParamKind, ParamSpec, ParamValue, PortSpec, Type as PortType, check_params};
