// SPDX-License-Identifier: MIT
//! A wall longer than its timber comes (mitcad#536): its plates spliced,
//! pieces butted end to end from end A as long as the stock allows, each
//! joint over a stud's centre (or a backing stud), the joints of a double
//! top plate a stud spacing apart, the pieces in the cut list and each
//! fastened to the stud under the joint; a refused preview still names the
//! wall and the length its auto end worked out; the auto end stops at the
//! nearest face along the run (a partition nearer than the far wall, a low
//! one only the lowest line meets, a post only the wall's side meets, the
//! wall over a door in the wall ahead). On the test library's 48 x 98
//! timber: its longest 6000 mm, its longest stock length 5400 mm.

use serde_json::{Value, json};

use super::test_library::{all_ok, command, design, part, query};
use crate::Document;
use crate::testing::MockKernel;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("no number: {v}"))
}

/// A wall from the origin along +X, `length` long, 2400 high, studs at
/// 600, with the extra fields: `build_frame`'s fields.
fn wall_fields(length: f64, extra: Value) -> Value {
    let mut fields = json!({"kind": "wall", "part": part("rows98"), "start": [0, 0, 0],
                            "end": [length, 0, 0], "height": 2400, "spacing": 600});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    fields
}

fn build(doc: &mut Document<MockKernel>, fields: Value) -> Value {
    let mut c = fields;
    c["cmd"] = json!("build_frame");
    command(doc, c).unwrap_or_else(|e| panic!("build_frame: {e}"))
}

fn preview(doc: &Document<MockKernel>, fields: Value) -> Value {
    query(doc, json!({"query": "build_preview", "frame": fields}))
}

/// A frame's members as `build_parts` gives them.
fn members(doc: &Document<MockKernel>, lead: &Value) -> Vec<Value> {
    query(doc, json!({"query": "build_parts"}))["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["frame"] == *lead)
        .cloned()
        .collect()
}

fn member<'a>(list: &'a [Value], key: &str) -> &'a Value {
    list.iter()
        .find(|p| p["member"] == key)
        .unwrap_or_else(|| panic!("no member {key}: {:?}", places(list)))
}

