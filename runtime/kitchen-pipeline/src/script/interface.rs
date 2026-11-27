//! What a script declares: its interface (ports, parameters, context, its name and place), read
//! without running it.
//!
//! Two ways to write it, the same rules for both (`python/kitchen_pipeline.py` says them for
//! people):
//!
//! - **A decorated Python function**: `@<app>.op(...)`, `@<app>.param(...)`, `@<app>.input(...)`,
//!   `@<app>.output(...)` over a `def` whose signature gives the rest. `<app>` is any of the
//!   names the app accepts ([`Options::aliases`]: `kitchen`, `lucem`, `dspk`).
//! - **Header lines** (Python `#`, Rust `//`), for Rust scripts and for Python ones without the
//!   decorators:
//!   ```text
//!   # kitchen: name = Local contrast
//!   # kitchen: family = Filter
//!   # kitchen: halo = 12                    (or: before = 300, after = 0)
//!   # kitchen: input image: image
//!   # kitchen: input mask: mask optional
//!   # kitchen: param sigma = 3.0 (0.2..20) px
//!   # kitchen: param mode = reflect (reflect|constant|wrap)
//!   # kitchen: output image: image
//!   # kitchen: node = in image:image; out counts:table       (kitchen-editor's form)
//!   ```
//!
//! Both are first turned into a [`RawSignature`] (values as the Python literals they were written
//! as); the probe returns one too, from the function itself ([`super::probe`]), and
//! [`Interface::from_raw`] applies the rules to either, so the two can be compared.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::op::{Language, OpSource, OpSpec};
use crate::types::{Context, ParamKind, ParamSpec, ParamValue, PortSpec, PortType};

/// How an app reads scripts.
#[derive(Clone, Debug)]
pub struct Options {
    /// The names its decorators and headers go by (`kitchen` always; `lucem`, `lumen`, `dspk`).
    pub aliases: Vec<String>,
    /// Its port types.
    pub types: Vec<PortType>,
    /// The type of an array argument that says nothing more (`x: np.ndarray`): lucem `image`,
    /// dsp-kitchen `signal`.
    pub default_type: PortType,
}

impl Options {
    /// An app's options: its own name as an alias besides `kitchen`.
    pub fn new(app: &str, types: &[&str], default_type: &str) -> Self {
        let mut aliases = vec!["kitchen".to_string()];
        if app != "kitchen" {
            aliases.push(app.to_string());
        }
        Self { aliases, types: types.iter().map(PortType::new).collect(), default_type: PortType::new(default_type) }
    }

    /// The same options, also reading `alias` (an app's former name).
    pub fn alias(mut self, alias: &str) -> Self {
        self.aliases.push(alias.to_string());
        self
    }
}

/// A function argument as written: its annotation and default as source text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawArg {
    /// Its name.
    pub name: String,
    /// Its annotation (`float`, `np.ndarray`, `port:image`), if any.
    #[serde(default)]
    pub annotation: Option<String>,
    /// Its default as a Python literal (`3.0`, `'reflect'`, `None`), if any.
    #[serde(default)]
    pub default: Option<String>,
}

/// What a script's text (or the probe) says, before the rules: values are Python literals.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSignature {
    /// The function (`local_contrast_boost`, `run`).
    #[serde(default)]
    pub function: String,
    /// The docstring's first line.
    #[serde(default)]
    pub doc: Option<String>,
    /// The `op` decorator's (or header's) settings: `name`, `family`, `halo`…; `None` when the
    /// script is not marked as an operation.
    #[serde(default)]
    pub op: Option<BTreeMap<String, String>>,
    /// The function's arguments.
    #[serde(default)]
    pub args: Vec<RawArg>,
    /// Its return annotation.
    #[serde(default)]
    pub returns: Option<String>,
    /// `param` declarations (`name`, `default`, `min`, `max`, `unit`, `choices`, `hint`).
    #[serde(default)]
    pub params: Vec<BTreeMap<String, String>>,
    /// `input` declarations (`name`, `type`, `optional`).
    #[serde(default)]
    pub inputs: Vec<BTreeMap<String, String>>,
    /// `output` declarations (`name`, `type`).
    #[serde(default)]
    pub outputs: Vec<BTreeMap<String, String>>,
}

