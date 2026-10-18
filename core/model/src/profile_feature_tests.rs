// SPDX-License-Identifier: MIT
//! The profile features (extrude options, revolve, hole, thread) with the
//! mock kernel: what they ask the kernel for, their parameters, checks,
//! errors, references and the queries about them.

use serde_json::{Value, json};

use crate::Document;
use crate::document_tests::{Block, block, def};
use crate::features::FeatureDef;
use crate::ids::{BodyUid, FeatureUid};
use crate::testing::MockKernel;
use crate::{FeatureStatus, ModelError};

const RECT: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";
const TOP: &str = "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";

fn last_spec(doc: &Document<MockKernel>) -> String {
    doc.kernel()
        .specs
        .borrow()
        .last()
        .cloned()
        .unwrap_or_default()
}

fn status(doc: &Document<MockKernel>, uid: FeatureUid) -> FeatureStatus {
    doc.status(uid).cloned().expect("recomputed")
}

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

fn extrude_def(extent: Value, extra: Value) -> FeatureDef<crate::ValueInput> {
    let mut value = json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": RECT}],
                           "extent": extent, "operation": "new_body"});
    for (key, field) in extra.as_object().unwrap() {
        value[key] = field.clone();
    }
    def(value)
}

/// A second extrude of the block's sketch, its kernel spec and status.
fn extrude_with(b: &mut Block, extent: Value, extra: Value) -> (String, FeatureStatus) {
    let uid = b
        .doc
        .add_feature(&extrude_def(extent, extra), None)
        .unwrap()
        .uid;
    let spec = last_spec(&b.doc);
    let status = status(&b.doc, uid);
    b.doc.undo();
    (spec, status)
}

#[test]
fn extents_starts_tapers_and_walls_reach_the_kernel() {
    let mut b = block();
    let cases = [
        (
            json!({"type": "distance", "distance": 20, "taper": -0.1}),
            json!({}),
            "along [0.0, 0.0, 1.0] from offset 0: distance 20 taper -0.1",
        ),
        (
            json!({"type": "symmetric", "distance": 20, "full_length": true, "taper": 0.2}),
            json!({}),
            "from offset 0: distance 10 taper 0.2; distance 10 taper 0.2",
        ),
        (
            json!({"type": "two_sides", "side1": {"type": "distance", "distance": 15},
                   "side2": {"type": "through_all", "taper": 0.1}}),
            json!({"flip": true}),
            "along [-0.0, -0.0, -1.0] from offset 0: distance 15 taper 0; distance 26 taper 0.1",
        ),
        (
            json!({"type": "to_object", "object": {"type": "face", "body": "F2.b0", "face": TOP},
                   "offset": 2}),
            json!({"start": {"type": "offset", "offset": 40}, "flip": true}),
            "from offset 40: to face F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}) extend true offset 2",
        ),
        (
            json!({"type": "to_object", "object": {"type": "body", "body": "F2.b0", "through": true}}),
            json!({}),
            "to body prism(F2:0..20) through true offset 0",
        ),
        (
            json!({"type": "distance", "distance": 5}),
            json!({"start": {"type": "object", "object": {"type": "plane", "plane": "xz"}, "offset": 3}}),
            "from object plane [0.0, 0.0, 0.0] [0.0, 1.0, 0.0]: distance 5",
        ),
        (
            json!({"type": "symmetric", "distance": 4}),
            json!({"thin": {"location": "side1", "thickness": 2,
                            "side2": {"location": "center", "thickness": 1}}}),
            "distance 4 taper 0 thin Side1 2; distance 4 taper 0 thin Center 1",
        ),
    ];
    for (extent, extra, expected) in cases {
        let (spec, status) = extrude_with(&mut b, extent.clone(), extra);
        assert!(spec.contains(expected), "{extent}: {spec}");
        assert_eq!(status, FeatureStatus::Ok, "{extent}");
    }
    // A side of nothing leaves one side.
    let (spec, _) = extrude_with(
        &mut b,
        json!({"type": "two_sides", "side1": {"type": "distance", "distance": 0},
               "side2": {"type": "distance", "distance": 5}}),
        json!({}),
    );
    assert!(
        spec.ends_with("along [-0.0, -0.0, -1.0] from offset 0: distance 5 taper 0"),
        "{spec}"
    );
    // The sign of a two-sided extrusion's distance does not turn its side:
    // each goes its own way by the size.
    for (side1, side2, flip, expected) in [
        (
            json!({"type": "distance", "distance": 15}),
            json!({"type": "distance", "distance": -4}),
            false,
            "along [0.0, 0.0, 1.0] from offset 0: distance 15 taper 0; distance 4 taper 0",
        ),
        (
            json!({"type": "distance", "distance": -4}),
            json!({"type": "distance", "distance": -15}),
            true,
            "along [-0.0, -0.0, -1.0] from offset 0: distance 4 taper 0; distance 15 taper 0",
        ),
    ] {
        let (spec, status) = extrude_with(
            &mut b,
            json!({"type": "two_sides", "side1": side1, "side2": side2}),
            json!({"flip": flip}),
        );
        assert!(spec.ends_with(expected), "{side1} {side2} {flip}: {spec}");
        assert_eq!(status, FeatureStatus::Ok);
    }
    // Through all one way with a negative distance the other way.
    let (spec, status) = extrude_with(
        &mut b,
        json!({"type": "two_sides", "side1": {"type": "through_all"},
               "side2": {"type": "distance", "distance": -8}}),
        json!({}),
    );
    assert!(
        spec.contains("distance 26 taper 0; distance 8 taper 0"),
        "{spec}"
    );
    assert_eq!(status, FeatureStatus::Ok);
}

