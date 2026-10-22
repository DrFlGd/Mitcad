// SPDX-License-Identifier: MIT
//! Timber frames (mitcad#479): a wall with a window, a floor with blocking
//! and a stair opening, a gable and a mono-pitch roof over two walls, a
//! corner between walls, walls started on another wall's side or end face,
//! butting or with lapped plates (mitcad#493, mitcad#510), the frames edited
//! and their fasteners, on the test library's timber parts.

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
    // the second's outer face and has a stud for the corner's inside. The
    // plates lap (mitcad#493): the second's run on over the first to its
    // outer face.
    let second = members(&doc, &lead2);
    let plate = member(&second, "plate.top.0");
    assert!(
        (length(plate) - (3000.0 + 49.0)).abs() < 1e-6,
        "{}",
        length(plate)
    );
    assert!((end_a(&doc, plate)[1] + 49.0).abs() < 1e-6);
    assert!((end_a(&doc, member(&second, "stud.start"))[1] - 73.0).abs() < 1e-6);
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

// --- Walls started on another wall's side (mitcad#493).

/// A frame's members as its preview lays them out: (place, box min, box
/// max) in the design, from each member's axis, `x` and section.
fn boxes(doc: &Document<MockKernel>, lead: &Value) -> Vec<(String, [f64; 3], [f64; 3])> {
    let preview = query(
        doc,
        json!({"query": "build_preview", "frame": {}, "component": lead}),
    );
    let v = |p: &Value| -> [f64; 3] {
        let a = p.as_array().unwrap();
        [0, 1, 2].map(|i| a[i].as_f64().unwrap())
    };
    preview["members"]
        .as_array()
        .unwrap_or_else(|| panic!("{preview}"))
        .iter()
        .map(|m| {
            let (s, e, x) = (v(&m["start"]), v(&m["end"]), v(&m["x"]));
            let w = m["section"][0].as_f64().unwrap();
            let t = m["section"][1].as_f64().unwrap();
            let l = ((e[0] - s[0]).powi(2) + (e[1] - s[1]).powi(2) + (e[2] - s[2]).powi(2)).sqrt();
            let d = [0, 1, 2].map(|i| (e[i] - s[i]) / l);
            let y = [
                d[1] * x[2] - d[2] * x[1],
                d[2] * x[0] - d[0] * x[2],
                d[0] * x[1] - d[1] * x[0],
            ];
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for a in [0.0, l] {
                for b in [-w / 2.0, w / 2.0] {
                    for c in [-t / 2.0, t / 2.0] {
                        for i in 0..3 {
                            let p = s[i] + d[i] * a + x[i] * b + y[i] * c;
                            lo[i] = lo[i].min(p);
                            hi[i] = hi[i].max(p);
                        }
                    }
                }
            }
            (m["member"].as_str().unwrap().to_owned(), lo, hi)
        })
        .collect()
}

/// The pairs of members of two frames whose boxes overlap (by more than
/// 1e-6 mm each way).
fn overlaps(doc: &Document<MockKernel>, a: &Value, b: &Value) -> Vec<(String, String)> {
    let (one, two) = (boxes(doc, a), boxes(doc, b));
    let mut out = Vec::new();
    for (ka, la, ha) in &one {
        for (kb, lb, hb) in &two {
            if (0..3).all(|i| ha[i].min(hb[i]) - la[i].max(lb[i]) > 1e-6) {
                out.push((ka.clone(), kb.clone()));
            }
        }
    }
    out.sort();
    out
}

