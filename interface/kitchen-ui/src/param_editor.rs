//! The parameters of one op, editable: numbers as inputs (Enter or leaving the field applies
//! them), a short list of numbers as one input each, choices as menus, switches as checkboxes.
//! It works on any op that serializes to a JSON object ([`Params`]): lumen's ops, dsp-kitchen's
//! pipeline nodes. Whoever owns the op decides how a change is applied (a [`Commit`]: a project
//! or session command), so a side panel and a pipeline inspector can share it.

use std::rc::Rc;

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, v_flex};
use gpui_kit::{App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::theme::{self, color};
use crate::widgets::{MenuSelect, row};

/// An op the editor can edit: it serializes to a JSON object whose numbers, short number lists,
/// choices and switches are its parameters, and deserializing checks new values. The hooks say
/// what JSON alone cannot (which text fields are choices, labels of list elements); each has a
/// default, so `impl Params for MyOp {}` is enough for an op with numbers only.
pub trait Params: Serialize + DeserializeOwned + Clone + PartialEq + 'static {
    /// The options of a text field shown as a menu (`band`: low pass, high pass…); `None` for a
    /// field that is not a choice (it is not shown).
    fn choices(_field: &str) -> Option<&'static [&'static str]> {
        None
    }

    /// The label of element `i` of a list of `len` numbers (`cutoff`, 2, 0 → `Low (Hz)`).
    fn element_label(field: &str, _len: usize, i: usize) -> String {
        format!("{} {}", label_of(field), i + 1)
    }

    /// Adjusts the other values when a choice changes (a band change keeps the cutoffs it can).
    fn on_choice(_value: &mut Value, _field: &str, _choice: &str) {}
}

/// Sets the new op; an error is shown under the fields.
pub type Commit<T> = Rc<dyn Fn(T, &mut App) -> Result<(), String>>;

/// Longest list of numbers edited as separate inputs; longer lists are only counted.
const MAX_LIST: usize = 2;

/// One editable number: its place in the op's JSON (`cutoff`, index 1).
struct Field {
    key: String,
    index: Option<usize>,
    label: String,
    input: Entity<InputState>,
}

/// The editor of one op's parameters (a view: `cx.new(|_| ParamEditor::<MyOp>::new())`).
pub struct ParamEditor<T: Params> {
    fields: Vec<Field>,
    /// What the fields were built for (key and op JSON).
    fields_for: String,
    field_subs: Vec<Subscription>,
    error: Option<String>,
    target: Option<(T, Commit<T>)>,
}

impl<T: Params> Default for ParamEditor<T> {
    fn default() -> Self {
        Self { fields: Vec::new(), fields_for: String::new(), field_subs: Vec::new(), error: None, target: None }
    }
}

impl<T: Params> ParamEditor<T> {
    /// An editor with nothing to edit yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Edits `op`, known as `key` (its node id, `v1/iir`); inputs are rebuilt when the key or the
    /// values change, so calling it on every render is cheap.
    pub fn set_target(&mut self, key: &str, op: &T, commit: Commit<T>, window: &mut Window, cx: &mut Context<Self>) {
        let value = serde_json::to_value(op).unwrap_or(Value::Null);
        self.target = Some((op.clone(), commit));
        let fresh = format!("{key}/{value}");
        if fresh == self.fields_for {
            return;
        }
        if !self.fields_for.starts_with(&format!("{key}/")) {
            self.error = None;
        }
        self.fields_for = fresh;
        self.fields.clear();
        self.field_subs.clear();
        let Value::Object(map) = &value else { return };
        for (k, v) in map {
            match v {
                Value::Number(n) => self.add_field(k, None, label_of(k), n.as_f64().unwrap_or(0.0), window, cx),
                Value::Array(a) if a.len() <= MAX_LIST && !a.is_empty() && a.iter().all(Value::is_number) => {
                    for (i, n) in a.iter().enumerate() {
                        self.add_field(k, Some(i), T::element_label(k, a.len(), i), n.as_f64().unwrap_or(0.0), window, cx);
                    }
                }
                _ => {}
            }
        }
    }

    /// The op being edited, if any.
    pub fn target(&self) -> Option<&T> {
        self.target.as_ref().map(|(op, _)| op)
    }

