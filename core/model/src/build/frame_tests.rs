// SPDX-License-Identifier: MIT
//! Timber frames (mitcad#479): a wall with a window, a floor with blocking
//! and a stair opening, a gable and a mono-pitch roof over two walls, a
//! corner between walls, the frames edited and their fasteners, on the test
//! library's timber parts.

use serde_json::{Value, json};

use super::test_library::{all_ok, command, design, part, query};
use crate::Document;
use crate::testing::MockKernel;

fn frame(doc: &mut Document<MockKernel>, fields: Value) -> Value {
    let mut c = fields;
    c["cmd"] = json!("build_frame");
    command(doc, c).unwrap_or_else(|e| panic!("build_frame: {e}"))
}

fn edit(doc: &mut Document<MockKernel>, component: &Value, fields: Value) -> Value {
    command(
        doc,
        json!({"cmd": "build_edit", "component": component, "frame": fields}),
    )
    .unwrap_or_else(|e| panic!("build_edit: {e}"))
}

fn parts_of(doc: &Document<MockKernel>) -> Vec<Value> {
    query(doc, json!({"query": "build_parts"}))["parts"]
        .as_array()
        .unwrap()
        .clone()
}

/// A frame's members as `build_parts` gives them, by their place.
fn members(doc: &Document<MockKernel>, lead: &Value) -> Vec<Value> {
    parts_of(doc)
        .into_iter()
        .filter(|p| p["frame"] == *lead)
        .collect()
}

fn member<'a>(list: &'a [Value], key: &str) -> &'a Value {
    list.iter()
        .find(|p| p["member"] == key)
        .unwrap_or_else(|| panic!("no member {key}: {:?}", keys(list)))
}

