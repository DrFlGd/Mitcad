// SPDX-License-Identifier: MIT
//! A wall's openings framed by the wall's kind (mitcad#519, `load_bearing`):
//! a partition's window with one flat header between its king studs and no
//! trimmer studs; a load-bearing wall's with trimmers and a header of plies
//! on edge, as a wall saved without the field keeps; switched by an edit
//! and back, on the test library's timber parts.

use serde_json::{Value, json};

use super::test_library::{all_ok, command, design, part, query};
use crate::Document;
use crate::testing::MockKernel;

const WINDOW: &str = r#"[{"offset": 600, "width": 900, "height": 1200, "sill": 900}]"#;

fn window() -> Value {
    serde_json::from_str(WINDOW).unwrap()
}

/// A wall 3600 x 2400 from the origin along +X, studs at 600, with the
/// window at 600 (900 x 1200, its sill at 900) and the extra fields.
fn wall_with_window(doc: &mut Document<MockKernel>, extra: Value) -> Value {
    let mut fields = json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
                            "start": [0, 0, 0], "end": [3600, 0, 0], "height": 2400, "spacing": 600,
                            "openings": window()});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    let made = command(doc, fields).unwrap_or_else(|e| panic!("build_frame: {e}"));
    all_ok(doc);
    made
}

