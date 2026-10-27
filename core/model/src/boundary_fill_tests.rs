// SPDX-License-Identifier: MIT
//! Boundary Fill (mitcad#457) in documents, with the mock kernel: its
//! cells are worked out from the tools' bounding boxes
//! (`MockKernel::boundary_cells`).

use serde_json::{Value, json};

use crate::features::FeatureDef;
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::BoundingBox;
use crate::test_support::def;
use crate::testing::{MockKernel, MockShape};
use crate::topo::FaceName;
use crate::{Document, FeatureStatus};

fn add(doc: &mut Document<MockKernel>, value: Value) -> FeatureUid {
    doc.add_feature(&def(value), None).unwrap().uid
}

fn shape(doc: &Document<MockKernel>, body: &str) -> MockShape {
    let uid: BodyUid = body.parse().unwrap();
    doc.body_shape(uid)
        .unwrap_or_else(|| panic!("no body {body}"))
        .clone()
}

fn bounds(doc: &Document<MockKernel>, body: &str) -> BoundingBox {
    shape(doc, body).bounds.expect("bounds")
}

fn uids(doc: &Document<MockKernel>) -> Vec<String> {
    doc.bodies().iter().map(|b| b.uid.to_string()).collect()
}

fn status(doc: &Document<MockKernel>, uid: FeatureUid) -> FeatureStatus {
    doc.status(uid).cloned().expect("recomputed")
}

fn face(name: &str) -> FaceName {
    name.parse().unwrap()
}

/// A cube of `size` with its lowest corner at `corner`, a new body.
fn cube(doc: &mut Document<MockKernel>, corner: [f64; 3], size: f64) -> FeatureUid {
    add(
        doc,
        json!({"type": "box",
               "plane": {"type": "fixed", "origin": corner, "normal": [0, 0, 1], "x_axis": [1, 0, 0]},
               "length": size, "width": size, "height": size, "operation": "new_body"}),
    )
}

/// Two overlapping 20 mm cubes: A (F1) at the origin and B (F2) at
/// (10, 10, 10); they share the cube from 10 to 20.
fn two_cubes() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    cube(&mut doc, [0.0, 0.0, 0.0], 20.0);
    cube(&mut doc, [10.0, 10.0, 10.0], 20.0);
    doc
}

fn fill(cells: Value, operation: &str, extra: Value) -> Value {
    let mut value = json!({"type": "boundary_fill", "tools": ["F1.b0", "F2.b0"],
        "cells": cells, "operation": operation});
    for (key, item) in extra.as_object().unwrap() {
        value[key] = item.clone();
    }
    value
}

#[test]
fn cells_become_new_bodies_with_the_tools_face_names() {
    let mut doc = two_cubes();
    let added = doc
        .add_feature(
            &def(fill(
                json!([[15, 15, 15], [5, 5, 5]]),
                "new_body",
                json!({}),
            )),
            None,
        )
        .unwrap();
    assert_eq!(added.name, "BoundaryFill1");
    assert!(added.parameters.is_empty());
    let uid = added.uid;
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    // One body per cell, in the order of the points; the tools stay.
    assert_eq!(uids(&doc), ["F1.b0", "F2.b0", "F3.b0", "F3.b1"]);
    assert_eq!(doc.kernel().count("boundary_cells"), 1);
    // The common cell: inside both tools, bounded by faces of both.
    let common = shape(&doc, "F3.b0");
    assert!(
        common.history.starts_with("cell(box("),
        "{}",
        common.history
    );
    assert!(common.history.ends_with("|)"), "{}", common.history);
    assert_eq!(
        bounds(&doc, "F3.b0"),
        BoundingBox {
            min: [10.0; 3],
            max: [20.0; 3]
        }
    );
    assert!(common.faces.contains(&face("F1:top")));
    assert!(common.faces.contains(&face("F2:bottom")));
    // The cell inside A only: B bounds it too.
    let outer = shape(&doc, "F3.b1");
    assert!(!outer.history.ends_with("|)"), "{}", outer.history);
    assert!(outer.faces.contains(&face("F1:side0")));
    assert!(outer.faces.contains(&face("F2:side0")));
    // The tools are not changed.
    assert!(shape(&doc, "F1.b0").history.starts_with("box("));

    // Removing the tools leaves the cells.
    doc.edit_feature(
        uid,
        &def(fill(
            json!([[15, 15, 15], [5, 5, 5]]),
            "new_body",
            json!({"remove_tools": true}),
        )),
    )
    .unwrap();
    assert_eq!(uids(&doc), ["F3.b0", "F3.b1"]);
}

#[test]
fn two_points_in_one_cell_fill_it_once() {
    let mut doc = two_cubes();
    let uid = add(
        &mut doc,
        fill(json!([[15, 15, 15], [12, 18, 11]]), "new_body", json!({})),
    );
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(uids(&doc), ["F1.b0", "F2.b0", "F3.b0"]);
    assert!(
        doc.warnings(uid)
            .iter()
            .any(|w| w.contains("points 1 and 2 pick the same cell")),
        "{:?}",
        doc.warnings(uid)
    );
}

