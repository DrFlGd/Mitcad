// SPDX-License-Identifier: MIT
//! The .solidcraft import on the model's mock kernel: the structure the
//! import makes (timeline, parameters, sketches, components, placeholders).
//! Geometry (volumes, boxes, references by point) is checked with OCCT by
//! `tools/cli/solidcraft-test.cmake`.

use std::path::{Path, PathBuf};

use mitcad_model::testing::MockKernel;
use mitcad_model::{Document, FeatureDef};
use serde_json::json;

use super::*;

fn design(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../solidcraft/tests/designs")
        .join(name)
}

fn import(name: &str, options: &SolidcraftOptions) -> (Document<MockKernel>, SolidcraftReport) {
    let mut doc = Document::new(MockKernel::default());
    let report = import_solidcraft(&mut doc, &design(name), options)
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    (doc, report)
}

fn outcome(report: &SolidcraftReport, name: &str) -> Outcome {
    report
        .features
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no {name} in {:?}", report.features))
        .outcome
}

/// The timeline, parameters and sketches of a part: user parameters keep
/// their names and expressions, a dimension's parameter is its own
/// (`d1 = width`), a feature's input its slot's (`d3 = height`).
#[test]
fn a_part_comes_in_with_its_parameters_and_sketches() {
    let (doc, report) = import(
        "box-fillet-hole.solidcraft.json",
        &SolidcraftOptions::default(),
    );
    assert_eq!(report.format, 27);
    let names: Vec<&str> = report.features.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["Sketch1", "Extrude1", "Fillet1", "Sketch2", "Hole cut"]
    );
    assert_eq!(outcome(&report, "Sketch1"), Outcome::Parametric);
    assert_eq!(outcome(&report, "Extrude1"), Outcome::Parametric);
    assert_eq!(outcome(&report, "Sketch2"), Outcome::Parametric);
    assert_eq!(outcome(&report, "Hole cut"), Outcome::Parametric);
    let p = doc.parameters();
    let expr = |n: &str| {
        let id = p.find(n).unwrap_or_else(|| panic!("no parameter {n}"));
        p.get(id).unwrap().expression().to_owned()
    };
    assert_eq!(expr("width"), "40 mm");
    assert_eq!(expr("height"), "20 mm");
    assert_eq!(expr("d1"), "width");
    assert_eq!(expr("d3"), "height");
    assert_eq!(expr("d5"), "10 mm");
    // Parameters the dimensions and features own.
    let owner = |n: &str| p.owner(p.find(n).unwrap());
    let extrude = doc.features().find(|f| f.name == "Extrude1").unwrap().uid;
    assert_eq!(owner("d3"), Some(extrude));
    assert_eq!(owner("width"), None);
    let s1 = report
        .sketches
        .iter()
        .find(|s| s.name == "Sketch1")
        .unwrap();
    assert_eq!((s1.curves, s1.constraints, s1.dimensions), (4, 5, 2));
    assert_eq!((s1.constraints_mapped, s1.dimensions_mapped), (5, 2));
    assert!(s1.dropped.is_empty(), "{:?}", s1.dropped);
    // The sketch's dimensions are Mitcad's, with the file's parameters.
    let sketch = doc.features().find(|f| f.name == "Sketch1").unwrap();
    let FeatureDef::Sketch(def) = &sketch.def else {
        panic!("Sketch1 is not a sketch")
    };
    assert_eq!(def.entities.len(), 9);
    assert_eq!(def.constraints.len(), 5);
    assert_eq!(def.dimensions.len(), 2);
    // The body has the file's name.
    let body = doc.bodies().into_iter().find(|b| b.name == "Block");
    assert!(
        body.is_some(),
        "{:?}",
        doc.bodies()
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<_>>()
    );
}