/// What a script is, as an operation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interface {
    /// Its name for people.
    pub title: String,
    /// Its family.
    pub family: String,
    /// One line.
    pub summary: String,
    /// The function the worker calls.
    pub function: String,
    /// Its inputs.
    pub inputs: Vec<PortSpec>,
    /// Its outputs.
    pub outputs: Vec<PortSpec>,
    /// Its parameters.
    pub params: Vec<ParamSpec>,
    /// Its context.
    pub context: Context,
    /// Whether the function takes the piece (`tile` / `piece` argument).
    pub takes_piece: bool,
}

impl Interface {
    /// As an operation `id` from `source`.
    pub fn spec(&self, id: impl Into<String>, source: OpSource) -> OpSpec {
        OpSpec {
            id: id.into(),
            title: self.title.clone(),
            family: self.family.clone(),
            summary: self.summary.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            params: self.params.clone(),
            context: self.context,
            source,
        }
    }

    /// The rules applied to `raw`: the interface, and what did not read (the script is not usable
    /// until there is nothing).
    pub fn from_raw(raw: &RawSignature, opts: &Options) -> (Interface, Vec<String>) {
        let mut problems = Vec::new();
        let op = raw.op.clone().unwrap_or_default();
        let text = |k: &str| op.get(k).and_then(|v| lit(v)).and_then(|l| l.text());
        let count = |k: &str, problems: &mut Vec<String>| -> usize {
            match op.get(k).map(|v| lit(v)) {
                None | Some(Some(Lit::None)) => 0,
                Some(Some(Lit::Int(n))) if n >= 0 => n as usize,
                _ => {
                    problems.push(format!("{k} is a whole number of samples or pixels"));
                    0
                }
            }
        };
        let halo = count("halo", &mut problems);
        let (before, after) = (count("before", &mut problems), count("after", &mut problems));
        let context = Context::around(halo, 3).max(Context::time(before, after));

        let type_of = |annotation: Option<&str>, name: &str| -> PortType {
            if let Some(a) = annotation {
                if let Some(t) = a.strip_prefix("port:") {
                    return PortType::new(t);
                }
                let words: Vec<String> = a.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
                if let Some(t) = opts.types.iter().find(|t| words.iter().any(|w| w == t.as_str() || w.trim_end_matches('s') == t.as_str())) {
                    return t.clone();
                }
            }
            let n = name.to_lowercase();
            opts.types.iter().find(|t| n == t.as_str() || n.split('_').any(|w| w == t.as_str())).cloned().unwrap_or_else(|| opts.default_type.clone())
        };

        // Inputs: declared first, then the signature's arrays
        let mut inputs: Vec<PortSpec> = Vec::new();
        for d in &raw.inputs {
            let (Some(name), Some(ty)) = (d.get("name").and_then(|v| lit(v)).and_then(|l| l.text()), d.get("type").and_then(|v| lit(v)).and_then(|l| l.text())) else {
                problems.push("an input is declared with a name and a type".into());
                continue;
            };
            let optional = matches!(d.get("optional").and_then(|v| lit(v)), Some(Lit::Bool(true)));
            let mut p = PortSpec::new(name, ty);
            p.optional = optional;
            inputs.push(p);
        }
        let declared_params: Vec<String> = raw.params.iter().filter_map(|d| d.get("name").and_then(|v| lit(v)).and_then(|l| l.text())).collect();
        let mut params: Vec<ParamSpec> = Vec::new();
        let mut takes_piece = false;
        for a in &raw.args {
            if a.name == "tile" || a.name == "piece" {
                takes_piece = true;
                continue;
            }
            if a.name == "self" || inputs.iter().any(|p| p.name == a.name) {
                continue;
            }
            let ann = a.annotation.as_deref().map(str::trim);
            let scalar = ann.is_some_and(is_scalar);
            let default = a.default.as_deref().and_then(lit);
            match (&default, scalar) {
                // A value with a default: a parameter (refined by @param below)
                (Some(d), _) if !matches!(d, Lit::None) => match param_from(&a.name, d, ann) {
                    Ok(p) => params.push(p),
                    Err(e) => problems.push(e),
                },
                // A plain value without a default
                (None, true) if !declared_params.contains(&a.name) => problems.push(format!("{}: a parameter needs a default (`{}: {} = …`)", a.name, a.name, ann.unwrap_or(""))),
                (None, true) => {}
                // An array: an input (optional when it defaults to None)
                (d, false) => {
                    let mut p = PortSpec::new(a.name.clone(), type_of(ann, &a.name).as_str());
                    p.optional = matches!(d, Some(Lit::None));
                    inputs.push(p);
                }
                (Some(_), true) => problems.push(format!("{}: a plain value cannot default to None", a.name)),
            }
        }
        // @param refines or adds
        for d in &raw.params {
            let Some(name) = d.get("name").and_then(|v| lit(v)).and_then(|l| l.text()) else {
                problems.push("a param is declared with a name".into());
                continue;
            };
            let existing = params.iter().position(|p| p.name == name);
            let default = d.get("default").and_then(|v| lit(v));
            let mut p = match (existing, &default) {
                (Some(i), None) => params[i].clone(),
                (_, Some(dl)) => match param_from(&name, dl, None) {
                    Ok(p) => p,
                    Err(e) => {
                        problems.push(e);
                        continue;
                    }
                },
                (None, None) => {
                    problems.push(format!("{name}: a parameter needs a default"));
                    continue;
                }
            };
            if let Some(choices) = d.get("choices").and_then(|v| lit(v)).and_then(|l| l.texts()) {
                p.kind = ParamKind::Choice { options: choices };
            }
            let num = |k: &str| d.get(k).and_then(|v| lit(v)).and_then(|l| l.number());
            match &mut p.kind {
                ParamKind::Number { min, max, unit } => {
                    *min = num("min").or(*min);
                    *max = num("max").or(*max);
                    if let Some(u) = d.get("unit").and_then(|v| lit(v)).and_then(|l| l.text()) {
                        *unit = Some(u);
                    }
                }
                ParamKind::Integer { min, max } => {
                    *min = num("min").map(|v| v as i64).or(*min);
                    *max = num("max").map(|v| v as i64).or(*max);
                }
                _ => {}
            }
            if let Some(h) = d.get("hint").and_then(|v| lit(v)).and_then(|l| l.text()) {
                p.hint = h;
            }
            if let Err(e) = p.check(&p.default) {
                problems.push(format!("the default of {e}"));
            }
            match existing {
                Some(i) => params[i] = p,
                None => params.push(p),
            }
        }

        // Outputs: declared, else op(output=), else the return annotation
        let mut outputs: Vec<PortSpec> = Vec::new();
        for d in &raw.outputs {
            let name = d.get("name").and_then(|v| lit(v)).and_then(|l| l.text());
            let ty = d.get("type").and_then(|v| lit(v)).and_then(|l| l.text()).or_else(|| name.clone());
            match (name, ty) {
                (Some(n), Some(t)) => outputs.push(PortSpec::new(n, t)),
                _ => problems.push("an output is declared with a name and a type".into()),
            }
        }
        if outputs.is_empty() {
            let ty = text("output").map(PortType::new).or_else(|| {
                let r = raw.returns.as_deref()?.trim();
                if r == "None" {
                    return None;
                }
                let lower = r.to_lowercase();
                if lower.starts_with("dict") || lower.starts_with("tuple") || lower.starts_with("typing.dict") {
                    problems.push("a function returning several results declares each with @output(name, type)".into());
                    return None;
                }
                Some(type_of(Some(r), ""))
            });
            match ty {
                Some(t) => outputs.push(PortSpec::new(t.as_str(), t.as_str())),
                None if raw.returns.is_none() => outputs.push(PortSpec::new(opts.default_type.as_str(), opts.default_type.as_str())),
                None => {}
            }
        }
        if outputs.is_empty() {
            problems.push("no output: annotate the return (`-> Image`) or declare @output".into());
        }
        for p in inputs.iter().chain(&outputs) {
            if p.ty.as_str() != PortType::ANY && !opts.types.contains(&p.ty) {
                let known: Vec<&str> = opts.types.iter().map(PortType::as_str).collect();
                problems.push(format!("{}: type {} is not one of {}", p.name, p.ty, known.join(", ")));
            }
        }

        let function = if raw.function.is_empty() { "run".to_string() } else { raw.function.clone() };
        let title = text("name").unwrap_or_else(|| pretty(&function));
        let summary = text("description").or_else(|| raw.doc.clone()).unwrap_or_default();
        let family = text("family").unwrap_or_default();
        (Interface { title, family, summary, function, inputs, outputs, params, context, takes_piece }, problems)
    }

