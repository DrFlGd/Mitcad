// SPDX-License-Identifier: MIT
//! `add_feature` at a timeline position (mitcad#403): `after` a feature or
//! at an `index`, in one undo step and one recompute; the marker stays
//! after the feature it was after, and a refused insert changes nothing.

use serde_json::{Value, json};

use crate::Document;
use crate::testing::MockKernel;

const REGION: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

fn rejected(doc: &mut Document<MockKernel>, command: Value) -> String {
    doc.command(&command.to_string())
        .map(|r| panic!("{command} must fail, got {r}"))
        .unwrap_err()
        .to_string()
}

fn timeline(doc: &Document<MockKernel>) -> (Vec<String>, usize) {
    let timeline: Value =
        serde_json::from_str(&doc.query(r#"{"query": "timeline"}"#).unwrap()).unwrap();
    let uids = timeline["features"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["uid"].as_str().unwrap().to_owned())
        .collect();
    (uids, timeline["marker"].as_u64().unwrap() as usize)
}

/// Sketch1 (F1) with a rectangle, Extrude1 (F2) of it, and Sketch2 (F3)
/// with a circle on top.
fn part() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    command(&mut doc, json!({"cmd": "sketch.create"}));
    command(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    command(&mut doc, extrude("F1", REGION, 20.0));
    command(&mut doc, json!({"cmd": "sketch.create"}));
    command(
        &mut doc,
        json!({"cmd": "sketch.add_circle", "sketch": "F3", "center": [30, 20], "diameter": 10}),
    );
    doc
}

fn extrude(sketch: &str, region: &str, distance: f64) -> Value {
    json!({"cmd": "add_feature", "def": {"type": "extrude",
           "profiles": [{"sketch": sketch, "region": region}],
           "extent": {"type": "distance", "distance": distance}, "operation": "new_body"}})
}

#[test]
fn a_feature_is_inserted_after_another_in_one_step() {
    let mut doc = part();
    let (depth, recomputes) = (doc.undo_depth(), doc.recompute_count());
    let mut insert = extrude("F1", REGION, 5.0);
    insert["after"] = json!("F1");
    let added = command(&mut doc, insert);
    assert_eq!(
        (&added["uid"], &added["name"]),
        (&json!("F4"), &json!("Extrude2"))
    );
    assert_eq!(
        timeline(&doc),
        (vec!["F1".into(), "F4".into(), "F2".into(), "F3".into()], 4)
    );
    // One undo step and one recompute, of the inserted feature and those
    // after it only: Sketch1 comes from the cache.
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.undo_label(), Some("Add Extrude2"));
    assert_eq!(doc.recompute_count(), recomputes + 1);
    let evaluated = added["recomputed"].as_u64().unwrap();
    assert!((1..=3).contains(&evaluated), "{added}");
    assert!(!doc.stats().evaluated.iter().any(|f| f.to_string() == "F1"));
    assert!(added.get("failed_features").is_none(), "{added}");
    // The new body is there with the others.
    let bodies: Value =
        serde_json::from_str(&doc.query(r#"{"query": "bodies"}"#).unwrap()).unwrap();
    assert_eq!(bodies.as_array().unwrap().len(), 2, "{bodies}");

    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(
        timeline(&doc),
        (vec!["F1".into(), "F2".into(), "F3".into()], 3)
    );
}

#[test]
fn an_index_inserts_before_a_rolled_back_marker_and_keeps_it() {
    let mut doc = part();
    command(&mut doc, json!({"cmd": "set_marker", "position": 2}));
    let mut insert = extrude("F1", REGION, 5.0);
    insert["index"] = json!(1);
    command(&mut doc, insert);
    // The marker stays after Extrude1; Sketch2 stays rolled back.
    assert_eq!(
        timeline(&doc),
        (vec!["F1".into(), "F4".into(), "F2".into(), "F3".into()], 3)
    );
    // At the marker an index is the ordinary add_feature.
    let mut at_marker = extrude("F1", REGION, 8.0);
    at_marker["index"] = json!(3);
    command(&mut doc, at_marker);
    assert_eq!(
        timeline(&doc),
        (
            vec![
                "F1".into(),
                "F4".into(),
                "F2".into(),
                "F5".into(),
                "F3".into()
            ],
            4
        )
    );
}

#[test]
fn a_refused_insert_leaves_the_document_as_it_was() {
    let mut doc = part();
    command(&mut doc, json!({"cmd": "set_marker", "position": 2}));
    let before = doc.to_json();
    let (depth, revision) = (doc.undo_depth(), doc.revision());
    let mut late = extrude("F1", REGION, 5.0);
    late["index"] = json!(3);
    assert!(rejected(&mut doc, late).contains("after the timeline marker"));
    let mut both = extrude("F1", REGION, 5.0);
    both["after"] = json!("F1");
    both["index"] = json!(1);
    assert!(rejected(&mut doc, both).contains("either after or index"));
    let mut missing = extrude("F1", REGION, 5.0);
    missing["after"] = json!("F9");
    assert!(rejected(&mut doc, missing).contains("F9 does not exist"));
    // Before the sketch it uses: refused by the model's own checks.
    let mut early = extrude("F3", "r{c1}", 5.0);
    early["after"] = json!("F1");
    rejected(&mut doc, early);
    assert_eq!(doc.to_json(), before);
    assert_eq!((doc.undo_depth(), doc.revision()), (depth, revision));
    assert_eq!(timeline(&doc).1, 2);
}

#[test]
fn an_inserted_feature_that_fails_is_the_commands_failure() {
    let mut doc = part();
    doc.kernel().fail.borrow_mut().insert("extrude");
    let mut insert = extrude("F1", REGION, 5.0);
    insert["after"] = json!("F2");
    let added = command(&mut doc, insert);
    assert_eq!(
        added["failed_features"],
        json!([{"uid": "F4", "name": "Extrude2", "error": added["failed_features"][0]["error"],
                "edited": true, "new": true}])
    );
    assert_eq!(
        timeline(&doc),
        (vec!["F1".into(), "F2".into(), "F4".into(), "F3".into()], 4)
    );
}
