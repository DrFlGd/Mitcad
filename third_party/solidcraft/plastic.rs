// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 SolidCraft contributors
//
// Adapted from SolidCraft (https://github.com/tangency-labs/solidcraft, commit
// 0b787f8eae4244cd40f07dddccd2e1b27e4cf9f8): the plastic rule library of
// `crates/doc/src/plastic.rs` (`PlasticRule::new`, `PlasticRule::library`)
// and the order of a rule's fields (`PlasticRule::fields`). Changes: the
// rules are the design file's JSON form of a rule (what a design's own
// rules are), not SolidCraft's struct. See README.md in this folder.

use serde_json::{Value, json};

/// A rule's expressions by field, in SolidCraft's order (`Thickness` in
/// them is the rule's own thickness), and whether each is an angle.
pub const FIELDS: [(&str, bool); 10] = [
    ("thickness", false),
    ("nominal_radius", false),
    ("clearance", false),
    ("knife_edge", false),
    ("reveal_height", false),
    ("thickness_variation", false),
    ("draft", true),
    ("max_thickness", false),
    ("min_thickness", false),
    ("min_draft", true),
];

fn rule(name: &str, material: &str, inch: bool, thickness: &str, max: &str, min: &str) -> Value {
    let (clearance, knife, reveal) = if inch {
        ("0.004 in", "0.04 in", "0.02 in")
    } else {
        ("0.1 mm", "1 mm", "0.5 mm")
    };
    json!({
        "name": name,
        "material": material,
        "thickness": thickness,
        "nominal_radius": "0.5 * Thickness",
        "clearance": clearance,
        "knife_edge": knife,
        "reveal_height": reveal,
        "thickness_variation": "0.15 * Thickness",
        "draft": "2 deg",
        "max_thickness": max,
        "min_thickness": min,
        "min_draft": "0.5 deg",
    })
}

/// SolidCraft's rule library (the first is its default), as design file
/// rules.
pub fn library() -> Vec<Value> {
    vec![
        rule(
            "ABS (1.5mm)",
            "ABS Plastic",
            false,
            "1.5 mm",
            "3.5 mm",
            "1.2 mm",
        ),
        rule(
            "Nylon (PA6) (2.5mm)",
            "Nylon 6",
            false,
            "2.5 mm",
            "3 mm",
            "0.8 mm",
        ),
        rule(
            "Polypropylene (PP) (3 mm)",
            "Polypropylene",
            false,
            "3 mm",
            "3.8 mm",
            "0.8 mm",
        ),
        rule(
            "ABS (0.1 in)",
            "ABS Plastic",
            true,
            "0.1 in",
            "3.5 mm",
            "1.2 mm",
        ),
        rule(
            "Nylon (PA6) (0.1 in)",
            "Nylon 6",
            true,
            "0.1 in",
            "3 mm",
            "0.8 mm",
        ),
        rule(
            "Polypropylene (PP) (0.1 in)",
            "Polypropylene",
            true,
            "0.1 in",
            "3.8 mm",
            "0.8 mm",
        ),
    ]
}
