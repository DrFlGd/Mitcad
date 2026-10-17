// SPDX-License-Identifier: MIT
//! Bound lengths with the mock kernel: a profile's length set by its ray
//! to another profile (face, centre, through, with an overlap), following
//! the target when it moves and the part when it slides along its slot,
//! keeping its last value when the ray meets nothing, detached into a
//! plain length, and the refusals of a cycle and of a target placed later.

use serde_json::json;

use super::test_library::*;
use crate::Document;
use crate::features::FeatureDef;
use crate::testing::MockKernel;

/// T at the origin and X 500 along +X, both 1000 long along +Z, and P on
/// T's x+ slot 100 up, its length bound to X's x- slot frame.
fn bound_frame() -> (
    Document<MockKernel>,
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
) {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let x = first(&mut doc, "p30", [500.0, 0.0, 0.0], 1000.0);
    let tp = derived(&doc, component(&t));
    let xp = derived(&doc, component(&x));
    let p = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_x+_1)")},
                      "offset": 100},
               "length": {"bind": {"occurrence": x["occurrence"],
                                   "geometry": {"mount": format!("{xp}:mount(slot_x-_1)")}}}}),
    );
    (doc, t, x, p)
}

fn length(doc: &Document<MockKernel>, part: &serde_json::Value) -> f64 {
    doc.part_info(component(part)).unwrap().length.unwrap().1
}

fn bound_of(doc: &Document<MockKernel>, part: &serde_json::Value) -> crate::ids::FeatureUid {
    doc.part_info(component(part)).unwrap().record.bound[0]
}

#[test]
fn a_bound_length_runs_to_the_target_s_face() {
    let (doc, _, _, p) = bound_frame();
    all_ok(&doc);
    assert!(
        (length(&doc, &p) - 470.0).abs() < 1e-9,
        "{}",
        length(&doc, &p)
    );
    // The order: the bound length, the derived part, the joint.
    let bound = bound_of(&doc, &p);
    let part = derived(&doc, component(&p));
    let joint = doc.part_info(component(&p)).unwrap().record.joint.unwrap();
    let position = |uid| doc.state().position(uid).unwrap();
    assert!(position(bound) < position(part) && position(part) < position(joint));
    // Its parameter is a reference: read only.
    let FeatureDef::BoundLength(b) = &doc.feature(bound).unwrap().def else {
        panic!("a bound length");
    };
    assert!(doc.parameters().is_reference(b.parameter));
    assert_eq!(b.from.shift.len(), 1);
    // The part's body is that long.
    let bounds = doc.part_bounds(component(&p)).unwrap();
    assert!(
        (bounds.max[2] - bounds.min[2] - 470.0).abs() < 1e-9,
        "{bounds:?}"
    );
    assert_eq!(p["bound"].as_array().unwrap().len(), 1);
}

#[test]
fn the_length_follows_the_target_and_the_slide() {
    let (mut doc, _, x, p) = bound_frame();
    command(
        &mut doc,
        json!({"cmd": "ground_occurrence", "occurrence": x["occurrence"], "grounded": false}),
    )
    .unwrap();
    command(
        &mut doc,
        json!({"cmd": "set_occurrence_transform", "occurrence": x["occurrence"],
               "transform": {"translation": [600, 0, 0]}}),
    )
    .unwrap();
    all_ok(&doc);
    assert!(
        (length(&doc, &p) - 570.0).abs() < 1e-9,
        "{}",
        length(&doc, &p)
    );
    // Slid along its slot: the ray goes along.
    command(
        &mut doc,
        json!({"cmd": "build_edit", "component": p["component"], "at": {"offset": 300}}),
    )
    .unwrap();
    all_ok(&doc);
    assert!((length(&doc, &p) - 570.0).abs() < 1e-9);
    assert!(close(
        placed(&doc, occurrence(&p)).translation,
        [15.0, 0.0, 300.0]
    ));
    // Centre and through.
    let xp = derived(&doc, component(&x));
    for (mode, expected) in [("centre", 585.0), ("through", 600.0)] {
        command(
            &mut doc,
            json!({"cmd": "build_edit", "component": p["component"],
                   "length": {"bind": {"occurrence": x["occurrence"],
                                       "geometry": {"mount": format!("{xp}:mount(slot_x-_1)")}},
                              "mode": mode}}),
        )
        .unwrap();
        all_ok(&doc);
        assert!(
            (length(&doc, &p) - expected).abs() < 1e-9,
            "{mode}: {}",
            length(&doc, &p)
        );
    }
    // Behind the start: the ray meets nothing, the length stays.
    command(
        &mut doc,
        json!({"cmd": "set_occurrence_transform", "occurrence": x["occurrence"],
               "transform": {"translation": [-500, 0, 0]}}),
    )
    .unwrap();
    let bound = bound_of(&doc, &p);
    assert!(
        doc.status(bound)
            .and_then(|s| s.error())
            .is_some_and(|e| e.contains("meets nothing")),
        "{:?}",
        doc.status(bound)
    );
    assert!(
        (length(&doc, &p) - 600.0).abs() < 1e-9,
        "{}",
        length(&doc, &p)
    );
}

