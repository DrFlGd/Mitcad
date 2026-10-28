// SPDX-License-Identifier: MIT
//! Configurations in commands and queries (mitcad#797, mitcad#798,
//! mitcad#799): the `configure` command and query, `check_rules`, and the
//! JSON of the cascade's moves and refusals that `set_parameter`,
//! `apply_configuration` and `import_parameters_csv` add to their answers.
//! See `commands.md`, "Configurations: options, rows and rules".

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::ApiError;
use mitcad_model::Document;
use mitcad_model::configurations::{
    ConfigurationMove, Domain, DomainValue, OptionReport, OptionValue, RuleViolation, RulesReport,
};
use mitcad_model::expr::{DEFAULT_DECIMALS, format_value};
use mitcad_model::kernel::Kernel;

/// The `configure` query.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigureQuery {
    /// Values to try ({option: value}); none: the current ones.
    #[serde(default)]
    pub values: Map<String, Value>,
    /// An inserted component's options instead of the design's
    /// (mitcad#801).
    #[serde(default)]
    pub component: Option<String>,
    /// With `component`: its values to try ({option: expression of the
    /// design's parameters, or a number}).
    #[serde(default)]
    pub parameters: BTreeMap<String, mitcad_model::features::ValueInput>,
}

/// The `check_rules` query.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckRulesQuery {
    /// The most combinations gone through; sampled above it.
    #[serde(default)]
    pub limit: Option<u64>,
}

/// Check Rules' default limit of combinations.
pub(crate) const CHECK_LIMIT: u64 = 100_000;

/// The moves of the cascade as the answers list them.
pub(crate) fn moves_json(moves: &[ConfigurationMove]) -> Value {
    Value::Array(
        moves
            .iter()
            .map(|m| {
                json!({
                    "option": m.option,
                    "from": m.from,
                    "to": m.to,
                    "from_expression": m.from_expression,
                    "to_expression": m.to_expression,
                    "rule": m.rule,
                    "text": m.text(),
                })
            })
            .collect(),
    )
}

/// A refusal: `rule`, `message` and `text`, or null.
pub(crate) fn refused_json(refused: Option<&RuleViolation>) -> Value {
    match refused {
        None => Value::Null,
        Some(v) => json!({"rule": v.rule, "message": v.message, "text": v.text()}),
    }
}

fn value_json(value: &OptionValue) -> Value {
    match value {
        OptionValue::Number(v) => json!(v),
        OptionValue::Text(text) | OptionValue::Expression(text) => json!(text),
    }
}

fn domain_value_json(value: &DomainValue) -> Value {
    json!({
        "value": value_json(&value.value),
        "text": value.text,
        "expression": value.expression,
    })
}

/// A domain or the allowed values: `{"kind": "list", "values"}`,
/// `{"kind": "range", "min", "max", "step"}` with their `_text`, or
/// `{"kind": "free"}`.
fn domain_json(domain: &Domain, report: &OptionReport) -> Value {
    match domain {
        Domain::Free => json!({"kind": "free"}),
        Domain::List(values) => json!({
            "kind": "list",
            "values": values.iter().map(domain_value_json).collect::<Vec<_>>(),
        }),
        Domain::Range { min, max, step } => {
            let text = |v: &Option<f64>| v.map(|v| format_value(v, report.unit, DEFAULT_DECIMALS));
            json!({
                "kind": "range",
                "min": min, "max": max, "step": step,
                "min_text": text(min), "max_text": text(max), "step_text": text(step),
            })
        }
    }
}

fn option_json(report: &OptionReport) -> Value {
    json!({
        "name": report.name,
        "kind": if report.unit.is_text() { "text" } else { "number" },
        "unit": report.unit.to_string(),
        "value": value_json(&report.value),
        "text": report.text,
        "expression": report.expression,
        "driven": report.driven,
        "domain": domain_json(&report.domain, report),
        "allowed": domain_json(&report.allowed, report),
        "restricted_by": report.restricted_by,
    })
}

