// SPDX-License-Identifier: MIT
//! Options of inserted components in commands and queries (mitcad#801;
//! `commands.md`, "Options of inserted components"): the answer of
//! `set_component_parameters`, the `configure` query of a component, and
//! what `library_parts` and `parts_list` say of a component with options.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::ApiError;
use super::prelude::*;
use mitcad_model::features::ValueInput;
use mitcad_model::ids::ComponentUid;
use mitcad_model::kernel::Kernel;
use mitcad_model::part_options::{self, BrokenRule, OptionMove};
use mitcad_model::{Document, OptionsChanged};

/// What a component with options says of them: `part` (its derived part),
/// `source`, `config` (the row it starts from), `parameters` (its values as
/// written, by option), `row` (its combination's row, or null),
/// `designation`, `error` (the rule a bound value breaks, or why its
/// source cannot be made; null) and `options` (as the `configure` query
/// gives them, its bound ones driven). None for another component.
pub(crate) fn options_json<K: Kernel>(doc: &Document<K>, component: ComponentUid) -> Option<Value> {
    let options = doc.component_options(component)?;
    let choosers = options.configured.as_ref().map_or_else(
        || json!([]),
        |c| part_options::options_json(&c.state, &options.parameters),
    );
    Some(json!({
        "part": options.part,
        "source": options.source,
        "config": options.config,
        "parameters": options.parameters,
        "row": options.combination.row,
        "designation": options.combination.designation,
        "error": options.error,
        "options": choosers,
    }))
}

/// The answer of `set_component_parameters`: the component, its part, the
/// `moves` of its constants, `refused` (the rule a bound value breaks, or
/// null) and its options as [`options_json`] gives them.
pub(crate) fn changed_json<K: Kernel>(doc: &Document<K>, changed: &OptionsChanged) -> Value {
    let mut value = options_json(doc, changed.component).unwrap_or_else(|| json!({}));
    value["component"] = json!(changed.component);
    value["name"] = json!(doc.assembly().name(changed.component));
    value["part"] = json!(changed.part);
    value["moves"] = json!(
        changed
            .moves
            .iter()
            .map(OptionMove::json)
            .collect::<Vec<_>>()
    );
    value["refused"] = changed
        .broken
        .as_ref()
        .map_or(Value::Null, BrokenRule::json);
    value
}

/// The `configure` query with `component`: the choosers' view of that
/// component's options, `parameters` tried instead of its own.
pub(crate) fn configure_json<K: Kernel>(
    doc: &Document<K>,
    component: &str,
    parameters: &BTreeMap<String, ValueInput>,
) -> Result<Value, ApiError> {
    let uid = doc.component_ref(component)?;
    let mut value = doc.component_configure_json(uid, parameters)?;
    value["component"] = json!(uid);
    Ok(value)
}