#[test]
fn cells_join_cut_or_intersect_the_participants() {
    let mut doc = two_cubes();
    // C (F3), a plate the cells reach into.
    cube(&mut doc, [12.0, 12.0, -5.0], 6.0);
    let uid = add(
        &mut doc,
        fill(
            json!([[15, 15, 15]]),
            "cut",
            json!({"participants": ["F3.b0"], "remove_tools": true}),
        ),
    );
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    // The tools are gone before the cut; C is cut by the cell.
    assert_eq!(uids(&doc), ["F3.b0"]);
    let cut = shape(&doc, "F3.b0").history;
    assert!(cut.starts_with("cut(box("), "{cut}");
    assert!(cut.contains(",cell(box("), "{cut}");

    // Several cells join as one tool.
    let unions = doc.kernel().count("unite");
    doc.edit_feature(
        uid,
        &def(fill(
            json!([[15, 15, 15], [5, 5, 5]]),
            "join",
            json!({"participants": ["F3.b0"]}),
        )),
    )
    .unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(uids(&doc), ["F1.b0", "F2.b0", "F3.b0"]);
    assert!(doc.kernel().count("unite") > unions);
    let joined = shape(&doc, "F3.b0").history;
    assert!(joined.starts_with("join(box("), "{joined}");
    assert!(joined.contains("unite(cell("), "{joined}");

    // An intersection that meets no participant fails.
    *doc.kernel().touch.borrow_mut() = Some(vec![false]);
    doc.edit_feature(
        uid,
        &def(fill(
            json!([[15, 15, 15]]),
            "intersect",
            json!({"participants": ["F3.b0"]}),
        )),
    )
    .unwrap();
    assert!(matches!(status(&doc, uid), FeatureStatus::Failed(m)
            if m.contains("the filled cells do not intersect any participant body")));
    *doc.kernel().touch.borrow_mut() = None;

    // Without participants a cut works on all bodies, the kept tools too.
    doc.edit_feature(uid, &def(fill(json!([[15, 15, 15]]), "cut", json!({}))))
        .unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(uids(&doc), ["F1.b0", "F2.b0", "F3.b0"]);
    for body in ["F1.b0", "F2.b0", "F3.b0"] {
        let history = shape(&doc, body).history;
        assert!(history.starts_with("cut(box("), "{body}: {history}");
    }
}

#[test]
fn points_outside_the_tools_or_on_their_faces_fail() {
    let mut doc = two_cubes();
    let uid = add(&mut doc, fill(json!([[50, 50, 50]]), "new_body", json!({})));
    assert!(matches!(status(&doc, uid), FeatureStatus::Failed(m)
            if m.contains("is inside no tool body")));
    assert_eq!(uids(&doc), ["F1.b0", "F2.b0"]);
    doc.edit_feature(uid, &def(fill(json!([[20, 5, 5]]), "new_body", json!({}))))
        .unwrap();
    assert!(matches!(status(&doc, uid), FeatureStatus::Failed(m)
            if m.contains("lies on the boundary of tool 1")));
}

#[test]
fn definitions_are_checked() {
    let mut doc = two_cubes();
    for (value, message) in [
        (
            json!({"type": "boundary_fill", "tools": [], "cells": [[5, 5, 5]],
                   "operation": "new_body"}),
            "no tool bodies",
        ),
        (
            json!({"type": "boundary_fill", "tools": ["F1.b0", "F1.b0"],
                   "cells": [[5, 5, 5]], "operation": "new_body"}),
            "listed more than once",
        ),
        (
            json!({"type": "boundary_fill", "tools": ["F1.b0"], "cells": [],
                   "operation": "new_body"}),
            "no cells picked",
        ),
        (
            fill(
                json!([[5, 5, 5]]),
                "new_body",
                json!({"participants": ["F1.b0"]}),
            ),
            "a new body has no participant bodies",
        ),
        (
            fill(
                json!([[5, 5, 5]]),
                "cut",
                json!({"participants": ["F2.b0"], "remove_tools": true}),
            ),
            "is a tool the fill removes",
        ),
        (
            json!({"type": "boundary_fill", "tools": ["F9.b0"], "cells": [[5, 5, 5]],
                   "operation": "new_body"}),
            "does not exist",
        ),
    ] {
        let error = doc.add_feature(&def(value), None).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn the_definition_keeps_its_fields() {
    let value = fill(
        json!([[15.0, 15.0, 15.0], [5.0, 5.0, 5.0]]),
        "cut",
        json!({"participants": ["F3.b0"], "remove_tools": true}),
    );
    let parsed: FeatureDef<String> = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(parsed.type_name(), "boundary_fill");
    assert_eq!(parsed.base_name(), "BoundaryFill");
    assert_eq!(serde_json::to_value(&parsed).unwrap(), value);
    // Defaults are left out.
    let plain = fill(json!([[15.0, 15.0, 15.0]]), "new_body", json!({}));
    let parsed: FeatureDef<String> = serde_json::from_value(plain.clone()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), plain);
    let unknown = fill(json!([[1, 2, 3]]), "new_body", json!({"keep_tools": true}));
    let error = serde_json::from_value::<FeatureDef<String>>(unknown).unwrap_err();
    assert!(error.to_string().contains("unknown field"), "{error}");
}
