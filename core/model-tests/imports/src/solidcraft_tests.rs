// SPDX-License-Identifier: MIT
//! The .solidcraft import on the model's mock kernel, checked through the
//! api's queries: contact sets, motion studies and motion links (mitcad#831;
//! the import's other tests are in `core/import-solidcraft`).

use mitcad_import_solidcraft::{Outcome, SolidcraftOptions, import_solidcraft_file};
use mitcad_model::testing::MockKernel;
use mitcad_model::{Document, FeatureDef};
use mitcad_model_api::DocumentApi;
use serde_json::json;

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
