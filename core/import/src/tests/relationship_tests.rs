// SPDX-License-Identifier: MIT
//! Assembly relationships (mitcad#155) on synthetic dumps with the mock
//! kernel: Base (component object 10) is grounded at the origin
//! (occurrence object 100), Arm (20, occurrence 200) is placed where the
//! test says, and occurrence 900 places a component of another document.
//! The mock's components have no faces: sides go on fixed geometry.

use mitcad_model::features::{ConstraintRow, ConstraintType, GeomRef};
use mitcad_model::{OccurrenceUid, Transform};

use super::*;

const PI: f64 = std::f64::consts::PI;

/// A placement or frame (mm) as the dump's matrix (cm).
fn cm(t: &Transform) -> Value {
    let row = |r: usize| {
        json!([
            t.linear[r][0],
            t.linear[r][1],
            t.linear[r][2],
            t.translation[r] / 10.0
        ])
    };
    json!([row(0), row(1), row(2), [0.0, 0.0, 0.0, 1.0]])
}

fn occurrence(path: Value) -> Value {
    json!({"kind": "occurrence", "_f3d": {"path": path, "context_component": 3}})
}

/// The offset parameter `d20` of the item at `index` (cm).
fn offset_param(value: f64, index: i64) -> (Value, Value) {
    let expression = format!("{} mm", value * 10.0);
    (
        json!({"kind": "parameter", "name": "d20", "expression": expression, "value": value,
               "unit": "mm"}),
        json!({"name": "d20", "role": "Offset", "expression": expression, "value": value,
               "unit": "mm", "createdBy": {"kind": "feature",
               "objectType": "GeometricRelationship", "name": "GeometricRelationship1",
               "timeline_index": index}}),
    )
}

/// A relationship item at `index` with one row: side one on `one`, side
/// two on `two`, their frames (mm), geometry and offset (cm).
fn relationship(
    index: i64,
    (one, two): (Value, Value),
    frames: [Transform; 2],
    geometry: [&str; 2],
    offset: f64,
) -> (Value, Vec<Value>) {
    let (offset, param) = offset_param(offset, index);
    let row = json!({"type": "align", "geometry": geometry, "sense": "opposed",
        "occurrenceOne": one, "occurrenceTwo": two, "offset": offset,
        "_f3d": {"type_code": 5, "frames": [cm(&frames[0]), cm(&frames[1])]}});
    let item = json!({"index": index, "name": "GeometricRelationship1",
        "objectType": "GeometricRelationship", "detail": {"rows": [row]},
        "_f3d": {"component": 3}});
    (item, vec![param])
}

/// The design with Arm placed at `arm`, the timeline `items` and the
/// model parameters `params`.
fn assembly(arm: &Transform, items: Vec<Value>, params: Vec<Value>) -> Dump {
    let v = json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "relationships.f3d"},
        "document": {"root_component": "Assembly"},
        "parameters": {"user": [], "model": params},
        "components": [
            {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
            {"name": "Base", "_f3d": {"object_id": 10}},
            {"name": "Arm", "_f3d": {"object_id": 20}}],
        "occurrences": [
            {"component": "Base", "transform": cm(&Transform::IDENTITY), "isGrounded": true,
             "_f3d": {"object_id": 100, "component_object": 10}, "children": []},
            {"component": "Arm", "transform": cm(arm),
             "_f3d": {"object_id": 200, "component_object": 20}, "children": []},
            {"component": null, "isReferencedComponent": true, "transform": cm(arm),
             "_f3d": {"object_id": 900, "component_object": 3, "external_key": "00ff"}}],
        "timeline": {"items": items}
    });
    Dump::from_json(&v.to_string()).expect("a dump")
}

/// Arm's bottom (its z = 0, facing down) and Base's top (z = 0, up).
fn bottom_and_top() -> [Transform; 2] {
    let down = Transform::rotation([0.0; 3], [1.0, 0.0, 0.0], PI).expect("unit axis");
    [down, Transform::IDENTITY]
}

fn set_of(doc: &Document<MockKernel>, report: &DesignReport) -> Vec<ConstraintRow> {
    let uid: FeatureUid = report.items[0].features[0].parse().unwrap();
    match &doc.feature(uid).unwrap().def {
        FeatureDef::ConstraintSet(set) => set.rows.clone(),
        other => panic!("a constraint set, not {other:?}"),
    }
}

fn arm_moved(doc: &Document<MockKernel>) -> f64 {
    let a = doc.assembly();
    let arm: OccurrenceUid = a
        .occurrences
        .iter()
        .find(|o| a.occurrence_name(o.uid) == "Arm:1")
        .map(|o| o.uid)
        .expect("Arm:1");
    let p = doc.placement(arm).unwrap();
    let o = a.occurrence(arm).unwrap();
    (0..3).fold(0.0_f64, |m, i| {
        m.max((p.translation[i] - o.transform.translation[i]).abs())
    })
}

