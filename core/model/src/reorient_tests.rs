// SPDX-License-Identifier: MIT
//! Reorient Design (mitcad#502) on the mock kernel: a design with a sketch
//! on XY, an extrusion, a construction plane offset from XZ with a sketch
//! and an extrusion of its own, a fillet, a hole and an imported body,
//! turned by quarter turns about each axis: the same bodies turned, face
//! names unchanged, references to origin planes rewritten with their
//! sense, one undo step that restores the design exactly.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::features::{FeatureDef, SketchPlane};
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::BoundingBox;
use crate::reorient::{Turn, TurnAxis};
use crate::testing::MockKernel;
use crate::{Document, FeatureStatus, GeomRef, OriginDatum};

const RECT: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";

fn command(doc: &mut Document<MockKernel>, command: Value) -> Result<Value, String> {
    doc.command(&command.to_string())
        .map(|r| serde_json::from_str(&r).unwrap())
        .map_err(|e| e.to_string())
}

fn run(doc: &mut Document<MockKernel>, value: Value) -> Value {
    command(doc, value.clone()).unwrap_or_else(|e| panic!("{value}: {e}"))
}

fn uid(text: &str) -> FeatureUid {
    text.parse().unwrap()
}

fn body(text: &str) -> BodyUid {
    text.parse().unwrap()
}

fn entry<'a>(doc: &'a Document<MockKernel>, f: &str) -> &'a crate::FeatureEntry {
    doc.features().find(|e| e.uid == uid(f)).unwrap()
}

/// Sketch1 (F1) on XY: 60 x 40 at the origin, Extrude1 (F2) 20 up; Plane1
/// (F3) offset 40 from XZ, Sketch2 (F4) on it with a 20 x 10 rectangle,
/// Extrude2 (F5) 15 joined to the block; Fillet1 (F6) on a vertical edge;
/// Hole1 (F7) through the top; Base1 (F8), an imported body.
fn design() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    run(&mut doc, json!({"cmd": "sketch.create"}));
    run(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": RECT}],
            "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "construction_plane",
            "definition": {"type": "offset", "plane": "xz", "distance": 40}}}),
    );
    run(&mut doc, json!({"cmd": "sketch.create", "plane": "F3"}));
    run(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F4", "corner": [20, -15], "width": 20, "height": 10}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F4", "region": RECT}],
            "extent": {"type": "distance", "distance": 15}, "operation": "join",
            "participants": ["F2.b0"]}}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "fillet", "body": "F2.b0",
            "edges": ["E{F2:side(c1[c4,c2])|F2:side(c2[c1,c3])}"], "radius": 5}}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "hole",
            "placement": {"type": "face", "body": "F2.b0", "face": format!("F2:end({RECT})"),
                          "points": [[15, 20, 20]]},
            "diameter": 8, "extent": {"type": "through_all"}}}),
    );
    let brep = serde_json::to_value(crate::features::Brep::new(b"6 box".to_vec())).unwrap();
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "base", "bodies": [{"brep": brep, "name": "Imported"}]}}),
    );
    for f in ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8"] {
        assert_eq!(doc.status(uid(f)), Some(&FeatureStatus::Ok), "{f}");
    }
    doc
}

fn boxes(doc: &Document<MockKernel>) -> BTreeMap<BodyUid, BoundingBox> {
    [body("F2.b0"), body("F8.b0")]
        .into_iter()
        .map(|b| (b, doc.body_shape(b).unwrap().bounds.unwrap()))
        .collect()
}

fn faces(doc: &Document<MockKernel>, b: &str) -> Vec<String> {
    let mut names: Vec<String> = doc
        .body_shape(body(b))
        .unwrap()
        .faces
        .iter()
        .map(|f| f.to_string())
        .collect();
    names.sort();
    names
}

fn turned_box(turn: Turn, b: &BoundingBox) -> BoundingBox {
    let (p, q) = (turn.vector(b.min), turn.vector(b.max));
    BoundingBox {
        min: std::array::from_fn(|i| p[i].min(q[i])),
        max: std::array::from_fn(|i| p[i].max(q[i])),
    }
}

fn near(a: &BoundingBox, b: &BoundingBox) -> bool {
    (0..3).all(|i| (a.min[i] - b.min[i]).abs() < 1e-9 && (a.max[i] - b.max[i]).abs() < 1e-9)
}

fn sketch_frame(doc: &Document<MockKernel>, f: &str) -> crate::SketchFrame {
    doc.sketch_output(uid(f)).unwrap().frame
}

