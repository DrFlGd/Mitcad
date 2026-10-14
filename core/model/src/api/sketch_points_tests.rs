// SPDX-License-Identifier: MIT
//! Points that become one, with the mock kernel: Extend joins the end it
//! reaches (mitcad#259), and a Coincident constraint between two curve ends
//! merges them (mitcad#260).

use serde_json::{Value, json};

use crate::Document;
use crate::testing::MockKernel;

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

fn rejected(doc: &mut Document<MockKernel>, command: Value) -> String {
    doc.command(&command.to_string())
        .map(|r| panic!("{command} must fail, got {r}"))
        .unwrap_err()
        .to_string()
}

fn sketch(doc: &Document<MockKernel>) -> Value {
    serde_json::from_str(
        &doc.query(&json!({"query": "sketch", "uid": "F1"}).to_string())
            .unwrap(),
    )
    .unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(1.0)
}

fn near(p: &Value, q: [f64; 2]) -> bool {
    close(p[0].as_f64().unwrap(), q[0]) && close(p[1].as_f64().unwrap(), q[1])
}

fn areas(doc: &Document<MockKernel>) -> Vec<f64> {
    let mut areas: Vec<f64> = sketch(doc)["regions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["area"].as_f64().unwrap())
        .collect();
    areas.sort_by(f64::total_cmp);
    areas
}

fn entity(s: &Value, id: &str) -> Value {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {s}"))
        .clone()
}

fn count(s: &Value, kind: &str) -> usize {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == kind)
        .count()
}

fn constraints_of(s: &Value, kind: &str) -> Vec<Value> {
    s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["type"] == kind)
        .cloned()
        .collect()
}

/// The id of the point at a position.
fn point_id(s: &Value, at: [f64; 2]) -> String {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "point" && near(&e["at"], at))
        .unwrap_or_else(|| panic!("no point at {at:?} in {s}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A new document with an empty Sketch1 (F1).
fn with_sketch() -> Document<MockKernel> {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    d
}

fn line(d: &mut Document<MockKernel>, start: Value, end: Value) -> Value {
    command(
        d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": start, "end": end}),
    )
}

fn extend(d: &mut Document<MockKernel>, curve: &str, at: [f64; 2]) -> Value {
    command(
        d,
        json!({"cmd": "sketch.extend", "sketch": "F1", "curve": curve, "at": at}),
    )
}

fn coincident(d: &mut Document<MockKernel>, point: &str, other: &str) -> Value {
    command(
        d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "coincident", "point": point, "entity": other}}),
    )
}

fn drag(d: &mut Document<MockKernel>, point: &str, to: [f64; 2]) {
    command(
        d,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entity": point, "to": to}),
    );
}

// --- Extend (mitcad#259)

#[test]
fn an_extended_line_joins_the_end_of_a_collinear_line() {
    // The reported case: two collinear lines with a gap.
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0])); // c1: p2, p3
    line(&mut d, json!([20, 0]), json!([30, 0])); // c4: p5, p6
    assert_eq!(count(&sketch(&d), "point"), 4);
    let result = extend(&mut d, "c1", [9.0, 0.0]);
    assert_eq!(result["joined"], "p5", "{result}");
    assert_eq!(result["removed"], json!(["p3"]), "{result}");
    let s = sketch(&d);
    assert_eq!(entity(&s, "c1")["end"], "p5", "{s}");
    assert_eq!(entity(&s, "c4")["start"], "p5");
    assert_eq!(count(&s, "point"), 3);
    assert_eq!(count(&s, "line"), 2, "two lines stay two lines");
    assert!(constraints_of(&s, "coincident").is_empty(), "{s}");
    // The joint holds: dragging c4's start takes c1's end with it.
    drag(&mut d, "p5", [22.0, 1.0]);
    let s = sketch(&d);
    assert!(
        near(&entity(&s, "c1")["geometry"]["end"], [22.0, 1.0]),
        "{s}"
    );
    assert!(
        near(&entity(&s, "c4")["geometry"]["start"], [22.0, 1.0]),
        "{s}"
    );
}