/// A changed parameter recomputes the features that use it.
#[test]
fn a_changed_parameter_rebuilds() {
    let options = SolidcraftOptions {
        set_parameters: vec![("height".to_owned(), "35 mm".to_owned())],
        ..SolidcraftOptions::default()
    };
    let (doc, report) = import("box-fillet-hole.solidcraft.json", &options);
    assert_eq!(report.changed, [("height".to_owned(), "35 mm".to_owned())]);
    let p = doc.parameters();
    assert_eq!(p.value(p.find("d3").unwrap()), Some(35.0));
    assert!(
        import_solidcraft(
            &mut Document::new(MockKernel::default()),
            &design("box-fillet-hole.solidcraft.json"),
            &SolidcraftOptions {
                set_parameters: vec![("nothing".to_owned(), "1".to_owned())],
                ..SolidcraftOptions::default()
            }
        )
        .unwrap_err()
        .contains("nothing")
    );
}

/// An older format is upgraded first; components get their occurrences.
#[test]
fn an_old_format_comes_in_upgraded() {
    let (doc, report) = import("format-1.solidcraft.json", &SolidcraftOptions::default());
    assert_eq!(report.format, 1);
    assert!(report.upgrades.iter().any(|u| u == "5 to 6"));
    assert_eq!(report.components, 1);
    assert_eq!(report.occurrences, 1);
    // The old XZ plane is a plane of its own with the axes it had.
    let sketch = report
        .features
        .iter()
        .find(|f| f.name == "Sketch1")
        .unwrap();
    assert_eq!(sketch.outcome, Outcome::Parametric, "{:?}", sketch.notes);
    assert!(
        doc.features()
            .any(|f| f.def.type_name() == "construction_plane")
    );
}

/// Kinds without a Mitcad counterpart are named in the report and hold
/// their place as suppressed placeholders; the timeline goes on.
#[test]
fn unmapped_features_are_placeholders() {
    let v = json!({"format": "solidcraft/27", "name": "U", "params": [], "features": [
        {"id": 1, "name": "Box1", "type": "box", "corner": [0, 0, 0], "length": "10", "width": "10", "height": "10",
         "body_names": ["B"]},
        {"id": 2, "name": "SurfaceOffset1", "type": "surface_offset", "bodies": ["B"], "distance": "1"},
        {"id": 3, "name": "Lip1", "type": "lip", "face": [0, 0, 10], "width": "1", "height": "1"},
        {"id": 4, "name": "Box2", "type": "box", "corner": [20, 0, 0], "length": "5", "width": "5", "height": "5"}]});
    let file = mitcad_solidcraft::SolidcraftFile::from_value(v).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_solidcraft_file(
        &mut doc,
        file,
        "u.solidcraft",
        &SolidcraftOptions::default(),
    )
    .unwrap();
    assert_eq!(outcome(&report, "SurfaceOffset1"), Outcome::Unmapped);
    assert_eq!(outcome(&report, "Lip1"), Outcome::Unmapped);
    assert_eq!(outcome(&report, "Box2"), Outcome::Parametric);
    assert_eq!(
        report.unmapped,
        [("SurfaceOffset".to_owned(), 1), ("Lip".to_owned(), 1)]
    );
    let placeholder = doc.features().find(|f| f.name == "SurfaceOffset1").unwrap();
    assert!(placeholder.suppressed);
    assert!(
        doc.import_note(placeholder.uid)
            .unwrap()
            .contains("no Mitcad counterpart")
    );
    // Every input is a parameter, the unmapped ones' too.
    assert!(
        report
            .parameters
            .iter()
            .any(|p| p.name == "d4" && p.error.is_none())
    );
}

/// A design whose expected values disagree fails the check.
#[test]
fn the_check_compares_with_expected_values() {
    let options = SolidcraftOptions {
        expected: Some(json!({"timeline": [{"name": "Sketch1"}], "body_count": 7})),
        ..SolidcraftOptions::default()
    };
    let (_, report) = import("box-fillet-hole.solidcraft.json", &options);
    let check = report.check.unwrap();
    assert!(!check.passed);
    assert_eq!(check.failures.len(), 2, "{:?}", check.failures);
}

