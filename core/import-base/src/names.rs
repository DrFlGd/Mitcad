// SPDX-License-Identifier: MIT
//! Names of the files' parameters that Mitcad cannot use as they are.

use std::collections::BTreeSet;

use mitcad_model::expr::{is_reserved_name, is_valid_name};

/// A free Mitcad name for a name of the file Mitcad cannot use.
pub fn free_name(name: &str, taken: &BTreeSet<String>) -> String {
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = if base.starts_with(|c: char| c.is_alphabetic() || c == '_') {
        base
    } else {
        format!("p_{base}")
    };
    if is_valid_name(&base) && !taken.contains(&base) {
        return base;
    }
    (1u32..)
        .map(|n| format!("{base}_{n}"))
        .find(|c| !taken.contains(c) && !is_reserved_name(c) && is_valid_name(c))
        .expect("a free name exists")
}
