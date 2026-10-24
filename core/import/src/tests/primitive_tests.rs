// SPDX-License-Identifier: MIT
//! Primitives and coils read from their matrices.

use super::*;

/// Imports one primitive or coil item and returns its definition and the
/// provenance of its inputs (mitcad#25).
fn primitive_item(object_type: &str, detail: Value) -> (Value, Option<InputProvenance>) {
    let item = json!({"index": 0, "name": "Primitive1", "objectType": object_type,
                      "detail": detail});
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(vec![item]),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[0].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F1"}"#).unwrap()).unwrap();
    (def["def"].clone(), report.items[0].input_provenance)
}

fn xy_plane() -> Value {
    json!({"kind": "construction_plane", "name": "XY", "origin": "XY"})
}

#[test]
fn primitives_come_in_from_their_matrices() {
    // A box on the XY plane whose own frame is turned a quarter and moved to
    // x = 2 cm: its width (the own x) runs along the plane's y and its
    // length along the plane's -x, so it spans x 0..20 and y 0..10.
    let (def, provenance) = primitive_item(
        "BoxFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "placement": [[0.0, -1.0, 0.0, 2.0], [1.0, 0.0, 0.0, 0.0],
                             [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
               "distanceX": param("d7", "10 mm", 1.0, "mm"),
               "distanceY": param("d8", "20 mm", 2.0, "mm"),
               "distanceZ": param("d9", "15 mm", 1.5, "mm")}),
    );
    assert_eq!(provenance, Some(InputProvenance::Read));
    assert_eq!(def["plane"], "xy");
    assert_eq!(def["corner"], json!([0.0, 0.0]));
    // The sizes are the file's parameters (the length is `DistanceY`, the
    // width `DistanceX`: created in the order the import reads them).
    assert!(def["length"].is_string() && def["width"].is_string() && def["height"].is_string());
    assert_ne!(def["length"], def["width"]);
    assert_eq!(def["operation"], "new_body");

    // A cylinder on a face-like frame turned half round, centred at (1, 1)
    // cm of it, against the plane's normal (a negative height).
    let (def, provenance) = primitive_item(
        "CylinderFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "frame": [[-1.0, 0.0, 0.0, 0.0], [0.0, -1.0, 0.0, 0.0],
                         [0.0, 0.0, -1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
               "placement": [[1.0, 0.0, 0.0, -1.0], [0.0, 1.0, 0.0, -2.0],
                             [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
               "height": param("d7", "15 mm", 1.5, "mm"),
               "diameter": param("d8", "10 mm", 1.0, "mm")}),
    );
    assert_eq!(provenance, Some(InputProvenance::Read));
    assert_eq!(def["center"], json!([10.0, 20.0]));
    // The height is a parameter of its own: the file's, negated.
    assert!(def["height"].is_string() && def["height"] != def["diameter"]);
    assert_eq!(def["operation"], "new_body");

    // A sphere and a torus inside the diameter at the plane's origin.
    let (def, _) = primitive_item(
        "SphereFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "diameter": param("d7", "20 mm", 2.0, "mm")}),
    );
    assert_eq!(
        (def["type"].clone(), def["center"].clone()),
        (json!("sphere"), json!([0.0, 0.0]))
    );
    let (def, _) = primitive_item(
        "TorusFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "majorDiameter": param("d8", "40 mm", 4.0, "mm"),
               "minorDiameter": param("d7", "6 mm", 0.6, "mm"),
               "torusPosition": "InsideTorusPosition"}),
    );
    assert_eq!(def["position"], "inside");
}

#[test]
fn coils_come_in_with_their_choices() {
    let (def, provenance) = primitive_item(
        "CoilFeature",
        json!({"operation": "NewBodyFeatureOperation", "plane": xy_plane(),
               "coilType": "PitchAndRevolutionCoilType",
               "sectionType": "SquareCoilSectionType",
               "sectionPosition": "OutsideCoilSectionPosition",
               "isClockwise": false,
               "diameter": param("d1", "40 mm", 4.0, "mm"),
               "sectionSize": param("d2", "4 mm", 0.4, "mm"),
               "angle": param("d3x", "0 deg", 0.0, "deg"),
               "revolutions": param("d4", "3", 3.0, ""),
               "pitch": param("d5x", "10 mm", 1.0, "mm")}),
    );
    assert_eq!(provenance, Some(InputProvenance::Read));
    assert_eq!(def["type"], "coil");
    assert_eq!(
        (
            def["section"].clone(),
            def["section_position"].clone(),
            def["clockwise"].clone()
        ),
        (json!("square"), json!("outside"), json!(true))
    );
    assert_eq!(def["helix"]["type"], "revolutions_and_pitch");
}