#[test]
fn extrude_values_get_parameters_and_checks() {
    let mut b = block();
    let added = b
        .doc
        .add_feature(
            &extrude_def(
                json!({"type": "distance", "distance": 10, "taper": 0.1}),
                json!({"start": {"type": "offset", "offset": 3},
                       "thin": {"location": "center", "thickness": 2}}),
            ),
            None,
        )
        .unwrap();
    let comments: Vec<String> = added
        .parameters
        .iter()
        .map(|name| {
            let id = b.doc.parameters().find(name).unwrap();
            b.doc.parameters().get(id).unwrap().comment().to_owned()
        })
        .collect();
    assert_eq!(
        comments,
        [
            "Extrude2 offset",
            "Extrude2 distance",
            "Extrude2 taper",
            "Extrude2 thickness"
        ]
    );
    let rejected = [
        (
            extrude_def(
                json!({"type": "two_sides", "side1": {"type": "distance", "distance": 0},
                       "side2": {"type": "distance", "distance": 0}}),
                json!({}),
            ),
            "add up to more than zero",
        ),
        (
            extrude_def(
                json!({"type": "distance", "distance": 5}),
                json!({"thin": {"location": "center", "thickness": 0}}),
            ),
            "wall thickness must be greater than zero",
        ),
        (
            extrude_def(
                json!({"type": "distance", "distance": 5}),
                json!({"thin": {"location": "center", "thickness": 1,
                                "side2": {"location": "side1", "thickness": 1}}}),
            ),
            "only an extrusion to two sides has a second wall",
        ),
        (
            extrude_def(
                json!({"type": "distance", "distance": 5}),
                json!({"start": {"type": "object", "object": {"type": "body", "body": "F2.b0"}}}),
            ),
            "starts from a plane or a planar face",
        ),
        (
            extrude_def(
                json!({"type": "to_object", "object": {"type": "face", "body": "F2.b0",
                       "face": "F9:end(r{c1})"}}),
                json!({}),
            ),
            "feature F9 does not exist",
        ),
    ];
    for (definition, expected) in rejected {
        let error = b
            .doc
            .add_feature(&definition, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "'{expected}' not in '{error}'");
    }
}