/// Expressions keep their meaning in Mitcad's language.
#[test]
fn expressions_are_translated() {
    let v = json!({"format": "solidcraft/27", "name": "E",
        "params": [{"name": "a", "expr": "2,5 mm", "unit": "mm"},
                   {"name": "E", "expr": "hypot(3, 4) mm", "unit": "mm"},
                   {"name": "n", "expr": "3", "unit": ""}],
        "features": []});
    let file = mitcad_solidcraft::SolidcraftFile::from_value(v).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_solidcraft_file(
        &mut doc,
        file,
        "e.solidcraft",
        &SolidcraftOptions::default(),
    )
    .unwrap();
    let p = doc.parameters();
    assert_eq!(p.value(p.find("a").unwrap()), Some(2.5));
    // `E` is Mitcad's constant: the parameter is `E_`.
    assert_eq!(p.value(p.find("E_").unwrap()), Some(5.0));
    assert_eq!(p.value(p.find("n").unwrap()), Some(3.0));
    let e = report.parameters.iter().find(|r| r.name == "E").unwrap();
    assert_eq!(e.renamed.as_deref(), Some("E_"));
}

/// Contact sets and motion studies come in as Mitcad's (mitcad#454): the
/// sets' occurrences by their paths, the studies' keys on the joint
/// features the file's joints became, a value index as the kind's motion.
/// Exploded views are named in the report as not mapped.
#[test]
fn motion_data_comes_in() {
    let frame = |o: u64| {
        json!({"occurrence": o,
               "snap": {"type": "frame", "origin": [0, 0, 0], "z": [0, 0, 1], "x": [1, 0, 0]}})
    };
    let v = json!({"format": "solidcraft/27", "name": "M", "params": [], "features": [],
        "components": [{"id": 1, "name": "Base"}, {"id": 2, "name": "Arm"}, {"id": 3, "name": "Cap"}],
        "occurrences": [{"id": 4, "component": 1, "name": "Base:1", "grounded": true},
                        {"id": 5, "component": 2, "name": "Arm:1"},
                        {"id": 6, "component": 3, "name": "Cap:1"}],
        "assembly": {
            "joints": [
                {"id": 7, "name": "Turn", "kind": "revolute", "a": frame(5), "b": frame(4),
                 "flip": true, "values": [0.0]},
                {"id": 8, "name": "Lift", "kind": "cylindrical", "a": frame(6), "b": frame(4),
                 "flip": true, "values": [0.0, 0.0]}],
            "contact_sets": [{"id": 9, "name": "Stops", "occurrences": [5, 6]},
                             {"id": 10, "name": "Off", "occurrences": [4, 5], "suppressed": true}],
            "contact_enabled": true,
            "studies": [{"id": 11, "name": "Open", "steps": 40, "keys": [
                {"joint": 7, "index": 0, "points": [[0, 0.0], [40, 1.5]]},
                {"joint": 8, "index": 1, "points": [[10, 0.0], [30, 12.0]]},
                {"joint": 99, "index": 0, "points": [[0, 1.0]]}]}],
            "exploded": [{"id": 12, "name": "E", "moves": []}]}});
    let file = mitcad_solidcraft::SolidcraftFile::from_value(v).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_solidcraft_file(
        &mut doc,
        file,
        "m.solidcraft",
        &SolidcraftOptions::default(),
    )
    .unwrap();
    let query = |q: &str| -> serde_json::Value {
        serde_json::from_str(&doc.query(&json!({"query": q}).to_string()).unwrap()).unwrap()
    };
    let sets = query("contact_sets");
    assert_eq!(sets[0]["name"], "Stops", "{sets}");
    assert_eq!(sets[0]["suppressed"], false);
    let names: Vec<&str> = sets[0]["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Arm:1", "Cap:1"]);
    assert_eq!(sets[1]["suppressed"], true);
    let studies = query("motion_studies");
    assert_eq!(studies[0]["name"], "Open", "{studies}");
    assert_eq!(studies[0]["steps"], 40);
    let keys = studies[0]["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 2, "{studies}");
    assert_eq!(keys[0]["joint_name"], "Turn");
    assert_eq!(keys[0]["motion"], "rz");
    // The cylindrical joint's value 1 is its slide (SolidCraft: turn, slide).
    assert_eq!(keys[1]["joint_name"], "Lift");
    assert_eq!(keys[1]["motion"], "tz");
    assert_eq!(keys[1]["points"], json!([[10.0, 0.0], [30.0, 12.0]]));
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("motion study Open: the keys of joint 99 left out")),
        "{:?}",
        report.notes
    );
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.starts_with("1 exploded views") && n.ends_with("not mapped yet")),
        "{:?}",
        report.notes
    );
    assert!(!report.notes.iter().any(|n| n.contains("mitcad#454")));
}

