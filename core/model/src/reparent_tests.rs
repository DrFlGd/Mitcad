// SPDX-License-Identifier: MIT
//! Moving an occurrence to another parent (mitcad#219) with the mock
//! kernel: to the root and into another component under translated and
//! rotated parents, with what is placed in it; repeated instances; links,
//! joints and Isolate following it; refusals that change nothing; undo
//! and redo as one step; the query the destination picker reads.

use std::f64::consts::FRAC_PI_2;

use serde_json::{Value, json};

use crate::Document;
use crate::document_tests::{def, extrude, num};
use crate::features::SketchPlane;
use crate::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use crate::kernel::Kernel;
use crate::testing::MockKernel;
use crate::transform::Transform;

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let text = command.to_string();
    match doc.command(&text) {
        Ok(result) => serde_json::from_str(&result).unwrap(),
        Err(e) => panic!("{text}: {e}"),
    }
}

fn rejected(doc: &mut Document<MockKernel>, command: Value) -> String {
    doc.command(&command.to_string()).unwrap_err().to_string()
}

fn reparent(occurrence: &str, parent: &str) -> Value {
    json!({"cmd": "reparent_occurrence", "occurrence": occurrence, "parent": parent})
}

fn shift(x: f64, y: f64, z: f64) -> Transform {
    Transform::translation([x, y, z])
}

fn turn(axis: [f64; 3], angle: f64) -> Transform {
    Transform::rotation([0.0; 3], axis, angle).unwrap()
}