#[test]
fn extrusions_to_an_object_depend_on_its_body() {
    let mut b = block();
    let uid = b
        .doc
        .add_feature(
            &extrude_def(
                json!({"type": "to_object", "object": {"type": "face", "body": "F2.b0", "face": TOP}}),
                json!({}),
            ),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    match b.doc.delete_feature(b.extrude, false) {
        Err(ModelError::Dependents { dependents, .. }) => {
            assert_eq!(dependents, vec!["Extrude2".to_owned()]);
        }
        other => panic!("{other:?}"),
    }
    // A missing face fails the feature.
    let mut edited = extrude_def(
        json!({"type": "to_object", "object": {"type": "face", "body": "F2.b0",
               "face": "F2:end(r{c9})"}}),
        json!({}),
    );
    b.doc.edit_feature(uid, &edited).unwrap();
    assert_eq!(
        status(&b.doc, uid),
        FeatureStatus::Failed("body F2.b0 has no face F2:end(r{c9})".to_owned())
    );
    // A sketch is not a plane to extrude to.
    edited = extrude_def(
        json!({"type": "to_object", "object": {"type": "plane", "plane": "F1"}}),
        json!({}),
    );
    let error = b.doc.edit_feature(uid, &edited).unwrap_err().to_string();
    assert!(error.contains("is not construction geometry"), "{error}");

    // A construction plane before the extrusion.
    let plane = b
        .doc
        .add_feature(
            &def(json!({"type": "construction_plane",
                        "definition": {"type": "offset", "plane": "xy", "distance": 30}})),
            None,
        )
        .unwrap()
        .uid;
    let uid = b
        .doc
        .add_feature(
            &extrude_def(
                json!({"type": "to_object", "object": {"type": "plane", "plane": plane.to_string()}}),
                json!({}),
            ),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    let spec = last_spec(&b.doc);
    assert!(
        spec.contains("plane [0.0, 0.0, 30.0] [0.0, 0.0, 1.0]"),
        "{spec}"
    );
    match b.doc.delete_feature(plane, false) {
        Err(ModelError::Dependents { dependents, .. }) => {
            assert_eq!(dependents, vec!["Extrude3".to_owned()]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn new_component_operations_make_components() {
    let mut b = block();
    b.doc
        .add_feature(
            &extrude_def(
                json!({"type": "distance", "distance": 5}),
                json!({"operation": "new_component"}),
            ),
            None,
        )
        .unwrap();
    let bodies = query(&b.doc, json!({"query": "bodies"}));
    assert_eq!(
        bodies,
        json!([{"uid": "F2.b0", "name": "Body1"},
               {"uid": "F3.b0", "name": "Body2", "component": "C1"}])
    );
    let components = query(&b.doc, json!({"query": "components"}));
    assert_eq!(components["components"][1]["name"], "Component1");
    assert_eq!(components["components"][1]["created_by"], "F3");
    assert_eq!(components["occurrences"][0]["name"], "Component1:1");
}

#[test]
fn revolutions_take_axes_and_angles() {
    let mut b = block();
    let revolve = |axis: Value, extent: Value| {
        def(
            json!({"type": "revolve", "profiles": [{"sketch": "F1", "region": RECT}],
                   "axis": axis, "extent": extent, "operation": "new_body"}),
        )
    };
    let cases = [
        (
            json!("y"),
            json!({"type": "full"}),
            "axis [0.0, 0.0, 0.0] [0.0, 1.0, 0.0] angles 6.283185307179586 None",
        ),
        // Line c4 of the rectangle runs from (0, 40) down to (0, 0).
        (
            json!({"sketch": "F1", "curve": "c4"}),
            json!({"type": "symmetric", "angle": 0.5}),
            "axis [0.0, 40.0, 0.0] [0.0, -1.0, 0.0] angles 0.5 Some(0.5)",
        ),
        // A negative angle turns about the reversed axis.
        (
            json!("x"),
            json!({"type": "angle", "angle": -1}),
            "axis [0.0, 0.0, 0.0] [-1.0, -0.0, -0.0] angles 1 None",
        ),
        (
            json!({"body": "F2.b0", "edge": "E{F2:side(c1[c4,c2])|F2:side(c2[c1,c3])}"}),
            json!({"type": "two_sides", "angle1": 0.25, "angle2": 0.5}),
            "axis [0.0, 0.0, 0.0] [1.0, 0.0, 0.0] angles 0.25 Some(0.5)",
        ),
        (
            json!("y"),
            json!({"type": "to_object", "object": {"type": "plane", "plane": "yz"}}),
            "target true",
        ),
    ];
    for (axis, extent, expected) in cases {
        let uid = b
            .doc
            .add_feature(&revolve(axis.clone(), extent), None)
            .unwrap()
            .uid;
        assert_eq!(status(&b.doc, uid), FeatureStatus::Ok, "{axis}");
        let spec = last_spec(&b.doc);
        assert!(spec.contains(expected), "{axis}: {spec}");
        assert!(
            b.doc.body_shape(BodyUid::new(uid, 0)).is_some(),
            "a revolution makes a body"
        );
        b.doc.undo();
    }
    for (extent, expected) in [
        (json!({"type": "angle", "angle": 0}), "must not be zero"),
        (
            json!({"type": "symmetric", "angle": 4}),
            "more than a full turn",
        ),
        (
            json!({"type": "two_sides", "angle1": -1, "angle2": 2}),
            "must not be negative",
        ),
    ] {
        let error = b
            .doc
            .add_feature(&revolve(json!("y"), extent), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "'{expected}' not in '{error}'");
    }
    // A line the sketch does not have: references are checked when added.
    let error = b
        .doc
        .add_feature(
            &revolve(
                json!({"sketch": "F1", "curve": "c7"}),
                json!({"type": "full"}),
            ),
            None,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("Sketch1 has no curve c7"), "{error}");
    // Revolve profiles are consumed like extrude profiles.
    assert!(b.doc.profiles().iter().all(|p| p.consumed));
}

fn hole_def(placement: Value, extra: Value) -> FeatureDef<crate::ValueInput> {
    let mut value = json!({"type": "hole", "placement": placement, "diameter": 6,
                           "extent": {"type": "distance", "depth": 10}});
    for (key, field) in extra.as_object().unwrap() {
        value[key] = field.clone();
    }
    def(value)
}

#[test]
fn holes_start_on_a_plane() {
    // A hole placed on an origin plane (mitcad#25): the point is put on the
    // plane, and the hole goes against the plane's normal, or with it
    // flipped.
    let mut b = block();
    let on_plane = json!({"type": "plane", "plane": "xy", "points": [[10, 20, 5]]});
    let uid = b
        .doc
        .add_feature(&hole_def(on_plane.clone(), json!({})), None)
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    let spec = last_spec(&b.doc);
    assert!(
        spec.contains("([10.0, 20.0, 0.0], [-0.0, -0.0, -1.0])"),
        "{spec}"
    );
    b.doc.undo();
    let uid = b
        .doc
        .add_feature(&hole_def(on_plane, json!({"flip": true})), None)
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    let spec = last_spec(&b.doc);
    assert!(
        spec.contains("([10.0, 20.0, 0.0], [0.0, 0.0, 1.0])"),
        "{spec}"
    );
}

#[test]
fn holes_go_into_faces_and_sketch_points() {
    let mut b = block();
    let on_top = json!({"type": "face", "body": "F2.b0", "face": TOP, "points": [[10, 10, 50], [50, 30, 20]]});
    let uid = b
        .doc
        .add_feature(&hole_def(on_top.clone(), json!({})), None)
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    // Points are put on the face's plane; the holes point into the body.
    let spec = last_spec(&b.doc);
    assert!(
        spec.contains(
            "([10.0, 10.0, 20.0], [-0.0, -0.0, -1.0]), ([50.0, 30.0, 20.0], [-0.0, -0.0, -1.0])"
        ),
        "{spec}"
    );
    assert!(
        spec.contains("diameter 6.000000 Simple tip Some(\"2.0594885173533086\") depth 10"),
        "{spec}"
    );
    let shape = b.doc.body_shape(b.body).unwrap();
    assert_eq!(shape.history, "cut(prism(F2:0..20),hole(F3:2))");
    b.doc.undo();

    // Flat, counterbore, flipped, at offsets from two edges.
    let offsets = json!({"type": "face_offsets", "body": "F2.b0", "face": TOP,
        "edge1": "E{F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})|F2:side(c1[c4,c2])}", "offset1": 5,
        "edge2": "E{F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})|F2:side(c4[c3,c1])}", "offset2": 7});
    let uid = b
        .doc
        .add_feature(
            &hole_def(
                offsets,
                json!({"flat": true, "flip": true,
                       "kind": {"type": "counterbore", "diameter": 10, "depth": 3}}),
            ),
            None,
        )
        .unwrap()
        .uid;
    // The mock's edges all run along x from the body's corner, so both are
    // parallel: no point.
    assert_eq!(
        status(&b.doc, uid),
        FeatureStatus::Failed("the edges of the hole's position are parallel".to_owned())
    );
    b.doc.undo();

    // Sketch points: coordinates, a circle's centre and a sketch point (the
    // circle's centre point p2), against the normal.
    let sketch = b
        .doc
        .add_sketch(crate::features::SketchPlane::Xy)
        .unwrap()
        .uid;
    b.doc
        .add_circle(sketch, [30.0, 20.0], &crate::ValueInput::Number(3.0))
        .unwrap();
    let uid = b
        .doc
        .add_feature(
            &hole_def(
                json!({"type": "sketch_points", "sketch": sketch, "points": [[5, 5], "c1", "p2"]}),
                json!({"extent": {"type": "through_all"}}),
            ),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    let spec = last_spec(&b.doc);
    assert!(
        spec.contains(
            "([5.0, 5.0, 0.0], [-0.0, -0.0, -1.0]), ([30.0, 20.0, 0.0], [-0.0, -0.0, -1.0]), \
             ([30.0, 20.0, 0.0], [-0.0, -0.0, -1.0])"
        ),
        "{spec}"
    );
    assert!(spec.ends_with("through 1 bodies"), "{spec}");
    b.doc.undo();

    // A tapered, counterdrilled hole (FreeCAD's, mitcad#4): the taper and
    // the counterdrill's sizes reach the kernel; the definition keeps them.
    let uid = b
        .doc
        .add_feature(
            &hole_def(
                on_top,
                json!({"flat": true, "taper": 0.1,
                       "kind": {"type": "counterdrill", "diameter": 10, "depth": 3, "angle": 1.5}}),
            ),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    let spec = last_spec(&b.doc);
    assert!(
        spec.contains(
            "Counterdrill { diameter: 10.0, depth: 3.0, angle: 1.5 } tip None depth 10 taper 0.1"
        ),
        "{spec}"
    );
    let def = query(&b.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    assert_eq!(def["kind"]["type"], "counterdrill");
    assert!(def["taper"].is_string(), "{def}");
}

#[test]
fn hole_values_are_checked() {
    let mut b = block();
    let on_top = json!({"type": "face", "body": "F2.b0", "face": TOP, "points": [[10, 10, 20]]});
    for (extra, expected) in [
        (
            json!({"kind": {"type": "counterbore", "diameter": 5, "depth": 2}}),
            "counterbore must be wider than the hole",
        ),
        (
            json!({"kind": {"type": "countersink", "diameter": 12, "angle": 4}}),
            "countersink angle must be between 0 and 180 degrees",
        ),
        (json!({"tip_angle": 0}), "drill point angle"),
        (
            json!({"kind": {"type": "counterdrill", "diameter": 10, "depth": 3, "angle": 0}}),
            "counterdrill angle must be between 0 and 180 degrees",
        ),
        (
            json!({"kind": {"type": "counterdrill", "diameter": 4, "depth": 3, "angle": 1}}),
            "counterdrill must be wider than the hole",
        ),
        (json!({"taper": 2}), "taper must be less than 90 degrees"),
        (
            json!({"taper": 0.1, "thread": {"designation": "M6x1"}}),
            "a tapered hole has no thread",
        ),
        (
            json!({"thread": {"designation": "M7x3"}}),
            "no ISO metric thread M7x3",
        ),
        (
            json!({"thread": {"designation": "M6x1", "class": "6g"}}),
            "class 6g is for external threads",
        ),
    ] {
        let error = b
            .doc
            .add_feature(&hole_def(on_top.clone(), extra), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "'{expected}' not in '{error}'");
    }
    let error = b
        .doc
        .add_feature(
            &hole_def(
                json!({"type": "face", "body": "F2.b0", "face": TOP, "points": []}),
                json!({}),
            ),
            None,
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("no position"), "{error}");
}

#[test]
fn tapped_holes_take_the_thread_size_and_list_their_threads() {
    let mut b = block();
    let on_top = json!({"type": "face", "body": "F2.b0", "face": TOP, "points": [[10, 10, 20]]});
    let uid = b
        .doc
        .add_feature(
            &hole_def(
                on_top.clone(),
                json!({"thread": {"designation": "M6x1", "class": "6H"}}),
            ),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    // The bore is the minor diameter; a cosmetic thread changes nothing more.
    assert!(
        last_spec(&b.doc).contains("diameter 4.917468"),
        "{}",
        last_spec(&b.doc)
    );
    assert_eq!(b.doc.kernel().count("thread"), 0);
    let threads = query(&b.doc, json!({"query": "threads"}));
    assert_eq!(threads.as_array().unwrap().len(), 1);
    let thread = &threads[0];
    assert_eq!(thread["feature"], "F3");
    assert_eq!(thread["face"], "F3:hole0.wall");
    assert_eq!(thread["designation"], "M6x1");
    assert_eq!(thread["internal"], true);
    assert_eq!(thread["modeled"], false);
    b.doc.undo();

    // Modelled: cut into the wall, 8 long from 1 below the start (the high
    // end of the mock wall's axis, which points up).
    b.doc
        .add_feature(
            &hole_def(
                on_top,
                json!({"thread": {"designation": "M6x1", "modeled": true, "length": 8, "offset": 1}}),
            ),
            None,
        )
        .unwrap();
    assert_eq!(b.doc.kernel().count("thread"), 1);
    assert!(
        last_spec(&b.doc).starts_with(
            "thread F3:hole0.wall pitch 1 diameters 6.0000/4.9175/5.3505 angle 0.000000 right true part Some((8.0, 1.0, true))"
        ),
        "{}",
        last_spec(&b.doc)
    );
}

fn thread_def(face: &str, extra: Value) -> FeatureDef<crate::ValueInput> {
    let mut value = json!({"type": "thread", "faces": [{"body": "F2.b0", "face": face}],
                           "thread": {"designation": "M10x1.5", "class": "6g"}});
    for (key, field) in extra.as_object().unwrap() {
        value[key] = field.clone();
    }
    def(value)
}

/// Sketch1 with a 10 mm circle extruded 20 mm into a rod (F2.b0).
fn rod() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    let sketch = doc
        .add_sketch(crate::features::SketchPlane::Xy)
        .unwrap()
        .uid;
    let region = doc
        .add_circle(sketch, [0.0, 0.0], &crate::ValueInput::Number(10.0))
        .unwrap()
        .region;
    doc.add_feature(
        &def(
            json!({"type": "extrude", "profiles": [{"sketch": sketch, "region": region}],
                    "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}),
        ),
        None,
    )
    .unwrap();
    doc
}

#[test]
fn cosmetic_threads_are_data_and_modelled_ones_geometry() {
    let mut doc = rod();
    let uid = doc
        .add_feature(
            &thread_def("F2:side(c1)", json!({"length": 12, "offset": 2})),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(doc.kernel().count("thread"), 0);
    assert_eq!(
        doc.body_shape(BodyUid::new(FeatureUid(2), 0))
            .unwrap()
            .history,
        "prism(F2:0..20)"
    );
    let threads = query(&doc, json!({"query": "threads"}));
    assert_eq!(
        threads,
        json!([{"feature": "F3", "name": "Thread1", "body": "F2.b0", "face": "F2:side(c1)",
                "standard": "iso_metric", "designation": "M10x1.5", "class": "6g",
                "right_handed": true, "modeled": false, "major_diameter": 10.0,
                "minor_diameter": 8.376202367904177, "pitch": 1.5, "internal": false,
                "radius": 5.0, "start": [0.0, 0.0, 6.0], "end": [0.0, 0.0, 18.0]}])
    );
    // Suppressed, it is no thread.
    doc.set_suppressed(uid, true).unwrap();
    assert_eq!(query(&doc, json!({"query": "threads"})), json!([]));
    doc.undo();

    // Modelled.
    let modeled = thread_def("F2:side(c1)", json!({"modeled": true}));
    doc.edit_feature(uid, &modeled).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    // The basic profile: D, D1 and D2.
    assert_eq!(
        last_spec(&doc),
        "thread F2:side(c1) pitch 1.5 diameters 10.0000/8.3762/9.0257 angle 0.000000 right true part None"
    );
    assert_eq!(
        doc.body_shape(BodyUid::new(FeatureUid(2), 0))
            .unwrap()
            .history,
        "thread(prism(F2:0..20),1.5)"
    );
    // The class's diameters, turned.
    let class = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "diameters": {"major": 9.85, "minor": 8.141, "pitch": 8.928},
               "angle": "30 deg"}),
    );
    doc.edit_feature(uid, &class).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(
        last_spec(&doc),
        "thread F2:side(c1) pitch 1.5 diameters 9.8500/8.1410/8.9280 angle 0.523599 right true part None"
    );
    // Diameters out of order; diameters and an angle on a cosmetic thread.
    let wrong = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "diameters": {"major": 9.85, "minor": 8.928, "pitch": 8.141}}),
    );
    let error = doc.edit_feature(uid, &wrong).unwrap_err().to_string();
    assert!(error.contains("must grow from the minor"), "{error}");
    let cosmetic = thread_def("F2:side(c1)", json!({"angle": 0.5}));
    let error = doc.edit_feature(uid, &cosmetic).unwrap_err().to_string();
    assert!(error.contains("only a modelled thread"), "{error}");
    doc.edit_feature(uid, &modeled).unwrap();

    // Not a cylinder; an internal class on an external face.
    doc.edit_feature(uid, &thread_def("F2:end(r{c1})", json!({})))
        .unwrap();
    assert_eq!(
        status(&doc, uid),
        FeatureStatus::Failed("face F2:end(r{c1}) is not cylindrical".to_owned())
    );
    let internal_class = def(json!({"type": "thread",
        "faces": [{"body": "F2.b0", "face": "F2:side(c1)"}],
        "thread": {"designation": "M10x1.5", "class": "6H"}}));
    doc.edit_feature(uid, &internal_class).unwrap();
    assert_eq!(
        status(&doc, uid),
        FeatureStatus::Failed("class 6H is for internal threads".to_owned())
    );
    // A modelled thread too big for the rod.
    let too_big = def(
        json!({"type": "thread", "faces": [{"body": "F2.b0", "face": "F2:side(c1)"}],
        "thread": {"designation": "M20x2.5"}, "modeled": true}),
    );
    doc.edit_feature(uid, &too_big).unwrap();
    let FeatureStatus::Failed(message) = status(&doc, uid) else {
        panic!("too big");
    };
    assert!(message.contains("does not fit"), "{message}");
    // Values and references are checked.
    let error = doc
        .add_feature(&thread_def("F2:side(c1)", json!({"length": 0})), None)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("length must be greater than zero"),
        "{error}"
    );
    let error = doc
        .add_feature(&thread_def("F2:side(c1)", json!({"offset": 1})), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("has no offset"), "{error}");
}

#[test]
fn modelled_threads_take_extra_clearance() {
    let mut doc = rod();
    // Without clearance nothing changes: no fields, the same kernel call.
    let plain = thread_def("F2:side(c1)", json!({"modeled": true}));
    let uid = doc.add_feature(&plain, None).unwrap().uid;
    let basic = "thread F2:side(c1) pitch 1.5 diameters 10.0000/8.3762/9.0257 angle 0.000000 \
                 right true part None";
    assert_eq!(last_spec(&doc), basic);
    assert!(!doc.to_json().contains("clearance"), "{}", doc.to_json());

    // Radial and flank clearance go to the kernel with the size unchanged:
    // the same diameters, pitch, hand and part.
    let loose = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "radial_clearance": 0.2, "flank_clearance": "0.05 mm",
               "length": 12, "offset": 2, "thread": {"designation": "M10x1.5", "class": "6g",
               "right_handed": false}}),
    );
    doc.edit_feature(uid, &loose).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(
        last_spec(&doc),
        "thread F2:side(c1) pitch 1.5 diameters 10.0000/8.3762/9.0257 angle 0.000000 \
         right false part Some((12.0, 2.0, true)) clearance 0.2000/0.0500"
    );
    // Saved with the feature and loaded again as they were.
    let FeatureDef::Thread(saved) = &doc.feature(uid).unwrap().def else {
        panic!("a thread");
    };
    assert!(saved.radial_clearance.is_some() && saved.flank_clearance.is_some());
    let text = doc.to_json();
    assert!(text.contains("radial_clearance"), "{text}");
    let loaded = Document::from_json(&text, MockKernel::default()).unwrap();
    assert_eq!(loaded.to_json(), text);
    assert_eq!(
        loaded.feature(uid).unwrap().def,
        doc.feature(uid).unwrap().def
    );

    // Undo and redo bring the values back and forth.
    doc.undo();
    assert!(!doc.to_json().contains("clearance"));
    doc.redo();
    assert!(doc.to_json().contains("flank_clearance"));

    // A parameter expression: the thread follows the parameter, and fails
    // (not clamped) once the parameter goes negative.
    doc.add_parameter("gap", 0.1, "").unwrap();
    let by_name = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "radial_clearance": "gap * 2"}),
    );
    doc.edit_feature(uid, &by_name).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert!(
        last_spec(&doc).ends_with("clearance 0.2000/0.0000"),
        "{}",
        last_spec(&doc)
    );
    doc.set_parameter("gap", 0.15).unwrap();
    assert!(
        last_spec(&doc).ends_with("clearance 0.3000/0.0000"),
        "{}",
        last_spec(&doc)
    );
    doc.set_parameter("gap", -0.1).unwrap();
    let FeatureStatus::Failed(message) = status(&doc, uid) else {
        panic!("a negative clearance");
    };
    assert!(
        message.contains("radial clearance must be a finite length of 0 or more"),
        "{message}"
    );
    doc.set_parameter("gap", 0.1).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);

    // Zero is no clearance: the same kernel call as without.
    let zero = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "radial_clearance": 0, "flank_clearance": 0}),
    );
    doc.edit_feature(uid, &zero).unwrap();
    assert_eq!(last_spec(&doc), basic);

    // Negative values, clearance on a cosmetic thread, and a flank
    // clearance that leaves no tooth (the basic crest is P/8 = 0.1875 wide;
    // 0.1 per flank takes 0.231 off it).
    let negative = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "flank_clearance": -0.05}),
    );
    let error = doc.edit_feature(uid, &negative).unwrap_err().to_string();
    assert!(
        error.contains("flank clearance must be a finite length of 0 or more"),
        "{error}"
    );
    let cosmetic = thread_def("F2:side(c1)", json!({"radial_clearance": 0.2}));
    let error = doc.edit_feature(uid, &cosmetic).unwrap_err().to_string();
    assert!(error.contains("only a modelled thread"), "{error}");
    let too_much = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "flank_clearance": 0.1}),
    );
    doc.edit_feature(uid, &too_much).unwrap();
    let FeatureStatus::Failed(message) = status(&doc, uid) else {
        panic!("no tooth left");
    };
    assert!(message.contains("leaves no usable tooth"), "{message}");
    // A radial clearance that leaves an external thread no core.
    let no_core = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "radial_clearance": 4.5}),
    );
    doc.edit_feature(uid, &no_core).unwrap();
    let FeatureStatus::Failed(message) = status(&doc, uid) else {
        panic!("no core left");
    };
    assert!(message.contains("no core"), "{message}");
}

