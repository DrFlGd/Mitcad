// SPDX-License-Identifier: MIT
//! A roof's fasteners at its junctions (mitcad#521): a gable roof at 30
//! degrees on two walls, with ring nails and with countersunk screws, has
//! every fastener's shaft through both members of its junction (through
//! the ridge board into each rafter's plumb cut end, square to it) and its
//! head on the entry face; no junction between the two rafters of a pair,
//! whose plumb cut ends' boxes overlap above the ridge board where neither
//! has material; a mono-pitch roof has no junction of two of its members.
//! The mock kernel keeps a cut part's box (`cuts_keep_bounds`), as a plumb
//! cut and a birdsmouth take material from inside it.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::geometry::{V3, dot, norm, sub};
use super::test_library::{all_ok, command, design, part, query};
use crate::Document;
use crate::ids::ComponentUid;
use crate::testing::MockKernel;

fn uid(v: &Value) -> ComponentUid {
    v.as_str().unwrap().parse().unwrap()
}

fn parts_of(doc: &Document<MockKernel>) -> Vec<Value> {
    query(doc, json!({"query": "build_parts"}))["parts"]
        .as_array()
        .unwrap()
        .clone()
}

fn wall(doc: &mut Document<MockKernel>, y: f64, height: f64) -> Value {
    command(
        doc,
        json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
               "start": [0, y, 0], "end": [4000, y, 0], "height": height, "spacing": 600}),
    )
    .unwrap_or_else(|e| panic!("build_frame: {e}"))
}

/// Two walls 4000 long along X, 3000 apart, and a roof on them with the
/// fastener `fastener`.
fn roof(roof: &str, high: f64, fastener: &str) -> (Document<MockKernel>, Value) {
    let mut doc = design();
    doc.kernel().cuts_keep_bounds.set(true);
    let a = wall(&mut doc, 0.0, 2400.0)["frame"].clone();
    let b = wall(&mut doc, 3000.0, high)["frame"].clone();
    let mut fields = json!({"cmd": "build_frame", "kind": "roof", "roof": roof,
                            "part": part("beam148"), "walls": [a, b], "spacing": 600,
                            "overhang": 300, "fastener": part(fastener)});
    if roof == "gable" {
        fields["pitch"] = json!("30 deg");
    }
    let made = command(&mut doc, fields).unwrap_or_else(|e| panic!("build_frame: {e}"));
    all_ok(&doc);
    (doc, made)
}

