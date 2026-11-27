//! Every op an app offers, by id: its own, the shared ones, its tools', and the script ops and
//! recipes in its folders ([standard §2.2](../../../standard/spec/02-ops.md)). Another app's ops
//! are not in it. The Operations menu, the search, the pipeline check and the runtime's `ops` call
//! read it; it holds descriptions only (the runtime holds what runs).

use std::collections::BTreeMap;

use crate::ops::{OpSource, OpSpec};
use crate::types::Type;

/// The operations an app offers, and the port types it declares.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    specs: BTreeMap<String, OpSpec>,
    types: Vec<Type>,
}

impl Catalog {
    /// An empty catalog for an app whose data flows as `types`.
    pub fn new(types: impl IntoIterator<Item = Type>) -> Self {
        Self { specs: BTreeMap::new(), types: types.into_iter().collect() }
    }

    /// The port types the app declares.
    pub fn types(&self) -> &[Type] {
        &self.types
    }

    /// Whether `ty` is one of the app's types (or `any`).
    pub fn knows(&self, ty: &Type) -> bool {
        ty.as_str() == Type::ANY || self.types.contains(ty)
    }

    /// Adds `spec` (replacing one with its id), after checking it is well formed and names only
    /// the app's types.
    pub fn add(&mut self, spec: OpSpec) -> Result<(), String> {
        spec.check()?;
        if let Some(p) = spec.inputs.iter().chain(&spec.outputs).find(|p| !self.knows(&p.ty)) {
            let known: Vec<&str> = self.types.iter().map(Type::as_str).collect();
            return Err(format!("{}: port {} has type {}, not one of {}", spec.id, p.name, p.ty, known.join(", ")));
        }
        self.specs.insert(spec.id.clone(), spec);
        Ok(())
    }

    /// Takes the operation `id` out; whether it was there.
    pub fn remove(&mut self, id: &str) -> bool {
        self.specs.remove(id).is_some()
    }

    /// The operation `id`.
    pub fn get(&self, id: &str) -> Option<&OpSpec> {
        self.specs.get(id)
    }

    /// Every operation, by id.
    pub fn all(&self) -> impl Iterator<Item = &OpSpec> {
        self.specs.values()
    }

    /// How many operations.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// The operations by family, families and operations by name (the Operations menu). Scripts
    /// without a family go under "Scripts", tools' under their tool's name.
    pub fn by_family(&self) -> Vec<(String, Vec<&OpSpec>)> {
        let mut out: BTreeMap<String, Vec<&OpSpec>> = BTreeMap::new();
        for s in self.specs.values() {
            let family = if !s.family.is_empty() {
                s.family.clone()
            } else {
                match &s.source {
                    OpSource::Builtin => "Other".into(),
                    OpSource::Script { .. } => "Scripts".into(),
                    OpSource::Tool { tool, .. } => tool.clone(),
                    OpSource::Recipe { .. } => "Recipes".into(),
                }
            };
            out.entry(family).or_default().push(s);
        }
        let mut out: Vec<(String, Vec<&OpSpec>)> = out.into_iter().collect();
        for (_, ops) in &mut out {
            ops.sort_by_key(|o| o.title.to_lowercase());
        }
        out
    }

    /// The operations that take an input of type `ty` (what can follow a node's output).
    pub fn taking(&self, ty: &Type) -> Vec<&OpSpec> {
        self.specs.values().filter(|s| s.inputs.iter().any(|p| ty.feeds(&p.ty))).collect()
    }

    /// The ops a screen's offers can run: every required input filled by one of `offers` (the
    /// runtime's `ops` with `fillable_by`, [standard §6.1](../../../standard/spec/06-runtime-api.md)).
    pub fn fillable_by(&self, offers: &[Type]) -> Vec<&OpSpec> {
        self.specs.values().filter(|s| s.inputs.iter().filter(|p| !p.optional).all(|p| offers.iter().any(|o| o.feeds(&p.ty)))).collect()
    }

    /// The operations whose title, id, family or line contains every word of `query` (case
    /// ignored).
    pub fn search(&self, query: &str) -> Vec<&OpSpec> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.specs
            .values()
            .filter(|s| {
                let text = format!("{} {} {} {}", s.title, s.id, s.family, s.summary).to_lowercase();
                words.iter().all(|w| text.contains(w))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::PortSpec;
    use crate::params::ParamSpec;

    fn catalog() -> Catalog {
        let mut c = Catalog::new(["image", "mask", "table"].map(Type::new));
        c.add(OpSpec::new("blur", "Gaussian blur").family("Filter").input(PortSpec::new("image", "image")).output(PortSpec::new("image", "image")).param(ParamSpec::number("sigma", 2.0))).unwrap();
        c.add(OpSpec::new("otsu", "Otsu threshold").family("Threshold").input(PortSpec::new("image", "image")).output(PortSpec::new("mask", "mask"))).unwrap();
        c.add(OpSpec::new("count", "Count objects").summary("Counts the objects of a mask").input(PortSpec::new("mask", "mask")).output(PortSpec::new("counts", "table"))).unwrap();
        c
    }

    #[test]
    fn refuses_unknown_types() {
        let mut c = catalog();
        let err = c.add(OpSpec::new("hp", "High-pass").input(PortSpec::new("x", "signal")).output(PortSpec::new("x", "signal"))).unwrap_err();
        assert!(err.contains("signal") && err.contains("image, mask, table"), "{err}");
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn groups_finds_and_follows() {
        let c = catalog();
        let grouped = c.by_family();
        let families: Vec<&str> = grouped.iter().map(|(f, _)| f.as_str()).collect();
        assert_eq!(families, ["Filter", "Other", "Threshold"]);
        let after_mask: Vec<&str> = c.taking(&Type::new("mask")).iter().map(|s| s.id.as_str()).collect();
        assert_eq!(after_mask, ["count"]);
        assert_eq!(c.search("objects COUNT").len(), 1);
        assert_eq!(c.search("thresh").first().map(|s| s.id.as_str()), Some("otsu"));
    }

    #[test]
    fn a_screen_lists_the_ops_its_offers_can_run() {
        let c = catalog();
        let fill = |offers: &[&str]| -> Vec<String> { c.fillable_by(&offers.iter().map(Type::new).collect::<Vec<_>>()).iter().map(|s| s.id.clone()).collect() };
        assert_eq!(fill(&["image"]), ["blur", "otsu"]);
        assert_eq!(fill(&["mask"]), ["count"]);
        assert!(fill(&["table"]).is_empty());
    }
}
