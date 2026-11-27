//! What an op is tuned with ([standard §2.1](../../../standard/spec/02-ops.md), `params`).
//!
//! Parameters are numbers, whole numbers, switches, text or one of a list, with a default and, for
//! numbers, a range and a unit. The same description draws the Inspector's controls, checks a
//! pipeline's values and fills in what a pipeline leaves out.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What kind of value a parameter takes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
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

/// A parameter of an op: the Inspector draws a control for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParamSpec {
    /// Its name, as the function takes it (`sigma`).
    pub name: String,
    /// What it takes.
    #[serde(flatten)]
    pub kind: ParamKind,
    /// Its value when a pipeline gives none.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