/// Motion links come in as Mitcad's `motion_link` features (mitcad#466):
/// a gear pair with its sizes, a screw on a cylindrical joint's own turn
/// and slide; a link of a joint that did not come in is left out with a
/// note, and nothing says motion links are not mapped.
#[test]
fn motion_links_come_in() {
    let frame = |o: u64| {
        json!({"occurrence": o,
               "snap": {"type": "frame", "origin": [0, 0, 0], "z": [0, 0, 1], "x": [1, 0, 0]}})
    };
    let v = json!({"format": "solidcraft/27", "name": "L", "params": [], "features": [],
        "components": [{"id": 1, "name": "Base"}, {"id": 2, "name": "Gear"}, {"id": 3, "name": "Nut"}],
        "occurrences": [{"id": 4, "component": 1, "name": "Base:1", "grounded": true},
                        {"id": 5, "component": 2, "name": "Gear:1"},
                        {"id": 6, "component": 3, "name": "Nut:1"}],
        "assembly": {
            "joints": [
                {"id": 7, "name": "Turn", "kind": "revolute", "a": frame(5), "b": frame(4),
                 "flip": true, "values": [0.0]},
                {"id": 8, "name": "Lift", "kind": "cylindrical", "a": frame(6), "b": frame(4),
                 "flip": true}],
            "links": [
                {"a": 7, "b": 8, "ratio": -0.5,
                 "coupling": {"type": "gear", "ra": 10, "rb": 20}},
                {"a": 8, "ia": 0, "b": 8, "ib": 1, "ratio": 0.3,
                 "coupling": {"type": "screw", "pitch": 2}},
                {"a": 99, "b": 7, "ratio": 2.0}]}});
    let file = mitcad_solidcraft::SolidcraftFile::from_value(v).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_solidcraft_file(
        &mut doc,
        file,
        "l.solidcraft",
        &SolidcraftOptions::default(),
    )
    .unwrap();
    let joints: serde_json::Value =
        serde_json::from_str(&doc.query(&json!({"query": "joints"}).to_string()).unwrap()).unwrap();
    let links = joints["links"].as_array().unwrap();
    assert_eq!(links.len(), 2, "{joints}");
    assert_eq!(links[0]["coupling"], "gear");
    assert_eq!(links[0]["ratio"], -0.5);
    assert_eq!(links[0]["a"]["name"], "Turn");
    assert_eq!(links[0]["b"]["motion"], "rz");
    assert_eq!(links[0]["solved"], true);
    assert_eq!(links[1]["coupling"], "screw");
    let ratio = links[1]["ratio"].as_f64().unwrap();
    assert!(
        (ratio - 1.0 / std::f64::consts::PI).abs() < 1e-12,
        "{ratio}"
    );
    assert_eq!(links[1]["b"]["motion"], "tz");
    let outcomes: Vec<Outcome> = report.links.iter().map(|l| l.outcome).collect();
    assert_eq!(
        outcomes,
        [Outcome::Parametric, Outcome::Parametric, Outcome::Failed]
    );
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("did not come in: its joint 99 did not come in")),
        "{:?}",
        report.notes
    );
    assert!(!report.notes.iter().any(|n| n.contains("motion links")));
    // The links are on the timeline after the joints.
    assert!(
        doc.features()
            .any(|f| matches!(f.def, FeatureDef::MotionLink(_)))
    );
}

