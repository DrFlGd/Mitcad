// SPDX-License-Identifier: MIT
//! What the imports share of their sketch definitions: the references of
//! driven dimensions, and the inactive constraints and dimensions of a
//! sketch that does not solve to the file's geometry ([`inactive`]).

use serde_json::Value;

// Inactive constraints and dimensions (mitcad#721).
pub mod inactive;

/// Whether a sketch definition names references of driven dimensions
/// (mitcad#250).
pub fn has_references(def: &Value) -> bool {
    def["dimensions"]
        .as_array()
        .is_some_and(|list| list.iter().any(|d| d.get("reference").is_some()))
}

/// A sketch definition without its driven dimensions' references: the
/// model gives them parameters of their own. An import falls back on it
/// when the model refuses a file's parameter as a reference (one that a
/// feature before the sketch uses).
pub fn without_references(def: &Value) -> Value {
    let mut def = def.clone();
    if let Some(list) = def["dimensions"].as_array_mut() {
        for d in list {
            if let Some(map) = d.as_object_mut() {
                map.remove("reference");
            }
        }
    }
    def
}