/// A member's half laps: `with` each.
fn half_laps(p: &Value) -> Vec<Value> {
    p["cuts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "half_lap")
        .map(|c| c["with"].clone())
        .collect()
}

/// A wall started on the side of the wall `on` at `click`, square to it,
/// clicked to `end` or `length` long.
fn wall_on(doc: &mut Document<MockKernel>, on: &Value, click: [f64; 3], extra: Value) -> Value {
    let mut fields = json!({"kind": "wall", "part": part("rows98"), "start": click,
                            "joins": {"start": on}, "square": true, "height": 2400,
                            "spacing": 600});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    frame(doc, fields)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// A wall's start and end as its lead's definition keeps them.
fn ends_of(doc: &Document<MockKernel>, lead: &Value) -> ([f64; 3], [f64; 3]) {
    let p = parts_of(doc)
        .into_iter()
        .find(|p| p["component"] == *lead)
        .unwrap();
    let def = &p["framing"]["definition"];
    let v = |p: &Value| -> [f64; 3] { [0, 1, 2].map(|i| p[i].as_f64().unwrap()) };
    assert!(def.get("square").is_none(), "{def}");
    (v(&def["start"]), v(&def["end"]))
}

#[test]
fn an_l_corner_from_a_click_near_a_walls_end() {
    let mut doc = design();
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [3600.0, 0.0, 0.0], json!({}));
    let lead1 = w1["frame"].clone();
    // The preview names the corner before anything is made.
    let preview = query(
        &doc,
        json!({"query": "build_preview", "frame": {"kind": "wall", "part": part("rows98"),
               "start": [3500, 49, 1000], "end": [3300, 3000, 1000],
               "joins": {"start": lead1}, "square": true}}),
    );
    assert_eq!(
        preview["junction"],
        json!({"kind": "L", "corner": "lapped", "wall": "Wall 1", "frame": lead1, "end": "b",
               "at": 3600.0, "length": 3000.0}),
        "{preview}"
    );
    // A click on its +Y side 100 mm from its end B, the end clicked: an L
    // from that end's corner to +Y, as long as the end is along it.
    let w2 = wall_on(
        &mut doc,
        &lead1,
        [3500.0, 49.0, 1000.0],
        json!({"end": [3300, 3000, 1000]}),
    );
    all_ok(&doc);
    assert_eq!(w2["junction"]["kind"], "L", "{w2}");
    assert_eq!(w2["junction"]["end"], "b", "{w2}");
    assert_eq!(w2["relaid"], json!([lead1]), "{w2}");
    let lead2 = w2["frame"].clone();
    // At right angles from the corner, at the first one's base.
    let (s, e) = ends_of(&doc, &lead2);
    assert!(
        close(s[0], 3600.0) && close(s[1], 0.0) && close(s[2], 0.0),
        "{s:?}"
    );
    assert!(
        close(e[0], 3600.0) && close(e[1], 3000.0) && close(e[2], 0.0),
        "{e:?}"
    );
    // It butts: its start stud against the first one's face.
    let second = members(&doc, &lead2);
    let stud = end_a(&doc, member(&second, "stud.start"));
    assert!(
        close(stud[0], 3600.0) && close(stud[1], 49.0 + 24.0),
        "{stud:?}"
    );
    // The corner's studs: the first one's end stud under the second one's
    // end, and its stud just inside the corner.
    let first = members(&doc, &lead1);
    assert!(close(
        end_a(&doc, member(&first, "stud.end"))[0],
        3649.0 - 24.0
    ));
    assert!(close(
        end_a(&doc, member(&first, "junction.Wall_2.0"))[0],
        3600.0 - 49.0 - 24.0
    ));
    // The plates lap: the first one's to the second one's outer face, the
    // second one's over the first to its outer face, each half-lapped with
    // the other.
    for key in ["plate.top.0", "plate.bottom.0"] {
        let one = member(&first, key);
        let two = member(&second, key);
        assert!(close(length(one), 3649.0), "{key}: {}", length(one));
        assert!(close(length(two), 3049.0), "{key}: {}", length(two));
        assert!(close(end_a(&doc, two)[1], -49.0), "{key}");
        assert_eq!(half_laps(one), vec![lead2.clone()], "{key}");
        assert_eq!(half_laps(two), vec![lead1.clone()], "{key}");
    }
    // No overlap but the lapped plates.
    let lapped = vec![
        ("plate.bottom.0".to_owned(), "plate.bottom.0".to_owned()),
        ("plate.top.0".to_owned(), "plate.top.0".to_owned()),
    ];
    assert_eq!(overlaps(&doc, &lead1, &lead2), lapped);

    // A click on its -Y side near end A, the length typed: an L to -Y.
    let w3 = wall_on(
        &mut doc,
        &lead1,
        [180.0, -49.0, 300.0],
        json!({"length": "2000 mm"}),
    );
    all_ok(&doc);
    assert_eq!(
        w3["junction"],
        json!({"kind": "L", "corner": "lapped", "wall": "Wall 1", "frame": lead1, "end": "a",
               "at": 0.0, "length": 2000.0}),
        "{w3}"
    );
    let lead3 = w3["frame"].clone();
    let (s, e) = ends_of(&doc, &lead3);
    assert!(close(s[0], 0.0) && close(s[1], 0.0), "{s:?}");
    assert!(close(e[0], 0.0) && close(e[1], -2000.0), "{e:?}");
    let third = members(&doc, &lead3);
    assert!(close(length(member(&third, "plate.top.0")), 2049.0));
    assert!(close(end_a(&doc, member(&third, "stud.start"))[1], -73.0));
    let first = members(&doc, &lead1);
    assert!(close(length(member(&first, "plate.top.0")), 3698.0));
    assert!(close(
        end_a(&doc, member(&first, "junction.Wall_3.0"))[0],
        49.0 + 24.0
    ));
    assert_eq!(half_laps(member(&first, "plate.top.0")).len(), 2);
    assert_eq!(overlaps(&doc, &lead1, &lead3), lapped);
}

#[test]
fn a_t_from_a_click_mid_side() {
    let mut doc = design();
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [3600.0, 0.0, 0.0], json!({}));
    let lead1 = w1["frame"].clone();
    // A click on its -Y side at 2000, the length typed: a T to -Y.
    let w2 = wall_on(
        &mut doc,
        &lead1,
        [2000.0, -49.0, 1200.0],
        json!({"length": 2500}),
    );
    all_ok(&doc);
    assert_eq!(
        w2["junction"],
        json!({"kind": "T", "corner": "lapped", "wall": "Wall 1", "frame": lead1, "end": null,
               "at": 2000.0, "length": 2500.0}),
        "{w2}"
    );
    let lead2 = w2["frame"].clone();
    let (s, e) = ends_of(&doc, &lead2);
    assert!(close(s[0], 2000.0) && close(s[1], 0.0), "{s:?}");
    assert!(close(e[0], 2000.0) && close(e[1], -2500.0), "{e:?}");
    // It butts against the first one's face, its plates too.
    let second = members(&doc, &lead2);
    let plate = member(&second, "plate.top.0");
    assert!(close(length(plate), 2500.0 - 49.0));
    assert!(close(end_a(&doc, plate)[1], -49.0));
    assert!(half_laps(plate).is_empty());
    // The first gets a stud each side of it; its plates run on.
    let first = members(&doc, &lead1);
    assert!(close(
        end_a(&doc, member(&first, "junction.Wall_2.0.a"))[0],
        2000.0 - 49.0 - 24.0
    ));
    assert!(close(
        end_a(&doc, member(&first, "junction.Wall_2.0.b"))[0],
        2000.0 + 49.0 + 24.0
    ));
    assert!(close(length(member(&first, "plate.top.0")), 3600.0));
    assert!(half_laps(member(&first, "plate.top.0")).is_empty());
    assert_eq!(
        overlaps(&doc, &lead1, &lead2),
        Vec::<(String, String)>::new()
    );

    // A click on its +Y side at 1000, the end clicked off square: a T to
    // +Y, as long as the end is along it.
    let w3 = wall_on(
        &mut doc,
        &lead1,
        [1000.0, 49.0, 300.0],
        json!({"end": [1500, 2800, 0]}),
    );
    all_ok(&doc);
    assert_eq!(w3["junction"]["kind"], "T", "{w3}");
    assert_eq!(w3["junction"]["at"], 1000.0, "{w3}");
    let lead3 = w3["frame"].clone();
    let (s, e) = ends_of(&doc, &lead3);
    assert!(close(s[0], 1000.0) && close(s[1], 0.0), "{s:?}");
    assert!(close(e[0], 1000.0) && close(e[1], 2800.0), "{e:?}");
    let first = members(&doc, &lead1);
    assert_eq!(count_of(&first, "junction.Wall_3.0."), 2);
    assert_eq!(
        overlaps(&doc, &lead1, &lead3),
        Vec::<(String, String)>::new()
    );

    // An end on the other side of the wall is refused.
    let refused = command(
        &mut doc,
        json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
               "start": [2600, 49, 0], "end": [2600, -1000, 0], "joins": {"start": lead1},
               "square": true}),
    )
    .unwrap_err();
    assert!(refused.contains("not beyond its side"), "{refused}");
}