/// A 10 x 10 x 5 block in the active component: its extrude, body and top
/// face (z = 5 in the component).
fn block(doc: &mut Document<MockKernel>) -> (FeatureUid, BodyUid, String) {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let uid = doc
        .add_feature(&extrude(sketch, &[&region], 5.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    (uid, BodyUid::new(uid, 0), format!("{uid}:end({region})"))
}

/// Every placed body's box in the design, in a stable order.
fn world(doc: &Document<MockKernel>) -> Vec<(BodyUid, [f64; 3], [f64; 3])> {
    let mut out: Vec<_> = doc
        .instances()
        .into_iter()
        .map(|i| {
            let shape = doc.body_shape(i.body).unwrap();
            let placed = doc
                .kernel()
                .transform_shape(shape, &i.transform, None)
                .unwrap();
            let bounds = placed.bounds.unwrap();
            (i.body, bounds.min, bounds.max)
        })
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

fn assert_same_world(
    before: &[(BodyUid, [f64; 3], [f64; 3])],
    after: &[(BodyUid, [f64; 3], [f64; 3])],
) {
    assert_eq!(before.len(), after.len(), "{before:?} != {after:?}");
    for (a, b) in before.iter().zip(after) {
        assert_eq!(a.0, b.0);
        assert!(
            (0..3).all(|i| (a.1[i] - b.1[i]).abs() < 1e-9 && (a.2[i] - b.2[i]).abs() < 1e-9),
            "{before:?} != {after:?}"
        );
    }
}

fn parent_of(doc: &Document<MockKernel>, uid: OccurrenceUid) -> ComponentUid {
    doc.assembly().occurrence(uid).unwrap().parent
}

/// Assembly:1 (O1) at (100, 0, 0) turned a quarter about Z, holding Part:1
/// (O2) at (0, 0, 5) turned a quarter about X with a block, and Pin:1 (O3)
/// in Part at (2, 0, 0) with a block. Root active.
fn nested() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    doc.create_component(
        Some("Assembly"),
        shift(100.0, 0.0, 0.0).after(&turn([0.0, 0.0, 1.0], FRAC_PI_2)),
        true,
    )
    .unwrap();
    doc.create_component(
        Some("Part"),
        shift(0.0, 0.0, 5.0).after(&turn([1.0, 0.0, 0.0], FRAC_PI_2)),
        true,
    )
    .unwrap();
    block(&mut doc);
    doc.create_component(Some("Pin"), shift(2.0, 0.0, 0.0), true)
        .unwrap();
    block(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    doc
}

#[test]
fn a_component_moves_to_the_root_where_it_is_with_what_is_in_it() {
    let mut doc = nested();
    let before = world(&doc);
    let features = doc.features().count();
    let (part, pin) = (OccurrenceUid(2), OccurrenceUid(3));
    let part_component = doc.assembly().occurrence(part).unwrap().component;

    let result = command(&mut doc, reparent("Assembly:1/Part:1", ""));
    assert_eq!(result["path"], "O2");
    assert_eq!(result["path_name"], "Part:1");
    assert_eq!(result["parent"], "C0");
    assert_eq!(doc.undo_label(), Some("Move Part:1 to Root"));
    // The same occurrence and component, now in the root beside Assembly,
    // with its own occurrence still in it; nothing moved in the design.
    assert_eq!(parent_of(&doc, part), ComponentUid::ROOT);
    assert_eq!(
        doc.assembly().occurrence(part).unwrap().component,
        part_component
    );
    assert_eq!(parent_of(&doc, pin), part_component);
    assert_eq!(doc.assembly().occurrence_name(part), "Part:1");
    assert_eq!(doc.features().count(), features);
    assert_same_world(&before, &world(&doc));
    let paths: Vec<String> = doc.instances().into_iter().map(|i| i.path_name).collect();
    assert!(paths.contains(&"Part:1/Pin:1".to_owned()), "{paths:?}");
    // Its placement in the root is the one Assembly gave it.
    let placed = doc.assembly().occurrence(part).unwrap().transform;
    let expected = shift(100.0, 0.0, 0.0)
        .after(&turn([0.0, 0.0, 1.0], FRAC_PI_2))
        .after(&shift(0.0, 0.0, 5.0))
        .after(&turn([1.0, 0.0, 0.0], FRAC_PI_2));
    for r in 0..3 {
        assert!((placed.translation[r] - expected.translation[r]).abs() < 1e-9);
        for c in 0..3 {
            assert!((placed.linear[r][c] - expected.linear[r][c]).abs() < 1e-9);
        }
    }

    // One undo step takes all of it back; redo moves it again.
    assert_eq!(doc.undo().as_deref(), Some("Move Part:1 to Root"));
    assert_eq!(parent_of(&doc, part), ComponentUid(1));
    assert_same_world(&before, &world(&doc));
    doc.redo();
    assert_eq!(parent_of(&doc, part), ComponentUid::ROOT);
    assert_same_world(&before, &world(&doc));

    // The design file keeps the new hierarchy.
    let saved = doc.to_json();
    let mut loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    loaded.recompute();
    assert_eq!(parent_of(&loaded, part), ComponentUid::ROOT);
    assert_same_world(&before, &world(&loaded));
}

#[test]
fn a_component_moves_into_another_one_under_turned_parents() {
    let mut doc = nested();
    // Bracket:1 (O4) in the root at (0, 50, 0), turned about X and Y.
    doc.create_component(
        Some("Bracket"),
        shift(0.0, 50.0, 0.0)
            .after(&turn([1.0, 0.0, 0.0], 0.3))
            .after(&turn([0.0, 1.0, 0.0], -0.7)),
        true,
    )
    .unwrap();
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let before = world(&doc);
    let bracket = doc
        .assembly()
        .occurrence(OccurrenceUid(4))
        .unwrap()
        .component;

    command(&mut doc, reparent("O1/O2", "O4"));
    assert_eq!(parent_of(&doc, OccurrenceUid(2)), bracket);
    assert_same_world(&before, &world(&doc));
    // And back into Assembly: its first placement again.
    command(&mut doc, reparent("O4/O2", "O1"));
    assert_eq!(parent_of(&doc, OccurrenceUid(2)), ComponentUid(1));
    let placed = doc
        .assembly()
        .occurrence(OccurrenceUid(2))
        .unwrap()
        .transform;
    let first = shift(0.0, 0.0, 5.0).after(&turn([1.0, 0.0, 0.0], FRAC_PI_2));
    assert!((0..3).all(|r| (placed.translation[r] - first.translation[r]).abs() < 1e-9));
    assert_same_world(&before, &world(&doc));
    // Two steps, each undone on its own.
    doc.undo();
    assert_eq!(parent_of(&doc, OccurrenceUid(2)), bracket);
    doc.undo();
    assert_eq!(parent_of(&doc, OccurrenceUid(2)), ComponentUid(1));
}

#[test]
fn cycles_the_current_parent_and_the_root_are_refused_without_change() {
    let mut doc = nested();
    let saved = doc.to_json();
    let depth = doc.undo_depth();
    let refusals = [
        (reparent("", ""), "the root component cannot be moved"),
        (reparent("O1/O2", "O1"), "already in Assembly"),
        (reparent("O1/O2", "O1/O2"), "its own component"),
        (reparent("O1/O2", "O1/O2/O3"), "placed in it"),
        (reparent("O1", "O1/O2"), "placed in it"),
        (reparent("O1/O9", ""), "no occurrence path"),
    ];
    for (cmd, expected) in refusals {
        let error = rejected(&mut doc, cmd.clone());
        assert!(error.contains(expected), "{cmd}: {error}");
        assert_eq!(doc.to_json(), saved, "{cmd} changed the design");
        assert_eq!(doc.undo_depth(), depth);
    }
}

#[test]
fn repeated_instances_are_kept_or_refused() {
    let mut doc = nested();
    // A second Part in the root, of the same component: moving the nested
    // one leaves it as it is.
    command(
        &mut doc,
        json!({"cmd": "copy_occurrence", "occurrence": "O1/O2", "transform": {"translation": [0, -40, 0]}}),
    );
    let copy = OccurrenceUid(4);
    let copy_before = doc.assembly().occurrence(copy).unwrap().clone();
    let before = world(&doc);
    command(&mut doc, reparent("O1/O2", ""));
    assert_eq!(doc.assembly().occurrence(copy).unwrap(), &copy_before);
    assert_same_world(&before, &world(&doc));
    doc.undo();

    // Assembly placed twice: Part is in both, so it cannot leave one.
    doc.activate_component(ComponentUid::ROOT).unwrap();
    command(
        &mut doc,
        json!({"cmd": "copy_occurrence", "occurrence": "O1", "transform": {"translation": [0, 0, 80]}}),
    );
    let saved = doc.to_json();
    let error = rejected(&mut doc, reparent("O1/O2", ""));
    assert!(error.contains("Assembly is placed 2 times"), "{error}");
    // Nor can anything go into Assembly: it would appear twice.
    let error = rejected(&mut doc, reparent("O4", "O1"));
    assert!(error.contains("Assembly is placed 2 times"), "{error}");
    assert_eq!(doc.to_json(), saved);
}

#[test]
fn occurrences_placed_by_the_timeline_are_refused() {
    let mut doc = nested();
    doc.activate_component(ComponentUid(1)).unwrap();
    doc.add_feature(
        &def(json!({"type": "move_occurrence", "occurrences": ["O2"],
                    "transform": {"type": "translate_xyz", "x": 0, "y": 3, "z": 0}})),
        None,
    )
    .unwrap();
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let saved = doc.to_json();
    let error = rejected(&mut doc, reparent("O1/O2", ""));
    assert!(error.contains("Part:1 is placed by Move1"), "{error}");
    assert_eq!(doc.to_json(), saved);
    // The picker's query says why for every destination.
    let answer = query(
        &doc,
        json!({"query": "reparent_occurrence", "occurrence": "O1/O2"}),
    );
    assert!(
        answer["problems"][0].as_str().unwrap().contains("Move1"),
        "{answer}"
    );
    assert!(
        answer["destinations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["eligible"] == false),
        "{answer}"
    );
}

/// Root with Assembly:1 (O1) 10 up holding Part:1 (O2) with a block, and
/// Cover:1 (O3) beside it with a sketch on Part's top face, linked through
/// O1/O2, with a linked projection of that face.
fn linked() -> (Document<MockKernel>, Value) {
    let mut doc = Document::new(MockKernel::default());
    doc.create_component(Some("Assembly"), shift(0.0, 0.0, 10.0), true)
        .unwrap();
    doc.create_component(Some("Part"), shift(5.0, 0.0, 0.0), true)
        .unwrap();
    let (_, body, face) = block(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    doc.create_component(Some("Cover"), shift(0.0, 50.0, -3.0), true)
        .unwrap();
    // Cover's sketch on Part's face (mitcad#220: both sides allow it).
    crate::policy_tests::allow_attachments(&mut doc);
    let sketch = command(
        &mut doc,
        json!({"cmd": "sketch.create", "plane": {"face": face, "body": body},
               "occurrence": "O1/O2"}),
    )["uid"]
        .clone();
    doc.activate_component(ComponentUid::ROOT).unwrap();
    (doc, sketch)
}

fn origin(doc: &Document<MockKernel>, sketch: &Value) -> Vec<f64> {
    let s = query(doc, json!({"query": "sketch", "uid": sketch}));
    s["frame"]["origin"]
        .as_array()
        .unwrap_or_else(|| panic!("no frame: {s}"))
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

#[test]
fn links_and_isolate_through_the_occurrence_follow_it() {
    let (mut doc, sketch) = linked();
    let uid: FeatureUid = sketch.as_str().unwrap().parse().unwrap();
    assert!(doc.status(uid).unwrap().is_ok(), "{:?}", doc.status(uid));
    let at = origin(&doc, &sketch);
    command(
        &mut doc,
        json!({"cmd": "set_isolation", "items": [{"occurrence": "O1/O2"}]}),
    );

    let result = command(&mut doc, reparent("O1/O2", ""));
    assert_eq!(
        result["updated"],
        json!([doc.feature(uid).unwrap().name, "Isolate"])
    );
    let s = query(&doc, json!({"query": "sketch", "uid": sketch}));
    assert_eq!(s["plane_link"], json!({"source": "O2", "target": "O3"}));
    assert!(doc.status(uid).unwrap().is_ok(), "{:?}", doc.status(uid));
    let moved = origin(&doc, &sketch);
    assert!(
        at.iter().zip(&moved).all(|(a, b)| (a - b).abs() < 1e-9),
        "{at:?} != {moved:?}"
    );
    let display = query(&doc, json!({"query": "document"}))["display"].clone();
    assert_eq!(display["isolated"], json!([{"occurrence": "O2"}]));

    // Undo restores the hierarchy, the link and Isolate together.
    doc.undo();
    let s = query(&doc, json!({"query": "sketch", "uid": sketch}));
    assert_eq!(s["plane_link"], json!({"source": "O1/O2", "target": "O3"}));
    let display = query(&doc, json!({"query": "document"}))["display"].clone();
    assert_eq!(display["isolated"], json!([{"occurrence": "O1/O2"}]));
}

#[test]
fn links_across_a_parent_the_timeline_moves_are_refused() {
    let (mut doc, sketch) = linked();
    // Assembly is moved by the timeline after the sketch: the sketch sees
    // Part where Assembly is at its point of the timeline.
    doc.add_feature(
        &def(json!({"type": "move_occurrence", "occurrences": ["O1"],
                    "transform": {"type": "translate_xyz", "x": 0, "y": 0, "z": 7}})),
        None,
    )
    .unwrap();
    let name = doc
        .feature(sketch.as_str().unwrap().parse().unwrap())
        .unwrap()
        .name
        .clone();
    let saved = doc.to_json();
    let error = rejected(&mut doc, reparent("O1/O2", ""));
    assert!(
        error.contains(&name) && error.contains("Assembly:1 is placed by Move1"),
        "{error}"
    );
    assert_eq!(doc.to_json(), saved);
}

#[test]
fn joints_follow_when_the_occurrence_they_move_stays() {
    // Asm:1 (O1) holding Sub:1 (O2) holding Part:1 (O3) with a block, and
    // Plate:1 (O4) with a block, joined where they are in the root.
    let mut doc = Document::new(MockKernel::default());
    doc.create_component(Some("Asm"), Transform::IDENTITY, true)
        .unwrap();
    doc.create_component(Some("Sub"), shift(10.0, 0.0, 0.0), true)
        .unwrap();
    doc.create_component(Some("Part"), shift(0.0, 10.0, 0.0), true)
        .unwrap();
    block(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    doc.create_component(Some("Plate"), shift(0.0, 0.0, -20.0), true)
        .unwrap();
    block(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let joint = command(
        &mut doc,
        json!({"cmd": "add_as_built_joint", "kind": "rigid", "a": "O1/O2/O3", "b": "O4"}),
    );
    let uid: FeatureUid = joint["uid"].as_str().unwrap().parse().unwrap();
    assert!(doc.status(uid).unwrap().is_ok(), "{:?}", doc.status(uid));
    let name = doc.feature(uid).unwrap().name.clone();

    // Out of Sub into Asm: the joint still moves Asm:1, through the new path.
    let before = world(&doc);
    let result = command(&mut doc, reparent("O1/O2/O3", "O1"));
    assert_eq!(result["updated"], json!([name]));
    let def = query(&doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    assert_eq!(def["a"], "O1/O3");
    assert!(doc.status(uid).unwrap().is_ok(), "{:?}", doc.status(uid));
    assert_same_world(&before, &world(&doc));
    doc.undo();

    // To the root it would leave Asm:1, which the joint moves: refused,
    // naming the joint.
    let saved = doc.to_json();
    let error = rejected(&mut doc, reparent("O1/O2/O3", ""));
    assert!(error.contains(&name) && error.contains("Asm:1"), "{error}");
    assert_eq!(doc.to_json(), saved);
}

#[test]
fn the_query_lists_destinations_contents_and_the_plan() {
    let mut doc = nested();
    doc.create_component(Some("Bracket"), shift(0.0, 50.0, 0.0), true)
        .unwrap();
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let depth = doc.undo_depth();
    let answer = query(
        &doc,
        json!({"query": "reparent_occurrence", "occurrence": "O1/O2", "parent": ""}),
    );
    assert_eq!(answer["name"], "Part:1");
    assert_eq!(answer["path_name"], "Root/Assembly:1/Part:1");
    assert_eq!(answer["parent_path"], "Root/Assembly:1");
    assert_eq!(answer["contents"]["occurrences"], json!(["Pin:1"]));
    assert_eq!(answer["contents"]["components"], 2);
    assert_eq!(answer["contents"]["bodies"], 2);
    assert_eq!(answer["problems"], json!([]));
    let row = |path: &str| -> Value {
        answer["destinations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["occurrence"] == path)
            .unwrap_or_else(|| panic!("no destination {path}: {answer}"))
            .clone()
    };
    assert_eq!(row("")["eligible"], true);
    assert_eq!(row("")["path"], "Root");
    assert_eq!(row("O4")["eligible"], true);
    assert_eq!(row("O4")["path"], "Root/Bracket:1");
    let current = row("O1");
    assert_eq!(current["current"], true);
    assert_eq!(current["eligible"], false);
    assert!(current["reason"].as_str().unwrap().contains("already in"));
    assert!(
        row("O1/O2")["reason"]
            .as_str()
            .unwrap()
            .contains("its own component")
    );
    assert!(
        row("O1/O2/O3")["reason"]
            .as_str()
            .unwrap()
            .contains("placed in it")
    );
    assert_eq!(answer["plan"]["eligible"], true, "{answer}");
    assert_eq!(answer["plan"]["path"], "O2");
    assert_eq!(answer["plan"]["parent_path"], "Root");
    assert_eq!(answer["plan"]["from_path"], "Root/Assembly:1");
    let x = answer["plan"]["transform"][0][3].as_f64().unwrap();
    assert!((x - 100.0).abs() < 1e-9, "{answer}");
    // Queries change nothing.
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(parent_of(&doc, OccurrenceUid(2)), ComponentUid(1));
    let refused = query(
        &doc,
        json!({"query": "reparent_occurrence", "occurrence": "O1/O2", "parent": "O1/O2/O3"}),
    );
    assert_eq!(refused["plan"]["eligible"], false);
    assert!(
        refused["plan"]["problems"][0]
            .as_str()
            .unwrap()
            .contains("placed in it")
    );
}
