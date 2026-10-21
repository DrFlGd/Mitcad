// SPDX-License-Identifier: MIT
//! Profiled boards (mitcad#477): one of each family placed with its
//! section and mass, a tongue-and-groove floor and a shiplap wall laid as
//! rows (the step their cover width, the cut list's area), a deck board
//! row keeping its gap, and a board placed against another tongue into
//! groove.

use serde_json::{Value, json};

use super::test_library::{
    Part, all_ok, board_build, board_design, close, command, component, derived, design,
    fixed_frame, occurrence, part, place, placed, query,
};
use crate::Document;
use crate::testing::MockKernel;

/// A profiled board of the test library: `w` its full width (along X), `t`
/// its thickness, its `board` block, and for one that joins its `groove`
/// (at -w/2, z -X) and `tongue` (the cover width on from it, z +X) frames.
fn profiled(name: &str, w: f64, t: f64, board: Value) -> Part {
    let joined = matches!(
        board["profile"].as_str(),
        Some("tongue_and_groove" | "shiplap" | "feather_edge")
    );
    let mut doc: Document<MockKernel> =
        Document::from_json(&board_design(w, t, &[]), MockKernel::default()).unwrap();
    let mut build = board_build(w, t, "planed");
    build["nominal"] = json!({"width": w, "height": t});
    build["board"] = board.clone();
    if joined {
        let cover = board["cover"].as_f64().unwrap();
        fixed_frame(
            &mut doc,
            "groove",
            [-w / 2.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        );
        fixed_frame(
            &mut doc,
            "tongue",
            [-w / 2.0 + cover, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        );
        doc.recompute();
        let frames = build["frames"].as_array_mut().unwrap();
        frames.push(json!({"name": "groove", "kind": "groove"}));
        frames.push(json!({"name": "tongue", "kind": "tongue"}));
    }
    Part {
        text: doc.to_json(),
        build: Some(build),
        part_number: String::new(),
        name: name.to_owned(),
        designation: format!("{name} L={{L}}"),
    }
}

/// The test library's profiled boards.
pub(crate) fn boards() -> Vec<(&'static str, Part)> {
    vec![
        // A floorboard 28 x 120: a tongue 8 long and 8 thick, its groove 9
        // deep and 8.5 wide: 112 x 28 + 8 x 8 - 9 x 8.5.
        (
            "tg120",
            profiled(
                "Floorboard 28x120",
                120.0,
                28.0,
                json!({"profile": "tongue_and_groove", "cover": 112.0, "area": 3123.5,
                       "tongue": {"length": 8, "thickness": 8}}),
            ),
        ),
        // The same with a thinner tongue: it does not join the first.
        (
            "tg120-thin",
            profiled(
                "Floorboard 28x120 thin tongue",
                120.0,
                28.0,
                json!({"profile": "tongue_and_groove", "cover": 112.0, "area": 3127.0,
                       "tongue": {"length": 8, "thickness": 6}}),
            ),
        ),
        // Shiplap 22 x 145, overlapping by 12, its lip 9 thick: 145 x 22
        // less 12 x 13 at the front of the lip and 12 x 9 behind the overlap.
        (
            "shiplap145",
            profiled(
                "Shiplap 22x145",
                145.0,
                22.0,
                json!({"profile": "shiplap", "cover": 133.0, "area": 2926.0, "overlap": 12}),
            ),
        ),
        // Feather-edge 22/6 x 150, overlapping by 30: (22 + 6) / 2 x 150.
        (
            "feather150",
            profiled(
                "Feather-edge 22x150",
                150.0,
                22.0,
                json!({"profile": "feather_edge", "cover": 120.352, "area": 2100.0,
                       "overlap": 30}),
            ),
        ),
        // A deck board 28 x 120 with rounded edges (radius 5) and seven
        // grooves 5 x 3: 120 x 28 less the corners and the grooves.
        (
            "deck-grooved",
            profiled(
                "Deck board grooved 28x120",
                120.0,
                28.0,
                json!({"profile": "deck", "area": 3233.54, "edge_radius": 5, "grooves": 7}),
            ),
        ),
    ]
}

fn mount(doc: &Document<MockKernel>, answer: &Value, frame: &str) -> Value {
    json!({"mount": format!("{}:mount({frame})", derived(doc, component(answer)))})
}

fn member(
    doc: &mut Document<MockKernel>,
    part_id: &str,
    from: [f64; 3],
    to: [f64; 3],
    lie: &str,
) -> Value {
    place(
        doc,
        json!({"part": part(part_id), "at": {"point": from}, "to": {"point": to}, "lie": lie}),
    )
}

/// Where a component's occurrence is placed.
fn at(doc: &Document<MockKernel>, c: &str) -> crate::transform::Transform {
    let o = doc
        .assembly()
        .occurrences
        .iter()
        .find(|o| o.component.to_string() == c)
        .unwrap()
        .uid;
    placed(doc, o)
}

fn set_of(answer: &Value) -> Vec<String> {
    answer["set"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn one_of_each_family_has_its_section_mass_and_cover() {
    let mut doc = design();
    let families = [
        ("tg120", "tongue_and_groove", 120.0, 28.0, 112.0, 3123.5),
        ("shiplap145", "shiplap", 145.0, 22.0, 133.0, 2926.0),
        ("feather150", "feather_edge", 150.0, 22.0, 120.352, 2100.0),
        ("deck-grooved", "deck", 120.0, 28.0, 120.0, 3233.54),
    ];
    for (i, (id, ..)) in families.iter().enumerate() {
        place(
            &mut doc,
            json!({"part": part(id), "at": {"point": [500.0 * i as f64, 0, 0]}, "length": 3000}),
        );
    }
    all_ok(&doc);
    let list = query(&doc, json!({"query": "build_cut_list"}));
    assert_eq!(list["timber"], true);
    assert_eq!(list["profiled"], true);
    let rows = list["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "{rows:?}");
    let mut total = 0.0;
    for (id, profile, w, t, cover, area) in families {
        let row = rows
            .iter()
            .find(|r| r["profile"] == profile)
            .unwrap_or_else(|| panic!("no {id} in {rows:?}"));
        assert_eq!(row["kind"], "timber");
        assert_eq!(row["section"], json!({"width": w, "height": t}), "{id}");
        assert!(near(row["cover"].as_f64().unwrap(), cover), "{id}: {row}");
        // The mass from the section's own area, not W x T.
        let mass = 450.0 * area * 3000.0 / 1e9;
        assert!(near(row["mass"].as_f64().unwrap(), mass), "{id}: {row}");
        assert!(near(row["volume"].as_f64().unwrap(), area * 3000.0 / 1e9));
        // The area it covers: the cover width (a deck board's full width)
        // times its length.
        assert!(near(row["area"].as_f64().unwrap(), cover * 3.0 / 1000.0));
        total += cover * 3.0 / 1000.0;
    }
    assert!(near(list["totals"]["area"].as_f64().unwrap(), total));
    // build_parts names the profile and whether the boards join.
    let parts = query(&doc, json!({"query": "build_parts"}));
    let boards: Vec<&Value> = parts["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| &p["board"])
        .collect();
    assert_eq!(
        boards[0],
        &json!({"profile": "tongue_and_groove", "cover": 112.0, "joined": true})
    );
    assert_eq!(boards[3]["joined"], false);
    // Plain timber has no profile columns, and a design without profiled
    // boards none at all.
    let mut plain = design();
    place(
        &mut plain,
        json!({"part": part("deck120"), "at": {"point": [0, 0, 0]}, "length": 1000}),
    );
    let list = query(&plain, json!({"query": "build_cut_list"}));
    assert!(list.get("profiled").is_none());
    assert!(list["rows"][0].get("profile").is_none());
    assert!(list["totals"].get("area").is_none());
}

/// Three joists along Y on edge, 1000 apart, their tops at z 74.
fn joists(doc: &mut Document<MockKernel>) -> Value {
    let j1 = member(doc, "beam148", [0.0, 0.0, 0.0], [0.0, 2000.0, 0.0], "edge");
    member(
        doc,
        "beam148",
        [1000.0, 0.0, 0.0],
        [1000.0, 2000.0, 0.0],
        "edge",
    );
    member(
        doc,
        "beam148",
        [2000.0, 0.0, 0.0],
        [2000.0, 2000.0, 0.0],
        "edge",
    );
    j1
}

#[test]
fn a_tongue_and_groove_floor_steps_by_its_cover_width() {
    let mut doc = design();
    let j1 = joists(&mut doc);
    let top = mount(&doc, &j1, "face_xp");
    // Floorboards 28 x 120 flat across the joists over 1000: edge to edge,
    // 112 apart (the gap asked for is not kept); nine, the last ripped to
    // 1000 - 8 x 112 = 104.
    let floor = place(
        &mut doc,
        json!({"part": part("tg120"),
               "at": {"occurrence": j1["occurrence"], "geometry": top, "point": [-24, 0, 74]},
               "to": {"point": [2024, 0, 74]},
               "spacing": {"mode": "gap", "distance": 3, "along": [0, 1, 0], "length": 1000}}),
    );
    all_ok(&doc);
    assert_eq!(floor["count"], 9, "{floor}");
    assert!(
        floor["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("without the gap of 3 mm")),
        "{floor}"
    );
    let set = set_of(&floor);
    let first = at(&doc, &set[0]);
    // Lying flat: its width across the joists, 14 above their top.
    assert!(close(first.translation, [-24.0, 0.0, 88.0]));
    assert!(close(first.apply_vector([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
    for (i, c) in set.iter().enumerate() {
        assert!(
            near(at(&doc, c).translation[1], 112.0 * i as f64),
            "board {i}: {:?}",
            at(&doc, c).translation
        );
    }
    // The cut list: eight boards covering 112 each and the ripped one
    // 104, all 2048 long: the floor's 1000 x 2048.
    let list = query(&doc, json!({"query": "build_cut_list"}));
    let rows: Vec<&Value> = list["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| !r["profile"].is_null())
        .collect();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let full = rows.iter().find(|r| r["quantity"] == 8).unwrap();
    assert_eq!(full["profile"], "tongue_and_groove");
    assert!(near(full["cover"].as_f64().unwrap(), 112.0));
    assert!(near(full["area"].as_f64().unwrap(), 8.0 * 0.112 * 2.048));
    let ripped = rows.iter().find(|r| r["quantity"] == 1).unwrap();
    assert_eq!(ripped["machining"][0]["kind"], "rip");
    assert!(near(ripped["area"].as_f64().unwrap(), 0.104 * 2.048));
    assert!(near(list["totals"]["area"].as_f64().unwrap(), 2.048));
    // The joists too: three of 2000.
    assert!(near(
        list["totals"]["running_metres"].as_f64().unwrap(),
        9.0 * 2.048 + 3.0 * 2.0
    ));
}

#[test]
fn a_shiplap_wall_steps_up_by_its_cover_width() {
    let mut doc = design();
    // Three studs standing 600 apart, 2400 high, their 148 along X: their
    // -Y faces (y -24) are the wall's.
    let s1 = member(
        &mut doc,
        "beam148",
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 2400.0],
        "edge",
    );
    member(
        &mut doc,
        "beam148",
        [600.0, 0.0, 0.0],
        [600.0, 0.0, 2400.0],
        "edge",
    );
    member(
        &mut doc,
        "beam148",
        [1200.0, 0.0, 0.0],
        [1200.0, 0.0, 2400.0],
        "edge",
    );
    let face = mount(&doc, &s1, "face_yn");
    // Shiplap 22 x 145 along the wall, stepped up by 133 over 2400:
    // eighteen boards, the last ripped to 2400 - 17 x 133 = 139.
    let wall = place(
        &mut doc,
        json!({"part": part("shiplap145"),
               "at": {"occurrence": s1["occurrence"], "geometry": face, "point": [-74, -24, 0]},
               "to": {"point": [1274, -24, 0]},
               "spacing": {"mode": "gap", "along": [0, 0, 1], "length": 2400}}),
    );
    all_ok(&doc);
    assert_eq!(wall["count"], 18, "{wall}");
    assert!(wall["warnings"].as_array().unwrap().is_empty(), "{wall}");
    let set = set_of(&wall);
    let first = at(&doc, &set[0]);
    // On the studs' face, its visible face (+Y) outwards, its lip (+X)
    // up: 11 out from the face.
    assert!(close(first.apply_vector([0.0, 1.0, 0.0]), [0.0, -1.0, 0.0]));
    assert!(close(first.apply_vector([1.0, 0.0, 0.0]), [0.0, 0.0, 1.0]));
    assert!(near(first.translation[1], -35.0));
    for (i, c) in set.iter().enumerate() {
        let z = at(&doc, c).translation[2] - first.translation[2];
        assert!(near(z, 133.0 * i as f64), "board {i}: {z}");
    }
    // The cut list: 17 boards covering 133 and the ripped one its cover
    // too (139 left of it), 1348 long.
    let list = query(&doc, json!({"query": "build_cut_list"}));
    let area = 18.0 * 0.133 * 1.348;
    assert!(
        near(list["totals"]["area"].as_f64().unwrap(), area),
        "{}",
        list["totals"]
    );
    let row = list["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["quantity"] == 17)
        .unwrap()
        .clone();
    assert_eq!(row["profile"], "shiplap");
    assert!(near(row["cover"].as_f64().unwrap(), 133.0));
    assert!(near(
        row["mass"].as_f64().unwrap(),
        450.0 * 2926.0 * 1348.0 / 1e9
    ));
}

#[test]
fn deck_boards_keep_their_gap() {
    let mut doc = design();
    let j1 = joists(&mut doc);
    let top = mount(&doc, &j1, "face_xp");
    let deck = place(
        &mut doc,
        json!({"part": part("deck-grooved"),
               "at": {"occurrence": j1["occurrence"], "geometry": top, "point": [-24, 0, 74]},
               "to": {"point": [2024, 0, 74]},
               "spacing": {"mode": "gap", "distance": 5, "along": [0, 1, 0], "length": 1000}}),
    );
    all_ok(&doc);
    // 120 + 5 a step over 1000: eight boards, the last whole (1000 - 7 x
    // 125 = 125 left).
    let set = set_of(&deck);
    assert_eq!(set.len(), 8, "{deck}");
    assert!(near(at(&doc, &set[1]).translation[1], 125.0));
    let list = query(&doc, json!({"query": "build_cut_list"}));
    assert!(
        list["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["profile"] == "deck")
    );
    assert!(near(
        list["totals"]["area"].as_f64().unwrap(),
        8.0 * 0.12 * 2.048
    ));
}

#[test]
fn a_board_against_another_fits_tongue_into_groove() {
    let mut doc = design();
    let a = place(
        &mut doc,
        json!({"part": part("tg120"), "at": {"point": [0, 0, 0]}, "length": 2000}),
    );
    let tongue = mount(&doc, &a, "tongue");
    let groove = mount(&doc, &a, "groove");
    // On its tongue: the new board's groove on it, the cover width on.
    let b = place(
        &mut doc,
        json!({"part": part("tg120"), "length": 2000,
               "at": {"occurrence": a["occurrence"], "geometry": tongue}}),
    );
    all_ok(&doc);
    assert_eq!(b["place"], "frame");
    let t = placed(&doc, occurrence(&b));
    assert!(
        close(t.translation, [112.0, 0.0, 0.0]),
        "{:?}",
        t.translation
    );
    assert!(close(t.apply_vector([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]));
    assert!(close(t.apply_vector([0.0, 0.0, 1.0]), [0.0, 0.0, 1.0]));
    // In its groove: the new board's tongue in it, on the other side.
    let c = place(
        &mut doc,
        json!({"part": part("tg120"), "length": 2000,
               "at": {"occurrence": a["occurrence"], "geometry": groove}}),
    );
    let t = placed(&doc, occurrence(&c));
    assert!(
        close(t.translation, [-112.0, 0.0, 0.0]),
        "{:?}",
        t.translation
    );
    // Another tongue does not fit; nor does another profile.
    let thin = command(
        &mut doc,
        json!({"cmd": "build_place", "part": part("tg120-thin"),
               "at": {"occurrence": a["occurrence"], "geometry": tongue}}),
    )
    .unwrap_err();
    assert!(
        thin.contains("the profiles do not match: the tongues"),
        "{thin}"
    );
    let shiplap = command(
        &mut doc,
        json!({"cmd": "build_place", "part": part("shiplap145"),
               "at": {"occurrence": a["occurrence"], "geometry": tongue}}),
    )
    .unwrap_err();
    assert!(
        shiplap.contains("is shiplap, Floorboard 28x120 is tongue and groove")
            || shiplap.contains("the profiles do not match"),
        "{shiplap}"
    );
}