fn rules_report_json(report: &RulesReport) -> Value {
    json!({
        "combinations": report.combinations,
        "checked": report.checked,
        "sampled": report.sampled,
        "never_allowed": report.never_allowed.iter()
            .map(|(option, text)| json!({"option": option, "text": text}))
            .collect::<Vec<_>>(),
        "never_applies": report.never_applies,
        "empty": report.empty.iter()
            .map(|e| json!({"option": e.option, "rules": e.rules, "values": e.values}))
            .collect::<Vec<_>>(),
        "rows": report.rows.iter()
            .map(|(row, v)| json!({"row": row, "rule": v.rule, "message": v.message}))
            .collect::<Vec<_>>(),
        "unrepairable": report.unrepairable.iter()
            .map(|(values, v)| json!({"values": values, "rule": v.rule, "message": v.message}))
            .collect::<Vec<_>>(),
        "problems": report.problems,
        "lines": report.lines,
    })
}

/// The document's methods of this module, as an extension trait
/// (mitcad#777).
pub(crate) trait ConfigurationsOps<K: Kernel> {
    /// The values of a `configure` command or query: a text option's
    /// text, a number option's number (mm or rad) or constant expression.
    fn option_values(
        &self,
        values: &Map<String, Value>,
    ) -> Result<BTreeMap<String, OptionValue>, ApiError>;

    /// The `configure` command.
    fn configure_command(&mut self, values: &Map<String, Value>) -> Result<Value, ApiError>;

    /// The `configure` query.
    fn configure_json(&self, query: &ConfigureQuery) -> Result<Value, ApiError>;

    /// The `check_rules` query.
    fn check_rules_json(&self, query: &CheckRulesQuery) -> Value;
}

impl<K: Kernel> ConfigurationsOps<K> for Document<K> {
    fn option_values(
        &self,
        values: &Map<String, Value>,
    ) -> Result<BTreeMap<String, OptionValue>, ApiError> {
        let params = self.parameters();
        let mut out = BTreeMap::new();
        for (name, value) in values {
            let text = params
                .find(name)
                .and_then(|id| params.get(id))
                .is_some_and(|p| p.is_text());
            let value = match value {
                Value::Number(number) => OptionValue::Number(
                    number
                        .as_f64()
                        .ok_or_else(|| ApiError(format!("{name}: not a number")))?,
                ),
                Value::String(s) if text => OptionValue::Text(s.clone()),
                Value::String(s) => OptionValue::Expression(s.clone()),
                _ => {
                    return Err(ApiError(format!(
                        "{name}: a value is a text, a number or an expression"
                    )));
                }
            };
            out.insert(name.clone(), value);
        }
        Ok(out)
    }

    fn configure_command(&mut self, values: &Map<String, Value>) -> Result<Value, ApiError> {
        let values = self.option_values(values)?;
        if values.is_empty() {
            return Err(ApiError("configure: no values".to_owned()));
        }
        let outcome = self.configure(&values)?;
        Ok(json!({
            "changed": outcome.changed,
            "moves": moves_json(&outcome.moves),
            "row": outcome.row,
            "refused": refused_json(outcome.refused.as_ref()),
        }))
    }

    fn configure_json(&self, query: &ConfigureQuery) -> Result<Value, ApiError> {
        if let Some(component) = &query.component {
            if !query.values.is_empty() {
                return Err(ApiError(
                    "configure: a component's values to try are its parameters".to_owned(),
                ));
            }
            return super::component_options::configure_json(self, component, &query.parameters);
        }
        let values = self.option_values(&query.values)?;
        let report = self.configure_report(&values)?;
        Ok(json!({
            "options": report.options.iter().map(option_json).collect::<Vec<_>>(),
            "row": report.row,
            "designation_values": report.designation_values,
            "moves": moves_json(&report.moves),
            "refused": refused_json(report.refused.as_ref()),
            "problems": report.problems,
        }))
    }

    fn check_rules_json(&self, query: &CheckRulesQuery) -> Value {
        rules_report_json(&self.check_rules(query.limit.unwrap_or(CHECK_LIMIT).max(1)))
    }
}
