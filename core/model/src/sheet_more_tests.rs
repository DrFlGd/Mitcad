// SPDX-License-Identifier: MIT
//! Contour Flange, Convert to Sheet Metal and Perforate (mitcad#461) on the
//! mock kernel: definitions, references, refusals and the planar cases. The
//! bent geometry is checked with OCCT by the CLI fixtures `sheet_contour`,
//! `sheet_convert` and `sheet_perforate`.

use crate::document_tests::def;
use crate::features::SketchPlane;
use crate::ids::{BodyUid, FeatureUid};
use crate::testing::MockKernel;
use crate::{Document, FeatureStatus};
use serde_json::{Value, json};

fn rules() -> Value {
    json!({"type":"sheet_rules","thickness":2,"bend_radius":3,
        "allowance":{"type":"k_factor","k":0.3},"reference_side":"front",
        "bend_relief":{"type":"rectangular","width":2,"depth":3},
        "corner_relief":{"type":"round","width":2,"depth":3}})
}

fn failed(doc: &Document<MockKernel>, uid: FeatureUid, part: &str) -> bool {
    matches!(doc.status(uid), Some(FeatureStatus::Failed(m)) if m.contains(part))
}

/// Rules and a sketch with the chain (0,0)-(40,0)-(40,30)-(70,30) as
/// `c1`, `c4`, `c6`.
fn chain() -> (Document<MockKernel>, FeatureUid, FeatureUid) {
    let mut doc = Document::new(MockKernel::default());
    let rules = doc.add_feature(&def(rules()), None).unwrap().uid;
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    for (start, end) in [
        (json!([0, 0]), json!([40, 0])),
        (json!("p3"), json!([40, 30])),
        (json!("p5"), json!([70, 30])),
    ] {
        doc.command(
            &json!({"cmd":"sketch.add_line","sketch":sketch,"start":start,"end":end}).to_string(),
        )
        .unwrap();
    }
    (doc, rules, sketch)
}

fn contour(rules: FeatureUid, sketch: FeatureUid, curves: &[&str]) -> Value {
    json!({"type":"sheet_contour","rules":rules,"sketch":sketch,"curves":curves,"width":50})
}

