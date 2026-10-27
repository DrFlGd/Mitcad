// SPDX-License-Identifier: MIT
//! A wall started on another body's face (mitcad#524, case B): a box room
//! of plain bodies on a slab; a Start on an inner wall's face gives a wall
//! square to it on the room's side (for the room's +X and -X walls), its
//! end on the face, its bottom plate's underside on the slab's top, the
//! clicked body unchanged and not linked; the auto end at the opposite face
//! gives the room's inner width. A solid wall's end face makes a corner with
//! Butt and a wall in line with Straight; a horizontal face is refused. On a
//! timber wall's end face Straight continues it in line (case A). The mock
//! kernel's boxes have their faces on their bounds (`box_faces`).

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_build::prelude::*;
use mitcad_model_tests_support::test_library::{all_ok, command, design, part, query};

/// A box from `corner` (x, y on the XY plane) of `size`; its body.
fn solid(doc: &mut Document<MockKernel>, corner: [f64; 2], size: [f64; 3]) -> String {
    let added = command(
        doc,
        json!({"cmd": "add_feature", "def": {"type": "box",
               "plane": {"type": "origin", "plane": "xy"}, "corner": corner,
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

/// A face of a box body by its role (`side0` -Y, `side1` +X, `side2` +Y,
/// `side3` -X, `top`).
fn face(body: &str, role: &str) -> Value {
    let feature = body.split('.').next().unwrap();
    json!({"body": body, "face": format!("{feature}:{role}")})
}

/// A room 3600 x 3600 inside of four solid walls 200 thick and 2700 high
/// on a 200 thick slab: its west and east walls' bodies.
fn room(doc: &mut Document<MockKernel>) -> (String, String, String) {
    doc.kernel().box_faces.set(true);
    let slab = solid(doc, [-500.0, -500.0], [5000.0, 5000.0, 200.0]);
    let west = solid(doc, [0.0, 0.0], [200.0, 4000.0, 2700.0]);
    let east = solid(doc, [3800.0, 0.0], [200.0, 4000.0, 2700.0]);
    solid(doc, [200.0, 0.0], [3600.0, 200.0, 2700.0]);
    solid(doc, [200.0, 3800.0], [3600.0, 200.0, 2700.0]);
    all_ok(doc);
    (slab, west, east)
}

fn wall_from_face(
    doc: &mut Document<MockKernel>,
    on: Value,
    click: [f64; 3],
    extra: Value,
) -> Value {
    let mut fields = json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
                            "start": click, "square": true, "face": on, "height": 2400,
                            "spacing": 600, "corner": "butt"});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    command(doc, fields).unwrap_or_else(|e| panic!("build_frame: {e}"))
}

fn parts_of(doc: &Document<MockKernel>) -> Vec<Value> {
    query(doc, json!({"query": "build_parts"}))["parts"]
        .as_array()
        .unwrap()
        .clone()
}

/// A member's end A in the design.
fn end_a(doc: &Document<MockKernel>, lead: &Value, key: &str) -> [f64; 3] {
    let p = parts_of(doc)
        .into_iter()
        .find(|p| p["frame"] == *lead && p["member"] == key)
        .unwrap_or_else(|| panic!("no member {key}"));
    let c: mitcad_model::ids::ComponentUid = p["component"].as_str().unwrap().parse().unwrap();
    let path = doc.placed_in_host(c).unwrap();
    doc.path_transform(&path).translation
}

fn definition(doc: &Document<MockKernel>, lead: &Value) -> Value {
    parts_of(doc)
        .into_iter()
        .find(|p| p["component"] == *lead)
        .unwrap()["framing"]["definition"]
        .clone()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// The design's definitions of the room's bodies (its features).
fn features(doc: &Document<MockKernel>) -> Vec<String> {
    doc.features()
        .filter(|f| matches!(f.def, mitcad_model::features::FeatureDef::Box(_)))
        .map(|f| format!("{:?}", f.def))
        .collect()
}

#[test]
fn a_wall_from_a_rooms_inner_wall_face_is_square_to_it_on_the_room_side_on_the_slab() {
    let mut doc = design();
    let (_, west, east) = room(&mut doc);
    let before = features(&doc);
    // The west wall's room side (+X): square from it, towards the room,
    // ending at the east wall's face: the room's inner width.
    let made = wall_from_face(
        &mut doc,
        face(&west, "side1"),
        [200.0, 2000.0, 1200.0],
        json!({}),
    );
    all_ok(&doc);
    let j = &made["junction"];
    assert_eq!(j["kind"], "face", "{made}");
    assert!(j["frame"].is_null(), "{j}");
    assert_eq!(j["from"], "middle", "{j}");
    assert!(close(j["length"].as_f64().unwrap(), 3600.0), "{j}");
    assert_eq!(j["to"]["auto"], true, "{j}");
    assert_eq!(j["to"]["face"], true, "{j}");
    assert_eq!(j["base"]["on"], "floor", "{j}");
    assert!(close(j["base"]["z"].as_f64().unwrap(), 200.0), "{j}");
    let lead = made["frame"].clone();
    // Its plates from the face (x 200) to the east wall's (x 3800), the
    // bottom plate's underside on the slab's top.
    let plate = end_a(&doc, &lead, "plate.bottom.0");
    assert!(
        close(plate[0], 200.0) && close(plate[1], 2000.0) && close(plate[2], 224.0),
        "{plate:?}"
    );
    let def = definition(&doc, &lead);
    assert!(close(def["start"][0].as_f64().unwrap(), 200.0), "{def}");
    assert!(close(def["end"][0].as_f64().unwrap(), 3800.0), "{def}");
    assert!(close(def["start"][2].as_f64().unwrap(), 200.0), "{def}");
    // The clicked body is not changed, not linked, the face not kept.
    assert_eq!(features(&doc), before);
    assert!(
        def["joins"].is_null() || def["joins"]["start"].is_null(),
        "{def}"
    );
    assert!(def["face"].is_null() && def["square"].is_null(), "{def}");

    // The east wall's room side (-X), 1000 mm from the room's south end:
    // towards -X, 3600 long again, from end B (its left seen from the room
    // is +Y's end).
    let made = wall_from_face(
        &mut doc,
        face(&east, "side3"),
        [3800.0, 1000.0, 1500.0],
        json!({}),
    );
    all_ok(&doc);
    let j = &made["junction"];
    assert_eq!(j["kind"], "face", "{made}");
    assert!(close(j["length"].as_f64().unwrap(), 3600.0), "{j}");
    let lead = made["frame"].clone();
    let def = definition(&doc, &lead);
    assert!(close(def["start"][0].as_f64().unwrap(), 3800.0), "{def}");
    assert!(close(def["start"][1].as_f64().unwrap(), 1000.0), "{def}");
    assert!(close(def["end"][0].as_f64().unwrap(), 200.0), "{def}");
    assert!(close(def["start"][2].as_f64().unwrap(), 200.0), "{def}");
    // Without the auto end: the length typed.
    let made = wall_from_face(
        &mut doc,
        face(&west, "side1"),
        [200.0, 3000.0, 1200.0],
        json!({"auto_end": false, "length": 1500}),
    );
    assert!(
        close(made["junction"]["length"].as_f64().unwrap(), 1500.0),
        "{made}"
    );
    assert!(made["junction"]["to"].is_null(), "{made}");
}

#[test]
fn a_solid_walls_end_face_gives_a_corner_with_butt_and_a_wall_in_line_with_straight() {
    let mut doc = design();
    doc.kernel().box_faces.set(true);
    // A free-standing solid wall 2000 long along X, 200 thick, no slab.
    let solid_wall = solid(&mut doc, [1000.0, 5000.0], [2000.0, 200.0, 2700.0]);
    // Butt: its side against the end face (x 3000), running to the click's
    // side (+Y), from the solid wall's far side (y 5000), on the face's
    // bottom edge.
    let made = wall_from_face(
        &mut doc,
        face(&solid_wall, "side1"),
        [3000.0, 5150.0, 1000.0],
        json!({"auto_end": false, "length": 2000}),
    );
    all_ok(&doc);
    let j = &made["junction"];
    assert_eq!(j["kind"], "face_end", "{made}");
    assert_eq!(j["base"]["on"], "face", "{j}");
    assert!(close(j["base"]["z"].as_f64().unwrap(), 0.0), "{j}");
    let def = definition(&doc, &made["frame"]);
    assert!(close(def["start"][0].as_f64().unwrap(), 3049.0), "{def}");
    assert!(close(def["start"][1].as_f64().unwrap(), 5000.0), "{def}");
    assert!(close(def["end"][1].as_f64().unwrap(), 7000.0), "{def}");
    // Straight: in line beyond the end face, its centre line the solid
    // wall's.
    let made = wall_from_face(
        &mut doc,
        face(&solid_wall, "side1"),
        [3000.0, 5150.0, 1000.0],
        json!({"auto_end": false, "length": 2000, "corner": "straight"}),
    );
    all_ok(&doc);
    assert_eq!(made["junction"]["kind"], "face_straight", "{made}");
    assert_eq!(made["junction"]["corner"], "straight", "{made}");
    let def = definition(&doc, &made["frame"]);
    assert!(close(def["start"][0].as_f64().unwrap(), 3000.0), "{def}");
    assert!(close(def["start"][1].as_f64().unwrap(), 5100.0), "{def}");
    assert!(close(def["end"][0].as_f64().unwrap(), 5000.0), "{def}");
    // A horizontal face is refused with the reason.
    let refused = command(
        &mut doc,
        json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
               "start": [2000, 5100, 2700], "square": true, "face": face(&solid_wall, "top"),
               "length": 1000}),
    )
    .unwrap_err();
    assert!(refused.contains("starts from a vertical face"), "{refused}");
    // The preview says the same as the command.
    let preview = query(
        &doc,
        json!({"query": "build_preview", "frame": {"kind": "wall", "part": part("rows98"),
               "start": [3000, 5150, 1000], "square": true, "face": face(&solid_wall, "side1"),
               "length": 2000, "auto_end": false, "corner": "butt"}}),
    );
    assert_eq!(preview["junction"]["kind"], "face_end", "{preview}");
}

#[test]
fn straight_on_a_timber_walls_end_face_continues_it_in_line() {
    let mut doc = design();
    let w1 = command(
        &mut doc,
        json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
               "start": [0, 0, 0], "end": [3600, 0, 0], "height": 2400, "spacing": 600,
               "corner": "butt"}),
    )
    .unwrap();
    let lead1 = w1["frame"].clone();
    // A click on its end B's face, Straight: from that face along +X.
    let made = command(
        &mut doc,
        json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
               "start": [3600, 10, 1200], "square": true, "joins": {"start": lead1},
               "height": 2400, "spacing": 600, "corner": "straight", "length": 2000,
               "auto_end": false}),
    )
    .unwrap();
    all_ok(&doc);
    let j = &made["junction"];
    assert_eq!(j["kind"], "straight", "{made}");
    assert_eq!(j["end"], "b", "{j}");
    let def = definition(&doc, &made["frame"]);
    assert!(close(def["start"][0].as_f64().unwrap(), 3600.0), "{def}");
    assert!(close(def["start"][1].as_f64().unwrap(), 0.0), "{def}");
    assert!(close(def["end"][0].as_f64().unwrap(), 5600.0), "{def}");
    // Not joined: the first wall is not laid out again for it.
    assert_eq!(made["relaid"], json!([]), "{made}");
}
