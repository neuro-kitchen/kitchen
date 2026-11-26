//! What flows between operations, what an operation is tuned with, and how much it looks around.
//!
//! - **Port types** are the app's: lucem's `image`, `mask`, `labels`, `table`; dsp-kitchen's
//!   `signal`, `events`, `table`. kitchen only compares them, by name: an output feeds an input of
//!   the same type, or of `any`.
//! - **Parameters** are numbers, whole numbers, switches, text or one of a list, with a default
//!   and, for numbers, a range and a unit. The same description draws the Inspector's controls,
//!   checks a graph's values and fills in what a graph leaves out.
//! - **Context** is how far an operation looks beyond the piece it is given, per axis: a blur of
//!   σ 4 px needs about 12 px around a tile; a filter needs some milliseconds before a window.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A port type's name (`image`, `signal`). `any` accepts every type.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PortType(pub String);

impl PortType {
    /// The type every input of type `any` accepts, and every type is.
    pub const ANY: &'static str = "any";

    /// The type named `name` (lowercased).
    pub fn new(name: impl AsRef<str>) -> Self {
        PortType(name.as_ref().trim().to_lowercase())
    }

    /// Its name.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether an output of type `self` may feed an input of type `input`.
    pub fn feeds(&self, input: &PortType) -> bool {
        self == input || input.0 == Self::ANY || self.0 == Self::ANY
    }
}

impl std::fmt::Display for PortType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An input or an output of an operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    /// Its name, unique within the operation's inputs, or its outputs (`image`, `mask`).
    pub name: String,
    /// What flows through it.
    #[serde(rename = "type")]
    pub ty: PortType,
    /// An input that may stay unconnected.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

impl PortSpec {
    /// A required port `name` of type `ty`.
    pub fn new(name: impl Into<String>, ty: impl AsRef<str>) -> Self {
        Self { name: name.into(), ty: PortType::new(ty), optional: false }
    }

    /// The same port, optional.
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// What kind of value a parameter takes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParamKind {
    /// A number, within `min`..=`max` when given, in `unit` (`px`, `Hz`, `ms`).
    Number {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
    /// A whole number, within `min`..=`max` when given.
    Integer {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    /// On or off.
    Switch,
    /// Free text (a model's name, a column).
    Text,
    /// One of these.
    Choice {
        /// The choices.
        options: Vec<String>,
    },
}

/// A parameter's value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    /// On or off.
    Switch(bool),
    /// A whole number.
    Integer(i64),
    /// A number.
    Number(f64),
    /// Text, or a choice.
    Text(String),
}

impl ParamValue {
    /// As a number (whole numbers too).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ParamValue::Number(v) => Some(*v),
            ParamValue::Integer(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// As JSON, for a script.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// From JSON, if it is a value a parameter takes.
    pub fn from_json(v: &Value) -> Option<ParamValue> {
        serde_json::from_value(v.clone()).ok()
    }
}

/// A parameter of an operation: the Inspector draws a control for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    /// Its name, as the function takes it (`sigma`).
    pub name: String,
    /// What it takes.
    #[serde(flatten)]
    pub kind: ParamKind,
    /// Its value when a graph gives none.
    pub default: ParamValue,
    /// One line for people (the Inspector's tooltip).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
}

impl ParamSpec {
    /// A number `name`, `default` when not given.
    pub fn number(name: impl Into<String>, default: f64) -> Self {
        Self { name: name.into(), kind: ParamKind::Number { min: None, max: None, unit: None }, default: ParamValue::Number(default), hint: String::new() }
    }

    /// A whole number `name`.
    pub fn integer(name: impl Into<String>, default: i64) -> Self {
        Self { name: name.into(), kind: ParamKind::Integer { min: None, max: None }, default: ParamValue::Integer(default), hint: String::new() }
    }

    /// A switch `name`.
    pub fn switch(name: impl Into<String>, default: bool) -> Self {
        Self { name: name.into(), kind: ParamKind::Switch, default: ParamValue::Switch(default), hint: String::new() }
    }

    /// Text `name`.
    pub fn text(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self { name: name.into(), kind: ParamKind::Text, default: ParamValue::Text(default.into()), hint: String::new() }
    }