#[test]
fn an_extended_line_closes_the_corner_of_an_outline() {
    // An outline open at its bottom right corner (an L corner): the bottom
    // line stops short of the right side's end.
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0])); // c1: p2, p3
    line(&mut d, json!([20, 0]), json!([20, 20])); // c4: p5, p6
    line(&mut d, json!("p6"), json!([0, 20])); // c7: p8
    line(&mut d, json!("p8"), json!("p2")); // c9
    assert!(areas(&d).is_empty());
    let result = extend(&mut d, "c1", [9.0, 0.0]);
    assert_eq!(result["joined"], "p5", "{result}");
    let s = sketch(&d);
    assert_eq!(entity(&s, "c1")["end"], "p5");
    assert!(constraints_of(&s, "coincident").is_empty(), "{s}");
    let a = areas(&d);
    assert!(a.len() == 1 && close(a[0], 400.0), "{a:?}");
}

#[test]
fn an_extended_arc_joins_the_end_of_a_line() {
    let mut d = with_sketch();
    // A quarter arc counter-clockwise from (10, 0) to (0, 10) around the
    // origin: c1 from p2 to p3 around p4.
    let h = 50f64.sqrt();
    let arc = command(
        &mut d,
        json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "three_point",
               "start": [10, 0], "through": [h, h], "end": [0, 10]}),
    );
    assert_eq!(arc["entities"], json!(["p2", "p3", "p4", "c1"]));
    // A line out from the circle at (-10, 0): c5 from p6.
    line(&mut d, json!([-10, 0]), json!([-20, 0]));
    let s = sketch(&d);
    let arc_end = entity(&s, "c1")["end"].as_str().unwrap().to_owned();
    assert!(near(&entity(&s, &arc_end)["at"], [0.0, 10.0]), "{s}");
    let result = extend(&mut d, "c1", [0.0, 10.0]);
    assert_eq!(result["joined"], "p6", "{result}");
    let s = sketch(&d);
    assert_eq!(entity(&s, "c1")["end"], "p6", "{s}");
    assert_eq!(entity(&s, "c5")["start"], "p6");
    assert!(constraints_of(&s, "coincident").is_empty(), "{s}");
    // An arc extended all the way round still becomes a circle.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "three_point",
               "start": [10, 0], "through": [h, h], "end": [0, 10]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": "p2", "end": [20, 0]}),
    );
    extend(&mut d, "c1", [0.0, 10.0]);
    assert_eq!(entity(&sketch(&d), "c1")["type"], "circle");
}

#[test]
fn an_extension_into_the_middle_of_a_line_stays_a_point_on_it() {
    // A T junction: a new point on the line reached, which is not split.
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0])); // c1
    line(&mut d, json!([25, -5]), json!([25, 5])); // c4
    let result = extend(&mut d, "c1", [9.0, 0.0]);
    assert!(result.get("joined").is_none(), "{result}");
    let s = sketch(&d);
    let end = entity(&s, "c1")["end"].as_str().unwrap().to_owned();
    assert!(near(&entity(&s, &end)["at"], [25.0, 0.0]));
    let on = constraints_of(&s, "coincident");
    assert_eq!(on.len(), 1, "{s}");
    assert_eq!(on[0]["point"], end.as_str());
    assert_eq!(on[0]["entity"], "c4");
    assert_eq!(count(&s, "line"), 2);
    assert_eq!(entity(&s, "c4")["start"], "p5");
    assert_eq!(entity(&s, "c4")["end"], "p6");
}

