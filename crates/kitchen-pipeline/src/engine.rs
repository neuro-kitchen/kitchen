//! Runs a checked graph over an app's [`Domain`], piece by piece.
//!
//! For each piece: read the sources it needs over `piece.read` (the context around it included),
//! run the nodes in order (a disabled node passes its input on), drop each value after its last
//! reader, trim each output back to `piece.own` and write it. Progress is reported after each
//! piece; [`Cancel`] stops between pieces (and between nodes). A preview is the same work on one
//! piece, returned instead of written ([`Engine::preview`]).
//!
//! The engine runs only confirmed operations: an app's own, and scripts confirmed for what they
//! are now ([`crate::script::review`]).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::catalog::Catalog;
use crate::domain::{Domain, Piece};
use crate::graph::{Endpoint, Graph, Plan};
use crate::op::{Call, OpSource, Operation};
use crate::{PipelineError, Result};

/// Stops a run from another thread (a Stop button, a closed window).
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A handle that has not been pulled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the run to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether it was asked to stop.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Where a run is.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// Pieces done.
    pub done: usize,
    /// Pieces in all.
    pub pieces: usize,
    /// Since the run started.
    pub elapsed: Duration,
}

impl Progress {
    /// Done, from 0 to 1.
    pub fn fraction(&self) -> f32 {
        if self.pieces == 0 { 1.0 } else { self.done as f32 / self.pieces as f32 }
    }
}

/// What a run did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunReport {
    /// Pieces run.
    pub pieces: usize,
    /// Time in each node, summed over the pieces.
    pub node_time: BTreeMap<String, Duration>,
    /// Time reading the sources.
    pub read_time: Duration,
    /// Time trimming and writing the outputs.
    pub write_time: Duration,
    /// The whole run.
    pub elapsed: Duration,
    /// The scripts that ran, by node: their file's hash (for provenance).
    pub scripts: BTreeMap<String, String>,
}

/// The operations an app runs, and the run itself.
pub struct Engine<D: Domain> {
    ops: BTreeMap<String, Box<dyn Operation<D>>>,
    catalog: Catalog,
}

impl<D: Domain> Engine<D> {
    /// An engine for `domain`'s types, with no operations yet.
    pub fn new(domain: &D) -> Self {
        Self { ops: BTreeMap::new(), catalog: Catalog::new(domain.port_types()) }
    }

    /// Adds an operation (replacing one with its id), after checking its description.
    pub fn add(&mut self, op: impl Operation<D> + 'static) -> std::result::Result<(), String> {
        let spec = op.spec().clone();
        self.catalog.add(spec.clone())?;
        self.ops.insert(spec.id, Box::new(op));
        Ok(())
    }

    /// Takes the operation `id` out (a script deleted, or no longer confirmed).
    pub fn remove(&mut self, id: &str) -> bool {
        self.catalog.remove(id);
        self.ops.remove(id).is_some()
    }

    /// The descriptions of every operation (the Operations menu, the graph check).
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Checks `graph`: every problem, or the plan.
    pub fn check(&self, graph: &Graph) -> Result<Plan> {
        graph.check(&self.catalog).map_err(PipelineError::Invalid)
    }

    /// Runs `graph` over `domain`, writing its outputs; `progress` after each piece.
    pub fn run(&self, graph: &Graph, domain: &mut D, cancel: &Cancel, progress: &mut dyn FnMut(&Progress)) -> Result<RunReport> {
        let started = Instant::now();
        let plan = self.check(graph)?;
        let pieces = domain.pieces(graph, plan.context)?;
        let mut report = RunReport { scripts: self.scripts(&plan), ..Default::default() };
        for piece in &pieces {
            if cancel.is_cancelled() {
                return Err(PipelineError::Cancelled);
            }
            let outputs = self.piece(graph, &plan, domain, piece, cancel, &mut report)?;
            let t = Instant::now();
            for (name, data) in outputs {
                let ty = self.type_at(graph, &plan.outputs[&name]);
                let data = domain.trim(data, &ty, piece)?;
                domain.write(&name, &ty, piece, data)?;
            }
            report.write_time += t.elapsed();
            report.pieces += 1;
            progress(&Progress { done: report.pieces, pieces: pieces.len(), elapsed: started.elapsed() });
        }
        domain.finish()?;
        report.elapsed = started.elapsed();
        Ok(report)
    }

    /// The outputs of `graph` for one `piece`, trimmed, not written: a view's preview of the
    /// part on screen.
    pub fn preview(&self, graph: &Graph, domain: &D, piece: &Piece) -> Result<BTreeMap<String, D::Data>> {
        let plan = self.check(graph)?;
        let mut report = RunReport::default();
        let outputs = self.piece(graph, &plan, domain, piece, &Cancel::new(), &mut report)?;
        outputs
            .into_iter()
            .map(|(name, data)| {
                let ty = self.type_at(graph, &plan.outputs[&name]);
                Ok((name, domain.trim(data, &ty, piece)?))
            })
            .collect()
    }