    /// One of `options`, the first by default.
    pub fn choice(name: impl Into<String>, options: &[&str]) -> Self {
        let options: Vec<String> = options.iter().map(|s| s.to_string()).collect();
        let default = ParamValue::Text(options.first().cloned().unwrap_or_default());
        Self { name: name.into(), kind: ParamKind::Choice { options }, default, hint: String::new() }
    }

    /// The same parameter with a range (numbers and whole numbers).
    pub fn range(mut self, lo: f64, hi: f64) -> Self {
        match &mut self.kind {
            ParamKind::Number { min, max, .. } => {
                *min = Some(lo);
                *max = Some(hi);
            }
            ParamKind::Integer { min, max } => {
                *min = Some(lo as i64);
                *max = Some(hi as i64);
            }
            _ => {}
        }
        self
    }

    /// The same number with a unit.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        if let ParamKind::Number { unit, .. } = &mut self.kind {
            *unit = Some(u.into());
        }
        self
    }

    /// The same parameter with a line for people.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// `value` checked against this parameter, as the value to use (a whole number given for a
    /// number becomes a number), or why it does not fit.
    pub fn check(&self, value: &ParamValue) -> Result<ParamValue, String> {
        let name = &self.name;
        match (&self.kind, value) {
            (ParamKind::Number { min, max, .. }, v) => {
                let x = v.as_f64().ok_or_else(|| format!("{name} is a number"))?;
                if !x.is_finite() {
                    return Err(format!("{name} is a finite number"));
                }
                if let Some(lo) = min.filter(|lo| x < *lo) {
                    return Err(format!("{name} is at least {lo} (got {x})"));
                }
                if let Some(hi) = max.filter(|hi| x > *hi) {
                    return Err(format!("{name} is at most {hi} (got {x})"));
                }
                Ok(ParamValue::Number(x))
            }
            (ParamKind::Integer { min, max }, v) => {
                let x = match v {
                    ParamValue::Integer(i) => *i,
                    ParamValue::Number(f) if f.fract() == 0.0 && f.is_finite() => *f as i64,
                    _ => return Err(format!("{name} is a whole number")),
                };
                if let Some(lo) = min.filter(|lo| x < *lo) {
                    return Err(format!("{name} is at least {lo} (got {x})"));
                }
                if let Some(hi) = max.filter(|hi| x > *hi) {
                    return Err(format!("{name} is at most {hi} (got {x})"));
                }
                Ok(ParamValue::Integer(x))
            }
            (ParamKind::Switch, ParamValue::Switch(b)) => Ok(ParamValue::Switch(*b)),
            (ParamKind::Switch, _) => Err(format!("{name} is on or off (true or false)")),
            (ParamKind::Text, ParamValue::Text(t)) => Ok(ParamValue::Text(t.clone())),
            (ParamKind::Text, _) => Err(format!("{name} is text")),
            (ParamKind::Choice { options }, ParamValue::Text(t)) if options.contains(t) => Ok(ParamValue::Text(t.clone())),
            (ParamKind::Choice { options }, _) => Err(format!("{name} is one of {}", options.join(", "))),
        }
    }
}

/// Checks `given` against `specs`: every value fits, no name is unknown, and what is missing takes
/// its default. Returns the full set, or every problem.
pub fn check_params(specs: &[ParamSpec], given: &BTreeMap<String, ParamValue>) -> Result<BTreeMap<String, ParamValue>, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut problems = Vec::new();
    for spec in specs {
        match given.get(&spec.name) {
            Some(v) => match spec.check(v) {
                Ok(v) => {
                    out.insert(spec.name.clone(), v);
                }
                Err(e) => problems.push(e),
            },
            None => {
                out.insert(spec.name.clone(), spec.default.clone());
            }
        }
    }
    for name in given.keys().filter(|n| !specs.iter().any(|s| &s.name == *n)) {
        problems.push(format!("no parameter {name}"));
    }
    if problems.is_empty() { Ok(out) } else { Err(problems) }
}

/// How far an operation looks beyond its piece, per axis of the app's walk (lucem: z, y, x in
/// pixels; dsp-kitchen: samples before and after). Axes the app does not walk stay 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    /// Before the piece, per axis.
    pub before: [usize; 3],
    /// After the piece, per axis.
    pub after: [usize; 3],
}

