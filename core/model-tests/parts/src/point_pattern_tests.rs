// SPDX-License-Identifier: MIT
//! Pattern at Points: anchors, reference updates and shared occurrence lifetime.

use mitcad_model::ids::{BodyUid, ComponentUid, FeatureUid};
use mitcad_model::test_support::def;
use mitcad_model::testing::MockKernel;
use mitcad_model::transform::Transform;
use mitcad_model::{Document, FeatureStatus};
use mitcad_model_api::DocumentApi;
use serde_json::{Value, json};

fn command(doc: &mut Document<MockKernel>, value: Value) -> Value {
    serde_json::from_str(
        &doc.command(&value.to_string())
            .unwrap_or_else(|e| panic!("{value}: {e}")),
    )
    .unwrap()
}
fn preview(doc: &mut Document<MockKernel>, value: Value) -> Value {
    serde_json::from_str(&doc.preview(&value.to_string()).unwrap()).unwrap()
}
fn box_def(x: f64, height: f64) -> Value {
    json!({"type":"box", "plane":"xy", "corner":[x,0], "length":10,
        "width":10, "height":height, "operation":"new_body"})
}
fn box_body(doc: &mut Document<MockKernel>, x: f64) -> BodyUid {
    BodyUid::new(
        doc.add_feature(&def(box_def(x, 10.0)), None).unwrap().uid,
        0,
    )
}
fn target(point: [f64; 3]) -> Value {
    json!({"point":{"point":point}})
}
fn pattern(objects: Value, targets: Value) -> Value {
    json!({"type":"pattern_at_points", "objects":objects, "targets":targets})
}
fn add(definition: Value) -> Value {
    json!({"cmd":"add_feature", "def":definition})
}
fn bounds(doc: &Document<MockKernel>, body: BodyUid) -> ([f64; 3], [f64; 3]) {
    let bounds = doc.body_shape(body).unwrap().bounds.unwrap();
    (bounds.min, bounds.max)
}

#[test]
fn point_pattern_bodies_use_one_group_anchor_and_preserve_originals() {
    let mut doc = Document::new(MockKernel::default());
    let a = box_body(&mut doc, 0.0);
    let b = box_body(&mut doc, 20.0);
    let definition = pattern(
        json!({"type":"bodies", "bodies":[a,b]}),
        json!([target([100.0, 50.0, 5.0])]),
    );
    let result = preview(&mut doc, add(definition.clone()));
    assert_eq!(result["anchor"]["point"], json!([15.0, 5.0, 5.0]));
    assert_eq!(result["elements"].as_array().unwrap().len(), 2);
    assert_eq!(doc.bodies().len(), 2, "preview is read only");
    let uid = doc.add_feature(&def(definition), None).unwrap().uid;
    assert_eq!(
        doc.bodies().len(),
        4,
        "one copy of each source, no extra identity copy"
    );
    assert_eq!(bounds(&doc, a), ([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]));
    assert_eq!(
        bounds(&doc, BodyUid::new(uid, 0)),
        ([85.0, 45.0, 0.0], [95.0, 55.0, 10.0])
    );
    assert_eq!(
        bounds(&doc, BodyUid::new(uid, 1)),
        ([105.0, 45.0, 0.0], [115.0, 55.0, 10.0])
    );
    let reversed = pattern(
        json!({"type":"bodies", "bodies":[b,a]}),
        json!([target([100.0, 50.0, 5.0])]),
    );
    let result = preview(
        &mut doc,
        json!({"cmd":"edit_feature","uid":uid,"def":reversed}),
    );
    assert_eq!(result["anchor"]["point"], json!([15.0, 5.0, 5.0]));
}

