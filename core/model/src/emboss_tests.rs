// SPDX-License-Identifier: MIT
//! Emboss (mitcad#458) with the mock kernel: what reaches the kernel for a
//! planar face, the sketch plane and a cylindrical face, raised and sunk;
//! the participants; the faces of the result; refusals; the definition's
//! form.

use serde_json::json;

use crate::document_tests::{block, def, extrude, num};
use crate::features::{EmbossDef, EmbossMode, FeatureDef, SketchPlane};
use crate::ids::{BodyUid, FeatureUid};
use crate::testing::MockKernel;
use crate::topo::RegionKey;
use crate::{Document, FeatureStatus};

fn status(doc: &Document<MockKernel>, uid: FeatureUid) -> FeatureStatus {
    doc.status(uid).cloned().expect("recomputed")
}

fn error(doc: &Document<MockKernel>, uid: FeatureUid) -> String {
    match status(doc, uid) {
        FeatureStatus::Failed(message) => message,
        other => panic!("expected a failure, got {other:?}"),
    }
}

fn last_spec(doc: &Document<MockKernel>) -> String {
    doc.kernel()
        .specs
        .borrow()
        .last()
        .cloned()
        .unwrap_or_default()
}

fn history(doc: &Document<MockKernel>, body: BodyUid) -> String {
    doc.body_shape(body)
        .map_or_else(|| "none".to_owned(), |s| s.history.clone())
}

fn faces(doc: &Document<MockKernel>, body: BodyUid) -> Vec<String> {
    doc.body_shape(body)
        .map(|s| s.faces.iter().map(ToString::to_string).collect())
        .unwrap_or_default()
}

fn emboss(
    sketch: FeatureUid,
    region: &RegionKey,
    face: Option<(BodyUid, &str)>,
    depth: f64,
    mode: &str,
) -> serde_json::Value {
    let mut value = json!({"type": "emboss", "profiles": [{"sketch": sketch, "region": region}],
        "depth": depth, "mode": mode});
    if let Some((body, face)) = face {
        value["face"] = json!({"body": body, "face": face});
    }
    value
}

/// A sketch with a rectangle; its uid and region.
fn rectangle(
    doc: &mut Document<MockKernel>,
    plane: SketchPlane,
    corner: [f64; 2],
    size: [f64; 2],
) -> (FeatureUid, RegionKey) {
    let sketch = doc.add_sketch(plane).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, corner, &num(size[0]), &num(size[1]))
        .unwrap()
        .region;
    (sketch, region)
}

