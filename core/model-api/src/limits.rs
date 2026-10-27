// SPDX-License-Identifier: MIT
//! Parameter limits in commands and queries (mitcad#665): the `min`,
//! `max`, `step` and `strict` fields of `add_parameter` and
//! `set_parameter`, the limit fields of the `parameters` query and the
//! features' own (intrinsic) limits of the slots a parameter gives its
//! value to. See `commands.md`, "Parameter limits".

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value, json};

use super::ApiError;
use mitcad_model::Document;
use mitcad_model::expr::{
    Bounds, DEFAULT_DECIMALS, Limited, ParamId, ParamLimits, Parameter, Unit, format_number,
    format_number_exact, format_value,
};
use mitcad_model::kernel::Kernel;
use mitcad_model::parameters::{Parameters, slot_limits};

/// Keeps a field given as `null` (Some(Null): remove the limit) apart from
/// a field left out (None: keep it).
pub(super) fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// The limit fields of a command, as given.
pub(super) struct LimitInput {
    pub min: Option<Value>,
    pub max: Option<Value>,
    pub step: Option<Value>,
    pub strict: Option<bool>,
}

impl LimitInput {
    /// Whether the command gives any limit field.
    pub fn is_given(&self) -> bool {
        self.min.is_some() || self.max.is_some() || self.step.is_some() || self.strict.is_some()
    }

    /// `base` with the given fields replaced: an expression text, a number
    /// (in the parameter's unit), or `null` or `""` to remove the limit.
    pub fn apply_to(&self, mut base: ParamLimits) -> Result<ParamLimits, ApiError> {
        for (which, given) in [
            (Limited::Min, &self.min),
            (Limited::Max, &self.max),
            (Limited::Step, &self.step),
        ] {
            if let Some(given) = given {
                let text = match given {
                    Value::Null => None,
                    Value::String(text) if text.trim().is_empty() => None,
                    Value::String(text) => Some(text.clone()),
                    Value::Number(number) => {
                        let number =
                            number.as_f64().filter(|n| n.is_finite()).ok_or_else(|| {
                                ApiError(format!("\"{which}\" is not a finite number"))
                            })?;
                        Some(format_number_exact(number))
                    }
                    _ => {
                        return Err(ApiError(format!(
                            "\"{which}\" is an expression, a number or null"
                        )));
                    }
                };
                match which {
                    Limited::Min => base.min = text,
                    Limited::Max => base.max = text,
                    Limited::Step => base.step = text,
                }
            }
        }
        if let Some(strict) = self.strict {
            base.strict = strict;
        }
        Ok(base)
    }
}

/// A bound as the `parameters` query gives it: canonical, as text in
/// `unit` and as a number in `unit`.
fn bound_fields(out: &mut Map<String, Value>, which: Limited, value: Option<f64>, unit: Unit) {
    let key = which.as_str();
    out.insert(format!("{key}_value"), json!(value));
    out.insert(
        format!("{key}_text"),
        json!(value.map(|v| format_value(v, unit, DEFAULT_DECIMALS))),
    );
    out.insert(
        format!("{key}_number"),
        json!(value.map(|v| unit.canonical_to_unit(v))),
    );
}

/// Bounds as a JSON object of `min`/`max`/`step` with their `_value`,
/// `_text` and `_number` fields.
fn bounds_json(bounds: &Bounds, unit: Unit, out: &mut Map<String, Value>) {
    for which in [Limited::Min, Limited::Max, Limited::Step] {
        bound_fields(out, which, bounds.get(which), unit);
    }
}

/// Adds a parameter's limit fields to its `parameters` query entry, when it
/// has limits or features limit the slots it gives its value to:
/// `limits` (its own: `min`, `max`, `step` as written, `strict`, and each
/// bound evaluated), `unclamped` (the expression's result before them, mm
/// or rad), `unclamped_text`, `limited` (`min`, `max`, `step` or null),
/// `limit_message` (what the limit did, in words, or null),
/// `limit_references` (the parameters the limits use) and `intrinsic` (the
/// features' fixed limits).
pub(super) fn add_fields(value: &mut Value, p: &Parameter, intrinsic: Option<&Vec<Value>>) {
    let limits = p.limits();
    if !limits.is_none() {
        let mut out = Map::new();
        out.insert("min".to_owned(), json!(limits.min));
        out.insert("max".to_owned(), json!(limits.max));
        out.insert("step".to_owned(), json!(limits.step));
        out.insert("strict".to_owned(), json!(limits.strict));
        bounds_json(&p.bounds(), p.unit(), &mut out);
        value["limits"] = Value::Object(out);
        value["unclamped"] = json!(p.unclamped());
        value["unclamped_text"] = json!(format_value(p.unclamped(), p.unit(), DEFAULT_DECIMALS));
        value["limited"] = json!(p.limited().map(Limited::as_str));
        value["limit_message"] = json!(message(p));
        value["limit_references"] = json!(p.limit_references());
    }
    if let Some(intrinsic) = intrinsic {
        value["intrinsic"] = Value::Array(intrinsic.clone());
    }
}