#[test]
fn an_extension_onto_a_projected_end_is_held_there_by_a_constraint() {
    // A projected line's end is the projection's: the extended line gets a
    // point of its own, coincident with it.
    let mut d = Document::new(MockKernel::default());
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy", "entities": [
            {"id": "p1", "type": "point", "at": [0.0, 0.0]},
            {"id": "p2", "type": "point", "at": [10.0, 0.0]},
            {"id": "c3", "type": "line", "start": "p1", "end": "p2"},
            {"id": "p4", "type": "point", "at": [20.0, 0.0], "fixed": true, "reference": true},
            {"id": "p5", "type": "point", "at": [30.0, 0.0], "fixed": true, "reference": true},
            {"id": "c6", "type": "line", "start": "p4", "end": "p5", "fixed": true,
             "reference": true}]}}),
    );
    let result = extend(&mut d, "c3", [9.0, 0.0]);
    assert_eq!(result["joined"], "p4", "{result}");
    let s = sketch(&d);
    let end = entity(&s, "c3")["end"].as_str().unwrap().to_owned();
    assert_ne!(end, "p4");
    assert!(near(&entity(&s, &end)["at"], [20.0, 0.0]));
    let held = constraints_of(&s, "coincident");
    assert_eq!(held.len(), 1, "{s}");
    assert_eq!(held[0]["point"], end.as_str());
    assert_eq!(held[0]["entity"], "p4");
}

// --- Coincident (mitcad#260)

/// Two lines: c1 from (0, 0) to (10, 0) (p2, p3) and c4 from (10, 0) to
/// (10, 10) (p5, p6), not joined.
fn corner() -> Document<MockKernel> {
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0]));
    line(&mut d, json!([10, 0]), json!([10, 10]));
    d
}

#[test]
fn a_coincident_between_two_line_ends_merges_them() {
    let mut d = corner();
    let before = sketch(&d);
    assert_eq!(count(&before, "point"), 4);
    let result = coincident(&mut d, "p3", "p5");
    assert_eq!(
        result["merged"],
        json!({"kept": "p3", "removed": "p5"}),
        "{result}"
    );
    assert_eq!(result["constraints"], json!([]), "{result}");
    assert_eq!(result["removed"], json!(["p5"]), "{result}");
    let s = sketch(&d);
    assert_eq!(count(&s, "point"), 3);
    assert_eq!(entity(&s, "c1")["end"], "p3");
    assert_eq!(entity(&s, "c4")["start"], "p3");
    assert!(constraints_of(&s, "coincident").is_empty(), "{s}");
    assert_eq!(s["dof"], 6);
    // Dragging the shared point moves both lines.
    drag(&mut d, "p3", [12.0, 2.0]);
    let s = sketch(&d);
    assert!(
        near(&entity(&s, "c1")["geometry"]["end"], [12.0, 2.0]),
        "{s}"
    );
    assert!(
        near(&entity(&s, "c4")["geometry"]["start"], [12.0, 2.0]),
        "{s}"
    );
    // One undo step takes the merge back.
    command(&mut d, json!({"cmd": "undo"}));
    command(&mut d, json!({"cmd": "undo"}));
    let s = sketch(&d);
    assert_eq!(count(&s, "point"), 4);
    assert_eq!(entity(&s, "c4")["start"], "p5");
}

#[test]
fn ends_apart_meet_where_the_solve_puts_them() {
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0])); // c1: p2, p3
    line(&mut d, json!([12, 0]), json!([12, 10])); // c4: p5, p6
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": ["p2", "p6"]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "horizontal", "line": "c1"}}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "vertical", "line": "c4"}}),
    );
    let result = coincident(&mut d, "p5", "p3");
    assert_eq!(result["merged"]["removed"], "p5", "{result}");
    let s = sketch(&d);
    // The bottom line horizontal through (0, 0), the side vertical through
    // (12, 10): the corner at (12, 0).
    assert!(near(&entity(&s, "p3")["at"], [12.0, 0.0]), "{s}");
    assert_eq!(s["dof"], 0, "{s}");
}