#[test]
fn a_planar_face_raises_out_of_the_material_and_sinks_into_it() {
    let mut b = block();
    let top = format!("F2:end({})", b.region);
    // Sketch2 (F3) on XY, 20 below the top face it is embossed on.
    let (sketch, region) = rectangle(&mut b.doc, SketchPlane::Xy, [10.0, 10.0], [10.0, 5.0]);
    let uid = b
        .doc
        .add_feature(
            &def(emboss(sketch, &region, Some((b.body, &top)), 2.0, "raise")),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    // From the face's plane (z = 20), along the sketch normal: out.
    assert_eq!(
        last_spec(&b.doc),
        "extrude F4 along [0.0, 0.0, 1.0] from offset 20: distance 2 taper 0"
    );
    assert_eq!(
        history(&b.doc, b.body),
        "join(prism(F2:0..20),prism(F4:20..22))"
    );
    let named = faces(&b.doc, b.body);
    assert!(named.contains(&format!("F4:end({region})")), "{named:?}");

    // Sunk 3 into the block: down from the top face, cut.
    b.doc
        .edit_feature(
            uid,
            &def(emboss(sketch, &region, Some((b.body, &top)), 3.0, "sink")),
        )
        .unwrap();
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    assert_eq!(
        last_spec(&b.doc),
        "extrude F4 along [-0.0, -0.0, -1.0] from offset 20: distance 3 taper 0"
    );
    assert_eq!(
        history(&b.doc, b.body),
        "cut(prism(F2:0..20),prism(F4:-20..-17))"
    );

    // On the bottom face (normal down) the same sketch raises downwards.
    let bottom = format!("F2:start({})", b.region);
    b.doc
        .edit_feature(
            uid,
            &def(emboss(
                sketch,
                &region,
                Some((b.body, &bottom)),
                1.0,
                "raise",
            )),
        )
        .unwrap();
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    assert_eq!(
        last_spec(&b.doc),
        "extrude F4 along [-0.0, -0.0, -1.0] from offset 0: distance 1 taper 0"
    );
}

#[test]
fn without_a_face_the_sketch_plane_is_the_start() {
    let mut b = block();
    let (sketch, region) = rectangle(&mut b.doc, SketchPlane::Xy, [10.0, 10.0], [10.0, 5.0]);
    let uid = b
        .doc
        .add_feature(&def(emboss(sketch, &region, None, 4.0, "sink")), None)
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    assert_eq!(
        last_spec(&b.doc),
        "extrude F4 along [-0.0, -0.0, -1.0] from offset 0: distance 4 taper 0"
    );
    assert!(history(&b.doc, b.body).starts_with("cut(prism(F2:0..20),"));
    // The default mode is left out of the definition.
    let FeatureDef::Emboss(made) = &b.doc.feature(uid).unwrap().def else {
        panic!("an emboss");
    };
    assert_eq!(made.mode, EmbossMode::Sink);
    let text = serde_json::to_value(FeatureDef::<String>::Emboss(EmbossDef {
        profiles: made.profiles.clone(),
        face: None,
        depth: "d9".to_owned(),
        mode: EmbossMode::Raise,
        participants: Vec::new(),
    }))
    .unwrap();
    assert_eq!(
        text,
        json!({"type": "emboss", "profiles": [{"sketch": "F3", "region": region}], "depth": "d9"})
    );
}

/// A rod: a circle of diameter 20 about the origin (Sketch1 F1) extruded
/// 30 up (F2, Body1 F2.b0, its wall F2:side(c1)).
fn rod() -> (Document<MockKernel>, BodyUid) {
    let mut doc = Document::new(MockKernel::default());
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_circle(sketch, [0.0, 0.0], &num(20.0))
        .unwrap()
        .region;
    let uid = doc
        .add_feature(&extrude(sketch, &[&region], 30.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    (doc, BodyUid::new(uid, 0))
}

#[test]
fn a_cylindrical_face_takes_the_profiles_wrapped() {
    let (mut doc, rod) = rod();
    // Sketch2 (F3) on XZ, parallel to the rod's axis.
    let (sketch, region) = rectangle(&mut doc, SketchPlane::Xz, [-5.0, 10.0], [10.0, 6.0]);
    let uid = doc
        .add_feature(
            &def(emboss(
                sketch,
                &region,
                Some((rod, "F2:side(c1)")),
                1.5,
                "raise",
            )),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(
        last_spec(&doc),
        "wrap F4 about 0 0 0 along 0 0 1 radius 10 to 11.5"
    );
    assert_eq!(
        history(&doc, rod),
        "join(prism(F2:0..30),wrap(F4:10..11.5))"
    );
    let named = faces(&doc, rod);
    assert!(named.contains(&format!("F4:end({region})")), "{named:?}");

    // Sunk: radially in.
    doc.edit_feature(
        uid,
        &def(emboss(
            sketch,
            &region,
            Some((rod, "F2:side(c1)")),
            2.0,
            "sink",
        )),
    )
    .unwrap();
    assert_eq!(status(&doc, uid), FeatureStatus::Ok);
    assert_eq!(
        last_spec(&doc),
        "wrap F4 about 0 0 0 along 0 0 1 radius 10 to 8"
    );
    assert_eq!(history(&doc, rod), "cut(prism(F2:0..30),wrap(F4:10..8))");

    // As deep as the radius reaches the axis.
    doc.edit_feature(
        uid,
        &def(emboss(
            sketch,
            &region,
            Some((rod, "F2:side(c1)")),
            10.0,
            "sink",
        )),
    )
    .unwrap();
    assert!(error(&doc, uid).contains("reaches the axis of face F2:side(c1)"));
}

#[test]
fn a_wrap_needs_a_sketch_along_the_axis() {
    let (mut doc, rod) = rod();
    // On XY the sketch stands across the axis.
    let (sketch, region) = rectangle(&mut doc, SketchPlane::Xy, [-5.0, -5.0], [3.0, 3.0]);
    let uid = doc
        .add_feature(
            &def(emboss(
                sketch,
                &region,
                Some((rod, "F2:side(c1)")),
                1.0,
                "raise",
            )),
            None,
        )
        .unwrap()
        .uid;
    assert!(
        error(&doc, uid).contains("the sketch plane must be parallel to the cylinder's axis"),
        "{}",
        error(&doc, uid)
    );
}

#[test]
fn the_faces_body_is_the_participant_unless_bodies_are_listed() {
    let mut b = block();
    // A second body beside the block (F4).
    let (other, other_region) = rectangle(&mut b.doc, SketchPlane::Xy, [100.0, 0.0], [10.0, 10.0]);
    let second = b
        .doc
        .add_feature(
            &extrude(other, &[&other_region], 5.0, "new_body", &[]),
            None,
        )
        .unwrap()
        .uid;
    let second = BodyUid::new(second, 0);
    let top = format!("F2:end({})", b.region);
    let (sketch, region) = rectangle(&mut b.doc, SketchPlane::Xy, [10.0, 10.0], [10.0, 5.0]);
    let uid = b
        .doc
        .add_feature(
            &def(emboss(sketch, &region, Some((b.body, &top)), 2.0, "sink")),
            None,
        )
        .unwrap()
        .uid;
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    assert!(history(&b.doc, b.body).starts_with("cut("));
    assert_eq!(history(&b.doc, second), "prism(F4:0..5)");

    // Listed, the second body is cut too.
    let mut listed = emboss(sketch, &region, Some((b.body, &top)), 2.0, "sink");
    listed["participants"] = json!([b.body, second]);
    b.doc.edit_feature(uid, &def(listed)).unwrap();
    assert_eq!(status(&b.doc, uid), FeatureStatus::Ok);
    assert!(history(&b.doc, second).starts_with("cut(prism(F4:0..5),"));
}

#[test]
fn refusals() {
    let mut b = block();
    let (sketch, region) = rectangle(&mut b.doc, SketchPlane::Xy, [10.0, 10.0], [10.0, 5.0]);
    // A depth of zero.
    let zero = b
        .doc
        .add_feature(&def(emboss(sketch, &region, None, 0.0, "raise")), None)
        .unwrap_err()
        .to_string();
    assert!(
        zero.contains("the emboss depth must be greater than zero"),
        "{zero}"
    );
    // A side face of the block is neither planar nor cylindrical to the
    // mock.
    let side = crate::document_tests::side(b.extrude, 1, 0).to_string();
    let uid = b
        .doc
        .add_feature(
            &def(emboss(sketch, &region, Some((b.body, &side)), 1.0, "raise")),
            None,
        )
        .unwrap()
        .uid;
    assert!(
        error(&b.doc, uid).contains("is neither planar nor cylindrical"),
        "{}",
        error(&b.doc, uid)
    );
    // A face of a feature after it.
    let later = b
        .doc
        .add_feature(
            &def(emboss(
                sketch,
                &region,
                Some((b.body, "F9:end(r{c1})")),
                1.0,
                "raise",
            )),
            None,
        )
        .unwrap_err()
        .to_string();
    assert!(later.contains("F9"), "{later}");
    // An unknown mode.
    let mut bad = emboss(sketch, &region, None, 1.0, "raise");
    bad["mode"] = json!("lift");
    assert!(
        serde_json::from_value::<FeatureDef<crate::ValueInput>>(bad)
            .unwrap_err()
            .to_string()
            .contains("unknown variant `lift`")
    );
}
