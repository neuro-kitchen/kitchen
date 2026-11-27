//! A pipeline as data: sources, nodes, edges, outputs ([standard §4](../../../standard/spec/04-pipelines.md)).
//! JSON, versioned; the same file is what the Pipeline screen edits, what a headless run reads, and
//! what a recipe is made from. A pipeline lives in one app's workspace and holds only that app's
//! ops; a chain across apps is the orchestrator's.
//!
//! ```json
//! {
//!   "version": 1,
//!   "app": "lucem",
//!   "sources": {"nuclei": "image"},
//!   "nodes": {
//!     "blur": {"op": "gaussian_blur", "params": {"sigma": 2.0}},
//!     "mask": {"op": "otsu_threshold"}
//!   },
//!   "edges": [["nuclei", "blur.image"], ["blur.image", "mask.image"]],
//!   "outputs": {"nuclei_mask": "mask.mask"}
//! }
//! ```
//!
//! An endpoint is `node.port`, or a source's bare name. What a source is (which channel of which
//! image, which recording) is the app's to say, when it runs the graph.
//!
//! [`Pipeline::check`] finds every problem at once and gives a [`Plan`]: the order to run the nodes
//! in (only those that lead to an output), each input's feed, the parameters with their defaults,
//! what a disabled node passes on, and the margin the sources must be read with.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::catalog::Catalog;
use crate::ops::Margin;
use crate::params::{ParamValue, check_params};
use crate::types::Type;

/// The pipeline format's version this crate reads and writes.
pub const PIPELINE_VERSION: u32 = 1;

/// A source's name, or a node's port.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Endpoint {
    /// The node, or the source.
    pub node: String,
    /// The port; `None` for a source.
    pub port: Option<String>,
}

impl Endpoint {
    /// The port `port` of `node`.
    pub fn port(node: impl Into<String>, port: impl Into<String>) -> Self {
        Self { node: node.into(), port: Some(port.into()) }
    }

    /// The source `name`.
    pub fn source(name: impl Into<String>) -> Self {
        Self { node: name.into(), port: None }
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.port {
            Some(p) => write!(f, "{}.{p}", self.node),
            None => f.write_str(&self.node),
        }
    }
}

impl FromStr for Endpoint {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let ok = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        match s.split_once('.') {
            Some((n, p)) if ok(n) && ok(p) => Ok(Endpoint::port(n, p)),
            None if ok(s) => Ok(Endpoint::source(s)),
            _ => Err(format!("{s:?} is not `node.port` or a source's name")),
        }
    }
}

impl Serialize for Endpoint {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Endpoint {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Endpoint {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Endpoint".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "`node.port`, or a source's name.",
            "pattern": "^[A-Za-z0-9_-]+(\\.[A-Za-z0-9_-]+)?$"
        })
    }
}

/// A connection: `from` (a source or an output) feeds `to` (an input).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct Edge(pub Endpoint, pub Endpoint);

fn yes() -> bool {
    true
}

/// One op in the pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// The operation's id in the app's catalog.
    pub op: String,
    /// Its parameters; what is left out takes the default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, ParamValue>,
    /// Off: it passes its input on ([`Plan::forwards`]).
    #[serde(default = "yes", skip_serializing_if = "Clone::clone")]
    pub enabled: bool,
    /// A name for people, in place of the operation's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Where an editor draws it (canvas units); the run ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f32; 2]>,
}

impl Node {
    /// A node running `op` with its defaults.
    pub fn new(op: impl Into<String>) -> Self {
        Self { op: op.into(), params: BTreeMap::new(), enabled: true, label: None, at: None }
    }

    /// The same node with `name` set to `value`.
    pub fn with(mut self, name: impl Into<String>, value: ParamValue) -> Self {
        self.params.insert(name.into(), value);
        self
    }
}

fn version() -> u32 {
    PIPELINE_VERSION
}