#[test]
fn a_plane_on_a_plane_comes_in_where_it_holds() {
    // The file: side two's plane 2 mm from side one's along side one's
    // normal; Arm's bottom 2 mm above Base's top holds it.
    let sides = (occurrence(json!([200])), occurrence(json!([100])));
    let (item, params) = relationship(0, sides, bottom_and_top(), ["plane", "plane"], 0.2);
    let arm = Transform::translation([0.0, 0.0, 2.0]);
    let dump = assembly(&arm, vec![item], params);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let r = &report.relationships;
    assert_eq!(
        (r.sets, r.rows, r.translated, r.fixed_sides),
        (1, 1, 1, 2),
        "{}",
        report.text()
    );
    // No face of the mock's components gives the planes: fixed planes.
    assert_eq!(report.items[0].outcome, Outcome::Partial);
    let rows = set_of(&doc, &report);
    let row = &rows[0];
    assert_eq!(row.kind, ConstraintType::Align);
    // Side two (Base) is a, side one (Arm) b; the planes face each other.
    assert_eq!(row.a.occurrence.to_string(), "O1");
    assert_eq!(row.b.occurrence.to_string(), "O2");
    assert!(matches!(row.b.geometry, GeomRef::FixedPlane { .. }));
    assert!(!row.flip);
    assert_eq!(doc.parameters().name(row.offset.unwrap()), "d20");
    assert!(arm_moved(&doc) < 1e-9);
    // A parameter change keeps the relation: Arm follows the offset.
    doc.set_parameter("d20", 5.0).unwrap();
    assert!((arm_moved(&doc) - 3.0).abs() < 1e-6, "{}", arm_moved(&doc));
}

#[test]
fn a_relationship_that_does_not_hold_is_left_out() {
    // Arm 3 mm up: the file's 2 mm do not hold there.
    let sides = (occurrence(json!([200])), occurrence(json!([100])));
    let (item, params) = relationship(0, sides, bottom_and_top(), ["plane", "plane"], 0.2);
    let arm = Transform::translation([0.0, 0.0, 3.0]);
    let dump = assembly(&arm, vec![item], params);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(report.relationships.translated, 0, "{}", report.text());
    assert_eq!(report.items[0].outcome, Outcome::Skipped);
    let note = report.items[0].note.clone().unwrap_or_default();
    assert!(
        note.contains("it does not hold where the file places the occurrences"),
        "{note}"
    );
    assert!(arm_moved(&doc) < 1e-9);
}

#[test]
fn planes_the_same_way_are_flipped() {
    // Side one's frame facing up instead (Arm below Base): the planes'
    // normals the same way, the offset as it is.
    let up = Transform::IDENTITY;
    let sides = (occurrence(json!([200])), occurrence(json!([100])));
    let (item, params) = relationship(0, sides, [up, up], ["plane", "plane"], 0.2);
    let arm = Transform::translation([0.0, 0.0, -2.0]);
    let dump = assembly(&arm, vec![item], params);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(report.relationships.translated, 1, "{}", report.text());
    let rows = set_of(&doc, &report);
    // The planes' normals the same way: flipped.
    assert!(rows[0].flip);
    assert!(arm_moved(&doc) < 1e-9);
}

#[test]
fn sides_on_components_of_other_documents_and_inside_them() {
    // On occurrence 900 (a component of another document): fixed geometry
    // of the empty component standing for it.
    let sides = (occurrence(json!([900])), occurrence(json!([100])));
    let (item, params) = relationship(0, sides, bottom_and_top(), ["plane", "plane"], 0.2);
    let arm = Transform::translation([0.0, 0.0, 2.0]);
    let dump = assembly(&arm, vec![item], params);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let r = &report.relationships;
    assert_eq!(
        (r.translated, r.inserted_sides),
        (1, 1),
        "{}",
        report.text()
    );
    // Inside it (a level of the path in the other document): left out.
    let sides = (occurrence(json!([900, null])), occurrence(json!([100])));
    let (item, params) = relationship(0, sides, bottom_and_top(), ["plane", "plane"], 0.2);
    let dump = assembly(&arm, vec![item], params);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(report.relationships.translated, 0, "{}", report.text());
    let note = report.items[0].note.clone().unwrap_or_default();
    assert!(
        note.contains("inside a component of another document"),
        "{note}"
    );
}

#[test]
fn a_tangent_without_planar_cylindrical_or_spherical_faces_is_left_out() {
    let detail = json!({"occurrenceOne": occurrence(json!([200])),
        "occurrenceTwo": occurrence(json!([100])),
        "pointOne": [0.0, 0.0, 0.0], "pointTwo": [0.0, 0.0, 0.0]});
    let item = json!({"index": 0, "name": "Tangent Relationship1",
        "objectType": "TangentRelationship", "detail": detail, "_f3d": {"component": 3}});
    let dump = assembly(&Transform::IDENTITY, vec![item], Vec::new());
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    assert_eq!(report.relationships.tangents, 1);
    assert_eq!(report.items[0].outcome, Outcome::Skipped);
    let note = report.items[0].note.clone().unwrap_or_default();
    assert!(
        note.contains("not planar, cylindrical or spherical"),
        "{note}"
    );
}