impl Context {
    /// None: the operation looks only at its piece.
    pub const NONE: Context = Context { before: [0; 3], after: [0; 3] };

    /// `n` on both sides of every axis the app walks (`axes` of them, from the last: one axis
    /// is time, two are y and x).
    pub fn around(n: usize, axes: usize) -> Self {
        let mut c = Context::NONE;
        for a in (3 - axes.min(3))..3 {
            c.before[a] = n;
            c.after[a] = n;
        }
        c
    }

    /// `before` and `after` on the last axis (time).
    pub fn time(before: usize, after: usize) -> Self {
        Context { before: [0, 0, before], after: [0, 0, after] }
    }

    /// The context of two operations in a row: what each needs adds up.
    pub fn then(self, next: Context) -> Context {
        let mut c = self;
        for a in 0..3 {
            c.before[a] += next.before[a];
            c.after[a] += next.after[a];
        }
        c
    }

    /// The larger of two contexts, per side and axis (two branches meeting).
    pub fn max(self, other: Context) -> Context {
        let mut c = self;
        for a in 0..3 {
            c.before[a] = c.before[a].max(other.before[a]);
            c.after[a] = c.after[a].max(other.after[a]);
        }
        c
    }

    /// Whether it needs nothing around the piece.
    pub fn is_none(&self) -> bool {
        *self == Context::NONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_feed_their_own_and_any() {
        let (image, mask, any) = (PortType::new("Image"), PortType::new("mask"), PortType::new("any"));
        assert_eq!(image.as_str(), "image");
        assert!(image.feeds(&image));
        assert!(!image.feeds(&mask));
        assert!(image.feeds(&any));
        assert!(any.feeds(&mask));
    }

    #[test]
    fn params_are_checked_and_filled() {
        let specs = vec![
            ParamSpec::number("sigma", 2.0).range(0.2, 20.0).unit("px"),
            ParamSpec::integer("bins", 256).range(2.0, 4096.0),
            ParamSpec::switch("invert", false),
            ParamSpec::choice("band", &["highpass", "lowpass"]),
        ];
        let given: BTreeMap<String, ParamValue> = [("sigma".to_string(), ParamValue::Integer(3)), ("band".to_string(), ParamValue::Text("lowpass".into()))].into();
        let full = check_params(&specs, &given).unwrap();
        assert_eq!(full["sigma"], ParamValue::Number(3.0), "a whole number given for a number");
        assert_eq!(full["bins"], ParamValue::Integer(256), "the default");
        assert_eq!(full["band"], ParamValue::Text("lowpass".into()));
        let bad: BTreeMap<String, ParamValue> = [
            ("sigma".to_string(), ParamValue::Number(50.0)),
            ("bins".to_string(), ParamValue::Number(2.5)),
            ("band".to_string(), ParamValue::Text("notch".into())),
            ("gamma".to_string(), ParamValue::Number(1.0)),
        ]
        .into();
        let problems = check_params(&specs, &bad).unwrap_err();
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("at most 20")));
        assert!(problems.iter().any(|p| p.contains("no parameter gamma")));
    }

    #[test]
    fn values_read_from_json() {
        let v: Vec<ParamValue> = serde_json::from_str(r#"[true, 3, 2.5, "x"]"#).unwrap();
        assert_eq!(v, [ParamValue::Switch(true), ParamValue::Integer(3), ParamValue::Number(2.5), ParamValue::Text("x".into())]);
        let spec: ParamSpec = serde_json::from_str(r#"{"name":"sigma","kind":"number","min":0.2,"default":2.0}"#).unwrap();
        assert_eq!(spec.kind, ParamKind::Number { min: Some(0.2), max: None, unit: None });
    }

    #[test]
    fn contexts_add_along_a_chain_and_take_the_larger_at_a_join() {
        let blur = Context::around(12, 2);
        assert_eq!(blur.before, [0, 12, 12]);
        let chain = blur.then(Context::around(3, 2));
        assert_eq!(chain.after, [0, 15, 15]);
        let filter = Context::time(300, 0);
        assert_eq!(filter.max(Context::time(10, 40)), Context::time(300, 40));
        assert!(Context::NONE.is_none());
    }
}