#[test]
fn a_rounded_rectangle_joined_by_a_coincident_offsets_as_one_closed_chain() {
    // The reported case: lines and corner arcs, the last arc ending at a
    // point of its own where the first line starts.
    let mut d = with_sketch();
    let h = 5.0 - 5.0 / 2f64.sqrt();
    let arc = |d: &mut Document<MockKernel>, start: Value, through: [f64; 2], end: Value| {
        command(
            d,
            json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "three_point",
                   "start": start, "through": through, "end": end}),
        )
    };
    let at = |d: &Document<MockKernel>, p: [f64; 2]| json!(point_id(&sketch(d), p));
    line(&mut d, json!([5, 0]), json!([35, 0]));
    let p = at(&d, [35.0, 0.0]);
    arc(&mut d, p, [40.0 - h, h], json!([40, 5]));
    let p = at(&d, [40.0, 5.0]);
    line(&mut d, p, json!([40, 15]));
    let p = at(&d, [40.0, 15.0]);
    arc(&mut d, p, [40.0 - h, 20.0 - h], json!([35, 20]));
    let p = at(&d, [35.0, 20.0]);
    line(&mut d, p, json!([5, 20]));
    let p = at(&d, [5.0, 20.0]);
    arc(&mut d, p, [h, 20.0 - h], json!([0, 15]));
    let p = at(&d, [0.0, 15.0]);
    line(&mut d, p, json!([0, 5]));
    let p = at(&d, [0.0, 5.0]);
    arc(&mut d, p, [h, h], json!([5, 0]));
    let s = sketch(&d);
    let ends: Vec<String> = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "point" && near(&e["at"], [5.0, 0.0]))
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ends.len(), 2, "two points at the open corner: {s}");
    let merged = coincident(&mut d, &ends[1], &ends[0]);
    assert!(merged.get("merged").is_some(), "{merged}");
    let s = sketch(&d);
    let curves: Vec<String> = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "line" || e["type"] == "arc")
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(curves.len(), 8);
    let offset = command(
        &mut d,
        json!({"cmd": "sketch.offset", "sketch": "F1", "curves": curves, "distance": 1}),
    );
    let made: Vec<String> = offset["made"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(made.len(), 8, "{offset}");
    // Closed: eight corners for eight curves, each shared by two of them.
    let s = sketch(&d);
    let mut corners: Vec<String> = made
        .iter()
        .flat_map(|c| {
            let e = entity(&s, c);
            [e["start"].clone(), e["end"].clone()]
        })
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    corners.sort();
    corners.dedup();
    assert_eq!(corners.len(), 8, "{s}");
    assert_eq!(areas(&d).len(), 2, "the outline and the band of the offset");
}

#[test]
fn a_merge_drops_the_distance_between_the_points_and_keeps_other_dimensions() {
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0])); // c1: p2, p3
    line(&mut d, json!([12, 0]), json!([12, 10])); // c4: p5, p6
    let gap = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "distance", "a": "p3", "b": "p5"}, "value": 2}),
    );
    let gap = gap["dimensions"][0].as_str().unwrap().to_owned();
    let side = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "vertical_distance", "a": "p5", "b": "p6"}, "value": 10}),
    );
    let side = side["dimensions"][0].as_str().unwrap().to_owned();
    // p3 fixed: kept over the free p5, which more dimensions name.
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": ["p3"]}),
    );
    let result = coincident(&mut d, "p5", "p3");
    assert_eq!(
        result["merged"],
        json!({"kept": "p3", "removed": "p5"}),
        "{result}"
    );
    let removed = result["removed"].as_array().unwrap();
    assert!(removed.contains(&json!(gap)), "{result}");
    let s = sketch(&d);
    let dimensions = s["dimensions"].as_array().unwrap();
    assert!(dimensions.iter().all(|k| k["id"] != gap.as_str()), "{s}");
    let kept = dimensions
        .iter()
        .find(|k| k["id"] == side.as_str())
        .unwrap();
    assert_eq!(kept["a"], "p3", "{s}");
    assert!(
        close(entity(&s, "p6")["at"][1].as_f64().unwrap(), 10.0),
        "{s}"
    );
}

