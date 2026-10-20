// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 SolidCraft contributors
//
// Adapted from SolidCraft (https://github.com/tangency-labs/solidcraft, commit
// 0b787f8eae4244cd40f07dddccd2e1b27e4cf9f8): the JSON upgrade steps of
// `crates/doc/src/format.rs` (`v2_to_v3`, `v3_to_v4`, `v4_to_v5`, `v5_to_v6`,
// `v13_to_v14` and their helpers), the pending change
// `crates/doc/src/format/pending/two-sided-extrude-direction.rs` and the
// feature-input naming of `crates/doc/src/params.rs`
// (`Document::name_all_feature_inputs`). Changes: the steps work on
// `serde_json::Value` and return `Result<(), String>`; the input count per
// feature comes from `crate::inputs` (Mitcad's port of the feature walk);
// the upgrade path is written out for the format this import reads (see
// `upgrade`). See README.md in this folder.

use serde_json::Value;

/// The format this import reads: format 27 and the changes pending after
/// it (which designs saved between releases carry as format 28 with
/// `format_pending`).
pub const PINNED: u32 = 28;

/// The last format whose upgrade step is a hand-written one.
const HAND_WRITTEN: u32 = 22;

/// The changes pending after format 27 (`format_pending`), in name order,
/// each with its upgrade step when it has one.
pub const PENDING: [(&str, Option<JsonStep>); 9] = [
    ("component-metadata", None),
    ("joint-expressions", None),
    ("joint-origin-edge", None),
    ("joint-origin-frame-x", None),
    ("joint-relations", None),
    ("sketch-point-expressions", None),
    ("surface-trim-regions-curves-mutual", None),
    (
        "two-sided-extrude-direction",
        Some(two_sided_extrude_direction),
    ),
    ("vertex-edge-names", None),
];

/// The JSON steps up to [`HAND_WRITTEN`]: entry `i` turns format `i + 1`
/// into `i + 2`. Formats 23 to 27 have no steps that rewrite a design.
type JsonStep = fn(&mut Value) -> Result<(), String>;
const JSON_STEPS: [JsonStep; HAND_WRITTEN as usize - 1] = [
    |_| Ok(()),
    v2_to_v3,
    v3_to_v4,
    v4_to_v5,
    v5_to_v6,
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    v13_to_v14,
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
    |_| Ok(()),
];

/// The steps a design of format `version` (with the pending changes
/// `applied` it already has, when written while they were pending) takes
/// to [`PINNED`], by name: for the import's report.
pub fn upgrade(
    v: &mut Value,
    version: u32,
    applied: Option<&[String]>,
) -> Result<Vec<String>, String> {
    if version == 0 || version > PINNED {
        return Err(format!(
            "file format {version}: this import reads SolidCraft designs up to format {PINNED}"
        ));
    }
    let mut ran = Vec::new();
    for from in version..PINNED.min(HAND_WRITTEN) {
        let step = JSON_STEPS[from as usize - 1];
        step(v)?;
        ran.push(format!("{from} to {}", from + 1));
    }
    if version == PINNED {
        // A design written while the changes were pending: the steps of
        // those it does not have; a change this import does not know is a
        // newer build's.
        let applied = applied.unwrap_or(&[]);
        if let Some(unknown) = applied
            .iter()
            .find(|n| !PENDING.iter().any(|(p, _)| p == n))
        {
            return Err(format!(
                "file format {version} with `{unknown}`, a change newer than this import knows"
            ));
        }
        for (name, step) in PENDING {
            if !applied.iter().any(|a| a == name)
                && let Some(step) = step
            {
                step(v)?;
                ran.push(name.to_owned());
            }
        }
    } else {
        for (name, step) in PENDING {
            if let Some(step) = step {
                step(v)?;
                ran.push(name.to_owned());
            }
        }
    }
    Ok(ran)
}

/// The mark of a face or edge name saved before format 14.
pub const LEGACY_MARK: char = '?';