    /// What changed from `before` to `self`, one line each (for the confirmation).
    pub fn changes_from(&self, before: &Interface) -> Vec<String> {
        let mut out = Vec::new();
        let ports = |v: &[PortSpec]| v.iter().map(|p| (p.name.clone(), format!("{}{}", p.ty, if p.optional { " (optional)" } else { "" }))).collect::<BTreeMap<_, _>>();
        let params = |v: &[ParamSpec]| v.iter().map(|p| (p.name.clone(), serde_json::to_string(p).unwrap_or_default())).collect::<BTreeMap<_, _>>();
        for (what, a, b) in [("input", ports(&before.inputs), ports(&self.inputs)), ("output", ports(&before.outputs), ports(&self.outputs)), ("parameter", params(&before.params), params(&self.params))] {
            for (k, v) in &b {
                match a.get(k) {
                    None => out.push(format!("new {what} {k}")),
                    Some(old) if old != v => out.push(format!("{what} {k} changed")),
                    _ => {}
                }
            }
            for k in a.keys().filter(|k| !b.contains_key(*k)) {
                out.push(format!("{what} {k} removed"));
            }
        }
        if self.context != before.context {
            out.push("its context changed".into());
        }
        if self.function != before.function {
            out.push(format!("now runs {}", self.function));
        }
        out
    }
}