#[test]
fn contour_of_one_line_is_one_panel_on_the_chosen_side() {
    let (mut doc, rules, sketch) = chain();
    let uid = doc
        .add_feature(&def(contour(rules, sketch, &["c1"])), None)
        .unwrap()
        .uid;
    assert_eq!(doc.status(uid), Some(&FeatureStatus::Ok));
    let body = BodyUid::new(uid, 0);
    let bounds = doc.body_shape(body).unwrap().bounds.unwrap();
    // Right of travel along +x is -y; the width runs along +z.
    assert_eq!(bounds.min, [0.0, -2.0, 0.0]);
    assert_eq!(bounds.max, [40.0, 0.0, 50.0]);
    let state: Value = serde_json::from_str(
        &doc.query(&json!({"query":"sheet_state","body":body}).to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state["status"], "valid");
    assert_eq!(state["charts"].as_array().unwrap().len(), 1);
    let mut left = contour(rules, sketch, &["c1"]);
    left["side"] = json!("left");
    left["reverse"] = json!(true);
    doc.edit_feature(uid, &def(left)).unwrap();
    let bounds = doc.body_shape(body).unwrap().bounds.unwrap();
    assert_eq!(bounds.min, [0.0, 0.0, -50.0]);
    assert_eq!(bounds.max, [40.0, 2.0, 0.0]);
    // The rules and the sketch are references of the feature.
    let refs = doc.feature(uid).unwrap().def.info().references().features;
    assert!(refs.contains(&rules) && refs.contains(&sketch));
}

#[test]
fn contour_refuses_open_joints_short_lines_and_bad_values() {
    let (mut doc, rules, sketch) = chain();
    let uid = doc
        .add_feature(&def(contour(rules, sketch, &["c1", "c6"])), None)
        .unwrap()
        .uid;
    assert!(failed(&doc, uid, "not joined end to end"));
    let mut big = contour(rules, sketch, &["c1", "c4", "c6"]);
    big["radius"] = json!(20);
    doc.edit_feature(uid, &def(big)).unwrap();
    assert!(failed(
        &doc,
        uid,
        "is not longer than the setbacks of its bends"
    ));
    let mut zero = contour(rules, sketch, &["c1"]);
    zero["width"] = json!(0);
    assert!(doc.edit_feature(uid, &def(zero)).is_err());
    let twice = contour(rules, sketch, &["c1", "c1"]);
    assert!(doc.edit_feature(uid, &def(twice)).is_err());
    let not_rules = contour(sketch, sketch, &["c1"]);
    assert!(doc.edit_feature(uid, &def(not_rules)).is_err());
}

#[test]
fn contour_with_inline_rules_adds_them_in_one_undo_step() {
    let (mut doc, _, sketch) = chain();
    let count = doc.features().count();
    let answer: Value = serde_json::from_str(
        &doc.command(
            &json!({"cmd":"add_feature","def":{"type":"sheet_contour","rules":rules(),
                "sketch":sketch,"curves":["c1"],"width":50}})
            .to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(answer["rules"].is_string());
    assert_eq!(doc.features().count(), count + 2);
    doc.undo();
    assert_eq!(doc.features().count(), count);
}

#[test]
fn perforation_sizes_are_checked_and_the_body_is_its_footprint() {
    let (mut doc, rules, sketch) = chain();
    let uid = doc
        .add_feature(&def(contour(rules, sketch, &["c1"])), None)
        .unwrap()
        .uid;
    let body = BodyUid::new(uid, 0);
    for (diameter, pitch, margin) in [(0.0, 5.0, 1.0), (5.0, 5.0, 1.0), (2.0, 5.0, -1.0)] {
        let perforate = json!({"type":"sheet_perforate","body":body,"diameter":diameter,
            "pitch":pitch,"margin":margin});
        assert!(doc.add_feature(&def(perforate), None).is_err());
    }
    let perforate = json!({"type":"sheet_perforate","body":body,"diameter":2,"pitch":5,
        "pattern":"square","stationary":{"feature":uid,"slot":0}});
    let field = doc.add_feature(&def(perforate), None).unwrap().uid;
    let refs = doc.feature(field).unwrap().def.info().references();
    assert!(refs.features.contains(&uid));
    // The mock kernel cannot trim domains: the failure blocks the sheet.
    assert!(matches!(doc.status(field), Some(FeatureStatus::Failed(_))));
    let state: Value = serde_json::from_str(
        &doc.query(&json!({"query":"sheet_state","body":body}).to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state["status"], "blocked");
}

#[test]
fn conversion_refuses_repeated_bends_and_other_rules_and_names_its_references() {
    let mut doc = Document::new(MockKernel::default());
    let rules = doc.add_feature(&def(rules()), None).unwrap().uid;
    let block = doc
        .add_feature(
            &def(
                json!({"type":"box","plane":"xy","corner":[0,0],"length":40,"width":30,
                "height":20,"operation":"new_body"}),
            ),
            None,
        )
        .unwrap()
        .uid;
    let body = BodyUid::new(block, 0);
    let edge = format!("E{{{block}:bottom|{block}:side1}}");
    let twice = json!({"type":"sheet_convert","rules":rules,"body":body,
        "face":format!("{block}:bottom"),"bends":[edge, edge]});
    assert!(doc.add_feature(&def(twice), None).is_err());
    let not_rules = json!({"type":"sheet_convert","rules":block,"body":body,
        "face":format!("{block}:bottom")});
    assert!(doc.add_feature(&def(not_rules), None).is_err());
    let convert = json!({"type":"sheet_convert","rules":rules,"body":body,
        "face":format!("{block}:bottom"),"bends":[edge],"radius":1});
    let uid = doc.add_feature(&def(convert), None).unwrap().uid;
    let refs = doc.feature(uid).unwrap().def.info().references();
    assert!(refs.features.contains(&rules) && refs.features.contains(&block));
    // The mock kernel reads no face geometry: the body stays an ordinary
    // solid with the failure named.
    assert!(matches!(doc.status(uid), Some(FeatureStatus::Failed(_))));
    assert!(doc.body_shape(body).is_some());
}