fn same_frame(a: &crate::SketchFrame, b: &crate::SketchFrame) -> bool {
    let near = |u: [f64; 3], v: [f64; 3]| (0..3).all(|i| (u[i] - v[i]).abs() < 1e-12);
    near(a.origin, b.origin) && near(a.x_axis, b.x_axis) && near(a.y_axis, b.y_axis)
}

#[test]
fn ninety_degrees_about_x_turns_the_bodies_and_rewrites_the_origin_references() {
    let mut doc = design();
    let before = boxes(&doc);
    let frames = [sketch_frame(&doc, "F1"), sketch_frame(&doc, "F4")];
    let names = [faces(&doc, "F2.b0"), faces(&doc, "F8.b0")];
    let reply = run(
        &mut doc,
        json!({"cmd": "reorient_design", "axis": "x", "degrees": 90}),
    );
    assert_eq!(reply["turn"], "90 degrees about X");
    assert_eq!(reply["moves"], json!(["Move1"]));
    let turn = Turn::about(TurnAxis::X, 1);
    // The block (with its boss, fillet and hole) and the imported body,
    // where the turn takes them, with the same face names.
    let after = boxes(&doc);
    for (b, old) in &before {
        assert!(
            near(&after[b], &turned_box(turn, old)),
            "{b}: {:?}",
            after[b]
        );
    }
    assert_eq!(
        after[&body("F2.b0")].min,
        [0.0, -20.0, 0.0],
        "the block hangs below XZ"
    );
    assert_eq!(after[&body("F2.b0")].max, [60.0, 0.0, 55.0]);
    assert_eq!(faces(&doc, "F2.b0"), names[0]);
    assert_eq!(faces(&doc, "F8.b0"), names[1]);
    // Sketch1 lies on XZ, facing -Y (XY's +Z turned), not mirrored: its
    // frame is its old one turned.
    let FeatureDef::Sketch(sketch) = &entry(&doc, "F1").def else {
        panic!("a sketch");
    };
    assert_eq!(sketch.plane, SketchPlane::Xz);
    assert!(sketch.frame.is_some());
    assert!(same_frame(
        &sketch_frame(&doc, "F1"),
        &turn.frame(&frames[0])
    ));
    assert_eq!(sketch_frame(&doc, "F1").normal(), [0.0, -1.0, 0.0]);
    // Plane1 was offset from XZ, which the turn takes onto XY exactly.
    let FeatureDef::ConstructionPlane(plane) = &entry(&doc, "F3").def else {
        panic!("a plane");
    };
    assert_eq!(
        plane.definition,
        crate::features::construction::PlaneDefinition::Offset {
            plane: GeomRef::Origin(OriginDatum::Xy),
            distance: plane_distance(&plane.definition),
        }
    );
    assert!(same_frame(
        &sketch_frame(&doc, "F4"),
        &turn.frame(&frames[1])
    ));
    // Every feature still builds; the Move follows the base feature.
    for f in ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8"] {
        assert_eq!(doc.status(uid(f)), Some(&FeatureStatus::Ok), "{f}");
    }
    let timeline: Vec<String> = doc.features().map(|f| f.name.clone()).collect();
    assert_eq!(timeline.last().map(String::as_str), Some("Move1"));
    assert_eq!(
        timeline[timeline.len() - 2],
        "Base1",
        "the Move comes right after the imported body"
    );
}

fn plane_distance(
    definition: &crate::features::construction::PlaneDefinition,
) -> crate::parameters::ParamId {
    match definition {
        crate::features::construction::PlaneDefinition::Offset { distance, .. } => *distance,
        _ => panic!("an offset plane"),
    }
}

#[test]
fn every_axis_and_quarter_turn_turns_the_same_bodies() {
    for axis in ["x", "y", "z"] {
        for degrees in [90, 180, 270] {
            let mut doc = design();
            let before = boxes(&doc);
            let names = faces(&doc, "F2.b0");
            let frames = [sketch_frame(&doc, "F1"), sketch_frame(&doc, "F4")];
            run(
                &mut doc,
                json!({"cmd": "reorient_design", "axis": axis, "degrees": degrees}),
            );
            let turn = Turn::about(TurnAxis::parse(axis).unwrap(), degrees / 90);
            let after = boxes(&doc);
            for (b, old) in &before {
                assert!(
                    near(&after[b], &turned_box(turn, old)),
                    "{axis} {degrees}: {b}: {:?}",
                    after[b]
                );
            }
            assert_eq!(faces(&doc, "F2.b0"), names, "{axis} {degrees}");
            for (f, frame) in ["F1", "F4"].iter().zip(&frames) {
                assert!(
                    same_frame(&sketch_frame(&doc, f), &turn.frame(frame)),
                    "{axis} {degrees}: {f}"
                );
            }
        }
    }
}