fn is_scalar(annotation: &str) -> bool {
    let a = annotation.trim_start_matches("typing.");
    matches!(a, "float" | "int" | "bool" | "str") || a.starts_with("Literal[") || a.starts_with("Optional[float") || a.starts_with("Optional[int")
}

/// `name = 3 (lo..hi) unit` — a parameter from its default and annotation.
fn param_from(name: &str, default: &Lit, annotation: Option<&str>) -> Result<ParamSpec, String> {
    if let Some(choices) = annotation.and_then(|a| a.trim_start_matches("typing.").strip_prefix("Literal[")).and_then(|r| r.strip_suffix(']')).and_then(|inner| lit(&format!("[{inner}]"))).and_then(|l| l.texts()) {
        let mut p = ParamSpec::choice(name, &choices.iter().map(String::as_str).collect::<Vec<_>>());
        if let Some(d) = default.text() {
            p.default = ParamValue::Text(d);
        }
        return Ok(p);
    }
    let float_annotated = annotation.is_some_and(|a| a.contains("float"));
    Ok(match default {
        Lit::Bool(b) => ParamSpec::switch(name, *b),
        Lit::Int(i) if !float_annotated => ParamSpec::integer(name, *i),
        Lit::Int(i) => ParamSpec::number(name, *i as f64),
        Lit::Float(f) => ParamSpec::number(name, *f),
        Lit::Str(s) => ParamSpec::text(name, s.clone()),
        Lit::None | Lit::List(_) => return Err(format!("{name}: a parameter's default is a number, a switch or text")),
    })
}

/// `local_contrast_boost` → `Local contrast boost`.
pub fn pretty(function: &str) -> String {
    let words: Vec<&str> = function.split('_').filter(|w| !w.is_empty()).collect();
    let mut s = words.join(" ");
    if let Some(first) = s.get(0..1) {
        s = first.to_uppercase() + &s[1..];
    }
    s
}

/// A Python literal.
#[derive(Clone, Debug, PartialEq)]
pub enum Lit {
    /// `None`.
    None,
    /// `True`, `False`.
    Bool(bool),
    /// `3`, `-2`.
    Int(i64),
    /// `3.0`, `1e-3`.
    Float(f64),
    /// `'a'`, `"a"`.
    Str(String),
    /// `[…]`, `(…)`.
    List(Vec<Lit>),
}

impl Lit {
    fn text(&self) -> Option<String> {
        match self {
            Lit::Str(s) => Some(s.clone()),
            _ => None,
        }
    }

    fn number(&self) -> Option<f64> {
        match self {
            Lit::Int(i) => Some(*i as f64),
            Lit::Float(f) => Some(*f),
            _ => None,
        }
    }

    fn texts(&self) -> Option<Vec<String>> {
        match self {
            Lit::List(v) => v.iter().map(Lit::text).collect(),
            _ => None,
        }
    }
}

/// Reads a Python literal (what a decorator or a default is written as).
pub fn lit(text: &str) -> Option<Lit> {
    let t = text.trim();
    match t {
        "None" => return Some(Lit::None),
        "True" => return Some(Lit::Bool(true)),
        "False" => return Some(Lit::Bool(false)),
        _ => {}
    }
    if let Some(inner) = t.strip_prefix('[').and_then(|r| r.strip_suffix(']')).or_else(|| t.strip_prefix('(').and_then(|r| r.strip_suffix(')'))) {
        let items: Option<Vec<Lit>> = split_top(inner, ',').iter().filter(|s| !s.trim().is_empty()).map(|s| lit(s)).collect();
        return items.map(Lit::List);
    }
    for q in ['"', '\''] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) {
            return Some(Lit::Str(t[1..t.len() - 1].replace(&format!("\\{q}"), &q.to_string())));
        }
    }
    let n = t.replace('_', "");
    if let Ok(i) = n.parse::<i64>() {
        return Some(Lit::Int(i));
    }
    n.parse::<f64>().ok().filter(|f| f.is_finite()).map(Lit::Float)
}