#[test]
fn two_top_plates_lap_at_an_l_corner() {
    let mut doc = design();
    let two = json!({"plates": {"top": 2}});
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [3600.0, 0.0, 0.0], two.clone());
    let lead1 = w1["frame"].clone();
    let w2 = wall_on(
        &mut doc,
        &lead1,
        [3550.0, 49.0, 2000.0],
        json!({"length": 3000, "plates": {"top": 2}}),
    );
    all_ok(&doc);
    let lead2 = w2["frame"].clone();
    let (first, second) = (members(&doc, &lead1), members(&doc, &lead2));
    // The upper top plate of the wall that butts runs over the corner, the
    // other's stops at its inner face; the lower ones as at a butt.
    assert!(close(length(member(&second, "plate.top.0")), 3049.0));
    assert!(close(length(member(&second, "plate.top.1")), 2951.0));
    assert!(close(length(member(&first, "plate.top.0")), 3551.0));
    assert!(close(length(member(&first, "plate.top.1")), 3649.0));
    for key in ["plate.top.0", "plate.top.1"] {
        assert!(half_laps(member(&first, key)).is_empty(), "{key}");
        assert!(half_laps(member(&second, key)).is_empty(), "{key}");
    }
    // The one bottom plate of each half-lapped.
    assert_eq!(
        overlaps(&doc, &lead1, &lead2),
        vec![("plate.bottom.0".to_owned(), "plate.bottom.0".to_owned())]
    );
}