/// What a limit did to a parameter, in words: `Width is limited to its
/// maximum 300 mm (its expression gives 372 mm)`; None when no limit took
/// effect.
pub(super) fn message(p: &Parameter) -> Option<String> {
    let limited = p.limited()?;
    let text = |v: f64| format_value(v, p.unit(), DEFAULT_DECIMALS);
    let what = match limited {
        Limited::Min | Limited::Max => format!(
            "{} is limited to its {} {}",
            p.name(),
            limited.describe(),
            text(p.value())
        ),
        Limited::Step => format!(
            "{} is rounded to its step of {}: {}",
            p.name(),
            text(p.bounds().step.unwrap_or_default()),
            text(p.value())
        ),
    };
    Some(format!(
        "{what} (its expression gives {})",
        text(p.unclamped())
    ))
}

/// The parameters a limit changed, for the `document` query's
/// `limited_parameters`: `name`, `limited`, `text` (the value),
/// `unclamped_text` and `message`.
pub(super) fn limited_parameters(params: &Parameters) -> Vec<Value> {
    params
        .iter()
        .filter_map(|p| {
            let message = message(p)?;
            Some(json!({
                "name": p.name(),
                "limited": p.limited().map(Limited::as_str),
                "text": p.formatted_value(DEFAULT_DECIMALS),
                "unclamped_text": format_value(p.unclamped(), p.unit(), DEFAULT_DECIMALS),
                "message": message,
            }))
        })
        .collect()
}

/// A short description of a bound pair for messages: `1 to 10000`, `at
/// least 5`, `at most 3 mm`.
fn describe(bounds: &Bounds, unit: Unit) -> String {
    let text = |v: f64| format_value(v, unit, DEFAULT_DECIMALS);
    let mut out = match (bounds.min, bounds.max) {
        (Some(min), Some(max)) => format!("{} to {}", text(min), text(max)),
        (Some(min), None) => format!("at least {}", text(min)),
        (None, Some(max)) => format!("at most {}", text(max)),
        (None, None) => String::new(),
    };
    if let Some(step) = bounds.step {
        if !out.is_empty() {
            out.push_str(", ");
        }
        if step == 1.0 && unit.is_unitless() {
            out.push_str("whole");
        } else {
            out.push_str(&format!(
                "in steps of {}",
                format_number(step, DEFAULT_DECIMALS)
            ));
        }
    }
    out
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait LimitsOps<K: Kernel> {
    /// The features' own limits of the slots each parameter gives its
    /// value to (mitcad#665), as `parameters` query entries: `feature`,
    /// `feature_name`, `slot`, `fixed` (always true: read-only),
    /// `describe` and the bounds with their `_value`, `_text` and `_number`
    /// fields in the parameter's unit.
    fn intrinsic_limits(&self) -> HashMap<ParamId, Vec<Value>>;
}

impl<K: Kernel> LimitsOps<K> for Document<K> {
    fn intrinsic_limits(&self) -> HashMap<ParamId, Vec<Value>> {
        let params = self.parameters();
        let mut out: HashMap<ParamId, Vec<Value>> = HashMap::new();
        for entry in self.features() {
            let feature_type = entry.def.type_name();
            let _ = entry.def.map_params(&mut |slot, id: &ParamId| {
                if let (Some(bounds), Some(p)) = (slot_limits(feature_type, slot), params.get(*id))
                {
                    let mut item = Map::new();
                    item.insert("feature".to_owned(), json!(entry.uid));
                    item.insert("feature_name".to_owned(), json!(entry.name));
                    item.insert("slot".to_owned(), json!(slot));
                    item.insert("fixed".to_owned(), json!(true));
                    item.insert("describe".to_owned(), json!(describe(&bounds, p.unit())));
                    bounds_json(&bounds, p.unit(), &mut item);
                    out.entry(*id).or_default().push(Value::Object(item));
                }
                Ok::<_, ()>(())
            });
        }
        out
    }
}