/// `s` split at `sep` outside quotes and brackets.
pub fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in s.chars() {
        if let Some(q) = quote {
            cur.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                cur.push(c);
            }
            c if c == sep && depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur);
    }
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

/// The part of `s` from an opening `(` at byte 0 up to its matching `)`: (inside, rest after).
fn balanced(s: &str) -> Option<(&str, &str)> {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[1..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// A decorator's arguments as keywords, positionals named in `order`.
fn decl(args: &str, order: &[&str]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut pos = 0;
    for a in split_top(args, ',') {
        if a.is_empty() {
            continue;
        }
        // `name=value`, but not `==` and not an `=` inside a string
        let eq = a.find('=').filter(|i| a[..*i].chars().all(|c| c.is_alphanumeric() || c == '_' || c == ' ') && !a[*i + 1..].starts_with('='));
        match eq {
            Some(i) => {
                out.insert(a[..i].trim().to_string(), a[i + 1..].trim().to_string());
            }
            None => {
                if let Some(k) = order.get(pos) {
                    out.insert(k.to_string(), a.trim().to_string());
                }
                pos += 1;
            }
        }
    }
    out
}

/// Reads a script's text: the decorated function if there is one, else the header lines (and
/// `def run`'s signature in Python). `None` when the file declares nothing (not an operation).
pub fn read(text: &str, language: Language, opts: &Options) -> Option<RawSignature> {
    let mut raw = RawSignature::default();
    let header = read_header(text, language, opts, &mut raw);
    if language == Language::Python {
        if let Some(found) = read_decorated(text, opts) {
            // Header lines add to what the decorators say
            let mut r = found;
            if let (Some(op), Some(h)) = (&mut r.op, raw.op.take()) {
                for (k, v) in h {
                    op.entry(k).or_insert(v);
                }
            }
            r.params.extend(raw.params);
            r.inputs.extend(raw.inputs);
            r.outputs.extend(raw.outputs);
            return Some(r);
        }
        if header {
            if let Some(run) = read_def(text, "run") {
                raw.function = run.function;
                raw.args = run.args;
                raw.returns = run.returns;
                raw.doc = run.doc;
            }
            return Some(raw);
        }
        return None;
    }
    header.then(|| {
        raw.function = "run".into();
        raw
    })
}

/// Reads the header lines into `raw`; whether there were any.
fn read_header(text: &str, language: Language, opts: &Options, raw: &mut RawSignature) -> bool {
    let mut any = false;
    let q = |s: &str| format!("{:?}", s.trim());
    for line in text.lines() {
        let Some(comment) = line.trim_start().strip_prefix(language.comment()) else { continue };
        let comment = comment.trim();
        let Some(rest) = opts.aliases.iter().find_map(|a| comment.strip_prefix(a.as_str()).and_then(|r| r.strip_prefix(':'))) else { continue };
        any = true;
        let rest = rest.trim();
        let op = raw.op.get_or_insert_with(BTreeMap::new);
        if let Some((k, v)) = rest.split_once('=').filter(|(k, _)| matches!(k.trim(), "name" | "family" | "description" | "halo" | "before" | "after" | "node" | "command" | "hint" | "args")) {
            let (k, v) = (k.trim(), v.trim());
            match k {
                "halo" | "before" | "after" => {
                    op.insert(k.into(), v.to_string());
                }
                "node" => {
                    // kitchen-editor's `in a:image, b:mask; out counts:table`
                    for part in v.split(';') {
                        let part = part.trim();
                        let (dir, list) = part.split_once(char::is_whitespace).unwrap_or((part, ""));
                        for port in list.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                            let (n, t) = port.split_once(':').unwrap_or((port, port));
                            let d = BTreeMap::from([("name".to_string(), q(n)), ("type".to_string(), q(t))]);
                            if dir == "in" {
                                raw.inputs.push(d);
                            } else if dir == "out" {
                                raw.outputs.push(d);
                            }
                        }
                    }
                }
                "name" | "family" | "description" => {
                    op.insert(k.into(), q(v));
                }
                _ => {} // a command's lines are kitchen-editor's
            }
        } else if let Some(r) = rest.strip_prefix("input ") {
            let (n, t) = r.split_once(':').unwrap_or((r, ""));
            let mut words = t.split_whitespace();
            let ty = words.next().unwrap_or("");
            let optional = words.any(|w| w == "optional");
            raw.inputs.push(BTreeMap::from([("name".to_string(), q(n)), ("type".to_string(), q(ty)), ("optional".to_string(), if optional { "True".into() } else { "False".into() })]));
        } else if let Some(r) = rest.strip_prefix("output") {
            let r = r.trim();
            let (n, t) = r.split_once(':').unwrap_or((r, r));
            raw.outputs.push(BTreeMap::from([("name".to_string(), q(n)), ("type".to_string(), q(t))]));
        } else if let Some(r) = rest.strip_prefix("param ") {
            let Some((n, v)) = r.split_once('=') else { continue };
            let v = v.trim();
            // `3.0 (0.2..20) px`, `reflect (reflect|constant)`
            let (value, tail) = match v.find(" (") {
                Some(i) => (&v[..i], &v[i + 1..]),
                None => v.split_once(char::is_whitespace).map(|(a, b)| (a, b)).unwrap_or((v, "")),
            };
            let mut d = BTreeMap::from([("name".to_string(), q(n))]);
            let value = value.trim();
            d.insert("default".into(), if lit(value).is_some() { value.to_string() } else { q(value) });
            let tail = tail.trim();
            let (bracket, unit) = match tail.strip_prefix('(').and_then(|t| t.split_once(')')) {
                Some((inside, after)) => (Some(inside), after.trim()),
                None => (None, tail),
            };
            if let Some(inside) = bracket {
                if let Some((lo, hi)) = inside.split_once("..") {
                    d.insert("min".into(), lo.trim().to_string());
                    d.insert("max".into(), hi.trim().to_string());
                } else if inside.contains('|') {
                    let opts: Vec<String> = inside.split('|').map(|o| q(o)).collect();
                    d.insert("choices".into(), format!("[{}]", opts.join(", ")));
                }
            }
            if !unit.is_empty() {
                d.insert("unit".into(), q(unit));
            }
            raw.params.push(d);
        }
    }
    if raw.op.as_ref().is_some_and(BTreeMap::is_empty) && raw.inputs.is_empty() && raw.outputs.is_empty() && raw.params.is_empty() {
        raw.op = Some(BTreeMap::new());
    }
    any
}

/// The first function decorated with one of the app's `op` decorators.
fn read_decorated(text: &str, opts: &Options) -> Option<RawSignature> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].trim_start().starts_with('@') {
            i += 1;
            continue;
        }
        // The decorator block: from here to the `def`, joining lines inside brackets
        let mut decorators: Vec<String> = Vec::new();
        let mut j = i;
        while j < lines.len() && lines[j].trim_start().starts_with('@') {
            let mut d = lines[j].trim().to_string();
            while balanced_or_bare(&d).is_none() && j + 1 < lines.len() {
                j += 1;
                d.push(' ');
                d.push_str(lines[j].trim());
            }
            decorators.push(d);
            j += 1;
        }
        let rest = lines[j.min(lines.len())..].join("\n");
        let mut raw = RawSignature::default();
        let mut is_op = false;
        for d in &decorators {
            let body = &d[1..];
            let Some((alias, call)) = body.split_once('.') else { continue };
            if !opts.aliases.iter().any(|a| a == alias.trim()) {
                continue;
            }
            let (name, args) = match call.find('(') {
                Some(p) => (&call[..p], balanced(&call[p..]).map(|(inside, _)| inside).unwrap_or("")),
                None => (call.trim(), ""),
            };
            match name.trim() {
                "op" | "transform" | "operation" => {
                    is_op = true;
                    raw.op = Some(decl(args, &[]));
                }
                "param" => raw.params.push(decl(args, &["name", "default"])),
                "input" => raw.inputs.push(decl(args, &["name", "type"])),
                "output" => raw.outputs.push(decl(args, &["name", "type"])),
                _ => {}
            }
        }
        if is_op {
            if let Some(def) = read_def_at(&rest) {
                return Some(RawSignature { function: def.function, args: def.args, returns: def.returns, doc: def.doc, ..raw });
            }
        }
        i = j.max(i + 1);
    }
    None
}