#[test]
fn point_pattern_sketch_targets_follow_edits_undo_and_reopen() {
    let mut doc = Document::new(MockKernel::default());
    let body = box_body(&mut doc, 0.0);
    command(&mut doc, json!({"cmd":"sketch.create"}));
    command(
        &mut doc,
        json!({"cmd":"sketch.add_point","sketch":"F2","at":[30,20]}),
    );
    let definition = pattern(
        json!({"type":"bodies","bodies":[body]}),
        json!([
        {"point":{"sketch":"F2","point":"p1"}}]),
    );
    let uid = doc.add_feature(&def(definition), None).unwrap().uid;
    let copy = BodyUid::new(uid, 0);
    assert_eq!(bounds(&doc, copy).0, [25.0, 15.0, -5.0]);
    command(
        &mut doc,
        json!({"cmd":"sketch.move","sketch":"F2","entities":["p1"],"by":[7,9]}),
    );
    assert_eq!(bounds(&doc, copy).0, [32.0, 24.0, -5.0]);
    command(&mut doc, json!({"cmd":"undo"}));
    assert_eq!(bounds(&doc, copy).0, [25.0, 15.0, -5.0]);
    command(&mut doc, json!({"cmd":"redo"}));
    let mut reopened = Document::from_json(&doc.to_json(), MockKernel::default()).unwrap();
    reopened.recompute();
    assert_eq!(bounds(&reopened, copy).0, [32.0, 24.0, -5.0]);
}

#[test]
fn point_pattern_hole_anchor_is_placement_not_depth_midpoint_with_finishing() {
    let mut block = mitcad_model::test_support::block();
    let face = format!("{}:end({})", block.extrude, block.region);
    let hole = |depth| {
        json!({"type":"hole", "placement":{"type":"face","body":block.body,
        "face":face,"points":[[10,10,20]]}, "diameter":4,"flat":true,
        "extent":{"type":"distance","depth":depth}})
    };
    let uid = block.doc.add_feature(&def(hole(6)), None).unwrap().uid;
    let edge = format!("E{{{uid}:hole0.tip|{uid}:hole0.wall}}");
    let finish = block
        .doc
        .add_feature(
            &def(json!({"type":"fillet","body":block.body,
        "edges":[edge],"radius":0.5})),
            None,
        )
        .unwrap()
        .uid;
    let definition = pattern(
        json!({"type":"features","features":[uid,finish]}),
        json!([target([40.0, 20.0, 20.0])]),
    );
    let result = preview(&mut block.doc, add(definition.clone()));
    assert_eq!(
        result["anchor"]["point"],
        json!([10.0, 10.0, 20.0]),
        "{result}"
    );
    assert_eq!(result["anchor"]["policy"], "hole_placements");
    block.doc.edit_feature(uid, &def(hole(12))).unwrap();
    let result = preview(&mut block.doc, add(definition));
    assert_eq!(
        result["anchor"]["point"],
        json!([10.0, 10.0, 20.0]),
        "{result}"
    );
    assert_eq!(
        result["elements"][1][2][3], 0.0,
        "no axial drift when depth changes"
    );
}

#[test]
fn point_pattern_duplicate_and_coincident_points_are_explicit_errors() {
    let mut doc = Document::new(MockKernel::default());
    let body = box_body(&mut doc, 0.0);
    let objects = json!({"type":"bodies","bodies":[body]});
    let p = target([30.0, 0.0, 0.0]);
    let duplicate = add(pattern(objects.clone(), json!([p, p])));
    assert!(
        doc.command(&duplicate.to_string())
            .unwrap_err()
            .to_string()
            .contains("more than once")
    );
    let same_position = pattern(
        objects.clone(),
        json!([
        target([30.0,0.0,0.0]), {"point":{"point":[30,0,0]},"occurrence":""}]),
    );
    let result = preview(&mut doc, add(same_position));
    assert_eq!(result["status"], "error");
    assert!(result["error"].as_str().unwrap().contains("coincides"));
    let result = preview(
        &mut doc,
        add(pattern(objects, json!([target([5.0, 5.0, 5.0])]))),
    );
    assert_eq!(result["status"], "error");
    assert_eq!(doc.bodies().len(), 1);
}

#[test]
fn point_pattern_anchor_override_and_automatic_restore() {
    let mut doc = Document::new(MockKernel::default());
    let body = box_body(&mut doc, 0.0);
    let mut definition = pattern(
        json!({"type":"bodies","bodies":[body]}),
        json!([target([30.0, 0.0, 0.0])]),
    );
    definition["anchor"] = json!({"point":"origin"});
    let result = preview(&mut doc, add(definition.clone()));
    assert_eq!(result["anchor"]["point"], json!([0.0, 0.0, 0.0]));
    assert_eq!(result["anchor"]["mode"], "selected");
    definition.as_object_mut().unwrap().remove("anchor");
    let result = preview(&mut doc, add(definition));
    assert_eq!(result["anchor"]["point"], json!([5.0, 5.0, 5.0]));
    assert_eq!(result["anchor"]["mode"], "automatic");
}