#[test]
fn a_value_detaches_the_bound_length() {
    let (mut doc, _, _, p) = bound_frame();
    let bound = bound_of(&doc, &p);
    let FeatureDef::BoundLength(b) = &doc.feature(bound).unwrap().def else {
        panic!("a bound length");
    };
    let name = doc.parameters().name(b.parameter);
    let answer = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": p["component"], "length": 300}),
    )
    .unwrap();
    all_ok(&doc);
    assert!(
        answer["changed"]
            .as_array()
            .unwrap()
            .contains(&json!("detached"))
    );
    assert!(doc.feature(bound).is_none());
    // The same parameter, now a plain dimension of the part.
    let id = doc.parameters().find(&name).unwrap();
    assert!(!doc.parameters().is_reference(id));
    assert_eq!(
        doc.parameters().owner(id),
        Some(derived(&doc, component(&p)))
    );
    assert!((length(&doc, &p) - 300.0).abs() < 1e-9);
    assert!(
        doc.part_info(component(&p))
            .unwrap()
            .record
            .bound
            .is_empty()
    );
    assert_eq!(
        doc.undo_label(),
        Some(format!("Edit {}", p["name"].as_str().unwrap()).as_str())
    );
}

#[test]
fn cycles_and_later_targets_are_refused() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let x = first(&mut doc, "p30", [500.0, 0.0, 0.0], 1000.0);
    let tp = derived(&doc, component(&t));
    let xp = derived(&doc, component(&x));
    let on = |occurrence: &serde_json::Value, part, slot: &str, offset: f64| {
        json!({"occurrence": occurrence, "geometry": {"mount": format!("{part}:mount({slot})")},
               "offset": offset})
    };
    // P3 out from T, P4 out from X towards it, bound to P3's end B.
    let p3 = place(
        &mut doc,
        json!({"part": {"source": "S1"}, "at": on(&t["occurrence"], tp, "slot_x+_1", 600.0), "length": 200}),
    );
    let p3p = derived(&doc, component(&p3));
    let p4 = place(
        &mut doc,
        json!({"part": {"source": "S1"}, "at": on(&x["occurrence"], xp, "slot_x-_1", 600.0),
               "length": {"bind": {"occurrence": p3["occurrence"],
                                   "geometry": {"mount": format!("{p3p}:mount(end_b)")}}}}),
    );
    all_ok(&doc);
    assert!(
        (length(&doc, &p4) - 270.0).abs() < 1e-9,
        "{}",
        length(&doc, &p4)
    );
    let p4p = derived(&doc, component(&p4));
    let revision = doc.revision();
    let error = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": p3["component"],
               "length": {"bind": {"occurrence": p4["occurrence"],
                                   "geometry": {"mount": format!("{p4p}:mount(end_b)")}}}}),
    )
    .unwrap_err();
    assert!(error.contains("a cycle"), "{error}");
    // A part placed after this one.
    let p5 = place(
        &mut doc,
        json!({"part": {"source": "S1"}, "at": on(&t["occurrence"], tp, "slot_x+_1", 800.0), "length": 100}),
    );
    let p6 = place(
        &mut doc,
        json!({"part": {"source": "S1"}, "at": on(&x["occurrence"], xp, "slot_x-_1", 800.0), "length": 100}),
    );
    let p6p = derived(&doc, component(&p6));
    let revision_before = doc.revision();
    let error = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": p5["component"],
               "length": {"bind": {"occurrence": p6["occurrence"],
                                   "geometry": {"mount": format!("{p6p}:mount(end_b)")}}}}),
    )
    .unwrap_err();
    assert!(error.contains("placed after"), "{error}");
    assert_eq!(doc.revision(), revision_before);
    assert!(revision < revision_before);
}