#[test]
fn tapped_holes_take_extra_clearance() {
    let mut b = block();
    let on_top = json!({"type": "face", "body": "F2.b0", "face": TOP, "points": [[10, 10, 20]]});
    let uid = b
        .doc
        .add_feature(
            &hole_def(
                on_top.clone(),
                json!({"thread": {"designation": "M6x1", "class": "6H", "modeled": true,
                                  "radial_clearance": 0.15, "flank_clearance": 0.02}}),
            ),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    // The bore stays the size's minor diameter; the thread gets the
    // clearance with the size's own diameters.
    assert_eq!(
        last_spec(&b.doc),
        "thread F3:hole0.wall pitch 1 diameters 6.0000/4.9175/5.3505 angle 0.000000 \
         right true part None clearance 0.1500/0.0200"
    );
    let text = b.doc.to_json();
    let loaded = Document::from_json(&text, MockKernel::default()).unwrap();
    assert_eq!(loaded.to_json(), text);

    // Checked as the thread feature's: negative, cosmetic, no tooth left
    // (an internal M6x1's crest is P/4 = 0.25 wide; 0.12 per flank takes
    // 0.277 off it).
    let error = b
        .doc
        .edit_feature(
            uid,
            &hole_def(
                on_top.clone(),
                json!({"thread": {"designation": "M6x1", "modeled": true, "radial_clearance": -1}}),
            ),
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("radial clearance must be a finite length of 0 or more"),
        "{error}"
    );
    let error = b
        .doc
        .edit_feature(
            uid,
            &hole_def(
                on_top.clone(),
                json!({"thread": {"designation": "M6x1", "flank_clearance": 0.02}}),
            ),
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("only a modelled thread"), "{error}");
    b.doc
        .edit_feature(
            uid,
            &hole_def(
                on_top,
                json!({"thread": {"designation": "M6x1", "modeled": true, "flank_clearance": 0.12}}),
            ),
        )
        .unwrap();
    let FeatureStatus::Failed(message) = status(&b.doc, uid) else {
        panic!("no tooth left");
    };
    assert!(message.contains("leaves no usable tooth"), "{message}");
}

#[test]
fn profile_features_round_trip_through_the_design_file() {
    let mut b = block();
    let commands = [
        extrude_def(
            json!({"type": "two_sides", "side1": {"type": "distance", "distance": 5, "taper": 0.1},
                   "side2": {"type": "to_object", "object": {"type": "plane", "plane": "xy"}, "offset": 1}}),
            json!({"start": {"type": "offset", "offset": 2},
                   "thin": {"location": "side2", "thickness": 1}, "operation": "new_component"}),
        ),
        def(
            json!({"type": "revolve", "profiles": [{"sketch": "F1", "region": RECT}],
                   "axis": {"sketch": "F1", "curve": "c4"}, "project_axis": true,
                   "extent": {"type": "two_sides", "angle1": 1, "angle2": 0.5}, "operation": "new_body"}),
        ),
        hole_def(
            json!({"type": "face", "body": "F2.b0", "face": TOP, "points": [[10, 10, 20]]}),
            json!({"kind": {"type": "countersink", "diameter": 12, "angle": 1.5}, "tip_angle": 2,
                   "thread": {"designation": "M6x1", "class": "6H", "modeled": true, "length": 5}}),
        ),
    ];
    for command in &commands {
        b.doc.add_feature(command, None).unwrap();
    }
    let text = b.doc.to_json();
    let loaded = Document::from_json(&text, MockKernel::default()).unwrap();
    assert_eq!(loaded.to_json(), text);
    let defs: Vec<_> = loaded.features().map(|f| f.def.clone()).collect();
    let original: Vec<_> = b.doc.features().map(|f| f.def.clone()).collect();
    assert_eq!(defs, original);
}

/// The default chamfer diameter of a size, external or internal, as the
/// mock kernel's spec prints it.
fn default_chamfer(designation: &str, internal: bool) -> String {
    let data = crate::features::thread_table::lookup(
        crate::features::ThreadStandard::IsoMetric,
        designation,
    )
    .unwrap();
    format!(
        "{:.4}",
        crate::features::thread::default_chamfer_diameter(&data, internal, None)
    )
}

#[test]
fn thread_end_chamfers_default_to_the_size_and_cut_only_ends_the_thread_reaches() {
    // The defaults from the size table: a tenth of a pitch inside the minor
    // diameter outside, outside the major diameter inside (mitcad#304).
    assert_eq!(default_chamfer("M8", false), "6.5218");
    assert_eq!(default_chamfer("M8", true), "8.1250");
    let doc = rod();
    let answer = query(
        &doc,
        json!({"query": "thread_chamfer", "designation": "M8", "internal": true}),
    );
    assert!((answer["diameter"].as_f64().unwrap() - 8.125).abs() < 1e-9);
    assert!((answer["angle"].as_f64().unwrap() - 45.0).abs() < 1e-9);
    let answer = query(
        &doc,
        json!({"query": "thread_chamfer", "designation": "M8"}),
    );
    assert!(
        (answer["diameter"].as_f64().unwrap() - (8.0 - 1.25 * 0.8660254037844386 * 1.25 - 0.125))
            .abs()
            < 1e-9
    );

    // A modelled M10 on the rod, chamfered at both ends: the default
    // diameter from the size (here below the basic minor diameter), 45
    // degrees, both ends of the face.
    let mut doc = rod();
    let chamfered = thread_def("F2:side(c1)", json!({"modeled": true, "chamfer": "both"}));
    let uid = doc.add_feature(&chamfered, None).unwrap().uid;
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    let m10 = default_chamfer("M10x1.5", false);
    assert_eq!(
        last_spec(&doc),
        format!(
            "thread F2:side(c1) pitch 1.5 diameters 10.0000/8.3762/9.0257 angle 0.000000 right true \
             part None chamfer 45.0000 deg {m10} ends [(true, true)]"
        )
    );
    // The cones have names of their own.
    let shape = doc.body_shape(BodyUid::new(FeatureUid(2), 0)).unwrap();
    assert!(
        shape
            .faces
            .iter()
            .any(|f| f.to_string() == "F3:thread_chamfer_high(F2:side(c1))")
    );

    // Saved and loaded as they were; no chamfer fields without a chamfer.
    let text = doc.to_json();
    assert!(text.contains("\"chamfer\": \"both\""), "{text}");
    let loaded = Document::from_json(&text, MockKernel::default()).unwrap();
    assert_eq!(loaded.to_json(), text);
    assert_eq!(
        loaded.feature(uid).unwrap().def,
        doc.feature(uid).unwrap().def
    );
    doc.edit_feature(uid, &thread_def("F2:side(c1)", json!({"modeled": true})))
        .unwrap();
    assert!(!doc.to_json().contains("chamfer"), "{}", doc.to_json());
    assert!(!last_spec(&doc).contains("chamfer"));

    // The default follows the size; a given diameter and angle stay.
    let fine = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "chamfer": "both",
               "thread": {"designation": "M10x1.25", "class": "6g"}}),
    );
    doc.edit_feature(uid, &fine).unwrap();
    let m10_fine = default_chamfer("M10x1.25", false);
    assert_ne!(m10_fine, m10);
    assert!(
        last_spec(&doc).ends_with(&format!(
            "chamfer 45.0000 deg {m10_fine} ends [(true, true)]"
        )),
        "{}",
        last_spec(&doc)
    );
    doc.add_parameter("sink", 7.0, "mm").unwrap();
    let given = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "chamfer": "both", "chamfer_angle": "30 deg",
               "chamfer_diameter": "sink", "thread": {"designation": "M10x1.25", "class": "6g"}}),
    );
    doc.edit_feature(uid, &given).unwrap();
    assert!(
        last_spec(&doc).ends_with("chamfer 30.0000 deg 7.0000 ends [(true, true)]"),
        "{}",
        last_spec(&doc)
    );
    doc.set_parameter("sink", 7.5).unwrap();
    assert!(
        last_spec(&doc).ends_with("chamfer 30.0000 deg 7.5000 ends [(true, true)]"),
        "{}",
        last_spec(&doc)
    );

    // A partial thread chamfers only ends it reaches: measured from the
    // high end (the start) 12 long, the start; 2 below it, none.
    let partial = |extra: Value| {
        let mut value = json!({"modeled": true, "length": 12});
        for (key, field) in extra.as_object().unwrap() {
            value[key] = field.clone();
        }
        thread_def("F2:side(c1)", value)
    };
    let ends = |doc: &Document<MockKernel>| {
        let spec = last_spec(doc);
        spec.find(" ends ").map(|i| spec[i + 6..].to_owned())
    };
    doc.edit_feature(uid, &partial(json!({"chamfer": "both"})))
        .unwrap();
    assert_eq!(ends(&doc).as_deref(), Some("[(false, true)]"));
    doc.edit_feature(uid, &partial(json!({"chamfer": "start"})))
        .unwrap();
    assert_eq!(ends(&doc).as_deref(), Some("[(false, true)]"));
    // The far end lies inside the face: nothing to chamfer.
    doc.edit_feature(uid, &partial(json!({"chamfer": "end"})))
        .unwrap();
    assert_eq!(ends(&doc), None);
    doc.edit_feature(uid, &partial(json!({"chamfer": "both", "offset": 2})))
        .unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(ends(&doc), None);
    // From the low end: its start is the low end; over the whole face
    // measured from the high end, its end is the low end.
    doc.edit_feature(
        uid,
        &partial(json!({"chamfer": "start", "location": "low_end"})),
    )
    .unwrap();
    assert_eq!(ends(&doc).as_deref(), Some("[(true, false)]"));
    doc.edit_feature(uid, &partial(json!({"chamfer": "end", "length": 20})))
        .unwrap();
    assert_eq!(ends(&doc).as_deref(), Some("[(true, false)]"));

    // A cosmetic thread cuts its chamfers too (the kernel builds no
    // profile); without a chamfer it stays data.
    let calls = doc.kernel().count("thread");
    doc.edit_feature(uid, &thread_def("F2:side(c1)", json!({"chamfer": "start"})))
        .unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(doc.kernel().count("thread"), calls + 1);
    assert!(
        last_spec(&doc).ends_with(&format!(
            "cosmetic chamfer 45.0000 deg {m10} ends [(false, true)]"
        )),
        "{}",
        last_spec(&doc)
    );
    let shape = doc.body_shape(BodyUid::new(FeatureUid(2), 0)).unwrap();
    assert!(!shape.faces.iter().any(|f| f.role == "thread"));
    doc.edit_feature(uid, &thread_def("F2:side(c1)", json!({})))
        .unwrap();
    assert_eq!(doc.kernel().count("thread"), calls + 1);

    // Values that are none, and fields without a chamfer.
    for (extra, expected) in [
        (
            json!({"chamfer_angle": "30 deg"}),
            "has no chamfer angle or diameter",
        ),
        (
            json!({"chamfer": "both", "chamfer_angle": "90 deg"}),
            "between 0 and 90 degrees",
        ),
        (
            json!({"chamfer": "both", "chamfer_diameter": 0}),
            "chamfer diameter must be greater than zero",
        ),
    ] {
        let error = doc
            .edit_feature(uid, &thread_def("F2:side(c1)", extra.clone()))
            .map(|_| String::new())
            .unwrap_or_else(|e| e.to_string());
        assert!(
            error.contains(expected),
            "'{expected}' not in '{error}' ({extra})"
        );
    }
    // A chamfer wider than the rod cuts nothing.
    doc.edit_feature(
        uid,
        &thread_def(
            "F2:side(c1)",
            json!({"modeled": true, "chamfer": "both", "chamfer_diameter": 12}),
        ),
    )
    .unwrap();
    let FeatureStatus::Failed(message) = status(&doc, uid) else {
        panic!("too wide a chamfer");
    };
    assert!(message.contains("must be smaller than"), "{message}");
}

