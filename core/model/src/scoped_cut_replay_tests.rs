// SPDX-License-Identifier: MIT
//! Scoped Cut tools repeat locally without losing their target selection.

use serde_json::{Value, json};

use crate::document_tests::{def, num};
use crate::features::SketchPlane;
use crate::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use crate::testing::MockKernel;
use crate::{Document, FeatureStatus, Transform};

fn body(doc: &mut Document<MockKernel>) -> BodyUid {
    let uid = doc
        .add_feature(
            &def(json!({"type":"box", "plane":"xy",
        "length":50, "width":50, "height":20, "operation":"new_body"})),
            None,
        )
        .unwrap()
        .uid;
    BodyUid::new(uid, 0)
}

fn scope() -> Value {
    json!({"context":{"component":"C0", "occurrence":""},
        "targets":{"type":"automatic", "excluded":[], "excluded_bodies":[]}})
}

fn cut(doc: &mut Document<MockKernel>, scope: Option<Value>) -> FeatureUid {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_circle(sketch, [15.0, 25.0], &num(10.0))
        .unwrap()
        .region;
    let mut value = json!({"type":"extrude", "profiles":[{"sketch":sketch,"region":region}],
        "extent":{"type":"distance","distance":6},"operation":"cut"});
    if let Some(scope) = scope {
        value["cut_scope"] = scope;
    }
    doc.add_feature(&def(value), None).unwrap().uid
}

fn mirror(cut: FeatureUid, compute: &str) -> Value {
    json!({"type":"mirror", "objects":{"type":"features","features":[cut]},
        "plane":{"type":"fixed","origin":[25,0,0],"normal":[1,0,0]},"compute":compute})
}

fn history(doc: &Document<MockKernel>, body: BodyUid) -> String {
    doc.body_shape(body).unwrap().history.clone()
}

fn failed_scope(doc: &Document<MockKernel>, uid: FeatureUid) {
    assert!(
        matches!(doc.status(uid), Some(FeatureStatus::Failed(message))
        if message.contains("unsupported cut scope")),
        "{:?}",
        doc.status(uid)
    );
}

#[test]
fn scoped_cut_mirror_preview_edit_undo_and_reopen_match_legacy() {
    for scoped in [false, true] {
        for compute in ["adjust", "identical"] {
            let mut doc = Document::new(MockKernel::default());
            let target = body(&mut doc);
            let original = cut(&mut doc, scoped.then(scope));
            let before = history(&doc, target);
            let definition = def(mirror(original, compute));
            assert_eq!(
                doc.preview_add(&definition, None).unwrap().status,
                FeatureStatus::Ok
            );
            assert_eq!(
                history(&doc, target),
                before,
                "preview must not mutate the cut"
            );
            let repeated = doc.add_feature(&definition, None).unwrap().uid;
            assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
            assert!(history(&doc, target).starts_with("cut(cut("));
            assert_eq!(doc.body_component(target), Some(ComponentUid::ROOT));
            let mirrored = history(&doc, target);
            doc.undo();
            assert_eq!(history(&doc, target), before);
            doc.redo();
            assert_eq!(history(&doc, target), mirrored);
            // d1 is the box's length: an upstream dimension still invalidates
            // and recomputes both the source cut and its reflected tool.
            doc.set_parameter("d1", 55.0).unwrap();
            assert_eq!(doc.status(original), Some(&FeatureStatus::Ok));
            assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
            let saved = doc.to_json();
            let mut reopened = Document::from_json(&saved, MockKernel::default()).unwrap();
            reopened.recompute();
            assert_eq!(reopened.status(repeated), Some(&FeatureStatus::Ok));
            assert_eq!(history(&reopened, target), history(&doc, target));
            assert_eq!(reopened.body_component(target), Some(ComponentUid::ROOT));
            assert_eq!(reopened.to_json(), saved);
        }
    }
}