fn keys(list: &[Value]) -> Vec<String> {
    list.iter()
        .map(|p| p["member"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn count_of(list: &[Value], prefix: &str) -> usize {
    list.iter()
        .filter(|p| p["member"].as_str().is_some_and(|k| k.starts_with(prefix)))
        .count()
}

fn length(p: &Value) -> f64 {
    p["length"].as_f64().unwrap()
}

/// A member's end A, in the design.
fn end_a(doc: &Document<MockKernel>, p: &Value) -> [f64; 3] {
    let c: crate::ids::ComponentUid = p["component"].as_str().unwrap().parse().unwrap();
    let o = doc
        .assembly()
        .occurrences
        .iter()
        .find(|o| o.component == c)
        .unwrap()
        .uid;
    doc.placement(o).unwrap().translation
}

fn wall(doc: &mut Document<MockKernel>, from: [f64; 3], to: [f64; 3], extra: Value) -> Value {
    let mut fields = json!({"kind": "wall", "part": part("rows98"), "start": from, "end": to,
                            "height": 2400, "spacing": 600});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    frame(doc, fields)
}

#[test]
fn a_wall_3600_by_2400_with_a_window_at_600() {
    let mut doc = design();
    let window = json!([{"offset": 600, "width": 900, "height": 1200, "sill": 900}]);
    // The preview lays it out without making it.
    let preview = query(
        &doc,
        json!({"query": "build_preview", "frame": {"kind": "wall", "part": part("rows98"),
               "start": [0, 0, 0], "end": [3600, 0, 0], "height": 2400, "openings": window}}),
    );
    assert_eq!(preview["count"], 16, "{preview}");
    assert_eq!(preview["roles"]["stud"], 5, "{preview}");
    let made = wall(
        &mut doc,
        [0.0, 0.0, 0.0],
        [3600.0, 0.0, 0.0],
        json!({"openings": window}),
    );
    all_ok(&doc);
    assert_eq!(made["name"], "Wall 1");
    assert_eq!(made["kind"], "wall");
    assert_eq!(made["count"], 16, "{made}");
    let lead = made["frame"].clone();
    let list = members(&doc, &lead);
    assert_eq!(list.len(), 16, "{:?}", keys(&list));
    // Plates 3600, studs between them 2400 - 2 x 48.
    assert!((length(member(&list, "plate.top.0")) - 3600.0).abs() < 1e-6);
    assert!((length(member(&list, "plate.bottom.0")) - 3600.0).abs() < 1e-6);
    for key in ["stud.start", "stud.end", "stud.3", "stud.4", "stud.5"] {
        assert!((length(member(&list, key)) - 2304.0).abs() < 1e-6, "{key}");
    }
    // The stud at 600 gives way to the opening's king and trimmer studs;
    // the one at 1200 is in it: cripples above the header and below the sill.
    assert_eq!(count_of(&list, "stud.1"), 0);
    assert_eq!(count_of(&list, "stud.2"), 0);
    let king = member(&list, "opening.1.king.a");
    assert!((length(king) - 2304.0).abs() < 1e-6);
    let trimmer = member(&list, "opening.1.trimmer.a");
    assert!((length(trimmer) - 2052.0).abs() < 1e-6);
    // Studs stand at their centres: the trimmer's at 600 - 24, its foot on
    // the bottom plate.
    let at = end_a(&doc, trimmer);
    assert!(
        (at[0] - 576.0).abs() < 1e-6 && at[1].abs() < 1e-6 && (at[2] - 48.0).abs() < 1e-6,
        "{at:?}"
    );
    assert!((length(member(&list, "opening.1.header.0")) - 996.0).abs() < 1e-6);
    assert!(member(&list, "opening.1.header.1")["role"] == "profile");
    assert!((length(member(&list, "opening.1.sill")) - 900.0).abs() < 1e-6);
    assert!((length(member(&list, "opening.1.cripple.top.2")) - 154.0).abs() < 1e-6);
    assert!((length(member(&list, "opening.1.cripple.bottom.2")) - 804.0).abs() < 1e-6);
    // Every member a normal BUILD part of the library's timber.
    assert!(
        list.iter()
            .all(|p| p["kind"] == "timber" && p["frame"] == lead)
    );
    let lead_part = list.iter().find(|p| p["component"] == lead).unwrap();
    assert_eq!(lead_part["member"], "plate.top.0");
    assert_eq!(lead_part["framing"]["count"], 16);
    assert_eq!(lead_part["framing"]["definition"]["name"], "Wall 1");

    // The cut list: the frame's members with the other timber, by size.
    let list_rows = query(&doc, json!({"query": "build_cut_list"}));
    assert_eq!(list_rows["timber"], true);
    let rows = list_rows["rows"].as_array().unwrap();
    let studs = rows
        .iter()
        .find(|r| {
            r["length"]
                .as_f64()
                .is_some_and(|l| (l - 2304.0).abs() < 1e-6)
        })
        .unwrap();
    // Five studs and the two kings, one row.
    assert_eq!(studs["quantity"], 7, "{rows:?}");
    let total: u64 = rows.iter().map(|r| r["quantity"].as_u64().unwrap()).sum();
    assert_eq!(total, 16);

    // The spacing changes: studs at 400.
    let edited = edit(&mut doc, &lead, json!({"spacing": 400}));
    all_ok(&doc);
    assert_eq!(edited["changed"], json!(["frame"]));
    assert_eq!(edited["count"], 20, "{edited}");
    let list = members(&doc, &lead);
    assert_eq!(list.len(), 20);
    assert!(list.iter().any(|p| p["member"] == "stud.1"));
    assert_eq!(count_of(&list, "opening.1.cripple"), 4);
    // The kept members are the same components.
    let after = member(&list, "opening.1.trimmer.a");
    assert_eq!(after["component"], trimmer["component"]);

    // The opening moves to 1200 and gets wider: its studs go with it.
    let edited = edit(
        &mut doc,
        &lead,
        json!({"openings": [{"offset": 1200, "width": 1000, "height": 1200, "sill": 900}]}),
    );
    all_ok(&doc);
    let list = members(&doc, &lead);
    let moved = member(&list, "opening.1.trimmer.a");
    assert_eq!(moved["component"], trimmer["component"]);
    assert!((end_a(&doc, moved)[0] - 1176.0).abs() < 1e-6);
    assert!((length(member(&list, "opening.1.header.0")) - 1096.0).abs() < 1e-6);
    assert_eq!(edited["count"], list.len());

    // Length and height: the plates and studs follow.
    edit(&mut doc, &lead, json!({"length": 4200, "height": 2700}));
    all_ok(&doc);
    let list = members(&doc, &lead);
    assert!((length(member(&list, "plate.top.0")) - 4200.0).abs() < 1e-6);
    assert!((length(member(&list, "stud.end")) - 2604.0).abs() < 1e-6);

    // A door: the bottom plate cut at it, no sill.
    edit(
        &mut doc,
        &lead,
        json!({"openings": [{"offset": 1200, "width": 900, "height": 2100}]}),
    );
    all_ok(&doc);
    let list = members(&doc, &lead);
    assert!((length(member(&list, "plate.bottom.0.0")) - 1200.0).abs() < 1e-6);
    assert!((length(member(&list, "plate.bottom.0.1")) - 2100.0).abs() < 1e-6);
    assert_eq!(count_of(&list, "opening.1.sill"), 0);

    // An opening that does not fit is refused, and nothing changes.
    let refused = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": lead, "frame": {
            "openings": [{"offset": 3900, "width": 900, "height": 1200, "sill": 900}]}}),
    )
    .unwrap_err();
    assert!(refused.contains("does not fit"), "{refused}");

    // The whole frame goes at once.
    let removed = command(
        &mut doc,
        json!({"cmd": "build_remove", "component": lead, "set": true}),
    )
    .unwrap();
    assert_eq!(
        removed["removed"]["components"].as_array().unwrap().len(),
        list.len()
    );
    assert!(parts_of(&doc).is_empty());
}