/// Stored face and edge names were worked out from geometry before format
/// 14; they get the legacy mark (`?`). Every feature also keeps naming the
/// pieces it splits a body into in the kernel's shell order.
fn v13_to_v14(v: &mut Value) -> Result<(), String> {
    if let Some(Value::Array(features)) = v.get_mut("features") {
        for f in features.iter_mut() {
            let Value::Object(o) = f else { continue };
            let kind = o.get("type").and_then(Value::as_str).map(str::to_string);
            if matches!(kind.as_deref(), Some("fillet" | "chamfer")) {
                mark_all(o.get_mut("edge_names"));
            }
            mark_all(o.get_mut("face_names"));
            if kind.as_deref() == Some("sketch")
                && let Some(Value::Array(links)) =
                    o.get_mut("sketch").and_then(|s| s.get_mut("links"))
            {
                links
                    .iter_mut()
                    .filter_map(|l| l.get_mut("face_name"))
                    .for_each(mark);
            }
            o.insert("shell_piece_order".into(), Value::Bool(true));
            references(f);
        }
    }
    if let Some(Value::Array(joints)) = v.get_mut("assembly").and_then(|a| a.get_mut("joints")) {
        for j in joints.iter_mut() {
            for side in ["a", "b"] {
                snap(j.get_mut(side));
            }
        }
    }
    if let Some(Value::Array(origins)) = v.get_mut("assembly").and_then(|a| a.get_mut("origins")) {
        origins.iter_mut().for_each(|o| snap(o.get_mut("origin")));
    }
    if let Some(Value::Array(faces)) = v.get_mut("appearances").and_then(|a| a.get_mut("faces")) {
        for face in faces
            .iter_mut()
            .filter(|f| f.get("strict_name").and_then(Value::as_bool) != Some(true))
        {
            if let Some(n) = face.get_mut("name") {
                mark(n);
            }
        }
    }
    Ok(())
}

