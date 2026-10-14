// SPDX-License-Identifier: MIT
//! Extrude's automatic operation (mitcad#289), with the mock kernel, whose
//! contact test works on bounding boxes: a preview asked for it reports
//! Join for a tool on a body's face extruded away from it, Cut for one
//! extruded into it and New Body for one that touches no visible body of
//! its component; bodies whose boxes the tool's does not reach never get
//! the kernel's contact test.

use serde_json::{Value, json};

use crate::document_tests::{extrude, num};
use crate::features::SketchPlane;
use crate::ids::{BodyUid, FeatureUid};
use crate::testing::MockKernel;
use crate::{Document, RegionKey};

/// A 10 x 10 block 20 deep under the XY plane: its top face lies in XY.
fn block(doc: &mut Document<MockKernel>) -> BodyUid {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let uid = doc
        .add_feature(&extrude(sketch, &[&region], -20.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    BodyUid::new(uid, 0)
}

/// A 6 x 6 square on XY at `corner`, not used yet.
fn square(doc: &mut Document<MockKernel>, corner: [f64; 2]) -> (FeatureUid, RegionKey) {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, corner, &num(6.0), &num(6.0))
        .unwrap()
        .region;
    (sketch, region)
}

/// The preview of extruding the square `distance` with `operation`, the
/// automatic operation asked for or not.
fn preview(
    doc: &mut Document<MockKernel>,
    (sketch, region): &(FeatureUid, RegionKey),
    distance: f64,
    operation: &str,
    auto: bool,
) -> Value {
    let def = extrude(*sketch, &[region], distance, operation, &[]);
    let mut command = json!({"cmd": "add_feature", "def": def});
    if auto {
        command["auto_operation"] = json!(true);
    }
    let report: Value = serde_json::from_str(&doc.preview(&command.to_string()).unwrap()).unwrap();
    assert_eq!(report["status"], "ok", "{report}");
    report
}

#[test]
fn a_profile_on_a_face_joins_going_out_and_cuts_going_in() {
    let mut doc = Document::new(MockKernel::default());
    let body = block(&mut doc);
    let square = square(&mut doc, [2.0, 2.0]);

    let out = preview(&mut doc, &square, 5.0, "new_body", true);
    assert_eq!(
        out["auto_operation"],
        json!({"operation": "join", "objects": [body.to_string()]}),
        "{out}"
    );
    let into = preview(&mut doc, &square, -5.0, "new_body", true);
    assert_eq!(
        into["auto_operation"],
        json!({"operation": "cut", "objects": [body.to_string()]}),
        "{into}"
    );
    // Back out again: Join.
    let again = preview(&mut doc, &square, 3.0, "cut", true);
    assert_eq!(again["auto_operation"]["operation"], "join", "{again}");
}

#[test]
fn the_report_is_the_definitions_own() {
    let mut doc = Document::new(MockKernel::default());
    let body = block(&mut doc);
    let square = square(&mut doc, [2.0, 2.0]);
    // Asked as a cut while the tool goes out: the cut is what is previewed
    // (the body changed), and the choice says Join.
    let report = preview(&mut doc, &square, 5.0, "cut", true);
    assert_eq!(report["auto_operation"]["operation"], "join", "{report}");
    let changed = report["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["uid"] == body.to_string())
        .map(|b| b["changed"].clone());
    assert_eq!(changed, Some(json!(true)), "{report}");
    // As a new body the block stays as it is.
    let plain = preview(&mut doc, &square, 5.0, "new_body", true);
    let changed = plain["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["uid"] == body.to_string())
        .map(|b| b["changed"].clone());
    assert_eq!(changed, Some(json!(false)), "{plain}");
}

#[test]
fn a_profile_away_from_bodies_is_a_new_body_without_contact_tests() {
    let mut doc = Document::new(MockKernel::default());
    block(&mut doc);
    let square = square(&mut doc, [30.0, 30.0]);
    let report = preview(&mut doc, &square, -5.0, "new_body", true);
    assert_eq!(
        report["auto_operation"],
        json!({"operation": "new_body", "objects": []}),
        "{report}"
    );
    // The boxes are apart: the kernel's contact test is not asked.
    assert_eq!(doc.kernel().count("tool_contacts"), 0);
    // An empty design: New Body.
    let mut empty = Document::new(MockKernel::default());
    let square = self::square(&mut empty, [0.0, 0.0]);
    let report = preview(&mut empty, &square, 5.0, "new_body", true);
    assert_eq!(
        report["auto_operation"]["operation"], "new_body",
        "{report}"
    );
}

#[test]
fn hidden_bodies_and_other_components_bodies_are_not_touched() {
    let mut doc = Document::new(MockKernel::default());
    let body = block(&mut doc);
    let square = square(&mut doc, [2.0, 2.0]);
    doc.set_body_visible(body, false).unwrap();
    let report = preview(&mut doc, &square, -5.0, "new_body", true);
    assert_eq!(
        report["auto_operation"]["operation"], "new_body",
        "{report}"
    );
    doc.set_body_visible(body, true).unwrap();

    // A component of its own, placed where the root's block is: its
    // features work on its own bodies only.
    doc.create_component(
        Some("Part"),
        crate::transform::Transform::translation([0.0, 0.0, 0.0]),
        true,
    )
    .unwrap();
    let inner = self::square(&mut doc, [2.0, 2.0]);
    let report = preview(&mut doc, &inner, -5.0, "new_body", true);
    assert_eq!(
        report["auto_operation"]["operation"], "new_body",
        "{report}"
    );
}

#[test]
fn without_the_request_nothing_is_chosen() {
    let mut doc = Document::new(MockKernel::default());
    block(&mut doc);
    let square = square(&mut doc, [2.0, 2.0]);
    let report = preview(&mut doc, &square, 5.0, "new_body", false);
    assert!(report.get("auto_operation").is_none(), "{report}");
    assert_eq!(doc.kernel().count("tool_contacts"), 0);
}