// --- Butting walls, lapped plates an option (mitcad#510).

/// A member as it is: its place, length, end A and how many cuts.
type Kept = (String, f64, [f64; 3], usize);

/// A frame's members as they are, by their place.
fn snapshot(doc: &Document<MockKernel>, lead: &Value) -> Vec<Kept> {
    let mut out: Vec<_> = members(doc, lead)
        .iter()
        .map(|p| {
            (
                p["member"].as_str().unwrap().to_owned(),
                length(p),
                end_a(doc, p),
                p["cuts"].as_array().map_or(0, |c| c.len()),
            )
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A frame's members as they are but the backing studs where other walls
/// meet it (`junction.*`).
fn own_members(doc: &Document<MockKernel>, lead: &Value) -> Vec<Kept> {
    snapshot(doc, lead)
        .into_iter()
        .filter(|m| !m.0.starts_with("junction."))
        .collect()
}

/// A stud of the frame (an end, a common or a backing stud) behind the
/// point, on its faces too: what a sheathing board's edge there is fixed
/// to (mitcad#510).
fn backed(doc: &Document<MockKernel>, lead: &Value, p: [f64; 3]) -> bool {
    boxes(doc, lead).iter().any(|(key, lo, hi)| {
        (key.starts_with("stud") || key.starts_with("junction"))
            && (0..3).all(|i| lo[i] - 1e-6 <= p[i] && p[i] <= hi[i] + 1e-6)
    })
}

/// The place along X of a member's axis (a stud's), and along Y.
fn stud_at(doc: &Document<MockKernel>, list: &[Value], key: &str) -> [f64; 2] {
    let a = end_a(doc, member(list, key));
    [a[0], a[1]]
}

/// The frame is as it was: the same members, each as long, at the same
/// place, with as many cuts.
fn unchanged(before: &[Kept], after: &[Kept]) {
    let keys = |l: &[Kept]| -> Vec<String> { l.iter().map(|m| m.0.clone()).collect() };
    assert_eq!(keys(before), keys(after));
    for (b, a) in before.iter().zip(after) {
        assert!(close(b.1, a.1), "{}: {} -> {}", b.0, b.1, a.1);
        assert!(
            (0..3).all(|i| close(b.2[i], a.2[i])),
            "{}: {:?} -> {:?}",
            b.0,
            b.2,
            a.2
        );
        assert_eq!(b.3, a.3, "{}: its cuts", b.0);
    }
}

/// A wall's definition as its lead keeps it.
fn definition(doc: &Document<MockKernel>, lead: &Value) -> Value {
    parts_of(doc)
        .into_iter()
        .find(|p| p["component"] == *lead)
        .unwrap()["framing"]["definition"]
        .clone()
}

#[test]
fn a_wall_started_30_mm_from_a_walls_end_butts_square_on_its_side() {
    let mut doc = design();
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [3600.0, 0.0, 0.0], json!({}));
    let lead1 = w1["frame"].clone();
    let before = own_members(&doc, &lead1);
    let butt = json!({"length": 2000, "corner": "butt"});
    // The preview names where it starts: from the side, kept off the end
    // by half its depth (its outer face flush with the end face).
    let preview = query(
        &doc,
        json!({"query": "build_preview", "frame": {"kind": "wall", "part": part("rows98"),
               "start": [30, -49, 1000], "length": 2000, "joins": {"start": lead1},
               "square": true, "corner": "butt"}}),
    );
    let junction = json!({"kind": "side", "corner": "butt", "wall": "Wall 1", "frame": lead1,
                          "end": null, "at": 49.0, "length": 2000.0});
    assert_eq!(preview["junction"], junction, "{preview}");
    // A click on its -Y side 30 mm from its end A: no L corner there, no
    // overhang past the end.
    let w2 = wall_on(&mut doc, &lead1, [30.0, -49.0, 1000.0], butt);
    all_ok(&doc);
    assert_eq!(w2["junction"], junction, "{w2}");
    let lead2 = w2["frame"].clone();
    assert_eq!(definition(&doc, &lead2)["corner"], "butt");
    // Square to the first, its outer face at the first one's end face, at
    // its base, to -Y.
    let (s, e) = ends_of(&doc, &lead2);
    assert!(
        close(s[0], 49.0) && close(s[1], 0.0) && close(s[2], 0.0),
        "{s:?}"
    );
    assert!(close(e[0], 49.0) && close(e[1], -2000.0), "{e:?}");
    // Its end stud and plates butt against the first one's face, square,
    // neither lapped nor notched.
    let second = members(&doc, &lead2);
    let stud = end_a(&doc, member(&second, "stud.start"));
    assert!(
        close(stud[0], 49.0) && close(stud[1], -49.0 - 24.0),
        "{stud:?}"
    );
    for key in ["plate.top.0", "plate.bottom.0"] {
        let plate = member(&second, key);
        assert!(
            close(length(plate), 2000.0 - 49.0),
            "{key}: {}",
            length(plate)
        );
        assert!(close(end_a(&doc, plate)[1], -49.0), "{key}");
        assert!(half_laps(plate).is_empty(), "{key}");
    }
    // The first is neither run on, lapped nor notched, its plates as long:
    // only a backing stud just inside the second's inner face (the one
    // outside it would be its end stud's place).
    unchanged(&before, &own_members(&doc, &lead1));
    let first = members(&doc, &lead1);
    assert_eq!(count_of(&first, "junction."), 1);
    let backing = stud_at(&doc, &first, "junction.Wall_2.0.b");
    assert!(close(backing[0], 98.0 + 24.0), "{backing:?}");
    // Both walls' sheathing has a stud behind its edges at the corner: the
    // first's -Y face where the second's inner face meets it (the backing
    // stud), its +Y face at its end (its end stud), the second's outer and
    // inner faces where they meet the first (its start stud).
    for (lead, p) in [
        (&lead1, [98.0, -49.0, 1200.0]),
        (&lead1, [0.0, 49.0, 1200.0]),
        (&lead2, [0.0, -49.0, 1200.0]),
        (&lead2, [98.0, -49.0, 1200.0]),
    ] {
        assert!(backed(&doc, lead, p), "no stud of {lead} behind {p:?}");
    }
    assert!(close(length(member(&first, "plate.top.0")), 3600.0));
    assert_eq!(
        overlaps(&doc, &lead1, &lead2),
        Vec::<(String, String)>::new()
    );
    // Mid-side the same, from the side at the click: a backing stud each
    // side of it in the first.
    let w3 = wall_on(
        &mut doc,
        &lead1,
        [1800.0, 49.0, 300.0],
        json!({"end": [1700, 2500, 0], "corner": "butt"}),
    );
    all_ok(&doc);
    assert_eq!(w3["junction"]["kind"], "side", "{w3}");
    assert_eq!(w3["junction"]["at"], 1800.0, "{w3}");
    let (s, e) = ends_of(&doc, &w3["frame"]);
    assert!(close(s[0], 1800.0) && close(s[1], 0.0), "{s:?}");
    assert!(close(e[0], 1800.0) && close(e[1], 2500.0), "{e:?}");
    unchanged(&before, &own_members(&doc, &lead1));
    let first = members(&doc, &lead1);
    let a = stud_at(&doc, &first, "junction.Wall_3.0.a");
    let b = stud_at(&doc, &first, "junction.Wall_3.0.b");
    assert!(close(a[0], 1800.0 - 49.0 - 24.0), "{a:?}");
    assert!(close(b[0], 1800.0 + 49.0 + 24.0), "{b:?}");
    // The first's +Y face has a stud behind it each side of the third, the
    // third's faces its start stud where they meet the first.
    for (lead, p) in [
        (&lead1, [1751.0, 49.0, 1200.0]),
        (&lead1, [1849.0, 49.0, 1200.0]),
        (&w3["frame"], [1751.0, 49.0, 1200.0]),
        (&w3["frame"], [1849.0, 49.0, 1200.0]),
    ] {
        assert!(backed(&doc, lead, p), "no stud of {lead} behind {p:?}");
    }
    assert_eq!(
        overlaps(&doc, &lead1, &w3["frame"]),
        Vec::<(String, String)>::new()
    );
}

#[test]
fn a_wall_started_on_a_walls_end_face_makes_a_butt_corner() {
    let mut doc = design();
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [3600.0, 0.0, 0.0], json!({}));
    let lead1 = w1["frame"].clone();
    let before = snapshot(&doc, &lead1);
    // A click on its end B's face, the length typed: beyond that end, its
    // side against the face, from the first one's far side (-Y) to +Y (the
    // click's side).
    let w2 = wall_on(
        &mut doc,
        &lead1,
        [3600.0, 20.0, 1200.0],
        json!({"length": 2500, "corner": "butt"}),
    );
    all_ok(&doc);
    assert_eq!(
        w2["junction"],
        json!({"kind": "end", "corner": "butt", "wall": "Wall 1", "frame": lead1, "end": "b",
               "at": 3649.0, "length": 2500.0}),
        "{w2}"
    );
    // It stays joined to the first, its start on that one's end.
    assert_eq!(w2["relaid"], json!([lead1]), "{w2}");
    let lead2 = w2["frame"].clone();
    let def = definition(&doc, &lead2);
    assert_eq!(
        def["joins"],
        json!({"start": lead1, "start_on": "end"}),
        "{def}"
    );
    let (s, e) = ends_of(&doc, &lead2);
    assert!(
        close(s[0], 3649.0) && close(s[1], -49.0) && close(s[2], 0.0),
        "{s:?}"
    );
    assert!(close(e[0], 3649.0) && close(e[1], 2451.0), "{e:?}");
    // Its plates run its whole length past the first one's end face.
    let second = members(&doc, &lead2);
    for key in ["plate.top.0", "plate.bottom.0"] {
        let plate = member(&second, key);
        assert!(close(length(plate), 2500.0), "{key}: {}", length(plate));
        assert!(close(end_a(&doc, plate)[1], -49.0), "{key}");
        assert!(half_laps(plate).is_empty(), "{key}");
    }
    // Its backing stud just past the first one's near side (the one at its
    // far side would be its end stud's place); the first not changed at all.
    let backing = stud_at(&doc, &second, "junction.Wall_1.0.b");
    assert!(
        close(backing[0], 3649.0) && close(backing[1], -49.0 + 98.0 + 24.0),
        "{backing:?}"
    );
    assert_eq!(count_of(&second, "junction."), 1);
    // Both walls' sheathing has a stud behind its edges at the corner: the
    // first's faces at its end (its end stud), the second's inner face
    // where the first's +Y face meets it (the backing stud), its outer face
    // and the corner's -Y face at its start (its start stud).
    for (lead, p) in [
        (&lead1, [3600.0, -49.0, 1200.0]),
        (&lead1, [3600.0, 49.0, 1200.0]),
        (&lead2, [3600.0, 49.0, 1200.0]),
        (&lead2, [3698.0, -49.0, 1200.0]),
        (&lead2, [3649.0, -49.0, 1200.0]),
    ] {
        assert!(backed(&doc, lead, p), "no stud of {lead} behind {p:?}");
    }
    unchanged(&before, &snapshot(&doc, &lead1));
    assert_eq!(
        overlaps(&doc, &lead1, &lead2),
        Vec::<(String, String)>::new()
    );
    // On end A's face, the end clicked: to the end's side (-Y).
    let w3 = wall_on(
        &mut doc,
        &lead1,
        [0.0, 10.0, 500.0],
        json!({"end": [-500, -1800, 0], "corner": "butt"}),
    );
    all_ok(&doc);
    assert_eq!(w3["junction"]["kind"], "end", "{w3}");
    assert_eq!(w3["junction"]["end"], "a", "{w3}");
    assert_eq!(w3["junction"]["at"], -49.0, "{w3}");
    let lead3 = w3["frame"].clone();
    let (s, e) = ends_of(&doc, &lead3);
    assert!(close(s[0], -49.0) && close(s[1], 49.0), "{s:?}");
    assert!(close(e[0], -49.0) && close(e[1], -1800.0), "{e:?}");
    let third = members(&doc, &lead3);
    let backing = stud_at(&doc, &third, "junction.Wall_1.0.b");
    assert!(close(backing[1], 49.0 - 98.0 - 24.0), "{backing:?}");
    unchanged(&before, &snapshot(&doc, &lead1));
    // Moving the first lays both out again with it: their backing studs
    // follow its centre line.
    let moved = edit(
        &mut doc,
        &lead1,
        json!({"start": [0, 20, 0], "end": [3600, 20, 0]}),
    );
    all_ok(&doc);
    let relaid = moved["relaid"].as_array().unwrap();
    assert!(
        relaid.contains(&lead2) && relaid.contains(&lead3),
        "{moved}"
    );
    let backing = stud_at(&doc, &members(&doc, &lead2), "junction.Wall_1.0.b");
    assert!(close(backing[1], -49.0 + 69.0 + 49.0 + 24.0), "{backing:?}");
    let backing = stud_at(&doc, &members(&doc, &lead3), "junction.Wall_1.0.b");
    assert!(close(backing[1], 49.0 - 29.0 - 49.0 - 24.0), "{backing:?}");
    assert_eq!(definition(&doc, &lead2)["joins"]["start_on"], "end");
    // From its end B's face (the first now on y = 20), not reaching beyond
    // the first one's depth: refused.
    let refused = command(
        &mut doc,
        json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
               "start": [3600, 0, 0], "length": 80, "joins": {"start": lead1},
               "square": true, "corner": "butt"}),
    )
    .unwrap_err();
    assert!(refused.contains("not beyond its other side"), "{refused}");
}