/// The members of a frame by component, with their place (`rafter.a.3`).
fn members(doc: &Document<MockKernel>, lead: &Value) -> BTreeMap<ComponentUid, String> {
    parts_of(doc)
        .into_iter()
        .filter(|p| p["frame"] == *lead)
        .map(|p| {
            (
                uid(&p["component"]),
                p["member"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The components a junction id names.
fn sides(junction: &str) -> Vec<ComponentUid> {
    junction
        .split('|')
        .map(|s| s.split(':').next().unwrap().parse().unwrap())
        .collect()
}

/// Every fastener of the roof: its shaft through both members of its
/// junction (the ridge board and a rafter), square to the ridge board's
/// side (horizontal, across the walls), its head top on the entry face
/// within 1e-6 mm.
fn check_roof_fasteners(doc: &Document<MockKernel>, lead: &Value) -> usize {
    let roof = members(doc, lead);
    let ridge = roof
        .iter()
        .find(|(_, m)| *m == "ridge")
        .map(|(c, _)| *c)
        .expect("a ridge board");
    let mut junctions = BTreeMap::new();
    for f in parts_of(doc).iter().filter(|p| p["role"] == "fastener") {
        let junction = f["junction"].as_str().expect("at a junction");
        let pair = sides(junction);
        assert!(pair.contains(&ridge), "{junction}: not at the ridge board");
        let rafter = pair.iter().find(|c| **c != ridge).unwrap();
        assert!(roof[rafter].starts_with("rafter."), "{junction}");
        let seat = &f["seating"];
        let top = seat["head_top_distance"].as_f64().unwrap();
        assert!(
            top.abs() <= 1e-6,
            "{f}: head top {top} mm from the entry face"
        );
        assert_eq!(
            seat["members"], 2,
            "{f}: its shaft not through both members"
        );
        assert_eq!(seat["of"], 2, "{f}");
        // Its shaft: horizontal, across the walls (square to the ridge
        // board's side), from the far side of the ridge board.
        let c = uid(&f["component"]);
        let m = doc.member(c).unwrap();
        let z = m.placement.apply_vector([0.0, 0.0, 1.0]);
        assert!(
            z[2].abs() < 1e-9 && (z[1].abs() - 1.0).abs() < 1e-9,
            "{f}: shaft {z:?}"
        );
        let entry = doc
            .entry_point(doc.part_info(c).unwrap().record.joint.unwrap())
            .unwrap();
        let r = doc.member(ridge).unwrap();
        let (lo, hi) = r.extent(z);
        assert!(
            (dot(entry, z) - lo).abs() < 1e-6,
            "{f}: entry {entry:?} not on the ridge board's far side ({lo}, {hi})"
        );
        *junctions.entry(junction.to_owned()).or_insert(0) += 1;
    }
    // Two at each rafter's junction with the ridge board, every rafter's.
    let rafters = roof.values().filter(|m| m.starts_with("rafter.")).count();
    assert_eq!(junctions.len(), rafters, "{junctions:?}");
    assert!(junctions.values().all(|n| *n == 2), "{junctions:?}");
    junctions.len()
}

/// No junction joins two of the roof's rafters: a pair's plumb cut ends
/// stand on the two sides of the ridge board, 48 mm apart.
fn no_rafter_pairs(doc: &Document<MockKernel>, lead: &Value) {
    let roof = members(doc, lead);
    for j in doc.build_junctions(None) {
        let both = [j.a.component, j.b.component]
            .iter()
            .all(|c| roof.get(c).is_some_and(|m| m.starts_with("rafter.")));
        assert!(!both, "a junction of two rafters: {}", j.id);
    }
}

#[test]
fn a_gable_roofs_nails_go_through_the_ridge_board_into_each_rafter() {
    let (doc, made) = roof("gable", 2400.0, "ring-nail");
    let lead = made["frame"].clone();
    assert_eq!(made["fasteners"], 16, "{made}");
    assert_eq!(check_roof_fasteners(&doc, &lead), 16);
    no_rafter_pairs(&doc, &lead);
    // A rafter's junction with the ridge board: its plumb cut end on the
    // board's side, the cut face's middle where the junction is.
    let roof = members(&doc, &lead);
    let rafter = roof
        .iter()
        .find(|(_, m)| *m == "rafter.a.1")
        .map(|(c, _)| *c)
        .unwrap();
    let ridge = roof
        .iter()
        .find(|(_, m)| *m == "ridge")
        .map(|(c, _)| *c)
        .unwrap();
    // Its junction with the ridge board (it also crosses the wall's plate).
    let j = doc
        .build_junctions(None)
        .into_iter()
        .find(|j| j.a.component == rafter && j.b.component == ridge)
        .expect("rafter.a.1's junction with the ridge board");
    assert_eq!(j.a.at, "end_b");
    assert_eq!(j.kind, "T");
    // The ridge board's side faces the rafter at y = 1500 - 24.
    let at: V3 = j.a.frame.translation;
    assert!((at[1] - 1476.0).abs() < 1e-3, "{at:?}");
    let out = super::geometry::z_axis(&j.a.frame);
    assert!(norm(sub(out, [0.0, 1.0, 0.0])) < 1e-6, "{out:?}");
}

#[test]
fn a_gable_roofs_countersunk_screws_sit_flush_through_both_members() {
    let (doc, made) = roof("gable", 2400.0, "proud-screw");
    let lead = made["frame"].clone();
    assert_eq!(made["fasteners"], 16, "{made}");
    assert_eq!(check_roof_fasteners(&doc, &lead), 16);
    no_rafter_pairs(&doc, &lead);
    // Laid out again (another spacing): the same, at every rafter.
    let mut doc = doc;
    let edited = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": lead, "frame": {"spacing": 400}}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(edited["fasteners"], 22, "{edited}");
    assert_eq!(check_roof_fasteners(&doc, &lead), 22);
}

#[test]
fn a_mono_pitch_roof_has_no_fastener_off_its_members() {
    let (doc, made) = roof("mono", 3000.0, "ring-nail");
    let lead = made["frame"].clone();
    // Its rafters are notched over both walls and meet nothing of the roof:
    // no junction of two of its members, no fastener in the air.
    assert_eq!(made["fasteners"], 0, "{made}");
    assert!(
        parts_of(&doc).iter().all(|p| p["role"] != "fastener"),
        "{:?}",
        parts_of(&doc)
    );
    no_rafter_pairs(&doc, &lead);
    // Each rafter meets each wall's top plate where it bears on it (a
    // junction with the walls, which the roof does not fasten).
    let roof = members(&doc, &lead);
    let with_walls = doc
        .build_junctions(None)
        .into_iter()
        .filter(|j| roof.contains_key(&j.a.component) != roof.contains_key(&j.b.component))
        .count();
    assert!(
        with_walls >= 2 * roof.len(),
        "{with_walls} junctions with the walls for {} rafters",
        roof.len()
    );
}