fn edit(doc: &mut Document<MockKernel>, component: &Value, fields: Value) -> Value {
    let edited = command(
        doc,
        json!({"cmd": "build_edit", "component": component, "frame": fields}),
    )
    .unwrap_or_else(|e| panic!("build_edit: {e}"));
    all_ok(doc);
    edited
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

/// The members' places and lengths (µm), sorted: two layouts compared.
fn places(list: &[Value]) -> Vec<(String, i64)> {
    let mut found: Vec<(String, i64)> = list
        .iter()
        .map(|p| {
            (
                p["member"].as_str().unwrap_or_default().to_owned(),
                (length(p) * 1000.0).round() as i64,
            )
        })
        .collect();
    found.sort();
    found
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

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_partition_frames_its_window_with_a_flat_header_and_no_trimmers() {
    let mut doc = design();
    // The preview lays it out so: no trimmer studs, one flat header.
    let preview = query(
        &doc,
        json!({"query": "build_preview", "frame": {"kind": "wall", "part": part("rows98"),
               "start": [0, 0, 0], "end": [3600, 0, 0], "height": 2400, "openings": window(),
               "load_bearing": false}}),
    );
    assert_eq!(preview["count"], 13, "{preview}");
    assert_eq!(preview["roles"]["flat header"], 1, "{preview}");
    assert_eq!(preview["roles"]["king stud"], 2, "{preview}");
    assert!(preview["roles"].get("trimmer stud").is_none(), "{preview}");
    assert!(preview["roles"].get("header").is_none(), "{preview}");

    let made = wall_with_window(&mut doc, json!({"load_bearing": false}));
    assert_eq!(made["count"], 13, "{made}");
    let lead = made["frame"].clone();
    let list = members(&doc, &lead);
    assert_eq!(list.len(), 13, "{:?}", places(&list));
    assert_eq!(count_of(&list, "opening.1.trimmer"), 0);
    // The king studs are the opening's sides: their centres 600 - 24 and
    // 1500 + 24, full height between the plates.
    let king_a = member(&list, "opening.1.king.a");
    let at = end_a(&doc, king_a);
    assert!(
        near(at[0], 576.0) && near(at[1], 0.0) && near(at[2], 48.0),
        "{at:?}"
    );
    assert!(near(length(king_a), 2304.0));
    let at = end_a(&doc, member(&list, "opening.1.king.b"));
    assert!(near(at[0], 1524.0), "{at:?}");
    // One header lying flat between them, its 48 up: from 600 to 1500, its
    // axis 24 above the opening's head at 2100.
    let header = member(&list, "opening.1.header");
    assert!(near(length(header), 900.0), "{}", length(header));
    let at = end_a(&doc, header);
    assert!(
        near(at[0], 600.0) && near(at[1], 0.0) && near(at[2], 2124.0),
        "{at:?}"
    );
    // The cripple studs above from its top at 2148 to the top plate at 2352;
    // the sill and the cripples below as in a load-bearing wall.
    assert!(near(
        length(member(&list, "opening.1.cripple.top.2")),
        204.0
    ));
    assert!(near(
        length(member(&list, "opening.1.cripple.bottom.2")),
        804.0
    ));
    assert!(near(length(member(&list, "opening.1.sill")), 900.0));
    // The definition keeps it.
    let lead_part = list.iter().find(|p| p["component"] == lead).unwrap();
    assert_eq!(lead_part["framing"]["definition"]["load_bearing"], false);
    // The cut list has the flat header with the other timber.
    let rows = query(&doc, json!({"query": "build_cut_list"}));
    let total: u64 = rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["quantity"].as_u64().unwrap())
        .sum();
    assert_eq!(total, 13, "{rows}");
}

#[test]
fn a_wall_saved_without_the_field_keeps_its_load_bearing_framing() {
    // As saved before mitcad#519: no `load_bearing`.
    let mut old = design();
    let made = wall_with_window(&mut old, json!({}));
    assert_eq!(made["count"], 16, "{made}");
    let lead = made["frame"].clone();
    let before = members(&old, &lead);
    let lead_part = before.iter().find(|p| p["component"] == lead).unwrap();
    assert!(
        lead_part["framing"]["definition"]
            .get("load_bearing")
            .is_none(),
        "{lead_part}"
    );
    assert_eq!(count_of(&before, "opening.1.trimmer"), 2);
    assert_eq!(count_of(&before, "opening.1.header."), 2);
    // The same members as a wall made load-bearing on purpose.
    let mut explicit = design();
    let bearing = wall_with_window(&mut explicit, json!({"load_bearing": true}));
    assert_eq!(
        places(&before[..]),
        places(&members(&explicit, &bearing["frame"])[..])
    );
    // An edit of another value leaves it load-bearing.
    let edited = edit(&mut old, &lead, json!({"spacing": 400}));
    let list = members(&old, &lead);
    assert_eq!(count_of(&list, "opening.1.trimmer"), 2, "{edited}");
    assert_eq!(count_of(&list, "opening.1.header."), 2, "{edited}");
    // Made a partition: the trimmers go, one flat header; and back.
    let edited = edit(&mut old, &lead, json!({"load_bearing": false}));
    let list = members(&old, &lead);
    assert_eq!(count_of(&list, "opening.1.trimmer"), 0, "{edited}");
    assert_eq!(
        member(&list, "opening.1.header")["member"],
        "opening.1.header"
    );
    assert_eq!(count_of(&list, "opening.1.header."), 0, "{edited}");
    let edited = edit(&mut old, &lead, json!({"load_bearing": true}));
    let list = members(&old, &lead);
    assert_eq!(count_of(&list, "opening.1.trimmer"), 2, "{edited}");
    assert_eq!(count_of(&list, "opening.1.header."), 2, "{edited}");
}

#[test]
fn a_partition_takes_an_opening_nearer_its_end_studs() {
    // Without trimmers the opening's framing is a stud narrower each side:
    // at 3600 - 48 - 48 - 900 it fits a partition, not a load-bearing wall.
    let mut doc = design();
    let near_end = json!([{"offset": 2604, "width": 900, "height": 2100}]);
    let fields = |bearing: bool| {
        json!({"query": "build_preview", "frame": {"kind": "wall", "part": part("rows98"),
               "start": [0, 0, 0], "end": [3600, 0, 0], "height": 2400, "openings": near_end,
               "load_bearing": bearing}})
    };
    let partition = query(&doc, fields(false));
    assert_eq!(partition["roles"]["flat header"], 1, "{partition}");
    let refused = command(
        &mut doc,
        json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
               "start": [0, 0, 0], "end": [3600, 0, 0], "height": 2400, "openings": near_end,
               "load_bearing": true}),
    )
    .unwrap_err();
    assert!(refused.contains("does not fit"), "{refused}");
}