    /// Runs the plan's nodes on one piece; returns each output, untrimmed.
    fn piece(&self, graph: &Graph, plan: &Plan, domain: &D, piece: &Piece, cancel: &Cancel, report: &mut RunReport) -> Result<Vec<(String, D::Data)>> {
        let mut values: BTreeMap<Endpoint, D::Data> = BTreeMap::new();
        let mut left = plan.readers.clone();
        let t = Instant::now();
        for s in &plan.sources {
            let ty = &graph.sources[s];
            values.insert(Endpoint::source(s.clone()), domain.read(s, ty, piece)?);
        }
        report.read_time += t.elapsed();

        // A value for one reader: the last reader takes it, the others a clone
        fn take<T: Clone>(values: &mut BTreeMap<Endpoint, T>, left: &mut BTreeMap<Endpoint, usize>, e: &Endpoint) -> Option<T> {
            let n = left.get_mut(e)?;
            *n = n.saturating_sub(1);
            if *n == 0 { values.remove(e) } else { values.get(e).cloned() }
        }

        for id in &plan.order {
            if cancel.is_cancelled() {
                return Err(PipelineError::Cancelled);
            }
            if let Some(forward) = plan.forwards.get(id) {
                for (out, from) in forward {
                    let v = take(&mut values, &mut left, from).ok_or_else(|| PipelineError::Op { node: id.clone(), message: format!("{from} has no value") })?;
                    values.insert(Endpoint::port(id.clone(), out.clone()), v);
                }
                continue;
            }
            let op = &self.ops[&plan.ops[id]];
            let mut inputs = BTreeMap::new();
            for (port, from) in &plan.inputs[id] {
                let v = take(&mut values, &mut left, from).ok_or_else(|| PipelineError::Op { node: id.clone(), message: format!("{from} has no value") })?;
                inputs.insert(port.clone(), v);
            }
            let params = &plan.params[id];
            let t = Instant::now();
            let mut outs = op.run(Call { inputs, params, piece, domain, node: id }).map_err(|message| PipelineError::Op { node: id.clone(), message })?;
            *report.node_time.entry(id.clone()).or_default() += t.elapsed();
            for port in &op.spec().outputs {
                let v = outs.remove(&port.name).ok_or_else(|| PipelineError::Op { node: id.clone(), message: format!("gave no {}", port.name) })?;
                let e = Endpoint::port(id.clone(), port.name.clone());
                // Outputs nobody reads are dropped at once
                if plan.readers.contains_key(&e) {
                    values.insert(e, v);
                }
            }
        }

        let mut out = Vec::new();
        for (name, e) in &plan.outputs {
            let v = take(&mut values, &mut left, e).ok_or_else(|| PipelineError::Io(format!("output {name}: {e} has no value")))?;
            out.push((name.clone(), v));
        }
        Ok(out)
    }

    fn type_at(&self, graph: &Graph, e: &Endpoint) -> crate::types::PortType {
        match &e.port {
            None => graph.sources[&e.node].clone(),
            Some(p) => self.catalog.get(&graph.nodes[&e.node].op).and_then(|s| s.output_port(p)).map(|p| p.ty.clone()).unwrap_or_else(|| crate::types::PortType::new("any")),
        }
    }