/// Mark the `name` of every face or edge reference in `v`.
fn references(v: &mut Value) {
    match v {
        Value::Object(o) => {
            let reference = matches!(o.get("type").and_then(Value::as_str), Some("face" | "edge"))
                && o.contains_key("at");
            for (key, x) in o.iter_mut() {
                if reference && key == "name" {
                    mark(x);
                } else {
                    references(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(references),
        _ => {}
    }
}

/// Mark the face name of a joint origin's snap on a face.
fn snap(origin: Option<&mut Value>) {
    if let Some(snap) = origin.and_then(|o| o.get_mut("snap"))
        && matches!(
            snap.get("type").and_then(Value::as_str),
            Some("face_center" | "circle_center")
        )
        && let Some(n) = snap.get_mut("face")
    {
        mark(n);
    }
}

fn mark_all(names: Option<&mut Value>) {
    if let Some(Value::Array(names)) = names {
        names.iter_mut().for_each(mark);
    }
}

/// A stored name with the legacy mark (an empty name stays empty).
fn mark(n: &mut Value) {
    if let Value::String(s) = n
        && !s.is_empty()
        && !s.starts_with(LEGACY_MARK)
    {
        s.insert(0, LEGACY_MARK);
    }
}

/// Old XZ references keep their exact frame: they become explicit planes
/// with the axes they had (x = +X, y = +Z).
fn v5_to_v6(v: &mut Value) -> Result<(), String> {
    fn preserve(v: &mut Value) {
        match v {
            Value::Object(o) => {
                let old_xz = o.get("type").and_then(Value::as_str) == Some("origin")
                    && o.get("name").and_then(Value::as_str).is_some_and(|n| {
                        n.eq_ignore_ascii_case("XZ") || n.eq_ignore_ascii_case("FRONT")
                    });
                if old_xz {
                    *v = serde_json::json!({"type": "custom", "plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 0, 1]}});
                } else {
                    o.values_mut().for_each(preserve);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(preserve),
            _ => {}
        }
    }
    preserve(v);
    Ok(())
}

/// Features in designs with components keep reaching every body.
fn v4_to_v5(v: &mut Value) -> Result<(), String> {
    let has_components = v
        .get("components")
        .and_then(Value::as_array)
        .is_some_and(|c| !c.is_empty());
    if !has_components {
        return Ok(());
    }
    if let Some(Value::Array(fs)) = v.get_mut("features") {
        for f in fs.iter_mut() {
            if let Some(o) = f.as_object_mut()
                && o.get("type").and_then(Value::as_str) != Some("sketch")
                && o.get("participants").is_none()
            {
                o.insert("participants".into(), serde_json::json!(["*"]));
            }
        }
    }
    Ok(())
}

/// Joints placed with the old convention (aligned unless flipped) keep
/// their placement.
fn v3_to_v4(v: &mut Value) -> Result<(), String> {
    if let Some(Value::Array(js)) = v.get_mut("assembly").and_then(|a| a.get_mut("joints")) {
        for j in js.iter_mut() {
            if let Some(o) = j.as_object_mut() {
                let f = o.get("flip").and_then(Value::as_bool).unwrap_or(false);
                o.insert("flip".into(), Value::Bool(!f));
            }
        }
    }
    Ok(())
}

/// Body colours `{name: [r, g, b]}` become named, opaque body appearances.
fn v2_to_v3(v: &mut Value) -> Result<(), String> {
    let Some(Value::Object(old)) = v.get("appearances") else {
        return Ok(());
    };
    if !old.values().all(Value::is_array) {
        return Ok(());
    }
    let bodies: serde_json::Map<String, Value> = old
        .iter()
        .map(|(k, c)| {
            (
                k.clone(),
                serde_json::json!({"name": "Custom", "color": c, "opacity": 1.0}),
            )
        })
        .collect();
    if let Some(o) = v.as_object_mut() {
        o.insert(
            "appearances".into(),
            serde_json::json!({ "bodies": bodies }),
        );
    }
    Ok(())
}

/// A two-sided extrude follows its `direction`; older builds ignored a
/// `negative` direction beside a second distance, so such extrudes become
/// `positive`, which keeps their shape.
fn two_sided_extrude_direction(v: &mut Value) -> Result<(), String> {
    let Some(Value::Array(features)) = v.get_mut("features") else {
        return Ok(());
    };
    for feature in features {
        if !matches!(
            feature.get("type").and_then(Value::as_str),
            Some("extrude" | "surface_extrude")
        ) {
            continue;
        }
        if let Some(extent) = feature.get_mut("extent").and_then(Value::as_object_mut)
            && extent.get("distance2").is_some_and(|d| !d.is_null())
            && extent.get("direction").and_then(Value::as_str) == Some("negative")
        {
            extent.insert("direction".into(), Value::from("positive"));
        }
    }
    Ok(())
}

/// Gives every feature input without a parameter name the next free `d<n>`
/// (`Document::name_all_feature_inputs`): names of designs written before
/// feature inputs had names, as SolidCraft fills them in when it opens
/// them. `count` gives a feature's number of inputs.
pub fn name_all_feature_inputs(v: &mut Value, count: &dyn Fn(&Value) -> usize) {
    use std::collections::BTreeMap;
    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    let params = v
        .get("params")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for p in &params {
        if let Some(n) = p.get("name").and_then(Value::as_str) {
            *taken.entry(n.to_owned()).or_default() += 1;
        }
    }
    let Some(Value::Array(features)) = v.get_mut("features") else {
        return;
    };
    for f in features.iter() {
        for n in f
            .get("param_names")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(n) = n.as_str() {
                *taken.entry(n.to_owned()).or_default() += 1;
            }
        }
    }
    let mut next = 1usize;
    for f in features.iter_mut() {
        let n = count(f);
        let mut names: Vec<String> = f
            .get("param_names")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|x| x.as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for k in 0..n {
            if names.get(k).is_some_and(|name| !name.is_empty()) {
                continue;
            }
            let name = loop {
                let name = format!("d{next}");
                next += 1;
                if !taken.contains_key(&name) {
                    break name;
                }
            };
            if names.len() <= k {
                names.resize(k + 1, String::new());
            }
            names[k] = name.clone();
            taken.insert(name, 1);
        }
        for name in names.drain(n.min(names.len())..) {
            if let Some(count) = taken.get_mut(&name) {
                *count -= 1;
                if *count == 0 {
                    taken.remove(&name);
                    if let Some(i) = name.strip_prefix('d').and_then(|s| s.parse::<usize>().ok())
                        && name == format!("d{i}")
                        && i > 0
                    {
                        next = next.min(i);
                    }
                }
            }
        }
        if let Some(o) = f.as_object_mut() {
            if names.is_empty() {
                o.remove("param_names");
            } else {
                o.insert("param_names".into(), Value::from(names));
            }
        }
    }
}