#[test]
fn a_fixed_end_is_kept_over_a_free_one() {
    let mut d = corner();
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": ["p5"]}),
    );
    let result = coincident(&mut d, "p3", "p5");
    assert_eq!(
        result["merged"],
        json!({"kept": "p5", "removed": "p3"}),
        "{result}"
    );
    let s = sketch(&d);
    assert_eq!(entity(&s, "c1")["end"], "p5");
    assert_eq!(entity(&s, "p5")["fixed"], true);
}

#[test]
fn other_coincidences_stay_constraints() {
    let stored = |d: &mut Document<MockKernel>, point: &str, other: &str| {
        let result = coincident(d, point, other);
        assert!(result.get("merged").is_none(), "{result}");
        let k = result["constraints"][0].as_str().unwrap().to_owned();
        let s = sketch(d);
        assert!(
            s["constraints"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == k),
            "{s}"
        );
    };
    // A line's end at a circle's centre.
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0])); // c1: p2, p3
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center", "center": [12, 0],
               "radius": 3}),
    ); // c4 around p5
    assert_eq!(entity(&sketch(&d), "c4")["center"], "p5");
    stored(&mut d, "p3", "p5");
    // A line's end at a lone point.
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0]));
    command(
        &mut d,
        json!({"cmd": "sketch.add_point", "sketch": "F1", "at": [12, 0]}),
    ); // p4
    stored(&mut d, "p3", "p4");
    // A line's end at a projected line's end.
    let mut d = Document::new(MockKernel::default());
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy", "entities": [
            {"id": "p1", "type": "point", "at": [0.0, 0.0]},
            {"id": "p2", "type": "point", "at": [10.0, 0.0]},
            {"id": "c3", "type": "line", "start": "p1", "end": "p2"},
            {"id": "p4", "type": "point", "at": [12.0, 0.0], "fixed": true, "reference": true},
            {"id": "p5", "type": "point", "at": [20.0, 0.0], "fixed": true, "reference": true},
            {"id": "c6", "type": "line", "start": "p4", "end": "p5", "fixed": true,
             "reference": true}]}}),
    );
    stored(&mut d, "p2", "p4");
    // The two ends of one curve (a closed outline's last line drawn back
    // to its own start is the user's to make; the command does not fold a
    // curve onto itself).
    let mut d = with_sketch();
    let h = 50f64.sqrt();
    command(
        &mut d,
        json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "three_point",
               "start": [10, 0], "through": [h, h], "end": [0, 10]}),
    ); // c1 from p2 to p3 around p4
    let result = d.command(
        &json!({"cmd": "sketch.add_constraint", "sketch": "F1",
                "constraint": {"type": "coincident", "point": "p2", "entity": "p3"}})
        .to_string(),
    );
    if let Ok(result) = result {
        let result: Value = serde_json::from_str(&result).unwrap();
        assert!(result.get("merged").is_none(), "{result}");
    }
    assert_eq!(entity(&sketch(&d), "c1")["start"], "p2");
    assert_eq!(entity(&sketch(&d), "c1")["end"], "p3");
}

#[test]
fn a_merge_that_would_not_solve_is_refused() {
    // Both ends held apart by fixed points through horizontal lines at
    // different heights: they cannot meet.
    let mut d = with_sketch();
    line(&mut d, json!([0, 0]), json!([10, 0])); // c1: p2, p3
    line(&mut d, json!([20, 5]), json!([12, 5])); // c4: p5, p6
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": ["p2", "p5"]}),
    );
    for l in ["c1", "c4"] {
        command(
            &mut d,
            json!({"cmd": "sketch.add_constraint", "sketch": "F1",
                   "constraint": {"type": "horizontal", "line": l}}),
        );
    }
    rejected(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "coincident", "point": "p3", "entity": "p6"}}),
    );
    let s = sketch(&d);
    assert_eq!(count(&s, "point"), 4);
    assert_eq!(entity(&s, "c4")["end"], "p6");
}