#[test]
fn a_frame_is_one_undo_step_and_takes_its_fasteners() {
    let mut doc = design();
    let made = wall(
        &mut doc,
        [0.0, 0.0, 0.0],
        [1800.0, 0.0, 0.0],
        json!({"fastener": part("wood-screw")}),
    );
    all_ok(&doc);
    // Studs at 0, 600 (centre), 1200 and the end: each end on both plates.
    assert_eq!(made["count"], 6, "{made}");
    assert_eq!(made["fasteners"], 8, "{made}");
    let lead = made["frame"].clone();
    let screws = parts_of(&doc)
        .into_iter()
        .filter(|p| p["role"] == "fastener")
        .count();
    assert_eq!(screws, 16);
    assert_eq!(doc.undo_label(), Some("Place wall frame Wall 1"));
    // Laid out again: the fasteners anew, one undo step.
    let edited = edit(&mut doc, &lead, json!({"spacing": 400}));
    all_ok(&doc);
    // Studs at 0, 400, 800, 1200, 1600 and the end.
    assert_eq!(edited["count"], 8, "{edited}");
    assert_eq!(edited["fasteners"], 12, "{edited}");
    let screws = parts_of(&doc)
        .into_iter()
        .filter(|p| p["role"] == "fastener")
        .count();
    assert_eq!(screws, 24);
    assert_eq!(doc.undo_label(), Some("Edit Wall 1"));
    doc.undo();
    assert_eq!(members(&doc, &lead).len(), 6);
    doc.undo();
    assert!(parts_of(&doc).is_empty());
    doc.redo();
    // Removing the frame takes its fasteners.
    command(
        &mut doc,
        json!({"cmd": "build_remove", "component": lead, "set": true}),
    )
    .unwrap();
    assert!(parts_of(&doc).is_empty(), "{:?}", parts_of(&doc));
}