    /// The last error (a value that does not parse, or one the op or its owner rejected).
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn add_field(&mut self, key: &str, index: Option<usize>, label: String, v: f64, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(trim_number(v)));
        let sub = cx.subscribe(&input, |this, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                this.apply_fields(cx);
            }
        });
        self.field_subs.push(sub);
        self.fields.push(Field { key: key.to_string(), index, label, input });
    }

    /// The numbers typed in, on the op; a value that does not parse, or parameters the op
    /// rejects, leave it unchanged and say why.
    fn apply_fields(&mut self, cx: &mut Context<Self>) {
        let Some((op, _)) = &self.target else { return };
        let mut value = serde_json::to_value(op).unwrap_or(Value::Null);
        for f in &self.fields {
            let text = f.input.read(cx).value().to_string();
            let Ok(n) = text.trim().parse::<f64>() else {
                self.error = Some(format!("{}: {text:?} is not a number", f.label));
                cx.notify();
                return;
            };
            let slot = match f.index {
                Some(i) => value.get_mut(&f.key).and_then(|a| a.get_mut(i)),
                None => value.get_mut(&f.key),
            };
            if let Some(slot) = slot {
                *slot = number_like(slot, n);
            }
        }
        self.set_value(value, cx);
    }

    fn set_value(&mut self, value: Value, cx: &mut Context<Self>) {
        let Some((op, commit)) = self.target.clone() else { return };
        match serde_json::from_value::<T>(value) {
            Ok(new) if new == op => self.error = None,
            Ok(new) => self.error = commit(new, cx).err(),
            Err(e) => self.error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn set_choice(&mut self, key: &str, choice: &str, cx: &mut Context<Self>) {
        let Some((op, _)) = &self.target else { return };
        let mut value = serde_json::to_value(op).unwrap_or(Value::Null);
        value[key] = json!(choice);
        T::on_choice(&mut value, key, choice);
        self.set_value(value, cx);
    }

    fn set_bool(&mut self, key: &str, on: bool, cx: &mut Context<Self>) {
        let Some((op, _)) = &self.target else { return };
        let mut value = serde_json::to_value(op).unwrap_or(Value::Null);
        value[key] = json!(on);
        self.set_value(value, cx);
    }
}

impl<T: Params> Render for ParamEditor<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut out = v_flex().w_full().gap(gpui_kit::px(theme::SM));
        let Some((op, _)) = &self.target else { return out };
        let value = serde_json::to_value(op).unwrap_or(Value::Null);
        for f in &self.fields {
            out = out.child(row(f.label.clone(), div().w_full().child(Input::new(&f.input).small())));
        }
        let mut shown = !self.fields.is_empty();
        if let Value::Object(map) = &value {
            for (k, v) in map {
                if let (Some(opts), Some(cur)) = (T::choices(k), v.as_str()) {
                    let this = cx.entity().downgrade();
                    let key = k.clone();
                    let names: Vec<SharedString> = opts.iter().map(|o| SharedString::from(o.replace('_', " "))).collect();
                    let sel = opts.iter().position(|o| *o == cur);
                    out = out.child(row(
                        label_of(k),
                        MenuSelect::new(SharedString::from(format!("param-{k}")), names, sel, move |i, _, cx| {
                            let _ = this.update(cx, |p, cx| p.set_choice(&key, opts[i], cx));
                        }),
                    ));
                    shown = true;
                } else if let Some(b) = v.as_bool() {
                    let this = cx.entity().downgrade();
                    let key = k.clone();
                    out = out.child(row(
                        label_of(k),
                        Checkbox::new(SharedString::from(format!("param-{k}"))).checked(b).on_click(move |on: &bool, _, cx| {
                            let on = *on;
                            let _ = this.update(cx, |p, cx| p.set_bool(&key, on, cx));
                        }),
                    ));
                    shown = true;
                } else if let Some(a) = v.as_array().filter(|a| a.len() > MAX_LIST) {
                    out = out.child(row(label_of(k), div().text_xs().text_color(color::text_muted()).child(format!("{} values (edit in the file)", a.len()))));
                    shown = true;
                }
            }
        }
        if !shown {
            out = out.child(div().text_xs().text_color(color::text_muted()).child("No parameters."));
        }
        out.children(self.error.clone().map(|e| div().text_xs().text_color(color::danger()).child(e)))
    }
}

/// `n` in the kind of number `slot` held: integers stay integers (radii, orders, counts); a
/// negative number for an unsigned field is left for deserializing to reject, with its reason.
fn number_like(slot: &Value, n: f64) -> Value {
    if slot.is_u64() || slot.is_i64() { json!(n.round() as i64) } else { json!(n) }
}

/// `clip_limit` → `Clip limit`
pub fn label_of(key: &str) -> String {
    let mut s = key.replace('_', " ");
    if let Some(c) = s.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    s
}

/// `300` rather than `300.0`, at most 6 decimals.
pub fn trim_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_numbers() {
        assert_eq!(label_of("clip_limit"), "Clip limit");
        assert_eq!(label_of("sigma_samples"), "Sigma samples");
        assert_eq!(trim_number(300.0), "300");
        assert_eq!(trim_number(0.25), "0.25");
        assert_eq!(trim_number(-1.5), "-1.5");
    }

    #[test]
    fn integers_stay_integers() {
        assert_eq!(number_like(&json!(3), 4.6), json!(5));
        assert_eq!(number_like(&json!(-3), -4.4), json!(-4));
        assert_eq!(number_like(&json!(0.5), 2.0), json!(2.0));
        // Negative for an unsigned field: deserializing says why
        let v = number_like(&json!(3), -2.0);
        assert!(serde_json::from_value::<u32>(v).is_err());
    }
}