#[test]
fn an_origin_plane_becomes_xy() {
    let mut doc = design();
    let before = boxes(&doc);
    let reply = run(&mut doc, json!({"cmd": "reorient_design", "plane": "xz"}));
    assert_eq!(reply["turn"], "XZ becomes XY");
    let turn = Turn::about(TurnAxis::X, 1);
    for (b, old) in &before {
        assert!(near(&boxes(&doc)[b], &turned_box(turn, old)), "{b}");
    }
    let mut doc = design();
    run(&mut doc, json!({"cmd": "reorient_design", "plane": "yz"}));
    let turn = Turn::about(TurnAxis::Y, 3);
    for (b, old) in &before {
        assert!(near(&boxes(&doc)[b], &turned_box(turn, old)), "{b}");
    }
}

#[test]
fn undo_restores_the_design_exactly() {
    let mut doc = design();
    let file = doc.to_json();
    let before = boxes(&doc);
    run(
        &mut doc,
        json!({"cmd": "reorient_design", "axis": "y", "degrees": 270}),
    );
    assert_ne!(doc.to_json(), file);
    let undone = run(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(undone["label"], "Reorient Design");
    assert_eq!(doc.to_json(), file);
    for (b, old) in &before {
        assert!(near(&boxes(&doc)[b], old), "{b}");
    }
}

#[test]
fn moves_turn_in_turned_coordinates() {
    let mut doc = Document::new(MockKernel::default());
    run(&mut doc, json!({"cmd": "sketch.create"}));
    run(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 10, "height": 10}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": RECT}],
            "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "move", "bodies": ["F2.b0"],
            "transform": {"type": "translate_xyz", "x": 10, "y": 20, "z": 30}}}),
    );
    let before = boxes_of(&doc, "F2.b0");
    run(
        &mut doc,
        json!({"cmd": "reorient_design", "axis": "x", "degrees": 90}),
    );
    let turn = Turn::about(TurnAxis::X, 1);
    assert!(near(&boxes_of(&doc, "F2.b0"), &turned_box(turn, &before)));
    // The y value (20) now runs along +Z; the z value (30) along -Y.
    let values: BTreeMap<String, f64> = doc
        .parameters()
        .iter()
        .map(|p| (p.name().to_owned(), p.value()))
        .collect();
    let FeatureDef::Move(m) = &entry(&doc, "F3").def else {
        panic!("a move");
    };
    let crate::features::moves::MoveSpec::TranslateXyz { x, y, z } = &m.transform else {
        panic!("along the axes");
    };
    let value = |id: &crate::parameters::ParamId| values[&doc.parameters().name(*id)];
    assert_eq!([value(x), value(y), value(z)], [10.0, -30.0, 20.0]);
}

fn boxes_of(doc: &Document<MockKernel>, b: &str) -> BoundingBox {
    doc.body_shape(body(b)).unwrap().bounds.unwrap()
}

#[test]
fn a_design_turns_with_its_components() {
    // A component placed at (100, 0, 0) with a block of its own (made in
    // it), and the root's block: both turn, and the component's own
    // sketch is rewritten in its coordinates.
    let mut doc = Document::new(MockKernel::default());
    run(
        &mut doc,
        json!({"cmd": "create_component", "name": "Part", "transform": {"translation": [100, 0, 0]}}),
    );
    run(&mut doc, json!({"cmd": "sketch.create"}));
    run(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 10, "height": 20}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": RECT}],
            "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}}),
    );
    let instance = |doc: &Document<MockKernel>| {
        doc.instances()
            .into_iter()
            .find(|i| i.body == body("F2.b0"))
            .unwrap()
            .transform
    };
    let local = boxes_of(&doc, "F2.b0");
    run(
        &mut doc,
        json!({"cmd": "reorient_design", "axis": "z", "degrees": 90}),
    );
    let turn = Turn::about(TurnAxis::Z, 1);
    // In the component's turned coordinates, and placed at (0, 100, 0).
    assert!(near(&boxes_of(&doc, "F2.b0"), &turned_box(turn, &local)));
    assert_eq!(instance(&doc).translation, [0.0, 100.0, 0.0]);
    assert_eq!(instance(&doc).linear, crate::Transform::IDENTITY.linear);
}