#[test]
fn lapped_plates_give_the_l_corner_and_an_edit_to_butt_undoes_it() {
    let mut doc = design();
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [3600.0, 0.0, 0.0], json!({}));
    let lead1 = w1["frame"].clone();
    let before = own_members(&doc, &lead1);
    // The same click 30 mm from end A with Lapped plates: the L corner of
    // mitcad#493 at that end.
    let w2 = wall_on(
        &mut doc,
        &lead1,
        [30.0, -49.0, 1000.0],
        json!({"length": 2000, "corner": "lapped"}),
    );
    all_ok(&doc);
    assert_eq!(
        w2["junction"],
        json!({"kind": "L", "corner": "lapped", "wall": "Wall 1", "frame": lead1, "end": "a",
               "at": 0.0, "length": 2000.0}),
        "{w2}"
    );
    let lead2 = w2["frame"].clone();
    assert_eq!(definition(&doc, &lead2)["corner"], "lapped");
    let first = members(&doc, &lead1);
    let second = members(&doc, &lead2);
    for key in ["plate.top.0", "plate.bottom.0"] {
        assert!(close(length(member(&first, key)), 3649.0), "{key}");
        assert!(close(length(member(&second, key)), 2049.0), "{key}");
        assert_eq!(half_laps(member(&first, key)), vec![lead2.clone()], "{key}");
        assert_eq!(
            half_laps(member(&second, key)),
            vec![lead1.clone()],
            "{key}"
        );
    }
    assert_eq!(count_of(&first, "junction.Wall_2."), 1);
    // Butting instead: the first as it was but a backing stud (neither run
    // on nor lapped), the second's plates stopping at its face.
    edit(&mut doc, &lead2, json!({"corner": "butt"}));
    all_ok(&doc);
    unchanged(&before, &own_members(&doc, &lead1));
    let first = members(&doc, &lead1);
    assert_eq!(count_of(&first, "junction.Wall_2."), 1);
    let backing = stud_at(&doc, &first, "junction.Wall_2.0.b");
    assert!(close(backing[0], 49.0 + 24.0), "{backing:?}");
    let second = members(&doc, &lead2);
    for key in ["plate.top.0", "plate.bottom.0"] {
        assert!(close(length(member(&second, key)), 2000.0 - 49.0), "{key}");
        assert!(half_laps(member(&second, key)).is_empty(), "{key}");
    }
}
