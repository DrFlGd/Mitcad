// SPDX-License-Identifier: MIT
//! Body Transfer (mitcad#465) with the mock kernel: bodies moved into an
//! existing component at a point of the timeline keep their ids and their
//! place in the design, the destination's later features work on them and
//! the source's no longer see them; the conversion follows the placements
//! before it or is the fixed one given; the design file, undo, refusals and
//! copies of the destination.

use serde_json::{Value, json};

use crate::document_tests::{extrude, num};
use crate::features::SketchPlane;
use crate::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use crate::kernel::{BoundingBox, Kernel};
use crate::testing::MockKernel;
use crate::transform::Transform;
use crate::{Document, FeatureStatus};

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

fn command(doc: &mut Document<MockKernel>, command: Value) -> Result<Value, String> {
    doc.command(&command.to_string())
        .map(|r| serde_json::from_str(&r).unwrap())
        .map_err(|e| e.to_string())
}

fn shift(x: f64, y: f64, z: f64) -> Transform {
    Transform::translation([x, y, z])
}

/// A 10 x 10 x 5 block at the origin of the active component.
fn small_block(doc: &mut Document<MockKernel>) -> (FeatureUid, BodyUid) {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let uid = doc
        .add_feature(&extrude(sketch, &[&region], 5.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    (uid, BodyUid::new(uid, 0))
}

/// The body's box in the design, where its one instance is.
fn world_box(doc: &Document<MockKernel>, body: BodyUid) -> BoundingBox {
    let instance = doc
        .instances()
        .into_iter()
        .find(|i| i.body == body)
        .expect("placed");
    let shape = doc.body_shape(body).unwrap();
    doc.kernel()
        .transform_shape(shape, &instance.transform, None)
        .unwrap()
        .bounds
        .unwrap()
}

fn local_box(doc: &Document<MockKernel>, body: BodyUid) -> BoundingBox {
    doc.body_shape(body).unwrap().bounds.unwrap()
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
}

/// The root's block and an empty component Bracket placed at (100, 0, 0),
/// the root active again.
fn design() -> (Document<MockKernel>, BodyUid, ComponentUid, OccurrenceUid) {
    let mut doc = Document::new(MockKernel::default());
    let (_, body) = small_block(&mut doc);
    let (bracket, o1) = doc
        .create_component(Some("Bracket"), shift(100.0, 0.0, 0.0), false)
        .unwrap();
    (doc, body, bracket, o1)
}

fn transfer(doc: &mut Document<MockKernel>, body: BodyUid, into: &str) -> Result<Value, String> {
    command(
        doc,
        json!({"cmd": "transfer_bodies", "bodies": [body],
               "destination": {"component": into}}),
    )
}

#[test]
fn a_body_moves_into_an_existing_component_where_it_is() {
    let (mut doc, body, bracket, _) = design();
    let before = world_box(&doc, body);
    let result = transfer(&mut doc, body, "Bracket").unwrap();
    assert_eq!(result["name"], "BodyTransfer1");
    assert_eq!(result["component"], "C1");
    assert_eq!(result["component_name"], "Bracket");
    let uid: FeatureUid = result["uid"].as_str().unwrap().parse().unwrap();
    assert_eq!(doc.feature(uid).unwrap().component, bracket);
    assert_eq!(doc.undo_label(), Some("Move Body1 into Bracket"));
    // The same body (id and name) in the destination, where it was.
    assert_eq!(doc.body_component(body), Some(bracket));
    assert_eq!(doc.body_name(body), "Body1");
    let after = world_box(&doc, body);
    assert!(near(after.min, before.min) && near(after.max, before.max));
    // In the destination's coordinates, 100 to the left.
    let local = local_box(&doc, body);
    assert!(near(local.min, [-100.0, 0.0, 0.0]), "{local:?}");
    assert_eq!(doc.instances().len(), 1);
    // The file form: the link and no fixed transform.
    let file: Value = serde_json::from_str(&doc.to_json()).unwrap();
    let saved = file["features"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["type"] == "body_transfer")
        .unwrap()
        .clone();
    assert_eq!(saved["bodies"], json!([body.to_string()]));
    assert_eq!(saved["link"], json!({"target": "O1"}));
    assert_eq!(saved["component"], "C1");
    assert!(saved.get("transform").is_none());

    // Undo puts it back in the root.
    doc.undo();
    assert_eq!(doc.body_component(body), Some(ComponentUid::ROOT));
}

#[test]
fn the_destinations_later_features_work_on_the_body_and_the_sources_do_not() {
    let (mut doc, body, _, o1) = design();
    transfer(&mut doc, body, "Bracket").unwrap();
    // A move in Bracket, in its coordinates.
    let moved = command(
        &mut doc,
        json!({"cmd": "add_feature", "component": "C1",
               "def": {"type": "move", "bodies": [body],
                       "transform": {"type": "translate_xyz", "x": 0, "y": 0, "z": 7}}}),
    )
    .unwrap();
    assert!(moved["failed_features"].is_null(), "{moved}");
    assert!(near(local_box(&doc, body).min, [-100.0, 0.0, 7.0]));
    // The root's features no longer see it.
    let refused = command(
        &mut doc,
        json!({"cmd": "add_feature", "component": "C0",
               "def": {"type": "move", "bodies": [body],
                       "transform": {"type": "translate_xyz", "x": 1, "y": 0, "z": 0}}}),
    )
    .unwrap();
    let failed = refused["failed_features"].to_string();
    assert!(failed.contains("belongs to Bracket"), "{failed}");
    doc.undo();
    // A timeline move of the destination's occurrence after the transfer
    // takes the body along.
    let before = world_box(&doc, body);
    command(
        &mut doc,
        json!({"cmd": "add_feature", "component": "C0",
               "def": {"type": "move_occurrence", "occurrences": [o1],
                       "transform": {"type": "translate_xyz", "x": 0, "y": 30, "z": 0}}}),
    )
    .unwrap();
    let after = world_box(&doc, body);
    assert!(near(
        after.min,
        [before.min[0], before.min[1] + 30.0, before.min[2]]
    ));
}

#[test]
fn the_conversion_follows_the_placements_before_it_or_is_fixed() {
    let (mut doc, body, bracket, o1) = design();
    let before = world_box(&doc, body);
    transfer(&mut doc, body, "Bracket").unwrap();
    // The destination placed elsewhere: the body still where it was.
    doc.set_occurrence_transform(o1, shift(0.0, 50.0, 0.0), false)
        .unwrap();
    let after = world_box(&doc, body);
    assert!(near(after.min, before.min) && near(after.max, before.max));
    assert!(near(local_box(&doc, body).min, [0.0, -50.0, 0.0]));

    // A fixed conversion stays as given, wherever the components are.
    let (mut doc, body, _, _) = design();
    let added = command(
        &mut doc,
        json!({"cmd": "add_feature", "component": "C1",
               "def": {"type": "body_transfer", "bodies": [body],
                       "link": {"target": "O1"},
                       "transform": [[1, 0, 0, -20], [0, 1, 0, 0], [0, 0, 1, 0]]}}),
    )
    .unwrap();
    assert!(added["failed_features"].is_null(), "{added}");
    assert_eq!(doc.body_component(body), Some(bracket));
    assert!(near(local_box(&doc, body).min, [-20.0, 0.0, 0.0]));
    assert!(near(world_box(&doc, body).min, [80.0, 0.0, 0.0]));
    // Only rigid motions convert.
    let scaled = command(
        &mut doc,
        json!({"cmd": "add_feature", "component": "C1",
               "def": {"type": "body_transfer", "bodies": [body],
                       "transform": [[2, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]]}}),
    );
    let scaled = scaled.unwrap_err();
    assert!(scaled.contains("a rotation and a translation"), "{scaled}");
}

#[test]
fn bodies_move_between_nested_components_and_back_to_the_root() {
    let mut doc = Document::new(MockKernel::default());
    let (arm, _) = doc
        .create_component(Some("Arm"), shift(0.0, 0.0, 10.0), true)
        .unwrap();
    let (pin, _) = doc
        .create_component(Some("Pin"), shift(5.0, 0.0, 0.0), true)
        .unwrap();
    let (_, body) = small_block(&mut doc);
    assert_eq!(doc.body_component(body), Some(pin));
    let before = world_box(&doc, body);
    // Out of Arm/Pin into Arm, then into the root.
    command(
        &mut doc,
        json!({"cmd": "transfer_bodies", "bodies": [body], "source_occurrence": "O1/O2",
               "destination": {"component": "Arm", "occurrence": "O1"}}),
    )
    .unwrap();
    assert_eq!(doc.body_component(body), Some(arm));
    assert!(near(local_box(&doc, body).min, [5.0, 0.0, 0.0]));
    transfer(&mut doc, body, "C0").unwrap();
    assert_eq!(doc.body_component(body), Some(ComponentUid::ROOT));
    let after = world_box(&doc, body);
    assert!(near(after.min, before.min) && near(after.max, before.max));
    assert!(near(local_box(&doc, body).min, [5.0, 0.0, 10.0]));
    // The design file keeps them.
    let json = doc.to_json();
    let mut loaded = Document::from_json(&json, MockKernel::default()).unwrap();
    loaded.recompute();
    assert_eq!(loaded.body_component(body), Some(ComponentUid::ROOT));
    assert!(near(local_box(&loaded, body).min, [5.0, 0.0, 10.0]));
    assert!(
        loaded
            .features()
            .all(|f| loaded.status(f.uid).is_some_and(FeatureStatus::is_ok))
    );
}

#[test]
fn transfers_that_cannot_work_are_refused() {
    let (mut doc, body, _, _) = design();
    let same = transfer(&mut doc, body, "C0").unwrap_err();
    assert!(same.contains("the bodies are in Root already"), "{same}");
    // A body the root works on later.
    command(
        &mut doc,
        json!({"cmd": "add_feature", "component": "C0",
               "def": {"type": "move", "bodies": [body],
                       "transform": {"type": "translate_xyz", "x": 1, "y": 0, "z": 0}}}),
    )
    .unwrap();
    let move_position = doc.features().count() - 1;
    doc.set_marker(move_position).unwrap();
    doc.set_marker(move_position + 1).unwrap();
    let before = doc.features().count();
    // At the marker (after the move) it works: the root's move came first.
    let error = transfer(&mut doc, body, "Bracket");
    assert!(error.is_ok(), "{error:?}");
    assert_eq!(doc.features().count(), before + 1);
    doc.undo();
    // Rolled back before the move: the move would fail once computed. The
    // marker hides it, so the transfer goes in; moving the marker to the
    // end shows the failure on the move.
    doc.set_marker(move_position).unwrap();
    transfer(&mut doc, body, "Bracket").unwrap();
    doc.set_marker(doc.features().count()).unwrap();
    let message = doc
        .features()
        .find(|f| f.name == "Move1")
        .and_then(|f| doc.status(f.uid))
        .and_then(|s| s.error())
        .unwrap_or_default()
        .to_owned();
    assert!(message.contains("belongs to Bracket"), "{message}");
    // Bodies of two components in one transfer.
    let mut doc = Document::new(MockKernel::default());
    let (_, a) = small_block(&mut doc);
    doc.create_component(Some("Other"), Transform::IDENTITY, true)
        .unwrap();
    let (_, b) = small_block(&mut doc);
    doc.create_component(Some("Third"), Transform::IDENTITY, false)
        .unwrap();
    let error = command(
        &mut doc,
        json!({"cmd": "transfer_bodies", "bodies": [a, b],
               "destination": {"component": "Third"}}),
    )
    .unwrap_err();
    assert!(error.contains("from one component"), "{error}");
    // A definition naming its own component's body.
    let own = command(
        &mut doc,
        json!({"cmd": "add_feature", "component": "C0",
               "def": {"type": "body_transfer", "bodies": [a]}}),
    )
    .unwrap();
    assert!(
        own["failed_features"]
            .to_string()
            .contains("moves bodies of another component")
    );
}

#[test]
fn the_query_lists_where_the_bodies_can_go() {
    let (doc, body, _, _) = design();
    let plan = query(&doc, json!({"query": "transfer_bodies", "bodies": [body]}));
    assert_eq!(plan["component"], "C0");
    assert_eq!(
        plan["bodies"],
        json!([{"uid": body.to_string(), "name": "Body1"}])
    );
    let rows = plan["destinations"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["component"], "C0");
    assert_eq!(rows[0]["eligible"], false);
    assert_eq!(rows[1]["name"], "Bracket");
    assert_eq!(rows[1]["occurrence"], "O1");
    assert_eq!(rows[1]["path"], "Root/Bracket:1");
    assert_eq!(rows[1]["eligible"], true);
}

#[test]
fn copies_of_a_component_that_takes_bodies_in_copy_its_bodies() {
    let (mut doc, body, _, o1) = design();
    transfer(&mut doc, body, "Bracket").unwrap();
    let (copy, occurrence) = doc.paste_new(o1, Some(shift(0.0, 40.0, 0.0))).unwrap();
    assert!(
        doc.features()
            .all(|f| doc.status(f.uid).is_some_and(FeatureStatus::is_ok))
    );
    let copied: Vec<BodyUid> = doc
        .instances()
        .into_iter()
        .filter(|i| i.path.first() == Some(&occurrence))
        .map(|i| i.body)
        .collect();
    assert_eq!(copied.len(), 1);
    assert_eq!(doc.body_component(copied[0]), Some(copy));
    assert_ne!(copied[0], body);
}