#[test]
fn modelled_threads_that_take_their_whole_face_are_listed() {
    // The basic M10 on the 10 mm rod: its crest lies on the face, pieces of
    // which stay; the thread runs over the face.
    let mut doc = rod();
    let body = BodyUid::new(FeatureUid(2), 0);
    let has = |doc: &Document<MockKernel>, name: &str| {
        let shape = doc.body_shape(body).unwrap();
        shape.faces.iter().any(|f| f.to_string() == name)
    };
    let uid = doc
        .add_feature(&thread_def("F2:side(c1)", json!({"modeled": true})), None)
        .unwrap()
        .uid;
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert!(has(&doc, "F2:side(c1)"));
    let listed = json!([{"feature": "F3", "name": "Thread1", "body": "F2.b0",
        "face": "F2:side(c1)", "standard": "iso_metric", "designation": "M10x1.5",
        "class": "6g", "right_handed": true, "modeled": true, "major_diameter": 10.0,
        "minor_diameter": 8.376202367904177, "pitch": 1.5, "internal": false,
        "radius": 5.0, "start": [0.0, 0.0, 0.0], "end": [0.0, 0.0, 20.0]}]);
    assert_eq!(query(&doc, json!({"query": "threads"})), listed);

    // A radial clearance puts the crest inside the face, so no piece of the
    // face is left, and both ends get a cone (mitcad#433). The thread is
    // listed on the face as it was before the feature, from the cones'
    // inner edges, where the cones meet the crest (9.8): 45 degrees from
    // the default diameter, a tenth of a pitch inside the root (8.3762 -
    // 0.2), so (9.8 - 8.0262) / 2 = 0.8869 deep.
    let chamfered = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "radial_clearance": 0.1, "chamfer": "both"}),
    );
    doc.edit_feature(uid, &chamfered).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert!(!has(&doc, "F2:side(c1)"));
    assert!(has(&doc, "F3:thread_chamfer_low(F2:side(c1))"));
    let threads = query(&doc, json!({"query": "threads"}));
    assert_eq!(threads.as_array().map(Vec::len), Some(1), "{threads}");
    let depth = (9.8 - (8.376202367904177 - 0.2 - 0.15)) / 2.0;
    let mut expected = listed.clone();
    expected[0]["start"] = Value::Null;
    expected[0]["end"] = Value::Null;
    let mut got = threads.clone();
    let [start, end] = ["start", "end"].map(|key| got[0][key].take());
    assert_eq!(got, expected);
    let near = |point: &Value, z: f64| {
        let p: Vec<f64> = point
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        p.len() == 3 && p[0].abs() < 1e-9 && p[1].abs() < 1e-9 && (p[2] - z).abs() < 1e-9
    };
    assert!(near(&start, depth) && near(&end, 20.0 - depth), "{threads}");

    // A given chamfer diameter, at the high end only: from the low end of
    // the face to that cone's inner edge, (9.8 - 7) / 2 / tan 30 deep.
    let start_only = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "radial_clearance": 0.1, "chamfer": "start",
               "chamfer_diameter": 7, "chamfer_angle": "30 deg"}),
    );
    doc.edit_feature(uid, &start_only).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    let threads = query(&doc, json!({"query": "threads"}));
    let depth = (9.8 - 7.0) / 2.0 / 30f64.to_radians().tan();
    assert!(
        near(&threads[0]["start"], 0.0) && near(&threads[0]["end"], 20.0 - depth),
        "{threads}"
    );

    // A class's major diameter below the face's, without a chamfer: the
    // whole face, from end to end.
    let class = thread_def(
        "F2:side(c1)",
        json!({"modeled": true, "diameters": {"major": 9.85, "minor": 8.141, "pitch": 8.928}}),
    );
    doc.edit_feature(uid, &class).unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert!(!has(&doc, "F2:side(c1)"));
    assert_eq!(query(&doc, json!({"query": "threads"})), listed);
}