    fn scripts(&self, plan: &Plan) -> BTreeMap<String, String> {
        plan.ops
            .iter()
            .filter_map(|(node, op)| match &self.catalog.get(op)?.source {
                OpSource::Script { hash, .. } => Some((node.clone(), hash.clone())),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::windows;
    use crate::graph::Node;
    use crate::op::{FnOp, OpSpec};
    use crate::types::{Context, ParamSpec, ParamValue, PortSpec, PortType};

    /// A recording of numbers, walked in windows; writes into `out`.
    struct Line {
        data: Vec<f64>,
        window: usize,
        out: BTreeMap<String, Vec<f64>>,
        reads: usize,
    }

    impl Domain for Line {
        type Data = Vec<f64>;
        fn name(&self) -> &str {
            "line"
        }
        fn port_types(&self) -> Vec<PortType> {
            vec![PortType::new("signal")]
        }
        fn pieces(&self, _: &Graph, context: Context) -> Result<Vec<Piece>> {
            Ok(windows(self.data.len(), self.window, context))
        }
        fn read(&self, _: &str, _: &PortType, piece: &Piece) -> Result<Vec<f64>> {
            let r = piece.read;
            Ok(self.data[r.start[2]..r.end()[2]].to_vec())
        }
        fn trim(&self, data: Vec<f64>, _: &PortType, piece: &Piece) -> Result<Vec<f64>> {
            Ok(data[piece.inner()[2].clone()].to_vec())
        }
        fn write(&mut self, name: &str, _: &PortType, _: &Piece, data: Vec<f64>) -> Result<()> {
            self.reads += 1;
            self.out.entry(name.to_string()).or_default().extend(data);
            Ok(())
        }
    }

    /// A moving sum over `k` samples before: needs `k` of context before.
    fn engine(line: &Line) -> Engine<Line> {
        let mut e = Engine::new(line);
        let k = 3;
        let spec = OpSpec::new("sum", "Moving sum").input(PortSpec::new("x", "signal")).output(PortSpec::new("y", "signal")).context(Context::time(k, 0));
        e.add(FnOp::new(spec, move |mut call: Call<'_, Line>| {
            let x = call.take("x")?;
            let y = (0..x.len()).map(|i| x[i.saturating_sub(k)..=i].iter().sum()).collect();
            Ok(BTreeMap::from([("y".to_string(), y)]))
        }))
        .unwrap();
        let spec = OpSpec::new("gain", "Gain").input(PortSpec::new("x", "signal")).output(PortSpec::new("y", "signal")).param(ParamSpec::number("g", 1.0));
        e.add(FnOp::new(spec, |mut call: Call<'_, Line>| {
            let g = call.number("g")?;
            let x = call.take("x")?;
            Ok(BTreeMap::from([("y".to_string(), x.into_iter().map(|v| v * g).collect())]))
        }))
        .unwrap();
        e
    }

    fn graph() -> Graph {
        let mut g = Graph::new("line");
        g.sources.insert("raw".into(), PortType::new("signal"));
        g.nodes.insert("sum".into(), Node::new("sum"));
        g.nodes.insert("gain".into(), Node::new("gain").with("g", ParamValue::Number(2.0)));
        g.connect("raw".parse().unwrap(), "sum.x".parse().unwrap());
        g.connect("sum.y".parse().unwrap(), "gain.x".parse().unwrap());
        g.outputs.insert("out".into(), "gain.y".parse().unwrap());
        g.outputs.insert("summed".into(), "sum.y".parse().unwrap());
        g
    }

    /// The moving sum over the whole line, for comparison.
    fn whole(data: &[f64], g: f64) -> Vec<f64> {
        (0..data.len()).map(|i| data[i.saturating_sub(3)..=i].iter().sum::<f64>() * g).collect()
    }

    #[test]
    fn windows_give_the_same_result_as_the_whole() {
        let data: Vec<f64> = (0..100).map(|i| (i * 7 % 13) as f64).collect();
        let mut line = Line { data: data.clone(), window: 16, out: BTreeMap::new(), reads: 0 };
        let e = engine(&line);
        let mut seen = Vec::new();
        let report = e.run(&graph(), &mut line, &Cancel::new(), &mut |p| seen.push(p.done)).unwrap();
        assert_eq!(report.pieces, 7);
        assert_eq!(seen, (1..=7).collect::<Vec<_>>());
        assert_eq!(line.out["out"], whole(&data, 2.0), "no seams between windows");
        assert_eq!(line.out["summed"], whole(&data, 1.0), "one output feeds a node and an output");
        assert!(report.node_time.contains_key("sum") && report.scripts.is_empty());
    }

    #[test]
    fn a_disabled_node_passes_through_and_a_preview_writes_nothing() {
        let data: Vec<f64> = (0..40).map(f64::from).collect();
        let mut line = Line { data: data.clone(), window: 10, out: BTreeMap::new(), reads: 0 };
        let e = engine(&line);
        let mut g = graph();
        g.nodes.get_mut("gain").unwrap().enabled = false;
        e.run(&g, &mut line, &Cancel::new(), &mut |_| {}).unwrap();
        assert_eq!(line.out["out"], whole(&data, 1.0));
        let piece = windows(40, 10, Context::time(3, 0))[2];
        let p = e.preview(&graph(), &line, &piece).unwrap();
        assert_eq!(p["out"], whole(&data, 2.0)[20..30]);
    }

    #[test]
    fn cancel_and_failures_stop_the_run() {
        let mut line = Line { data: vec![1.0; 50], window: 10, out: BTreeMap::new(), reads: 0 };
        let e = engine(&line);
        let cancel = Cancel::new();
        let c = cancel.clone();
        let err = e.run(&graph(), &mut line, &cancel, &mut |p| if p.done == 2 { c.cancel() }).unwrap_err();
        assert!(matches!(err, PipelineError::Cancelled));
        assert_eq!(line.reads, 4, "two pieces, two outputs each");
        let mut bad = graph();
        bad.nodes.insert("x".into(), Node::new("nope"));
        bad.outputs.insert("x".into(), "x.y".parse().unwrap());
        assert!(matches!(e.run(&bad, &mut line, &Cancel::new(), &mut |_| {}), Err(PipelineError::Invalid(_))));
    }
}