#[test]
fn a_corner_and_a_t_junction_between_wall_frames() {
    let mut doc = design();
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [3600.0, 0.0, 0.0], json!({}));
    let lead1 = w1["frame"].clone();
    // The second wall's start butts against the first one's end: a corner.
    let w2 = wall(
        &mut doc,
        [3600.0, 0.0, 0.0],
        [3600.0, 3000.0, 0.0],
        json!({"joins": {"start": lead1}}),
    );
    all_ok(&doc);
    assert_eq!(w2["relaid"], json!([lead1]), "{w2}");
    let lead2 = w2["frame"].clone();
    // The second starts at the first one's inner face; the first runs to
    // the second's outer face and has a stud for the corner's inside.
    let second = members(&doc, &lead2);
    let plate = member(&second, "plate.top.0");
    assert!(
        (length(plate) - (3000.0 - 49.0)).abs() < 1e-6,
        "{}",
        length(plate)
    );
    assert!((end_a(&doc, plate)[1] - 49.0).abs() < 1e-6);
    let first = members(&doc, &lead1);
    assert!((length(member(&first, "plate.top.0")) - 3649.0).abs() < 1e-6);
    let corner = member(&first, "junction.Wall_2.0");
    // Its near face at 3600 - 49 - 48.
    assert!((end_a(&doc, corner)[0] - (3503.0 + 24.0)).abs() < 1e-6);
    // A third wall in the middle of the first: a T, a stud each side.
    let w3 = wall(
        &mut doc,
        [1800.0, 0.0, 0.0],
        [1800.0, 2000.0, 0.0],
        json!({"joins": {"start": lead1}}),
    );
    all_ok(&doc);
    let first = members(&doc, &lead1);
    let a = member(&first, "junction.Wall_3.0.a");
    let b = member(&first, "junction.Wall_3.0.b");
    assert!((end_a(&doc, a)[0] - (1800.0 - 49.0 - 24.0)).abs() < 1e-6);
    assert!((end_a(&doc, b)[0] - (1800.0 + 49.0 + 24.0)).abs() < 1e-6);
    // The stud at 1800 stays between them (under the third wall's end).
    assert_eq!(count_of(&first, "stud.3"), 1);
    let _ = w3;
    // Moving the first wall's end along: the second follows its corner.
    edit(&mut doc, &lead1, json!({"length": 4000}));
    all_ok(&doc);
    let first = members(&doc, &lead1);
    assert!((length(member(&first, "plate.top.0")) - 4000.0).abs() < 1e-6);
}

#[test]
fn a_floor_frame_4000_by_3000_at_400() {
    let mut doc = design();
    let outline = json!([[0, 0, 0], [4000, 0, 0], [4000, 3000, 0], [0, 3000, 0]]);
    let made = frame(
        &mut doc,
        json!({"kind": "floor", "part": part("beam148"), "outline": outline, "spacing": 400}),
    );
    all_ok(&doc);
    assert_eq!(made["name"], "Floor 1");
    let lead = made["frame"].clone();
    let list = members(&doc, &lead);
    // Four rims, the ones across the joists through the corners, and nine
    // joists between them.
    assert_eq!(count_of(&list, "rim."), 4);
    assert!((length(member(&list, "rim.0")) - 4000.0).abs() < 1e-6);
    assert!((length(member(&list, "rim.1")) - 2904.0).abs() < 1e-6);
    assert_eq!(count_of(&list, "joist."), 9, "{:?}", keys(&list));
    let joist = member(&list, "joist.1");
    assert!((length(joist) - 2904.0).abs() < 1e-6);
    // On edge, its foot at the outline's height: its axis 74 up, from the
    // first rim's inner face.
    let at = end_a(&doc, joist);
    assert!(
        (at[0] - 400.0).abs() < 1e-6 && (at[1] - 48.0).abs() < 1e-6 && (at[2] - 74.0).abs() < 1e-6,
        "{at:?}"
    );
    // At 600: six joists.
    let edited = edit(&mut doc, &lead, json!({"spacing": 600}));
    all_ok(&doc);
    assert_eq!(edited["count"], 10);
    // A row of blocking: one between each pair of neighbours.
    let edited = edit(&mut doc, &lead, json!({"spacing": 400, "blocking": 1}));
    all_ok(&doc);
    let list = members(&doc, &lead);
    assert_eq!(count_of(&list, "block."), 10, "{edited}");
    assert!((length(member(&list, "block.1.0")) - 328.0).abs() < 1e-6);
    // A stair opening with doubled trimmers and headers, tail joists.
    let edited = edit(
        &mut doc,
        &lead,
        json!({"blocking": 0, "openings": [{"offset": [1200, 800], "size": [900, 1500]}]}),
    );
    all_ok(&doc);
    let list = members(&doc, &lead);
    assert_eq!(count_of(&list, "opening.1.trimmer"), 4, "{edited}");
    assert_eq!(count_of(&list, "opening.1.header"), 4);
    assert!((length(member(&list, "opening.1.header.a.0")) - 900.0).abs() < 1e-6);
    assert!((length(member(&list, "opening.1.trimmer.a.0")) - 2904.0).abs() < 1e-6);
    // The joists at 1600 and 2000 end at the headers; the one at 1200 is
    // where the trimmers are.
    assert_eq!(count_of(&list, "joist.3"), 0);
    assert!((length(member(&list, "joist.4.tail.a")) - (704.0 - 48.0)).abs() < 1e-6);
    assert!((length(member(&list, "joist.4.tail.b")) - (2952.0 - 2396.0)).abs() < 1e-6);
    assert!(count_of(&list, "joist.5.tail") == 2);
}