#[test]
fn scoped_cut_pattern_respects_explicit_component_and_body_exclusion() {
    let mut doc = Document::new(MockKernel::default());
    let target = body(&mut doc);
    let excluded = body(&mut doc);
    let untouched = history(&doc, excluded);
    let mut selection = scope();
    selection["targets"] = json!({"type":"explicit", "included":[selection["context"].clone()],
        "excluded_bodies":[{"component":"C0", "occurrence":"", "body":excluded}]});
    let original = cut(&mut doc, Some(selection));
    let repeated = doc
        .add_feature(
            &def(json!({"type":"rectangular_pattern",
        "objects":{"type":"features","features":[original]},
        "direction1":{"axis":{"type":"origin","axis":"x"}, "quantity":2,"distance":20},
        "distance_type":"spacing"})),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
    assert!(history(&doc, target).starts_with("cut(cut("));
    assert_eq!(history(&doc, excluded), untouched);
    assert_eq!(doc.body_component(target), Some(ComponentUid::ROOT));
    assert_eq!(doc.body_component(excluded), Some(ComponentUid::ROOT));
}

#[test]
fn scoped_cut_replay_empty_selection_never_becomes_all_bodies() {
    for targets in [
        json!({"type":"explicit","included":[]}),
        json!({"type":"automatic","excluded":[{"component":"C0","occurrence":""}]}),
        json!({"type":"automatic","excluded_bodies":[{"component":"C0","occurrence":"","body":"F1.b0"}]}),
    ] {
        let mut doc = Document::new(MockKernel::default());
        let target = body(&mut doc);
        let untouched = history(&doc, target);
        let mut selection = scope();
        selection["targets"] = targets;
        let original = cut(&mut doc, Some(selection));
        let repeated = doc
            .add_feature(&def(mirror(original, "adjust")), None)
            .unwrap()
            .uid;
        assert!(matches!(
            doc.status(repeated),
            Some(FeatureStatus::Failed(_))
        ));
        assert_eq!(history(&doc, target), untouched);
    }
}

#[test]
fn scoped_cut_replay_rejects_foreign_targets_but_keeps_deliberate_exclusions() {
    for excluded in [false, true] {
        let mut doc = Document::new(MockKernel::default());
        let local = body(&mut doc);
        doc.create_component(Some("Other"), Transform::IDENTITY, true)
            .unwrap();
        let foreign = body(&mut doc);
        doc.activate_component(ComponentUid::ROOT).unwrap();
        let mut selection = scope();
        if excluded {
            selection["targets"]["excluded"] = json!([{"component":"C1","occurrence":"O1"}]);
        }
        let original = cut(&mut doc, Some(selection));
        let foreign_before = history(&doc, foreign);
        let repeated = doc
            .add_feature(&def(mirror(original, "adjust")), None)
            .unwrap()
            .uid;
        if excluded {
            assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
            assert!(history(&doc, local).starts_with("cut(cut("));
        } else {
            failed_scope(&doc, repeated);
            // A source selection edit repairs the cached replay without
            // changing the mirror or its local tool geometry.
            doc.edit_feature(original, &def(json!({"type":"extrude",
                "profiles":[{"sketch":FeatureUid(original.0 - 1),"region":"r{c1}"}],
                "extent":{"type":"distance","distance":6},"operation":"cut",
                "cut_scope":{"context":{"component":"C0","occurrence":""},
                    "targets":{"type":"explicit","included":[{"component":"C0","occurrence":""}]}}
            }))).unwrap();
            assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
            doc.undo();
            failed_scope(&doc, repeated);
        }
        assert_eq!(history(&doc, foreign), foreign_before);
        assert_eq!(doc.body_component(foreign), Some(ComponentUid(1)));
    }
}

#[test]
fn scoped_cut_replay_checks_explicit_foreign_scope_before_generic_tool_error() {
    let mut doc = Document::new(MockKernel::default());
    body(&mut doc);
    doc.create_component(Some("Other"), Transform::IDENTITY, true)
        .unwrap();
    body(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let mut selection = scope();
    selection["targets"] =
        json!({"type":"explicit","included":[{"component":"C1","occurrence":"O1"}]});
    let original = cut(&mut doc, Some(selection));
    let error = doc
        .add_feature(&def(mirror(original, "adjust")), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unsupported cut scope"), "{error}");
    assert!(!error.contains("no tool body"), "{error}");
}

#[test]
fn scoped_cut_replay_assembly_change_invalidates_cached_local_classification() {
    let mut doc = Document::new(MockKernel::default());
    let local = body(&mut doc);
    let original = cut(&mut doc, Some(scope()));
    let repeated = doc
        .add_feature(&def(mirror(original, "adjust")), None)
        .unwrap()
        .uid;
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
    doc.create_component(Some("Other"), Transform::IDENTITY, true)
        .unwrap();
    let foreign = body(&mut doc);
    // The new body is later than the mirror and must not affect its scope.
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
    // The same cached replay now sees a newly available eligible target.
    doc.move_feature(foreign.feature, 1).unwrap();
    failed_scope(&doc, repeated);
    assert_eq!(doc.body_component(local), Some(ComponentUid::ROOT));
    assert_eq!(doc.body_component(foreign), Some(ComponentUid(1)));
    doc.undo();
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
}

#[test]
fn scoped_cut_replay_keeps_own_shared_definition_and_skips_foreign_one() {
    let mut doc = Document::new(MockKernel::default());
    doc.create_component(
        Some("Part"),
        Transform::translation([100.0, 0.0, 0.0]),
        true,
    )
    .unwrap();
    let target = body(&mut doc);
    let selection = json!({"context":{"component":"C1","occurrence":"O1"},
        "targets":{"type":"explicit","included":[{"component":"C1","occurrence":"O1"}]}});
    let original = cut(&mut doc, Some(selection));
    let repeated = doc
        .add_feature(&def(mirror(original, "adjust")), None)
        .unwrap()
        .uid;
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
    assert!(history(&doc, target).starts_with("cut(cut("));
    assert_eq!(doc.body_component(target), Some(ComponentUid(1)));
    let replayed = history(&doc, target);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    doc.copy_occurrence(
        OccurrenceUid(1),
        Some(Transform::translation([200.0, 0.0, 0.0])),
    )
    .unwrap();
    // A cut inside Part edits Part's definition, which both occurrences
    // show (mitcad#769): a second occurrence refuses neither the cut nor
    // its mirrored replay, and the body keeps both cuts.
    assert_eq!(doc.status(original), Some(&FeatureStatus::Ok));
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
    assert_eq!(history(&doc, target), replayed);
    assert_eq!(doc.body_component(target), Some(ComponentUid(1)));
    doc.undo();
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));

    // A cut from the root, whose tool also reaches one occurrence of a
    // shared Part, stays local on replay: Part's shared definition is
    // refused and left uncut, the root's own body is cut by both.
    let mut doc = Document::new(MockKernel::default());
    doc.create_component(Some("Part"), Transform::IDENTITY, true)
        .unwrap();
    let shared = body(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let root = body(&mut doc);
    doc.copy_occurrence(
        OccurrenceUid(1),
        Some(Transform::translation([200.0, 0.0, 0.0])),
    )
    .unwrap();
    let untouched = history(&doc, shared);
    let original = cut(&mut doc, Some(scope()));
    let repeated = doc
        .add_feature(&def(mirror(original, "adjust")), None)
        .unwrap()
        .uid;
    assert_eq!(doc.status(original), Some(&FeatureStatus::Ok));
    assert_eq!(doc.status(repeated), Some(&FeatureStatus::Ok));
    assert!(history(&doc, root).starts_with("cut(cut("));
    assert_eq!(history(&doc, shared), untouched);
}