fn component_pattern() -> (Document<MockKernel>, Value, FeatureUid, BodyUid) {
    let mut doc = Document::new(MockKernel::default());
    let rotation =
        Transform::rotation([0.0; 3], [0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2).unwrap();
    let placed = Transform::translation([20.0, 10.0, 0.0]).after(&rotation);
    let (_, source) = doc.create_component(Some("Part"), placed, true).unwrap();
    let body = box_body(&mut doc, 0.0);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let definition = pattern(
        json!({"type":"components","occurrences":[source.to_string()]}),
        json!([target([50.0, 40.0, 0.0]), target([80.0, 60.0, 0.0])]),
    );
    let result = preview(&mut doc, add(definition.clone()));
    assert_eq!(
        result["anchor"]["point"],
        json!([20.0, 10.0, 0.0]),
        "{result}"
    );
    assert_eq!(result["instances"].as_array().unwrap().len(), 2);
    let uid = doc.add_feature(&def(definition.clone()), None).unwrap().uid;
    (doc, definition, uid, body)
}

#[test]
fn point_pattern_components_share_shapes_and_keep_orientation_and_ids() {
    let (mut doc, mut definition, uid, body) = component_pattern();
    assert_eq!(
        doc.bodies().len(),
        1,
        "shared component bodies are not duplicated"
    );
    let instances = doc.instances();
    assert_eq!(instances.len(), 3);
    assert!(instances.iter().all(|i| i.body == body));
    assert_eq!(instances[1].transform.translation, [50.0, 40.0, 0.0]);
    assert_eq!(instances[1].transform.linear, instances[0].transform.linear);
    let ids = [instances[1].path.clone(), instances[2].path.clone()];
    definition["targets"].as_array_mut().unwrap().reverse();
    doc.edit_feature(uid, &def(definition)).unwrap();
    assert_eq!(doc.instances()[1].path, ids[0]);
    assert_eq!(doc.instances()[2].path, ids[1]);
    doc.edit_feature(body.feature, &def(box_def(0.0, 25.0)))
        .unwrap();
    assert_eq!(doc.instances().len(), 3);
    assert_eq!(bounds(&doc, body).1, [10.0, 10.0, 25.0]);
    let mut reopened = Document::from_json(&doc.to_json(), MockKernel::default()).unwrap();
    reopened.recompute();
    assert_eq!(reopened.instances(), doc.instances());
}

#[test]
fn point_pattern_component_edits_back_to_an_old_definition_do_not_reuse_removed_ids() {
    let (mut doc, definition, uid, _) = component_pattern();
    let mut changed = definition.clone();
    changed["targets"] = json!([target([120.0, 80.0, 0.0])]);
    let result = preview(
        &mut doc,
        json!({"cmd":"edit_feature","uid":uid,"def":changed}),
    );
    assert_eq!(result["removed_instances"].as_array().unwrap().len(), 2);
    assert_eq!(result["instances"].as_array().unwrap().len(), 1);
    doc.edit_feature(uid, &def(changed)).unwrap();
    assert_eq!(doc.instances().len(), 2);
    doc.edit_feature(uid, &def(definition)).unwrap();
    assert_eq!(doc.status(uid), Some(&FeatureStatus::Ok));
    assert_eq!(doc.instances().len(), 3);
    assert_eq!(doc.instances()[1].transform.translation, [50.0, 40.0, 0.0]);
}

#[test]
fn point_pattern_paste_new_preserves_active_ownership_and_omits_rolled_back_instances() {
    for rolled_back in [false, true] {
        let mut doc = Document::new(MockKernel::default());
        let (assembly, assembly_occurrence) = doc
            .create_component(Some("Assembly"), Transform::IDENTITY, true)
            .unwrap();
        let (_, part) = doc
            .create_component(Some("Part"), Transform::translation([20.0, 0.0, 0.0]), true)
            .unwrap();
        box_body(&mut doc, 0.0);
        doc.activate_component(assembly).unwrap();
        let mut definition = pattern(
            json!({"type":"components","occurrences":[format!("{assembly_occurrence}/{part}")]}),
            json!([target([50.0, 0.0, 0.0])]),
        );
        definition["context"] = json!(assembly_occurrence.to_string());
        doc.add_feature(&def(definition), None).unwrap();
        doc.activate_component(ComponentUid::ROOT).unwrap();
        if rolled_back {
            doc.set_marker(1).unwrap();
        }
        let (copy, _) = doc
            .paste_new(
                assembly_occurrence,
                Some(Transform::translation([0.0, 100.0, 0.0])),
            )
            .unwrap();
        let copies = doc
            .assembly()
            .children(copy)
            .filter(|o| o.pattern.is_some())
            .count();
        assert_eq!(copies, usize::from(!rolled_back));
        assert_eq!(doc.instances().len(), if rolled_back { 2 } else { 4 });
        if !rolled_back {
            let pattern = doc
                .features()
                .find(|f| f.component == copy && f.def.type_name() == "pattern_at_points")
                .unwrap()
                .uid;
            command(&mut doc, json!({"cmd":"suppress_feature","uid":pattern}));
            assert_eq!(
                doc.instances().len(),
                3,
                "copied pattern owns its copied instances"
            );
        }
    }
}

#[test]
fn point_pattern_unavailable_instances_cannot_form_rigid_groups() {
    let (mut doc, _, uid, _) = component_pattern();
    let instances = doc.instances();
    let group = doc
        .add_feature(
            &def(json!({"type":"rigid_group",
        "occurrences":[instances[0].path[0],instances[1].path[0]]})),
            None,
        )
        .unwrap()
        .uid;
    command(&mut doc, json!({"cmd":"suppress_feature","uid":uid}));
    assert!(matches!(doc.status(group), Some(FeatureStatus::Failed(_))));
    assert_eq!(doc.instances().len(), 1);
}

#[test]
fn point_pattern_linked_geometry_cannot_read_an_unavailable_instance() {
    let (mut doc, _, uid, body) = component_pattern();
    // The mock resolves prism cap surfaces automatically; primitive box
    // faces need an explicit surface fixture for this linked-plane check.
    doc.kernel().surfaces.borrow_mut().insert(
        "F1:top".to_owned(),
        mitcad_model::datum::SurfaceGeometry::Plane {
            origin: [0.0, 0.0, 10.0],
            normal: [0.0, 0.0, 1.0],
        },
    );
    let occurrence = doc.instances()[1].path[0];
    mitcad_model::test_support::allow_attachments(&mut doc);
    let added = command(
        &mut doc,
        json!({"cmd":"sketch.create","plane":{"body":body,"face":"F1:top"},
        "occurrence":occurrence.to_string()}),
    );
    let sketch: FeatureUid = added["uid"].as_str().unwrap().parse().unwrap();
    assert_eq!(doc.status(sketch), Some(&FeatureStatus::Ok));
    assert!(
        doc.move_feature(sketch, 1).is_err(),
        "consumer must follow the occurrence producer"
    );
    assert!(
        doc.command(&json!({"cmd":"delete_feature","uid":uid}).to_string())
            .is_err(),
        "linked geometry is a dependency of the producing feature"
    );
    command(&mut doc, json!({"cmd":"suppress_feature","uid":uid}));
    assert!(matches!(doc.status(sketch), Some(FeatureStatus::Failed(_))));
    command(&mut doc, json!({"cmd":"undo"}));
    assert_eq!(doc.status(sketch), Some(&FeatureStatus::Ok));
}

#[test]
fn point_pattern_suppressed_instances_cannot_be_revived_by_a_later_capture() {
    let (mut doc, _, uid, _) = component_pattern();
    let instance = doc.instances()[1].path[0];
    let capture = doc
        .add_feature(
            &def(json!({"type":"capture_position","positions":[
        {"occurrence":instance,"transform":[[1,0,0,140],[0,1,0,10],[0,0,1,0]]}]})),
            None,
        )
        .unwrap()
        .uid;
    command(&mut doc, json!({"cmd":"suppress_feature","uid":uid}));
    assert!(matches!(
        doc.status(capture),
        Some(FeatureStatus::Failed(_))
    ));
    assert_eq!(doc.instances().len(), 1);
}

#[test]
fn point_pattern_component_instances_follow_suppression_rollback_delete_and_undo() {
    let (mut doc, _, uid, _) = component_pattern();
    let paths: Vec<_> = doc.instances().into_iter().map(|i| i.path).collect();
    command(&mut doc, json!({"cmd":"suppress_feature","uid":uid}));
    assert_eq!(doc.instances().len(), 1);
    command(&mut doc, json!({"cmd":"undo"}));
    assert_eq!(doc.instances().len(), 3);
    doc.set_marker(1).unwrap();
    assert_eq!(doc.instances().len(), 1);
    doc.set_marker(2).unwrap();
    assert_eq!(doc.instances().len(), 3);
    command(&mut doc, json!({"cmd":"delete_feature","uid":uid}));
    assert_eq!(doc.instances().len(), 1);
    command(&mut doc, json!({"cmd":"undo"}));
    assert_eq!(
        doc.instances()
            .into_iter()
            .map(|i| i.path)
            .collect::<Vec<_>>(),
        paths
    );
    let generated = doc.instances()[1].path[0];
    assert!(
        doc.delete_occurrence(generated).is_err(),
        "instances are owned by their feature"
    );
}

#[test]
fn point_pattern_missing_source_occurrence_fails_without_stale_copies_after_reopen() {
    let (mut doc, _, uid, _) = component_pattern();
    let source = doc.instances()[0].path[0];
    doc.delete_occurrence(source).unwrap();
    assert!(matches!(doc.status(uid), Some(FeatureStatus::Failed(_))));
    assert!(doc.instances().is_empty());
    let mut reopened = Document::from_json(&doc.to_json(), MockKernel::default()).unwrap();
    reopened.recompute();
    assert!(matches!(
        reopened.status(uid),
        Some(FeatureStatus::Failed(_))
    ));
    assert!(reopened.instances().is_empty());
    command(&mut doc, json!({"cmd":"undo"}));
    assert_eq!(doc.instances().len(), 3);
}

#[test]
fn point_pattern_resolves_rotated_occurrence_points_in_the_feature_frame() {
    let mut doc = Document::new(MockKernel::default());
    let (_, point_occurrence) = doc
        .create_component(
            Some("Target"),
            Transform::translation([100.0, 20.0, 0.0]),
            true,
        )
        .unwrap();
    command(
        &mut doc,
        add(json!({"type":"construction_point","definition":{"type":"fixed","point":[5,10,2]}})),
    );
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let body = box_body(&mut doc, 0.0);
    let definition = pattern(
        json!({"type":"bodies","bodies":[body]}),
        json!([
        {"point":"F1","occurrence":point_occurrence.to_string()}]),
    );
    let uid = doc.add_feature(&def(definition), None).unwrap().uid;
    assert_eq!(bounds(&doc, BodyUid::new(uid, 0)).0, [100.0, 25.0, -3.0]);
    let turn = Transform::rotation([0.0; 3], [0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2).unwrap();
    doc.set_occurrence_transform(
        point_occurrence,
        Transform::translation([100.0, 20.0, 0.0]).after(&turn),
        false,
    )
    .unwrap();
    let min = bounds(&doc, BodyUid::new(uid, 0)).0;
    assert!(
        (min[0] - 85.0).abs() < 1e-8 && (min[1] - 20.0).abs() < 1e-8 && (min[2] + 3.0).abs() < 1e-8
    );
}

#[test]
fn point_pattern_vertex_target_and_nonroot_context_have_world_preview_anchor() {
    let mut doc = Document::new(MockKernel::default());
    let (_, occurrence) = doc
        .create_component(
            Some("Part"),
            Transform::translation([100.0, 20.0, 0.0]),
            true,
        )
        .unwrap();
    let body = box_body(&mut doc, 0.0);
    let vertex = "V{F1:side0|F1:side1|F1:top}";
    doc.kernel().vertex_points.borrow_mut().insert(
        vertex
            .parse::<mitcad_model::topo::VertexName>()
            .unwrap()
            .to_string(),
        [10.0, 0.0, 10.0],
    );
    let mut definition = pattern(
        json!({"type":"bodies","bodies":[body]}),
        json!([
        {"point":{"body":body,"vertex":vertex},"occurrence":occurrence.to_string()}]),
    );
    definition["context"] = json!(occurrence.to_string());
    let result = preview(&mut doc, add(definition.clone()));
    assert_eq!(result["status"], "ok", "{result}");
    assert_eq!(result["anchor"]["world"], json!([105.0, 25.0, 5.0]));
    assert_eq!(result["elements"][1][0][3], 5.0);
    assert_eq!(result["elements"][1][1][3], -5.0);
    definition["context"] = json!("");
    assert!(
        doc.command(&add(definition).to_string())
            .unwrap_err()
            .to_string()
            .contains("context")
    );
}