fn balanced_or_bare(d: &str) -> Option<()> {
    match d.find('(') {
        Some(p) => balanced(&d[p..]).map(|_| ()),
        None => Some(()),
    }
}

/// The function `name`'s signature, wherever it is defined at the top level.
fn read_def(text: &str, name: &str) -> Option<RawSignature> {
    let at = text.lines().position(|l| l.starts_with(&format!("def {name}(")))?;
    read_def_at(&text.lines().skip(at).collect::<Vec<_>>().join("\n"))
}

/// The signature of the `def` that `text` starts with (after blank lines).
fn read_def_at(text: &str) -> Option<RawSignature> {
    let t = text.trim_start();
    let t = t.strip_prefix("async ").unwrap_or(t);
    let after = t.strip_prefix("def ")?;
    let p = after.find('(')?;
    let function = after[..p].trim().to_string();
    let (args_text, rest) = balanced(&after[p..])?;
    let colon = rest.find(':')?;
    let returns = rest[..colon].trim().strip_prefix("->").map(|r| r.trim().to_string());
    let mut args = Vec::new();
    for a in split_top(args_text, ',') {
        let a = a.trim();
        if a.is_empty() || a == "/" || a == "*" || a.starts_with('*') {
            continue;
        }
        let (lhs, default) = match split_top(a, '=').as_slice() {
            [l, d] => (l.clone(), Some(d.trim().to_string())),
            _ => (a.to_string(), None),
        };
        let (name, annotation) = match lhs.split_once(':') {
            Some((n, t)) => (n.trim().to_string(), Some(t.trim().to_string())),
            None => (lhs.trim().to_string(), None),
        };
        args.push(RawArg { name, annotation, default });
    }
    // The docstring: the first statement, if a string
    let body = rest[colon + 1..].trim_start();
    let doc = ["\"\"\"", "'''"].iter().find_map(|q| {
        let inner = body.strip_prefix(q)?;
        let end = inner.find(q)?;
        inner[..end].trim().lines().next().map(|l| l.trim().to_string())
    });
    Some(RawSignature { function, args, returns, doc, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lucem() -> Options {
        Options::new("lucem", &["image", "mask", "labels", "table"], "image").alias("lumen")
    }

    fn dspk() -> Options {
        Options::new("dspk", &["signal", "events", "table"], "signal")
    }

    const LUCEM_OP: &str = r#"
import numpy as np
import lucem


@lucem.op(family="Filter", halo=12)
@lucem.param("strength", min=0.0, max=5.0, hint="How much")
def local_contrast_boost(
    image: np.ndarray,
    sigma: float = 3.0,
    strength: float = 1.8,
    mode: Literal["reflect", "wrap"] = "reflect",
    mask: lucem.Mask | None = None,
    tile=None,
) -> np.ndarray:
    """Enhance fine cellular structures.

    More text.
    """
    return image
"#;

    #[test]
    fn a_decorated_lucem_function_becomes_an_operation() {
        let raw = read(LUCEM_OP, Language::Python, &lucem()).expect("an operation");
        let (i, problems) = Interface::from_raw(&raw, &lucem());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(i.title, "Local contrast boost");
        assert_eq!(i.family, "Filter");
        assert_eq!(i.summary, "Enhance fine cellular structures.");
        assert_eq!(i.function, "local_contrast_boost");
        assert_eq!(i.inputs, vec![PortSpec::new("image", "image"), PortSpec::new("mask", "mask").optional()]);
        assert_eq!(i.outputs, vec![PortSpec::new("image", "image")]);
        let names: Vec<&str> = i.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["sigma", "strength", "mode"]);
        assert_eq!(i.params[1].kind, ParamKind::Number { min: Some(0.0), max: Some(5.0), unit: None });
        assert_eq!(i.params[1].hint, "How much");
        assert_eq!(i.params[2].kind, ParamKind::Choice { options: vec!["reflect".into(), "wrap".into()] });
        assert_eq!(i.context, Context::around(12, 3));
        assert!(i.takes_piece);
    }

    #[test]
    fn a_dspk_function_with_several_outputs_and_a_time_context() {
        let text = r#"
import dspk

@dspk.op(name="Detect spikes", before=30, after=60)
@dspk.output("events", "events")
@dspk.output("noise", "table")
def detect(signal, threshold: float = 5.0, channels: int = 384) -> dict:
    return {}
"#;
        let raw = read(text, Language::Python, &dspk()).unwrap();
        let (i, problems) = Interface::from_raw(&raw, &dspk());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(i.title, "Detect spikes");
        assert_eq!(i.inputs, vec![PortSpec::new("signal", "signal")], "the default type");
        assert_eq!(i.outputs.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["events", "noise"]);
        assert_eq!(i.params[1], ParamSpec::integer("channels", 384));
        assert_eq!(i.context, Context::time(30, 60));
    }

    #[test]
    fn mistakes_are_reported() {
        let text = r#"
@kitchen.op
def f(x, k: float, labels_in: np.ndarray, gain: float = None) -> dict:
    pass
"#;
        let raw = read(text, Language::Python, &lucem()).unwrap();
        let (i, problems) = Interface::from_raw(&raw, &lucem());
        let all = problems.join(" | ");
        assert!(all.contains("k: a parameter needs a default"), "{all}");
        assert!(all.contains("gain: a plain value cannot default to None"), "{all}");
        assert!(all.contains("@output"), "{all}");
        assert_eq!(i.inputs[1], PortSpec::new("labels_in", "labels"), "the type from the name");
    }

    #[test]
    fn header_lines_for_rust_and_plain_python() {
        let rust = r#"
// kitchen: name = Band power
// kitchen: before = 300
// kitchen: input signal: signal
// kitchen: input events: events optional
// kitchen: param low = 300.0 (1..15000) Hz
// kitchen: param window = hann (hann|boxcar)
// kitchen: output power: table
pub fn run(inputs: &Inputs, params: &Params) -> Result<Array, String> { todo!() }
"#;
        let raw = read(rust, Language::Rust, &dspk()).unwrap();
        let (i, problems) = Interface::from_raw(&raw, &dspk());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(i.title, "Band power");
        assert_eq!(i.function, "run");
        assert!(i.inputs[1].optional);
        assert_eq!(i.params[0].kind, ParamKind::Number { min: Some(1.0), max: Some(15000.0), unit: Some("Hz".into()) });
        assert_eq!(i.params[1].kind, ParamKind::Choice { options: vec!["hann".into(), "boxcar".into()] });
        assert_eq!(i.outputs, vec![PortSpec::new("power", "table")]);
        assert_eq!(i.context, Context::time(300, 0));

        // The app's former name, and kitchen-editor's node line, with `def run`
        let py = "# lumen: halo = 4\n# kitchen: node = in image:image, mask:mask; out labels:labels\ndef run(image, mask, sigma: float = 2.0):\n    return image\n";
        let raw = read(py, Language::Python, &lucem()).unwrap();
        let (i, problems) = Interface::from_raw(&raw, &lucem());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(i.inputs.len(), 2);
        assert_eq!(i.outputs, vec![PortSpec::new("labels", "labels")]);
        assert_eq!(i.params, vec![ParamSpec::number("sigma", 2.0)]);
        assert_eq!(i.context, Context::around(4, 3));

        assert!(read("import numpy\nprint(1)\n", Language::Python, &lucem()).is_none(), "declares nothing");
    }

    #[test]
    fn unknown_types_and_changes() {
        let raw = read("# kitchen: input x: signal\n# kitchen: output image\n", Language::Python, &lucem()).unwrap();
        let (a, problems) = Interface::from_raw(&raw, &lucem());
        assert!(problems.iter().any(|p| p.contains("signal is not one of image, mask, labels, table")), "{problems:?}");
        let mut b = a.clone();
        b.params.push(ParamSpec::number("gain", 1.0));
        b.inputs.clear();
        assert_eq!(b.changes_from(&a), ["input x removed", "new parameter gain"]);
    }

    #[test]
    fn literals_and_splits() {
        assert_eq!(lit("3"), Some(Lit::Int(3)));
        assert_eq!(lit("1_000"), Some(Lit::Int(1000)));
        assert_eq!(lit("-2.5e-3"), Some(Lit::Float(-0.0025)));
        assert_eq!(lit("'a,b'"), Some(Lit::Str("a,b".into())));
        assert_eq!(lit("[\"x\", 'y']"), Some(Lit::List(vec![Lit::Str("x".into()), Lit::Str("y".into())])));
        assert_eq!(lit("np.pi"), None);
        assert_eq!(split_top(r#"a: Literal["x", "y"] = "x", b=(1, 2)"#, ','), [r#"a: Literal["x", "y"] = "x""#, "b=(1, 2)"]);
        assert_eq!(pretty("local_contrast_boost"), "Local contrast boost");
    }
}