/// Two walls 4000 long along X, 3000 apart (centres), 2400 high.
fn two_walls(doc: &mut Document<MockKernel>, high: f64) -> (Value, Value) {
    let a = wall(doc, [0.0, 0.0, 0.0], [4000.0, 0.0, 0.0], json!({}));
    let b = wall(
        doc,
        [0.0, 3000.0, 0.0],
        [4000.0, 3000.0, 0.0],
        json!({"height": high}),
    );
    (a["frame"].clone(), b["frame"].clone())
}

#[test]
fn a_gable_roof_over_two_walls() {
    let mut doc = design();
    let (a, b) = two_walls(&mut doc, 2400.0);
    let made = frame(
        &mut doc,
        json!({"kind": "roof", "roof": "gable", "part": part("beam148"), "walls": [a, b],
               "pitch": "30 deg", "spacing": 600, "overhang": 300}),
    );
    all_ok(&doc);
    let lead = made["frame"].clone();
    let list = members(&doc, &lead);
    // Eight rafters a side (the ends flush with the walls'), the ridge board.
    assert_eq!(count_of(&list, "rafter.a."), 8, "{:?}", keys(&list));
    assert_eq!(count_of(&list, "rafter.b."), 8);
    let ridge = member(&list, "ridge");
    assert_eq!(ridge["component"], lead);
    assert!((length(ridge) - 4000.0).abs() < 1e-6);
    // The 148 ridge board is shallower than the rafters' plumb cut.
    let warnings = made["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("plumb cut")),
        "{warnings:?}"
    );
    // A rafter: plumb cut at both ends at the pitch, a birdsmouth over its
    // wall's top plate.
    let rafter = member(&list, "rafter.a.start");
    let ends = &rafter["ends"];
    let pitch = 30f64.to_radians();
    assert!(
        (ends["a"]["angle"].as_f64().unwrap() - pitch).abs() < 1e-9,
        "{ends}"
    );
    assert!(
        (ends["b"]["angle"].as_f64().unwrap() - pitch).abs() < 1e-9,
        "{ends}"
    );
    // Plumb: end A leaning up (its long point the bottom edge), end B down.
    assert_eq!(ends["a"]["tilt"], "y", "{ends}");
    assert_eq!(ends["b"]["tilt"], "-y", "{ends}");
    let cuts = rafter["cuts"].as_array().unwrap();
    assert_eq!(cuts.len(), 1);
    assert_eq!(cuts[0]["kind"], "birdsmouth");
    assert_eq!(cuts[0]["with"], a);
    // Its run: from 300 beyond the wall's outer face (at -49) to the
    // ridge board's face (1500 - 24); plumb cut, its long points are the
    // bottom edge at the eave and the top edge at the ridge.
    let run = 1476.0 + 49.0 + 300.0;
    let expected = run / pitch.cos() + 148.0 * pitch.tan();
    assert!(
        (length(rafter) - expected).abs() < 1e-3,
        "{} {expected}",
        length(rafter)
    );
    // End A on its axis, half its depth above the bottom edge's tip at the
    // eave (at y -349, 2400 - (349 + 49) tan 30 high), lying on edge.
    let at = end_a(&doc, rafter);
    let tip = 2400.0 - 398.0 * pitch.tan();
    let axis = [24.0, -349.0 - 74.0 * pitch.sin(), tip + 74.0 * pitch.cos()];
    assert!(
        (0..3).all(|k| (at[k] - axis[k]).abs() < 1e-6),
        "{at:?} {axis:?}"
    );
    // The birdsmouth's box holds the wall's outer top edge (where the heel
    // cut meets the seat) and the plate's inner top edge, under the rafter.
    let c: crate::ids::ComponentUid = rafter["component"].as_str().unwrap().parse().unwrap();
    let placement = {
        let o = doc
            .assembly()
            .occurrences
            .iter()
            .find(|o| o.component == c)
            .unwrap()
            .uid;
        doc.placement(o).unwrap()
    };
    let part = super::test_library::derived(&doc, c);
    let Some(crate::features::FeatureDef::DerivedPart(d)) = doc.feature(part).map(|e| &e.def)
    else {
        panic!("no derived part");
    };
    let cut = &d.cuts[0];
    let params = doc.parameters();
    let origin = cut.origin.map(|id| params.value(id).unwrap());
    let size = cut.size.map(|id| params.value(id).unwrap());
    let z = super::geometry::cross(cut.x, cut.y);
    let inside = |host: [f64; 3]| {
        let local = super::geometry::inverse(&placement).apply_point(host);
        let rel = super::geometry::sub(local, origin);
        [cut.x, cut.y, z]
            .iter()
            .zip(size)
            .all(|(axis, s)| (-1e-6..=s + 1e-6).contains(&super::geometry::dot(rel, *axis)))
    };
    assert!(inside([24.0, -49.0, 2400.0]));
    assert!(inside([24.0, 49.0, 2400.0]));
    assert!(!inside([24.0, -60.0, 2400.0]), "the eave is not cut");
    assert!(
        !inside([24.0, 0.0, 2401.0]),
        "nothing above the plate is cut"
    );
    // The cut list: the rafters one row, notched.
    let cut_list = query(&doc, json!({"query": "build_cut_list"}));
    let rows = cut_list["rows"].as_array().unwrap();
    let rafters = rows
        .iter()
        .find(|r| {
            r["length"]
                .as_f64()
                .is_some_and(|l| (l - expected).abs() < 1e-3)
        })
        .unwrap();
    assert_eq!(rafters["quantity"], 16);
    assert!(
        rafters["machining"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["kind"] == "birdsmouth")
    );
    // Rafters at 400: eleven a side.
    let edited = edit(&mut doc, &lead, json!({"spacing": 400}));
    all_ok(&doc);
    assert_eq!(edited["count"], 23, "{edited}");
    // A wall's spacing changed: the roof laid out again with it.
    let relaid = edit(&mut doc, &a, json!({"spacing": 400}));
    all_ok(&doc);
    assert_eq!(relaid["relaid"], json!([lead]), "{relaid}");
}

#[test]
fn a_mono_pitch_roof_from_the_lower_wall() {
    let mut doc = design();
    let (a, b) = two_walls(&mut doc, 3000.0);
    let made = frame(
        &mut doc,
        json!({"kind": "roof", "roof": "mono", "part": part("beam148"), "walls": [a, b]}),
    );
    all_ok(&doc);
    assert_eq!(made["warnings"], json!([]), "{made}");
    let list = members(&doc, &made["frame"]);
    assert_eq!(count_of(&list, "rafter."), 8, "{:?}", keys(&list));
    assert_eq!(count_of(&list, "ridge"), 0);
    // Notched over both walls.
    let rafter = member(&list, "rafter.1");
    assert_eq!(rafter["cuts"].as_array().unwrap().len(), 2);
    // Walls of one height need a pitch.
    let mut doc = design();
    let (a, b) = two_walls(&mut doc, 2400.0);
    let refused = command(
        &mut doc,
        json!({"cmd": "build_frame", "kind": "roof", "roof": "mono", "part": part("beam148"),
               "walls": [a, b]}),
    )
    .unwrap_err();
    assert!(refused.contains("needs its pitch"), "{refused}");
}