/// Embosses come in as Mitcad's (mitcad#458): a deboss sinks, one without
/// a face starts from the sketch plane on its targets, one on a wall names
/// the face the import finds.
#[test]
fn embosses_come_in() {
    let (doc, report) = import("emboss.solidcraft.json", &SolidcraftOptions::default());
    assert!(report.unmapped.is_empty(), "{:?}", report.unmapped);
    use mitcad_model::features::{EmbossDef, EmbossMode};
    let def = |name: &str| -> EmbossDef {
        let f = doc.features().find(|f| f.name == name).unwrap();
        match &f.def {
            FeatureDef::Emboss(e) => e.clone(),
            other => panic!("{name}: {other:?}"),
        }
    };
    assert_eq!(outcome(&report, "Emboss3"), Outcome::Parametric);
    let top = def("Emboss3");
    assert!(top.face.is_none());
    assert_eq!(top.mode, EmbossMode::Raise);
    assert_eq!(top.participants.len(), 1);
    for (name, mode) in [
        ("Emboss1", EmbossMode::Raise),
        ("Emboss2", EmbossMode::Sink),
    ] {
        let f = report.features.iter().find(|f| f.name == name).unwrap();
        assert_ne!(f.outcome, Outcome::Unmapped, "{name}: {:?}", f.notes);
        if f.outcome != Outcome::Failed {
            assert_eq!(def(name).mode, mode);
        }
    }
}