/// A pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    /// The format's version ([`PIPELINE_VERSION`]); `kitchen_pipeline` before the standard.
    #[serde(default = "version", alias = "kitchen_pipeline")]
    pub version: u32,
    /// The app it was made in (`lucem`, `dspk`): its operations are named here.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app: String,
    /// The graph's inputs, by name, with their type.
    pub sources: BTreeMap<String, Type>,
    /// The operations, by node id.
    #[serde(default)]
    pub nodes: BTreeMap<String, Node>,
    /// The connections.
    #[serde(default)]
    pub edges: Vec<Edge>,
    /// What is written, by name: an output port of a node (or a source, written as read).
    pub outputs: BTreeMap<String, Endpoint>,
}

/// Something wrong with a graph, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The node it is about, if one.
    pub node: Option<String>,
    /// What is wrong.
    pub message: String,
}

impl Problem {
    fn at(node: &str, message: impl Into<String>) -> Self {
        Problem { node: Some(node.to_string()), message: message.into() }
    }

    fn graph(message: impl Into<String>) -> Self {
        Problem { node: None, message: message.into() }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.node {
            Some(n) => write!(f, "{n}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// A checked graph, ready to run.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The nodes to run, in order: only those that lead to an output.
    pub order: Vec<String>,
    /// Each node's operation id.
    pub ops: BTreeMap<String, String>,
    /// Each node's connected inputs: port → what feeds it.
    pub inputs: BTreeMap<String, BTreeMap<String, Endpoint>>,
    /// Each node's parameters, checked, defaults filled.
    pub params: BTreeMap<String, BTreeMap<String, ParamValue>>,
    /// Disabled nodes: each output port → the input it passes on.
    pub forwards: BTreeMap<String, BTreeMap<String, Endpoint>>,
    /// The sources the run reads.
    pub sources: BTreeSet<String>,
    /// What is written: name → endpoint.
    pub outputs: BTreeMap<String, Endpoint>,
    /// The margin every source is read with: what the ops need, added along each path to an
    /// output.
    pub margin: Margin,
    /// How many times each endpoint is read (by nodes and outputs): the engine drops a value after
    /// its last reader.
    pub readers: BTreeMap<Endpoint, usize>,
}

impl Pipeline {
    /// An empty pipeline of `app`.
    pub fn new(app: impl Into<String>) -> Self {
        Self { version: PIPELINE_VERSION, app: app.into(), sources: BTreeMap::new(), nodes: BTreeMap::new(), edges: Vec::new(), outputs: BTreeMap::new() }
    }

    /// Reads a pipeline from JSON; a newer format is refused with what to do.
    pub fn from_json(text: &str) -> Result<Pipeline, String> {
        let g: Pipeline = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if g.version > PIPELINE_VERSION {
            return Err(format!("format {} is newer than this app reads ({PIPELINE_VERSION}): update the app", g.version));
        }
        Ok(g)
    }

    /// The graph as JSON, keys in order (diffs stay small).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Connects `from` to `to` (replacing what fed `to`).
    pub fn connect(&mut self, from: Endpoint, to: Endpoint) {
        self.edges.retain(|e| e.1 != to);
        self.edges.push(Edge(from, to));
    }

    /// Checks the graph against `catalog`: every problem, or the plan.
    pub fn check(&self, catalog: &Catalog) -> Result<Plan, Vec<Problem>> {
        let mut problems = Vec::new();
        if self.version > PIPELINE_VERSION {
            problems.push(Problem::graph(format!("format {} is newer than this app reads", self.version)));
        }
        for (name, ty) in &self.sources {
            if self.nodes.contains_key(name) {
                problems.push(Problem::graph(format!("{name} is both a source and a node")));
            }
            if !catalog.knows(ty) {
                problems.push(Problem::graph(format!("source {name}: type {ty} is not one of the app's")));
            }
        }
        if self.outputs.is_empty() {
            problems.push(Problem::graph("nothing is written: add an output"));
        }

        // Nodes: operation, parameters
        let mut params = BTreeMap::new();
        let mut ops = BTreeMap::new();
        for (id, node) in &self.nodes {
            let Some(spec) = catalog.get(&node.op) else {
                problems.push(Problem::at(id, format!("no operation {:?} in this app (another app's op runs in a chain, on the orchestrator)", node.op)));
                continue;
            };
            ops.insert(id.clone(), node.op.clone());
            match check_params(&spec.params, &node.params) {
                Ok(p) => {
                    params.insert(id.clone(), p);
                }
                Err(es) => problems.extend(es.into_iter().map(|e| Problem::at(id, e))),
            }
        }

        // The type at an endpoint, if it exists
        let type_of = |e: &Endpoint| -> Option<Type> {
            match &e.port {
                None => self.sources.get(&e.node).cloned(),
                Some(p) => catalog.get(&self.nodes.get(&e.node)?.op)?.output_port(p).map(|p| p.ty.clone()),
            }
        };

        // Edges: ends exist, types fit, one feed per input
        let mut inputs: BTreeMap<String, BTreeMap<String, Endpoint>> = BTreeMap::new();
        for Edge(from, to) in &self.edges {
            let Some(from_ty) = type_of(from) else {
                problems.push(Problem::graph(format!("{from} → {to}: {from} is not a source or an output")));
                continue;
            };
            let Some(port) = &to.port else {
                problems.push(Problem::graph(format!("{from} → {to}: a source cannot be fed")));
                continue;
            };
            let Some(spec) = self.nodes.get(&to.node).and_then(|n| catalog.get(&n.op)) else {
                if !self.nodes.contains_key(&to.node) {
                    problems.push(Problem::graph(format!("{from} → {to}: no node {}", to.node)));
                }
                continue;
            };
            let Some(input) = spec.input_port(port) else {
                problems.push(Problem::at(&to.node, format!("{} has no input {port}", spec.title)));
                continue;
            };
            if !from_ty.feeds(&input.ty) {
                problems.push(Problem::at(&to.node, format!("{port} takes {}, but {from} gives {from_ty}", input.ty)));
            }
            if inputs.entry(to.node.clone()).or_default().insert(port.clone(), from.clone()).is_some() {
                problems.push(Problem::at(&to.node, format!("{port} is fed twice")));
            }
        }
        for (id, node) in &self.nodes {
            let Some(spec) = catalog.get(&node.op) else { continue };
            for p in spec.inputs.iter().filter(|p| !p.optional) {
                if !inputs.get(id).is_some_and(|i| i.contains_key(&p.name)) {
                    problems.push(Problem::at(id, format!("{} is not connected", p.name)));
                }
            }
        }

        // Disabled nodes pass on an input of each output's type
        let mut forwards = BTreeMap::new();
        for (id, node) in self.nodes.iter().filter(|(_, n)| !n.enabled) {
            let Some(spec) = catalog.get(&node.op) else { continue };
            let mut map = BTreeMap::new();
            for out in &spec.outputs {
                let feed = spec.inputs.iter().filter_map(|p| inputs.get(id).and_then(|i| i.get(&p.name)).filter(|_| p.ty == out.ty)).next();
                match feed {
                    Some(f) => {
                        map.insert(out.name.clone(), f.clone());
                    }
                    None => problems.push(Problem::at(id, format!("cannot be turned off: no input gives {} for {}", out.ty, out.name))),
                }
            }
            forwards.insert(id.clone(), map);
        }

        // Outputs exist
        for (name, e) in &self.outputs {
            if type_of(e).is_none() {
                problems.push(Problem::graph(format!("output {name}: {e} is not a source or a node's output")));
            }
        }

        // Order: depth first from the outputs; a node met again while open is a loop
        let mut order = Vec::new();
        let mut state: BTreeMap<String, u8> = BTreeMap::new();
        fn visit(id: &str, inputs: &BTreeMap<String, BTreeMap<String, Endpoint>>, nodes: &BTreeMap<String, Node>, state: &mut BTreeMap<String, u8>, order: &mut Vec<String>, problems: &mut Vec<Problem>) {
            if !nodes.contains_key(id) {
                return;
            }
            match state.get(id) {
                Some(1) => {
                    problems.push(Problem::at(id, "the graph loops back through it"));
                    return;
                }
                Some(_) => return,
                None => {}
            }
            state.insert(id.to_string(), 1);
            if let Some(feeds) = inputs.get(id) {
                for f in feeds.values() {
                    visit(&f.node, inputs, nodes, state, order, problems);
                }
            }
            state.insert(id.to_string(), 2);
            order.push(id.to_string());
        }
        for e in self.outputs.values() {
            visit(&e.node, &inputs, &self.nodes, &mut state, &mut order, &mut problems);
        }
        if !problems.is_empty() {
            problems.dedup();
            return Err(problems);
        }

        // Readers, sources
        let mut readers: BTreeMap<Endpoint, usize> = BTreeMap::new();
        let mut sources = BTreeSet::new();
        let read = |e: &Endpoint, readers: &mut BTreeMap<Endpoint, usize>, sources: &mut BTreeSet<String>| {
            *readers.entry(e.clone()).or_default() += 1;
            if e.port.is_none() {
                sources.insert(e.node.clone());
            }
        };
        for id in &order {
            let feeds: Vec<&Endpoint> = match forwards.get(id) {
                Some(f) => f.values().collect(),
                None => inputs.get(id).map(|i| i.values().collect()).unwrap_or_default(),
            };
            for e in feeds {
                read(e, &mut readers, &mut sources);
            }
        }
        for e in self.outputs.values() {
            read(e, &mut readers, &mut sources);
        }

        // Margin: from the outputs back, each node adds its own to what its readers need
        let mut need_out: BTreeMap<String, Margin> = BTreeMap::new();
        let mut at_sources = Margin::NONE;
        for id in order.iter().rev() {
            let own = if forwards.contains_key(id) { Margin::NONE } else { catalog.get(&self.nodes[id].op).map(|s| s.margin).unwrap_or_default() };
            let need_in = own.then(need_out.get(id).copied().unwrap_or_default());
            let feeds: Vec<&Endpoint> = match forwards.get(id) {
                Some(f) => f.values().collect(),
                None => inputs.get(id).map(|i| i.values().collect()).unwrap_or_default(),
            };
            for f in feeds {
                if f.port.is_none() {
                    at_sources = at_sources.max(need_in);
                } else {
                    let e = need_out.entry(f.node.clone()).or_default();
                    *e = e.max(need_in);
                }
            }
        }

        let inputs = order.iter().map(|id| (id.clone(), inputs.remove(id).unwrap_or_default())).collect();
        let params = order.iter().filter_map(|id| params.remove(id).map(|p| (id.clone(), p))).collect();
        let ops = order.iter().filter_map(|id| ops.remove(id).map(|o| (id.clone(), o))).collect();
        Ok(Plan { order, ops, inputs, params, forwards, sources, outputs: self.outputs.clone(), margin: at_sources, readers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{OpSpec, PortSpec};
    use crate::params::ParamSpec;

    fn catalog() -> Catalog {
        let mut c = Catalog::new(["image", "mask", "table"].map(Type::new));
        c.add(OpSpec::new("blur", "Blur").input(PortSpec::new("image", "image")).output(PortSpec::new("image", "image")).param(ParamSpec::number("sigma", 2.0).range(0.2, 20.0)).margin(Margin::around(6, 2))).unwrap();
        c.add(OpSpec::new("otsu", "Otsu").input(PortSpec::new("image", "image")).output(PortSpec::new("mask", "mask")).margin(Margin::around(1, 2))).unwrap();
        c.add(OpSpec::new("and", "And").input(PortSpec::new("a", "mask")).input(PortSpec::new("b", "mask").optional()).output(PortSpec::new("mask", "mask"))).unwrap();
        c.add(OpSpec::new("count", "Count").input(PortSpec::new("mask", "mask")).output(PortSpec::new("counts", "table"))).unwrap();
        c
    }

    fn e(s: &str) -> Endpoint {
        s.parse().unwrap()
    }

    /// nuclei → blur → otsu → and → count; blur2 (unused) is left out of the plan.
    fn graph() -> Pipeline {
        let mut g = Pipeline::new("test");
        g.sources.insert("nuclei".into(), Type::new("image"));
        g.nodes.insert("blur".into(), Node::new("blur").with("sigma", ParamValue::Integer(3)));
        g.nodes.insert("otsu".into(), Node::new("otsu"));
        g.nodes.insert("and".into(), Node::new("and"));
        g.nodes.insert("count".into(), Node::new("count"));
        g.nodes.insert("blur2".into(), Node::new("blur"));
        for (a, b) in [("nuclei", "blur.image"), ("blur.image", "otsu.image"), ("otsu.mask", "and.a"), ("and.mask", "count.mask"), ("nuclei", "blur2.image")] {
            g.connect(e(a), e(b));
        }
        g.outputs.insert("counts".into(), e("count.counts"));
        g.outputs.insert("mask".into(), e("otsu.mask"));
        g
    }

    #[test]
    fn a_good_pipeline_plans_in_order_with_its_margin() {
        let plan = graph().check(&catalog()).unwrap();
        assert_eq!(plan.order, ["blur", "otsu", "and", "count"], "blur2 leads nowhere");
        assert_eq!(plan.params["blur"]["sigma"], ParamValue::Number(3.0));
        assert_eq!(plan.margin, Margin::around(7, 2), "blur 6 then otsu 1");
        assert_eq!(plan.readers[&e("otsu.mask")], 2, "and, and the mask output");
        assert_eq!(plan.sources, BTreeSet::from(["nuclei".to_string()]));
        assert!(!plan.inputs["and"].contains_key("b"), "an optional input may stay open");
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let mut g = graph();
        g.nodes.insert("ghost".into(), Node::new("nope"));
        g.nodes.get_mut("blur").unwrap().params.insert("sigma".into(), ParamValue::Number(99.0));
        g.connect(e("nuclei"), e("count.mask")); // image into a mask input
        g.edges.retain(|x| x.1 != e("and.a")); // and.a open
        let problems = g.check(&catalog()).unwrap_err();
        let text: Vec<String> = problems.iter().map(|p| p.to_string()).collect();
        for want in ["ghost: no operation", "blur: sigma is at most 20", "count: mask takes mask, but nuclei gives image", "and: a is not connected"] {
            assert!(text.iter().any(|t| t.starts_with(want)), "{want:?} in {text:?}");
        }
    }

    #[test]
    fn loops_are_found() {
        let mut g = graph();
        g.connect(e("and.mask"), e("and.b"));
        let problems = g.check(&catalog()).unwrap_err();
        assert!(problems.iter().any(|p| p.message.contains("loops")), "{problems:?}");
    }

    #[test]
    fn a_disabled_node_passes_its_input_on() {
        let mut g = graph();
        g.nodes.get_mut("blur").unwrap().enabled = false;
        let plan = g.check(&catalog()).unwrap();
        assert_eq!(plan.forwards["blur"]["image"], e("nuclei"));
        assert_eq!(plan.margin, Margin::around(1, 2), "blur's margin no longer counts");
        // Otsu cannot pass an image on as a mask
        g.nodes.get_mut("otsu").unwrap().enabled = false;
        assert!(g.check(&catalog()).unwrap_err().iter().any(|p| p.message.contains("cannot be turned off")));
    }

    #[test]
    fn reads_and_writes_json() {
        let g = graph();
        let back = Pipeline::from_json(&g.to_json()).unwrap();
        assert_eq!(back, g);
        let text = r#"{"sources":{"a":"image"},"nodes":{"b":{"op":"blur"}},"edges":[["a","b.image"]],"outputs":{"o":"b.image"}}"#;
        let g = Pipeline::from_json(text).unwrap();
        assert_eq!(g.version, PIPELINE_VERSION);
        assert!(g.check(&catalog()).is_ok());
        assert!(Pipeline::from_json(r#"{"version":9,"sources":{},"outputs":{}}"#).is_err());
        // The name before the standard still reads
        assert!(Pipeline::from_json(r#"{"kitchen_pipeline":1,"sources":{},"outputs":{}}"#).is_ok());
        assert!("a.b.c".parse::<Endpoint>().is_err());
    }
}