#[test]
fn a_joint_to_the_origin_turns_with_the_design() {
    // Pin (a component of its own) jointed rigidly by its XY plane to the
    // root's XZ plane: after the turn the joint names the turned XZ (XY,
    // exactly) and the pin is where the turn takes it.
    let mut doc = Document::new(MockKernel::default());
    run(
        &mut doc,
        json!({"cmd": "create_component", "name": "Pin", "transform": {"translation": [100, 0, 0]}}),
    );
    run(&mut doc, json!({"cmd": "sketch.create"}));
    run(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 10, "height": 20}),
    );
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
            "profiles": [{"sketch": "F1", "region": RECT}],
            "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}}),
    );
    run(
        &mut doc,
        json!({"cmd": "activate_component", "component": "Root"}),
    );
    let joint = run(
        &mut doc,
        json!({"cmd": "add_joint", "kind": "rigid",
            "a": {"occurrence": "Pin:1", "geometry": "xy"}, "b": {"geometry": "xz"}}),
    );
    assert_eq!(joint["error"], Value::Null, "{joint}");
    let placed = |doc: &Document<MockKernel>| {
        doc.instances()
            .into_iter()
            .find(|i| i.body == body("F2.b0"))
            .unwrap()
            .transform
    };
    let before = placed(&doc);
    run(
        &mut doc,
        json!({"cmd": "reorient_design", "axis": "x", "degrees": 90}),
    );
    let turn = Turn::about(TurnAxis::X, 1);
    let expected = turn.conjugate(&before);
    let now = placed(&doc);
    for r in 0..3 {
        assert!(
            (now.translation[r] - expected.translation[r]).abs() < 1e-9,
            "{now:?}"
        );
        for c in 0..3 {
            assert!(
                (now.linear[r][c] - expected.linear[r][c]).abs() < 1e-9,
                "{now:?}"
            );
        }
    }
    let joint = doc
        .features()
        .find(|f| matches!(f.def, FeatureDef::Joint(_)))
        .unwrap();
    let FeatureDef::Joint(def) = &joint.def else {
        unreachable!()
    };
    assert_eq!(def.b.geometry, GeomRef::Origin(OriginDatum::Xy));
    // The pin's own XY turned faces -Y in its coordinates: a fixed plane.
    assert!(matches!(def.a.geometry, GeomRef::FixedPlane { .. }));
}

#[test]
fn a_sketch_on_a_face_keeps_its_place_on_the_face() {
    // A face's plane takes its in-plane x axis from the model's X: turned
    // about Z the top face faces +Z as before, so its frame alone would
    // turn the sketch back by the quarter turn. The sketch's own frame
    // keeps it turned with the design.
    let mut doc = design();
    run(
        &mut doc,
        json!({"cmd": "sketch.create", "plane": {"face": format!("F2:end({RECT})"), "body": "F2.b0"}}),
    );
    let sketch = "F9";
    run(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": sketch, "corner": [40, 25], "width": 10, "height": 8}),
    );
    let frame = sketch_frame(&doc, sketch);
    run(
        &mut doc,
        json!({"cmd": "reorient_design", "axis": "z", "degrees": 90}),
    );
    let turn = Turn::about(TurnAxis::Z, 1);
    assert!(same_frame(&sketch_frame(&doc, sketch), &turn.frame(&frame)));
    let FeatureDef::Sketch(def) = &entry(&doc, sketch).def else {
        panic!("a sketch");
    };
    assert!(matches!(def.plane, SketchPlane::Face { .. }));
    assert!(
        def.frame.is_some(),
        "an own frame turns it back onto the design"
    );
    let features: Vec<String> = doc.features().map(|f| f.name.clone()).collect();
    assert!(features.contains(&"Sketch3".to_owned()));
}

#[test]
fn a_base_feature_that_joins_is_refused() {
    let mut doc = design();
    let brep = serde_json::to_value(crate::features::Brep::new(b"6 box".to_vec())).unwrap();
    run(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "base", "bodies": [{"brep": brep}],
            "operation": "join", "participants": ["F2.b0"]}}),
    );
    let file = doc.to_json();
    let error = command(
        &mut doc,
        json!({"cmd": "reorient_design", "axis": "x", "degrees": 90}),
    )
    .unwrap_err();
    assert!(error.contains("Base2"), "{error}");
    assert_eq!(doc.to_json(), file, "nothing changed");
}

#[test]
fn identity_and_bad_turns_are_refused() {
    let mut doc = design();
    for (value, expected) in [
        (
            json!({"cmd": "reorient_design", "plane": "xy"}),
            "leaves the design",
        ),
        (
            json!({"cmd": "reorient_design", "axis": "x", "degrees": 45}),
            "90, 180 or 270",
        ),
        (
            json!({"cmd": "reorient_design", "axis": "w", "degrees": 90}),
            "unknown axis",
        ),
        (json!({"cmd": "reorient_design", "axis": "x"}), "takes"),
        (
            json!({"cmd": "reorient_design", "plane": "x"}),
            "unknown origin plane",
        ),
    ] {
        let error = command(&mut doc, value.clone()).unwrap_err();
        assert!(error.contains(expected), "{value}: {error}");
    }
}