/// Bounding solids come in as Mitcad's `bounding_solid` features
/// (mitcad#460): the bodies the file names (none: every body), the margin
/// its input's parameter (`gap`, or 0 when the file has none), and the
/// box named as SolidCraft names it (the stored name, else `Bounding`).
#[test]
fn bounding_solids_come_in() {
    let (doc, report) = import(
        "bounding-solid.solidcraft.json",
        &SolidcraftOptions::default(),
    );
    for name in ["Stock box", "Lug box", "Envelope"] {
        assert_eq!(outcome(&report, name), Outcome::Parametric, "{name}");
    }
    assert!(report.unmapped.is_empty(), "{:?}", report.unmapped);
    let def = |name: &str| {
        let entry = doc.features().find(|f| f.name == name).unwrap();
        let FeatureDef::BoundingSolid(def) = &entry.def else {
            panic!("{name} is not a bounding solid")
        };
        def.clone()
    };
    let p = doc.parameters();
    let expr = |id: mitcad_model::ParamId| p.get(id).unwrap().expression().to_owned();
    let stock = def("Stock box");
    assert_eq!(stock.bodies.len(), 2);
    assert_eq!(expr(stock.margin), "gap");
    let lug = def("Lug box");
    assert_eq!(lug.bodies.len(), 1);
    assert_eq!(p.value(lug.margin), Some(0.0));
    let envelope = def("Envelope");
    assert!(envelope.bodies.is_empty());
    assert_eq!(p.value(envelope.margin), Some(1.0));
    let mut names: Vec<String> = doc.bodies().iter().map(|b| b.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Block", "Bounding", "Bounding (2)", "Lug", "Stock"]);
}

/// Boundary fills come in as `boundary_fill` features (mitcad#457): their
/// new bodies named as the file names them, the tools they remove gone,
/// a cut's participants the feature's.
#[test]
fn boundary_fills_come_in() {
    let (doc, report) = import(
        "boundary-fill.solidcraft.json",
        &SolidcraftOptions::default(),
    );
    for name in ["CoreFill", "SidesFill", "SlotsFill"] {
        assert_eq!(outcome(&report, name), Outcome::Parametric, "{name}");
    }
    assert!(report.unmapped.is_empty(), "{:?}", report.unmapped);
    let mut names: Vec<String> = doc.bodies().iter().map(|b| b.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Core", "Left", "Plate", "Right"]);
    let fill = |name: &str| {
        let entry = doc.features().find(|f| f.name == name).unwrap();
        let FeatureDef::BoundaryFill(def) = &entry.def else {
            panic!("{name} is no boundary fill: {:?}", entry.def);
        };
        def.clone()
    };
    let sides = fill("SidesFill");
    assert_eq!(sides.cells, [[5.0, 5.0, 5.0], [25.0, 25.0, 25.0]]);
    assert!(sides.remove_tools);
    assert!(sides.participants.is_empty());
    let slots = fill("SlotsFill");
    let plate = doc.bodies().iter().find(|b| b.name == "Plate").unwrap().uid;
    assert_eq!(slots.participants, [plate]);
    assert!(!fill("CoreFill").remove_tools);
}

/// Convert Mesh (mitcad#462): a mesh import's body converted by Mitcad's
/// `mesh_convert`, its tolerance the input's parameter (`d1 = tol`); a
/// conversion of a body that is no mesh body fails and is a placeholder.
#[test]
fn a_mesh_conversion_comes_in_as_convert_mesh() {
    let (doc, report) = import(
        "mesh-convert.solidcraft.json",
        &SolidcraftOptions::default(),
    );
    assert_eq!(outcome(&report, "Mesh1"), Outcome::Parametric);
    assert_eq!(outcome(&report, "ConvertMesh1"), Outcome::Parametric);
    assert!(report.unmapped.is_empty(), "{:?}", report.unmapped);
    let convert = doc
        .features()
        .find(|f| f.name == "ConvertMesh1")
        .expect("ConvertMesh1");
    let FeatureDef::MeshConvert(def) = &convert.def else {
        panic!("{:?}", convert.def)
    };
    assert!(doc.status(convert.uid).is_some_and(|s| s.is_ok()));
    let p = doc.parameters();
    assert_eq!(p.name(def.tolerance), "d1");
    assert_eq!(p.get(def.tolerance).unwrap().expression(), "tol");
    let value = p.value(def.tolerance).unwrap();
    assert!((value - 0.1).abs() < 1e-12, "{value}");
    // The body keeps the file's name.
    assert!(
        doc.bodies()
            .iter()
            .any(|b| b.name == "Plate" && b.uid == def.body)
    );

    let v = json!({"format": "solidcraft/27", "name": "C", "params": [], "features": [
        {"id": 1, "name": "Box1", "type": "box", "corner": [0, 0, 0], "length": "10", "width": "10",
         "height": "10", "body_names": ["B"]},
        {"id": 2, "name": "ConvertMesh1", "type": "mesh_convert", "body": "B", "tolerance": "0.05"}]});
    let file = mitcad_solidcraft::SolidcraftFile::from_value(v).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_solidcraft_file(
        &mut doc,
        file,
        "c.solidcraft",
        &SolidcraftOptions::default(),
    )
    .unwrap();
    assert_eq!(outcome(&report, "Box1"), Outcome::Parametric);
    assert_eq!(outcome(&report, "ConvertMesh1"), Outcome::Failed);
}

/// Patch, Stitch and Thicken come in as Mitcad's (mitcad#456): a patch of
/// sketch regions makes a surface body of the file's name, a thicken new
/// bodies (the surface stays), a stitch keeps the first body's name; a
/// crown is noted as left out.
#[test]
fn surface_features_come_in() {
    let rectangle = |x: f64| {
        json!({"points": [
                {"id": "origin", "pos": [0.0, 0.0], "fixed": true},
                {"id": "p1", "pos": [x, 0.0], "fixed": true},
                {"id": "p2", "pos": [x + 20.0, 0.0], "fixed": true},
                {"id": "p3", "pos": [x + 20.0, 10.0], "fixed": true},
                {"id": "p4", "pos": [x, 10.0], "fixed": true}],
            "curves": [
                {"id": "l1", "type": "line", "a": 1, "b": 2},
                {"id": "l2", "type": "line", "a": 2, "b": 3},
                {"id": "l3", "type": "line", "a": 3, "b": 4},
                {"id": "l4", "type": "line", "a": 4, "b": 1}],
            "constraints": []})
    };
    let v = json!({"format": "solidcraft/27", "name": "S", "params": [], "features": [
        {"id": 1, "name": "Sketch1", "type": "sketch", "plane": {"type": "origin", "name": "XY"},
         "sketch": rectangle(0.0)},
        {"id": 2, "name": "Patch1", "type": "patch", "body_names": ["Sheet"], "sketch": 1,
         "profiles": {"type": "all"}},
        {"id": 3, "name": "Sketch2", "type": "sketch", "plane": {"type": "origin", "name": "XY"},
         "sketch": rectangle(40.0)},
        {"id": 4, "name": "Patch2", "type": "patch", "body_names": ["Other"], "sketch": 3,
         "profiles": {"type": "all"}, "crown": 0.5},
        {"id": 5, "name": "Thicken1", "type": "thicken", "body_names": ["Plate"], "bodies": ["Sheet"],
         "thickness": "2 mm", "symmetric": true, "operation": "new_body"},
        {"id": 6, "name": "Stitch1", "type": "stitch", "bodies": ["Sheet", "Other"], "tolerance": "0.01 mm"}]});
    let file = mitcad_solidcraft::SolidcraftFile::from_value(v).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import_solidcraft_file(
        &mut doc,
        file,
        "s.solidcraft",
        &SolidcraftOptions::default(),
    )
    .unwrap();
    for name in ["Patch1", "Thicken1", "Stitch1"] {
        assert_eq!(outcome(&report, name), Outcome::Parametric, "{name}");
    }
    assert_eq!(outcome(&report, "Patch2"), Outcome::Partial);
    let patch2 = report.features.iter().find(|f| f.name == "Patch2").unwrap();
    assert!(patch2.notes[0].contains("crown"), "{:?}", patch2.notes);
    assert!(report.unmapped.is_empty(), "{:?}", report.unmapped);
    let types: Vec<&str> = doc.features().map(|f| f.def.type_name()).collect();
    for t in ["patch", "thicken", "stitch"] {
        assert!(types.contains(&t), "{types:?}");
    }
    // The stitch consumed Other into Sheet; the thickened Plate is new.
    let mut names: Vec<String> = doc.bodies().iter().map(|b| b.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Plate", "Sheet"]);
    let thicken = doc.features().find(|f| f.name == "Thicken1").unwrap();
    let FeatureDef::Thicken(def) = &thicken.def else {
        panic!("Thicken1 is not a thicken")
    };
    assert!(def.symmetric);
    // Its thickness is the file's input, a parameter.
    assert_eq!(doc.parameters().value(def.thickness), Some(2.0));
}

/// A sweep's end scale and second rail and a pattern along a chain of
/// curves come in as Mitcad's options (mitcad#464); a spline fillet
/// radius is no placeholder any more (its edge is found by its pick, which
/// the mock kernel does not place; OCCT's run is `cli.import_solidcraft`).
#[test]
fn spline_radii_end_scales_two_rails_and_chains_come_in() {
    let (doc, report) = import(
        "fillet-sweep-pattern-options.solidcraft.json",
        &SolidcraftOptions::default(),
    );
    assert_ne!(outcome(&report, "Fillet1"), Outcome::Unmapped);
    for name in ["Sweep1", "Sweep2", "Pattern1"] {
        assert_eq!(outcome(&report, name), Outcome::Parametric, "{name}");
    }
    assert!(report.unmapped.is_empty(), "{:?}", report.unmapped);
    let def = |name: &str| {
        doc.features()
            .find(|f| f.name == name)
            .map(|f| f.def.clone())
            .unwrap()
    };
    assert!(matches!(def("Sweep1"), FeatureDef::Sweep(s) if s.end_scale.is_some()));
    assert!(matches!(def("Sweep2"), FeatureDef::Sweep(s) if s.guide_rail2.is_some()));
    // The rails' points where they cross the plane square to the path
    // (mitcad#469; the volume of a rail where it matters is checked in
    // `cli.import_solidcraft`, design sweep-rail-plane).
    assert!(matches!(def("Sweep2"),
        FeatureDef::Sweep(s) if s.rail_point == mitcad_model::RailPoint::Plane));
    assert!(matches!(def("Sweep1"),
        FeatureDef::Sweep(s) if s.rail_point == mitcad_model::RailPoint::Fraction));
    let FeatureDef::PathPattern(pattern) = def("Pattern1") else {
        panic!("Pattern1 is not a path pattern");
    };
    let path = format!("{:?}", pattern.path);
    assert!(path.starts_with("SketchCurves"), "{path}");
    let p = doc.parameters();
    let scale = p.find("taper").unwrap();
    assert_eq!(p.value(scale), Some(0.5));
}