fn places(list: &[Value]) -> Vec<String> {
    list.iter()
        .map(|p| p["member"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn length(p: &Value) -> f64 {
    num(&p["length"])
}

/// A member's end A in the design.
fn end_a(doc: &Document<MockKernel>, p: &Value) -> [f64; 3] {
    let c: crate::ids::ComponentUid = p["component"].as_str().unwrap().parse().unwrap();
    let path = doc.placed_in_host(c).unwrap();
    doc.path_transform(&path).translation
}

/// The splices a preview or `build_frame` answers, by plate: `(at, pieces)`.
fn splice<'a>(answer: &'a Value, plate: &str) -> &'a Value {
    answer["splices"]
        .as_array()
        .unwrap_or_else(|| panic!("no splices: {answer}"))
        .iter()
        .find(|s| s["plate"] == plate)
        .unwrap_or_else(|| panic!("{plate} not spliced: {}", answer["splices"]))
}

fn joints(s: &Value) -> Vec<f64> {
    s["at"].as_array().unwrap().iter().map(num).collect()
}

#[test]
fn a_wall_longer_than_its_timber_has_its_plates_spliced_over_studs() {
    let mut doc = design();
    let fields = wall_fields(7819.0, json!({"plates": {"top": 2}}));
    // The preview: each plate in two pieces, the double top plate's joints
    // a stud spacing apart.
    let shown = preview(&doc, fields.clone());
    assert_eq!(shown["problems"], json!([]), "{shown}");
    assert!(close(num(&shown["length"]), 7819.0), "{shown}");
    assert_eq!(shown["roles"]["top plate"], 4, "{shown}");
    assert_eq!(shown["roles"]["bottom plate"], 2, "{shown}");
    // 7819 > 6000: pieces of at most 5400 from end A, the joint over the
    // stud at 5400 (stud.9); the second top plate's over the one at 4800.
    assert_eq!(joints(splice(&shown, "plate.top.0")), vec![5400.0]);
    assert_eq!(joints(splice(&shown, "plate.top.1")), vec![4800.0]);
    assert_eq!(joints(splice(&shown, "plate.bottom.0")), vec![5400.0]);
    assert!(close(
        num(&splice(&shown, "plate.top.0")["longest"]),
        5400.0
    ));
    assert_eq!(splice(&shown, "plate.top.0")["role"], "top plate");
    assert_eq!(splice(&shown, "plate.top.0")["pieces"], 2);

    let made = build(&mut doc, fields);
    all_ok(&doc);
    // Six pieces of plate and fourteen studs (the ends and 1 to 12).
    assert_eq!(made["count"], 20, "{made}");
    assert_eq!(made["splices"].as_array().map(Vec::len), Some(3), "{made}");
    let lead = made["frame"].clone();
    let list = members(&doc, &lead);
    for (key, from, to) in [
        ("plate.top.0", 0.0, 5400.0),
        ("plate.top.0.1", 5400.0, 7819.0),
        ("plate.top.1", 0.0, 4800.0),
        ("plate.top.1.1", 4800.0, 7819.0),
        ("plate.bottom.0.0", 0.0, 5400.0),
        ("plate.bottom.0.1", 5400.0, 7819.0),
    ] {
        let p = member(&list, key);
        assert!(close(length(p), to - from), "{key}: {p}");
        assert!(
            close(end_a(&doc, p)[0], from),
            "{key}: {:?}",
            end_a(&doc, p)
        );
    }
    assert_eq!(list.len(), 20, "{:?}", places(&list));
    // The first piece keeps the plate's place: the top plate leads.
    assert_eq!(member(&list, "plate.top.0")["component"], lead);
    // A stud under each joint, its centre on it (2400 - 48 - 2 x 48 long:
    // two top plates).
    for (key, at) in [("stud.9", 5400.0), ("stud.8", 4800.0)] {
        let stud = member(&list, key);
        assert!(close(length(stud), 2256.0), "{key}: {stud}");
        assert!(
            close(end_a(&doc, stud)[0], at),
            "{key}: {:?}",
            end_a(&doc, stud)
        );
    }

    // The cut list lists the pieces, none longer than the stock.
    let cut = query(&doc, json!({"query": "build_cut_list"}));
    let mut by_length: Vec<(f64, u64)> = Vec::new();
    for row in cut["rows"].as_array().unwrap() {
        assert_eq!(row["frame"], "Wall 1", "{row}");
        let (l, q) = (num(&row["length"]), row["quantity"].as_u64().unwrap());
        match by_length.iter_mut().find(|(m, _)| close(*m, l)) {
            Some((_, n)) => *n += q,
            None => by_length.push((l, q)),
        }
    }
    by_length.sort_by(|a, b| a.0.total_cmp(&b.0));
    let expected = [
        (2256.0, 14),
        (2419.0, 2),
        (3019.0, 1),
        (4800.0, 1),
        (5400.0, 2),
    ];
    assert_eq!(by_length.len(), expected.len(), "{by_length:?}");
    for ((l, q), (el, eq)) in by_length.iter().zip(expected) {
        assert!(close(*l, el) && *q == eq, "{by_length:?}");
    }
    for entry in cut["stock"].as_array().unwrap() {
        assert_eq!(entry["too_long"], json!([]), "{entry}");
    }
}

#[test]
fn the_pieces_of_a_spliced_plate_are_each_fastened_to_the_stud_under_the_joint() {
    let mut doc = design();
    let made = build(
        &mut doc,
        wall_fields(7819.0, json!({"fastener": part("wood-screw")})),
    );
    all_ok(&doc);
    assert!(num(&made["fasteners"]) > 0.0, "{made}");
    let lead = made["frame"].clone();
    let list = members(&doc, &lead);
    let stud = member(&list, "stud.9")["component"].clone();
    let pieces = [
        member(&list, "plate.bottom.0.0")["component"].clone(),
        member(&list, "plate.bottom.0.1")["component"].clone(),
    ];
    let junctions = query(&doc, json!({"query": "build_junctions"}))["junctions"]
        .as_array()
        .unwrap()
        .clone();
    let between = |a: &Value, b: &Value| {
        junctions
            .iter()
            .find(|j| {
                (j["a"]["component"] == *a && j["b"]["component"] == *b)
                    || (j["a"]["component"] == *b && j["b"]["component"] == *a)
            })
            .cloned()
    };
    // The stud's foot is on both pieces: each piece fastened to it.
    for piece in &pieces {
        let j = between(&stud, piece)
            .unwrap_or_else(|| panic!("no junction of stud.9 and {piece}: {junctions:?}"));
        assert!(
            j["connectors"].as_array().is_some_and(|c| !c.is_empty()),
            "{j}"
        );
    }
    // The pieces butt end to end on one axis: no fastener there (an
    // inline joint takes a plate).
    let butt = between(&pieces[0], &pieces[1]).expect("the pieces' joint");
    assert_eq!(butt["kind"], "inline", "{butt}");
    assert_eq!(butt["connectors"], json!([]), "{butt}");
}

#[test]
fn a_backing_stud_carries_the_joint_where_no_stud_is_in_reach() {
    let mut doc = design();
    // Studs 5900 apart: none between 96 and 5400 mm from end A.
    let made = build(&mut doc, wall_fields(7819.0, json!({"spacing": 5900})));
    all_ok(&doc);
    let lead = made["frame"].clone();
    let list = members(&doc, &lead);
    let backing = member(&list, "stud.splice.1");
    assert_eq!(backing["role"], "profile", "{backing}");
    assert!(close(length(backing), 2304.0), "{backing}");
    assert!(
        close(end_a(&doc, backing)[0], 5400.0),
        "{:?}",
        end_a(&doc, backing)
    );
    assert!(close(length(member(&list, "plate.top.0")), 5400.0));
    assert!(close(length(member(&list, "plate.bottom.0.0")), 5400.0));
    // One backing stud for both plates.
    assert_eq!(
        places(&list)
            .iter()
            .filter(|k| k.starts_with("stud.splice."))
            .count(),
        1,
        "{:?}",
        places(&list)
    );
}

#[test]
fn a_wall_no_longer_than_its_timber_keeps_its_plates_whole() {
    let mut doc = design();
    // 5900 is over the longest stock length (5400) but not the timber's
    // longest (6000): one piece each, as before.
    let made = build(&mut doc, wall_fields(5900.0, json!({})));
    all_ok(&doc);
    let list = members(&doc, &made["frame"]);
    assert!(close(length(member(&list, "plate.top.0")), 5900.0));
    assert!(close(length(member(&list, "plate.bottom.0")), 5900.0));
    assert_eq!(made["splices"], json!([]), "{made}");
}

// --- A room of plain bodies (the mock kernel's boxes have their faces on
// their bounds).

/// A box from `corner` (x, y) at the height `z` of `size`; its body.
fn solid(doc: &mut Document<MockKernel>, corner: [f64; 2], z: f64, size: [f64; 3]) -> String {
    let plane = if z == 0.0 {
        json!({"type": "origin", "plane": "xy"})
    } else {
        json!({"origin": [0.0, 0.0, z], "normal": [0.0, 0.0, 1.0]})
    };
    let added = command(
        doc,
        json!({"cmd": "add_feature", "def": {"type": "box", "plane": plane, "corner": corner,
               "length": size[0], "width": size[1], "height": size[2],
               "operation": "new_body"}}),
    )
    .unwrap_or_else(|e| panic!("box: {e}"));
    let uid = added["uid"]
        .as_str()
        .or_else(|| added["feature"].as_str())
        .unwrap_or_else(|| panic!("no feature in {added}"))
        .to_owned();
    format!("{uid}.b0")
}

/// A room `width` mm wide inside (along X) of solid walls 200 thick and
/// 2700 high on a 200 thick slab, 4000 deep: its west wall's body (the
/// east wall's inner face at x = 200 + width). Without `east` no east wall.
fn room(doc: &mut Document<MockKernel>, width: f64, east: bool) -> String {
    doc.kernel().box_faces.set(true);
    solid(doc, [-500.0, -500.0], 0.0, [width + 1400.0, 5000.0, 200.0]);
    let west = solid(doc, [0.0, 0.0], 0.0, [200.0, 4000.0, 2700.0]);
    if east {
        solid(doc, [200.0 + width, 0.0], 0.0, [200.0, 4000.0, 2700.0]);
    }
    west
}

/// A wall's fields started on the west wall's room side (+X) at y 2000,
/// Butt, no end: its length Auto, `length` when its run meets nothing.
fn from_west(west: &str, extra: Value) -> Value {
    let feature = west.split('.').next().unwrap();
    let mut fields = json!({"kind": "wall", "part": part("rows98"), "start": [200, 2000, 1200],
                            "square": true, "face": {"body": west, "face": format!("{feature}:side1")},
                            "height": 2400, "spacing": 600, "corner": "butt", "length": 1000});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    fields
}

#[test]
fn a_wall_from_a_long_rooms_face_runs_to_the_far_wall_with_spliced_plates() {
    let mut doc = design();
    let west = room(&mut doc, 7819.0, true);
    all_ok(&doc);
    let fields = from_west(&west, json!({}));
    let shown = preview(&doc, fields.clone());
    assert_eq!(shown["problems"], json!([]), "{shown}");
    assert_eq!(shown["name"], "Wall 1", "{shown}");
    assert!(close(num(&shown["length"]), 7819.0), "{shown}");
    let j = &shown["junction"];
    assert_eq!(j["kind"], "face", "{j}");
    assert_eq!(j["to"]["auto"], true, "{j}");
    assert_eq!(j["to"]["face"], true, "{j}");
    assert!(close(num(&j["length"]), 7819.0), "{j}");
    assert_eq!(joints(splice(&shown, "plate.top.0")), vec![5400.0]);
    assert_eq!(joints(splice(&shown, "plate.bottom.0")), vec![5400.0]);
    let made = build(&mut doc, fields);
    all_ok(&doc);
    let list = members(&doc, &made["frame"]);
    assert!(close(length(member(&list, "plate.top.0")), 5400.0));
    assert!(close(length(member(&list, "plate.top.0.1")), 2419.0));
    assert!(close(length(member(&list, "plate.bottom.0.1")), 2419.0));
}

#[test]
fn a_refused_preview_names_the_wall_and_the_length_its_auto_end_worked_out() {
    let mut doc = design();
    let west = room(&mut doc, 7819.0, true);
    all_ok(&doc);
    // 7000 high: its studs (6904 mm) longer than the timber's longest; the
    // plates alone could be spliced.
    let fields = from_west(&west, json!({"height": 7000}));
    let shown = preview(&doc, fields.clone());
    let problems = shown["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{shown}");
    let reason = problems[0].as_str().unwrap();
    assert!(reason.contains("Wall 1"), "{reason}");
    assert!(
        reason.contains("longer than the part's longest"),
        "{reason}"
    );
    assert_eq!(shown["name"], "Wall 1", "{shown}");
    assert_eq!(shown["kind"], "wall", "{shown}");
    assert_eq!(shown["count"], 0, "{shown}");
    assert!(close(num(&shown["length"]), 7819.0), "{shown}");
    let j = &shown["junction"];
    assert_eq!(j["to"]["auto"], true, "{shown}");
    assert_eq!(j["to"]["face"], true, "{shown}");
    assert!(close(num(&j["length"]), 7819.0), "{shown}");
    // The command says the same, naming the wall.
    let mut c = fields;
    c["cmd"] = json!("build_frame");
    let refused = command(&mut doc, c).unwrap_err();
    assert!(refused.contains("Wall 1"), "{refused}");
    assert!(
        refused.contains("longer than the part's longest"),
        "{refused}"
    );
    // A start it cannot make is refused by name too.
    let shown = preview(
        &doc,
        json!({"kind": "wall", "part": part("rows98"), "start": [200, 2000, 1200],
               "square": true, "face": {"body": west, "face": "F999:side1"}, "length": 1000}),
    );
    assert_eq!(shown["name"], "Wall 1", "{shown}");
    assert_eq!(
        shown["problems"].as_array().map(Vec::len),
        Some(1),
        "{shown}"
    );
}

#[test]
fn the_auto_end_stops_at_a_partition_nearer_than_the_far_wall() {
    let mut doc = design();
    let west = room(&mut doc, 7819.0, true);
    // A partition 70 thick across the room, 3000 from the west wall.
    solid(&mut doc, [3200.0, 0.0], 0.0, [70.0, 4000.0, 2700.0]);
    all_ok(&doc);
    let shown = preview(&doc, from_west(&west, json!({})));
    assert!(close(num(&shown["length"]), 3000.0), "{shown}");
    assert_eq!(shown["junction"]["to"]["auto"], true, "{shown}");
    assert_eq!(shown["splices"], json!([]), "{shown}");
    let made = build(&mut doc, from_west(&west, json!({})));
    all_ok(&doc);
    assert!(close(num(&made["junction"]["length"]), 3000.0), "{made}");
}

#[test]
fn the_auto_end_stops_at_a_face_only_one_line_of_the_run_meets() {
    // A low partition (900 high): only the line a quarter up meets it.
    let mut doc = design();
    let west = room(&mut doc, 7819.0, true);
    solid(&mut doc, [3200.0, 0.0], 0.0, [70.0, 4000.0, 900.0]);
    all_ok(&doc);
    let shown = preview(&doc, from_west(&west, json!({})));
    assert!(close(num(&shown["length"]), 3000.0), "{shown}");

    // A post beside the run: only the wall's side meets it (the wall 98
    // deep at y 2000, the post from y 2030).
    let mut doc = design();
    let west = room(&mut doc, 7819.0, true);
    solid(&mut doc, [2000.0, 2030.0], 0.0, [100.0, 100.0, 2700.0]);
    all_ok(&doc);
    let shown = preview(&doc, from_west(&west, json!({})));
    assert!(close(num(&shown["length"]), 1800.0), "{shown}");

    // A door in the wall ahead (y 1500 to 2500, 2100 high, a lintel over
    // it): the lines at a quarter, half and three quarters of the wall's
    // height pass through it, the one under its top meets the lintel.
    let mut doc = design();
    let west = room(&mut doc, 7819.0, false);
    let x = 200.0 + 7819.0;
    solid(&mut doc, [x, 0.0], 0.0, [200.0, 1500.0, 2700.0]);
    solid(&mut doc, [x, 2500.0], 0.0, [200.0, 1500.0, 2700.0]);
    solid(&mut doc, [x, 1500.0], 2100.0, [200.0, 1000.0, 600.0]);
    all_ok(&doc);
    let shown = preview(&doc, from_west(&west, json!({})));
    assert!(close(num(&shown["length"]), 7819.0), "{shown}");
    assert_eq!(shown["junction"]["to"]["face"], true, "{shown}");
}
