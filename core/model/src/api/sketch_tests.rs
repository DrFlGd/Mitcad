// SPDX-License-Identifier: MIT
//! Sketch commands and the sketch query, with the mock kernel.

use std::f64::consts::PI;

use serde_json::{Value, json};

use crate::Document;
use crate::kernel::Curve3;
use crate::testing::MockKernel;

fn doc() -> Document<MockKernel> {
    Document::new(MockKernel::default())
}

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

fn sketch(doc: &Document<MockKernel>, uid: &str) -> Value {
    serde_json::from_str(
        &doc.query(&json!({"query": "sketch", "uid": uid}).to_string())
            .unwrap(),
    )
    .unwrap()
}

fn areas(doc: &Document<MockKernel>, uid: &str) -> Vec<f64> {
    let mut areas: Vec<f64> = sketch(doc, uid)["regions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["area"].as_f64().unwrap())
        .collect();
    areas.sort_by(f64::total_cmp);
    areas
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(1.0)
}

fn assert_areas(doc: &Document<MockKernel>, uid: &str, expected: &[f64]) {
    let actual = areas(doc, uid);
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!(close(*a, *e), "{actual:?} != {expected:?}");
    }
}

/// A new document with an empty Sketch1 (F1).
fn with_sketch() -> Document<MockKernel> {
    let mut d = doc();
    command(&mut d, json!({"cmd": "sketch.create"}));
    d
}

fn made(result: &Value) -> Vec<String> {
    result["made"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap().to_owned()).collect())
        .unwrap_or_default()
}

#[test]
fn lines_with_shared_points_constraints_and_dimensions() {
    let mut d = with_sketch();
    let first = command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [40, 1]}),
    );
    assert_eq!(first["entities"], json!(["p2", "p3", "c1"]));
    // Line c4 from p3 to a new p5, line c6 back to p2.
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": "p3", "end": [20, 30]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": "p5", "end": "p2"}),
    );
    // A closed triangle: one region.
    assert_eq!(areas(&d, "F1").len(), 1);
    let s = sketch(&d, "F1");
    assert_eq!(s["dof"], 6);
    // Fix it down: the first point, the base horizontal and 40 long, the
    // apex 30 above the base's middle.
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": ["p2"]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "horizontal", "line": "c1"}}),
    );
    let dim = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "length", "line": "c1"}, "value": 40}),
    );
    assert_eq!(dim["dimensions"], json!(["k2"]));
    assert_eq!(dim["parameters"], json!(["d1"]));
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "point_line_distance", "point": "p5", "line": "c1"},
               "value": "d1 * 3 / 4"}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "equal", "a": "c4", "b": "c6"}}),
    );
    let s = sketch(&d, "F1");
    assert_eq!(s["dof"], 0, "{}", s["fully_constrained"]);
    assert_area_close(&d, 600.0);
    // A parameter change drives the sketch.
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    assert_area_close(&d, 80.0 * 60.0 / 2.0);
    // An over-constraining constraint is refused, naming the conflict.
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "vertical", "line": "c1"}}),
    );
    assert!(
        error.contains("over-constrained") && error.contains("k1"),
        "{error}"
    );
    // A driven dimension measures; making it driving keeps the value.
    let driven = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "length", "line": "c4"}, "driven": true}),
    );
    let k = driven["dimensions"][0].as_str().unwrap().to_owned();
    let s = sketch(&d, "F1");
    let measured = s["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == k)
        .unwrap()["measured"]
        .as_f64()
        .unwrap();
    assert!(close(measured, (40f64.powi(2) + 60f64.powi(2)).sqrt()));
    assert!(
        rejected(
            &mut d,
            json!({"cmd": "sketch.set_driven", "sketch": "F1", "dimension": k, "driven": false})
        )
        .contains("over-constrain")
    );
}

fn assert_area_close(d: &Document<MockKernel>, expected: f64) {
    let a = areas(d, "F1");
    assert_eq!(a.len(), 1, "{a:?}");
    assert!(close(a[0], expected), "{a:?} != {expected}");
}

#[test]
fn rectangle_circle_arc_and_polygon_tools() {
    let mut d = with_sketch();
    let r = command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point",
               "a": [10, 20], "b": [0, 0]}),
    );
    assert_eq!(made(&r), ["c1", "c2", "c3", "c4"]);
    assert_eq!(r["status"]["dof"], 4);
    command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "three_point",
               "a": [100, 0], "b": [110, 10], "c": [100, 20]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "center",
               "center": [200, 0], "corner": [205, 3]}),
    );
    let expected_tilted = 200f64.sqrt() * (10.0 / 2f64.sqrt() + 10.0 * 2f64.sqrt()) / 2.0;
    let _ = expected_tilted;
    let a = areas(&d, "F1");
    assert_eq!(a.len(), 3);
    assert!(close(a[0], 60.0) && close(a[1], 200.0), "{a:?}");
    // Circles: by centre, two points and three points.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center", "center": [0, 0], "radius": 5}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "two_point", "a": [20, 0], "b": [30, 0]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "three_point",
               "a": [45, 0], "b": [50, 5], "c": [55, 0]}),
    );
    assert_areas(&d, "F1", &[PI * 25.0, PI * 25.0, PI * 25.0]);
    // A half disk: a three-point arc closed by a line through its ends.
    let mut d = with_sketch();
    let arc = command(
        &mut d,
        json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "three_point",
               "start": [10, 0], "through": [0, 10], "end": [-10, 0]}),
    );
    // c1 from p2 to p3 around p4.
    assert_eq!(arc["entities"], json!(["p2", "p3", "p4", "c1"]));
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": "p2", "end": "p3"}),
    );
    assert_areas(&d, "F1", &[PI * 50.0]);
    // A tangent arc continues a line.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [10, 0]}),
    );
    let tangent = command(
        &mut d,
        json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "tangent", "from": "p3", "end": [10, 10]}),
    );
    assert_eq!(tangent["constraints"], json!(["k1"]));
    let s = sketch(&d, "F1");
    let geometry = &s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "arc")
        .unwrap()["geometry"];
    assert!(
        close(geometry["radius"].as_f64().unwrap(), 5.0),
        "{geometry}"
    );
    // A hexagon on a construction circle: four degrees of freedom.
    let mut d = with_sketch();
    let hexagon = command(
        &mut d,
        json!({"cmd": "sketch.polygon", "sketch": "F1", "center": [0, 0], "vertex": [10, 0], "sides": 6}),
    );
    assert_eq!(made(&hexagon).len(), 6);
    assert_eq!(hexagon["status"]["dof"], 4);
    assert_areas(&d, "F1", &[3.0 * 3f64.sqrt() / 2.0 * 100.0]);
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.polygon", "sketch": "F1", "center": [0, 0], "vertex": [10, 0],
               "sides": 4, "inscribed": false}),
    );
    assert_areas(&d, "F1", &[400.0]);
}

#[test]
fn slots_ellipses_splines_and_text() {
    let mut d = with_sketch();
    let slot = command(
        &mut d,
        json!({"cmd": "sketch.slot", "sketch": "F1", "mode": "center_to_center",
               "a": [0, 0], "b": [30, 0], "width": 10}),
    );
    assert_eq!(made(&slot).len(), 4);
    assert_areas(&d, "F1", &[300.0 + PI * 25.0]);
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.slot", "sketch": "F1", "mode": "center_point",
               "center": [0, 0], "end": [15, 0], "width": 10}),
    );
    assert_areas(&d, "F1", &[300.0 + PI * 25.0]);
    // An arc slot: a ring sector with round ends.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.slot", "sketch": "F1", "mode": "arc",
               "center": [0, 0], "start": [50, 0], "end": [0, 50], "width": 10}),
    );
    let sector = PI / 4.0 * (55.0f64.powi(2) - 45.0f64.powi(2));
    assert_areas(&d, "F1", &[sector + PI * 25.0]);
    // An ellipse and a closed region of a spline and a line.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.ellipse", "sketch": "F1", "center": [0, 0], "major": [20, 0],
               "minor_radius": 10}),
    );
    assert_areas(&d, "F1", &[PI * 200.0]);
    let mut d = with_sketch();
    let spline = command(
        &mut d,
        json!({"cmd": "sketch.spline", "sketch": "F1", "points": [[0, 0], [10, 10], [20, 0]]}),
    );
    let pts = spline["entities"].as_array().unwrap().clone();
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": pts[2], "end": pts[0]}),
    );
    assert_eq!(areas(&d, "F1").len(), 1);
    let text = command(
        &mut d,
        json!({"cmd": "sketch.add_text", "sketch": "F1", "text": "Mitcad", "at": [0, -20], "height": 5}),
    );
    assert_eq!(text["texts"].as_array().unwrap().len(), 1);
    assert_eq!(sketch(&d, "F1")["texts"][0]["text"], "Mitcad");
}

#[test]
fn trim_extend_and_delete() {
    // A rectangle with a vertical line through it.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point", "a": [0, 0], "b": [40, 20]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [20, -10], "end": [20, 30]}),
    ); // c9
    assert_areas(&d, "F1", &[400.0, 400.0]);
    // Trimming the ends outside leaves the line from c1 to c3.
    command(
        &mut d,
        json!({"cmd": "sketch.trim", "sketch": "F1", "curve": "c9", "at": [20, -5]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.trim", "sketch": "F1", "curve": "c9", "at": [20, 25]}),
    );
    let s = sketch(&d, "F1");
    let line = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "c9")
        .unwrap()
        .clone();
    assert_eq!(
        line["geometry"],
        json!({"start": [20.0, 0.0], "end": [20.0, 20.0]})
    );
    assert_areas(&d, "F1", &[400.0, 400.0]);
    // Trimming the right half of the bottom opens the right square.
    command(
        &mut d,
        json!({"cmd": "sketch.trim", "sketch": "F1", "curve": "c1", "at": [30, 0]}),
    );
    assert_areas(&d, "F1", &[400.0]);
    // Trimming the middle of the top splits it into two lines.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [30, 0]}),
    ); // c1
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [10, -5], "end": [10, 5]}),
    ); // c4
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [20, -5], "end": [20, 5]}),
    ); // c7
    let trimmed = command(
        &mut d,
        json!({"cmd": "sketch.trim", "sketch": "F1", "curve": "c1", "at": [15, 0]}),
    );
    assert!(
        trimmed["constraints"].as_array().unwrap().len() >= 3,
        "{trimmed}"
    );
    let s = sketch(&d, "F1");
    let lines = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "line")
        .count();
    assert_eq!(lines, 4);
    // Extend: a line short of another reaches it.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [10, 0]}),
    ); // c1
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [25, -5], "end": [25, 5]}),
    ); // c4
    command(
        &mut d,
        json!({"cmd": "sketch.extend", "sketch": "F1", "curve": "c1", "at": [9, 0]}),
    );
    let s = sketch(&d, "F1");
    let line = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "c1")
        .unwrap()
        .clone();
    assert_eq!(line["geometry"]["end"], json!([25.0, 0.0]));
    // Deleting a point takes its curve and the constraints on them.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point", "a": [0, 0], "b": [40, 20]}),
    );
    let removed = command(
        &mut d,
        json!({"cmd": "sketch.remove", "sketch": "F1", "items": ["p5"]}),
    );
    assert_eq!(removed["removed"], json!(["c1", "c4", "p5", "k1", "k4"]));
    assert!(areas(&d, "F1").is_empty());
    assert!(
        rejected(
            &mut d,
            json!({"cmd": "sketch.remove", "sketch": "F1", "items": ["c99"]})
        )
        .contains("does not exist")
    );
}

#[test]
fn fillet_chamfer_and_offset() {
    // A 40 x 20 rectangle of fixed size (d1, d2).
    let rectangle = |d: &mut Document<MockKernel>| {
        command(
            d,
            json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0],
                   "width": 40, "height": 20}),
        );
    };
    let mut d = with_sketch();
    rectangle(&mut d);
    let fillet = command(
        &mut d,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "a": "c1", "b": "c2", "radius": 5}),
    );
    assert_eq!(fillet["parameters"], json!(["d3"]));
    let rounded = 800.0 - 25.0 * (1.0 - PI / 4.0);
    assert_areas(&d, "F1", &[rounded]);
    assert_eq!(sketch(&d, "F1")["dof"], 0);
    let chamfer = command(
        &mut d,
        json!({"cmd": "sketch.chamfer", "sketch": "F1", "a": "c3", "b": "c4", "distance": 4}),
    );
    assert_eq!(chamfer["parameters"], json!(["d4"]));
    assert_areas(&d, "F1", &[rounded - 8.0]);
    // Both follow their parameters.
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d4", "value": 6}),
    );
    assert_areas(&d, "F1", &[rounded - 18.0]);
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d3", "value": 2}),
    );
    assert_areas(&d, "F1", &[800.0 - 4.0 * (1.0 - PI / 4.0) - 18.0]);
    // Offset inside a counter-clockwise rectangle: negative distance.
    let mut d = with_sketch();
    rectangle(&mut d);
    let offset = command(
        &mut d,
        json!({"cmd": "sketch.offset", "sketch": "F1", "curves": ["c1", "c2", "c3", "c4"],
               "distance": -5}),
    );
    assert_eq!(made(&offset).len(), 4);
    assert_eq!(offset["parameters"], json!(["d3"]));
    assert_areas(&d, "F1", &[300.0, 500.0]);
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d3", "value": 2}),
    );
    assert_areas(&d, "F1", &[800.0 - 36.0 * 16.0, 36.0 * 16.0]);
    // A circle offsets out with a positive distance.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center", "center": [0, 0], "radius": 10}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.offset", "sketch": "F1", "curves": ["c1"], "distance": 5}),
    );
    assert_areas(&d, "F1", &[PI * 100.0, PI * 125.0]);
}

#[test]
fn mirror_patterns_move_and_drag() {
    // A triangle mirrored about a vertical line.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, -20], "end": [0, 20],
               "construction": true}),
    ); // c1
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [10, 0]}),
    ); // c4 from p5 to p6
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": "p6", "end": [0, 10]}),
    ); // c7 to p8
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": "p8", "end": "p5"}),
    ); // c9
    assert_areas(&d, "F1", &[50.0]);
    let mirrored = command(
        &mut d,
        json!({"cmd": "sketch.mirror", "sketch": "F1", "entities": ["c4", "c7", "c9"], "axis": "c1"}),
    );
    assert_eq!(made(&mirrored).len(), 3);
    assert_eq!(mirrored["constraints"].as_array().unwrap().len(), 3);
    assert_areas(&d, "F1", &[50.0, 50.0]);
    // Patterns of a circle.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center", "center": [20, 0], "radius": 2}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.circular_pattern", "sketch": "F1", "entities": ["c1"],
               "center": [0, 0], "count": 4}),
    );
    assert_eq!(areas(&d, "F1").len(), 4);
    let s = sketch(&d, "F1");
    let centers: Vec<Value> = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "circle")
        .map(|e| e["geometry"]["center"].clone())
        .collect();
    let top = centers.iter().any(|c| {
        (c[0].as_f64().unwrap()).abs() < 1e-9 && (c[1].as_f64().unwrap() - 20.0).abs() < 1e-9
    });
    assert!(top, "{centers:?}");
    command(
        &mut d,
        json!({"cmd": "sketch.rectangular_pattern", "sketch": "F1", "entities": ["c1"],
               "count": [2, 3], "spacing": [10, 10]}),
    );
    assert_eq!(areas(&d, "F1").len(), 9);
    // Move and copy keep constraints; drag pulls a free point.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point", "a": [0, 0], "b": [10, 10]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.move", "sketch": "F1", "entities": ["c1", "c2", "c3", "c4"], "by": [5, 0]}),
    );
    let s = sketch(&d, "F1");
    let p5 = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "p5")
        .unwrap()["at"]
        .clone();
    assert_eq!(p5, json!([5.0, 0.0]));
    let copy = command(
        &mut d,
        json!({"cmd": "sketch.move", "sketch": "F1", "entities": ["c1", "c2", "c3", "c4"],
               "rotate": {"center": [0, 0], "angle": PI / 2.0}, "copy": true}),
    );
    assert_eq!(made(&copy).len(), 4);
    assert_eq!(areas(&d, "F1").len(), 2);
    command(
        &mut d,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entity": "p7", "to": [20, 20]}),
    );
    let s = sketch(&d, "F1");
    let p7 = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "p7")
        .unwrap()["at"]
        .clone();
    assert!(
        close(p7[0].as_f64().unwrap(), 20.0) && close(p7[1].as_f64().unwrap(), 20.0),
        "{p7}"
    );
    // Horizontal and vertical sides held: the rectangle grew.
    assert_areas(&d, "F1", &[100.0, 15.0 * 20.0]);
}

#[test]
fn dimensions_take_expressions_and_flags_change() {
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    command(
        &mut d,
        json!({"cmd": "add_parameter", "name": "w", "value": "100 mm"}),
    );
    // Binding the width to a user parameter, then an expression.
    command(
        &mut d,
        json!({"cmd": "sketch.set_dimension", "sketch": "F1", "dimension": "k5", "value": "w"}),
    );
    assert_areas(&d, "F1", &[4000.0]);
    command(
        &mut d,
        json!({"cmd": "sketch.set_dimension", "sketch": "F1", "dimension": "k6", "value": "w / 4"}),
    );
    assert_areas(&d, "F1", &[2500.0]);
    // d1 was used by nothing any more and went with the change.
    let params: Value =
        serde_json::from_str(&d.query(r#"{"query": "parameters"}"#).unwrap()).unwrap();
    let names: Vec<&str> = params
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["d2", "w"]);
    // Construction lines leave the profile.
    command(
        &mut d,
        json!({"cmd": "sketch.set_construction", "sketch": "F1", "curves": ["c1"]}),
    );
    assert!(areas(&d, "F1").is_empty());
    command(
        &mut d,
        json!({"cmd": "sketch.set_construction", "sketch": "F1", "curves": ["c1"], "construction": false}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.set_centerline", "sketch": "F1", "lines": ["c1"]}),
    );
    assert_eq!(areas(&d, "F1").len(), 1, "centre lines bound profiles");
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "diagonal"}),
    );
    assert!(error.contains("unknown rectangle mode"), "{error}");
}

#[test]
fn sketches_on_faces_and_projections_follow_the_model() {
    let mut d = doc();
    command(&mut d, json!({"cmd": "sketch.create"}));
    command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
               "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}),
    );
    // The top face: z = 20, facing up (the mock's end caps). Its frame is
    // the XY frame raised.
    let face = "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";
    command(
        &mut d,
        json!({"cmd": "sketch.create", "plane": {"face": face}}),
    );
    // The face's edges projected, linked.
    let square = |z: f64, size: f64| -> Vec<Curve3> {
        let c = [[0.0, 0.0], [size, 0.0], [size, 40.0], [0.0, 40.0]];
        (0..4)
            .map(|i| Curve3::Line {
                start: [c[i][0], c[i][1], z],
                end: [c[(i + 1) % 4][0], c[(i + 1) % 4][1], z],
            })
            .collect()
    };
    *d.kernel().curves.borrow_mut() = square(20.0, 60.0);
    let projected = command(
        &mut d,
        json!({"cmd": "sketch.project", "sketch": "F3", "source": face, "linked": true}),
    );
    assert_eq!(projected["entities"].as_array().unwrap().len(), 8);
    let s = sketch(&d, "F3");
    assert_eq!(s["frame"]["origin"], json!([0.0, 0.0, 20.0]));
    assert_eq!(s["dof"], 0, "projected geometry is fixed");
    assert_eq!(s["projections"].as_array().unwrap().len(), 1);
    assert_areas(&d, "F3", &[2400.0]);
    // When the model changes, the linked projection follows.
    *d.kernel().curves.borrow_mut() = square(20.0, 80.0);
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    assert_areas(&d, "F3", &[3200.0]);
    // Planes: XZ has y along -Z.
    let mut d = doc();
    command(&mut d, json!({"cmd": "sketch.create", "plane": "xz"}));
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center", "center": [0, 0], "radius": 5}),
    );
    let s = sketch(&d, "F1");
    assert_eq!(s["frame"]["y_axis"], json!([0.0, 0.0, -1.0]));
    assert_eq!(s["frame"]["normal"], json!([0.0, 1.0, 0.0]));
    // YZ has the origin datum's frame.
    command(&mut d, json!({"cmd": "sketch.create", "plane": "yz"}));
    let s = sketch(&d, "F2");
    assert_eq!(s["frame"]["x_axis"], json!([0.0, 0.0, -1.0]));
    assert_eq!(s["frame"]["normal"], json!([1.0, 0.0, 0.0]));
}

/// A linked circle that an import set the start of (its seam) follows its
/// source's centre and radius and keeps where it starts (mitcad#225).
#[test]
fn a_linked_circle_keeps_where_it_starts() {
    let mut d = doc();
    command(&mut d, json!({"cmd": "sketch.create"}));
    command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
               "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}),
    );
    let face = "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";
    let circle = |radius: f64| Curve3::Conic {
        center: [30.0, 20.0, 20.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        major: radius,
        minor: radius,
        start: 0.0,
        end: 2.0 * PI,
        closed: true,
    };
    *d.kernel().curves.borrow_mut() = vec![circle(5.0)];
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": {"face": face},
               "entities": [
                   {"id": "p1", "type": "point", "at": [30.0, 20.0], "fixed": true, "reference": true},
                   {"id": "c2", "type": "circle", "center": "p1", "radius": 5.0, "start_angle": 1.0,
                    "fixed": true, "reference": true}],
               "projections": [{"source": face, "body": "F2.b0", "entities": ["p1", "c2"]}]}}),
    );
    *d.kernel().curves.borrow_mut() = vec![circle(6.0)];
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    let s = sketch(&d, "F3");
    let c2 = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "c2")
        .unwrap();
    assert_eq!(
        (&c2["radius"], &c2["start_angle"]),
        (&json!(6.0), &json!(1.0)),
        "{s}"
    );
}

#[test]
fn a_linked_projection_with_an_arc_follows_after_other_entities() {
    let mut d = doc();
    command(&mut d, json!({"cmd": "sketch.create"}));
    command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
               "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}),
    );
    let face = "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";
    command(
        &mut d,
        json!({"cmd": "sketch.create", "plane": {"face": face}}),
    );
    // A line of the user's first: the projection's ids come after its.
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F3", "start": [-50, 200], "end": [-45, 3]}),
    );
    // The top face with a rounded corner, as after a fillet.
    let rounded = |width: f64| -> Vec<Curve3> {
        let line = |a: [f64; 2], b: [f64; 2]| Curve3::Line {
            start: [a[0], a[1], 20.0],
            end: [b[0], b[1], 20.0],
        };
        vec![
            line([0.0, 0.0], [width - 2.0, 0.0]),
            Curve3::Conic {
                center: [width - 2.0, 2.0, 20.0],
                normal: [0.0, 0.0, 1.0],
                x_axis: [1.0, 0.0, 0.0],
                major: 2.0,
                minor: 2.0,
                start: -PI / 2.0,
                end: 0.0,
                closed: false,
            },
            line([width, 2.0], [width, 40.0]),
            line([width, 40.0], [0.0, 40.0]),
            line([0.0, 40.0], [0.0, 0.0]),
        ]
    };
    *d.kernel().curves.borrow_mut() = rounded(60.0);
    command(
        &mut d,
        json!({"cmd": "sketch.project", "sketch": "F3", "source": face, "linked": true}),
    );
    // Followed at every evaluation: the arc keeps three distinct points,
    // and the sketch evaluates (the application draws it by its frame).
    *d.kernel().curves.borrow_mut() = rounded(80.0);
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    assert_eq!(changed["error"], Value::Null);
    assert!(sketch(&d, "F3")["frame"].is_object());
    assert_areas(&d, "F3", &[80.0 * 40.0 - 4.0 + PI]);
}

/// The position of a point in the sketch query.
fn point_at(s: &Value, id: &str) -> [f64; 2] {
    let e = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no {id}"));
    [e["at"][0].as_f64().unwrap(), e["at"][1].as_f64().unwrap()]
}

/// The id of the point at a position in the sketch query.
fn point_id(s: &Value, at: [f64; 2]) -> String {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["type"] == "point"
                && close(e["at"][0].as_f64().unwrap(), at[0])
                && close(e["at"][1].as_f64().unwrap(), at[1])
        })
        .unwrap_or_else(|| panic!("no point at {at:?}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn measured(s: &Value, id: &str) -> f64 {
    s["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == id)
        .unwrap()["measured"]
        .as_f64()
        .unwrap()
}

#[test]
fn a_linked_projection_shows_its_followed_geometry_and_drags_its_constraints() {
    // As the application works (mitcad#40): a block, a sketch on its top
    // face with the face projected (linked), a line of the user's from a
    // projected corner and dimensions on both; then the block's sketch
    // changes. The sketch query, which the application draws and edits
    // from, shows the projection where its source is now.
    let mut d = doc();
    command(&mut d, json!({"cmd": "sketch.create"}));
    command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 60, "height": 40}),
    );
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
               "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}}),
    );
    let face = "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";
    command(
        &mut d,
        json!({"cmd": "sketch.create", "plane": {"face": face}}),
    );
    let square = |size: f64| -> Vec<Curve3> {
        let c = [[0.0, 0.0], [size, 0.0], [size, 40.0], [0.0, 40.0]];
        (0..4)
            .map(|i| Curve3::Line {
                start: [c[i][0], c[i][1], 20.0],
                end: [c[(i + 1) % 4][0], c[(i + 1) % 4][1], 20.0],
            })
            .collect()
    };
    *d.kernel().curves.borrow_mut() = square(60.0);
    // The command as the Project dialog builds it.
    command(
        &mut d,
        json!({"cmd": "sketch.project", "sketch": "F3", "source": face, "body": "F2.b0",
               "linked": true}),
    );
    let s = sketch(&d, "F3");
    let corner = point_id(&s, [60.0, 0.0]);
    let bottom = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["type"] == "line"
                && e["geometry"]["start"] == json!([0.0, 0.0])
                && e["geometry"]["end"] == json!([60.0, 0.0])
        })
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // The user's line on from the corner, horizontal and 15 long.
    let line = command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F3", "start": corner, "end": [75, 0]}),
    );
    let user = line["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e.as_str().unwrap().starts_with('c'))
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let end = point_id(&sketch(&d, "F3"), [75.0, 0.0]);
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F3",
               "constraint": {"type": "horizontal", "line": user}}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F3",
               "dimension": {"type": "length", "line": user}, "value": 15}),
    );
    // A driven dimension on the projection measures it.
    let driven = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F3",
               "dimension": {"type": "length", "line": bottom}, "driven": true}),
    );
    let k = driven["dimensions"][0].as_str().unwrap().to_owned();
    let s = sketch(&d, "F3");
    assert_eq!(point_at(&s, &end), [75.0, 0.0]);
    assert!(close(measured(&s, &k), 60.0));

    // The block's width changes (a dimension of its sketch): the
    // projection, the line hanging on it and the driven dimension follow.
    *d.kernel().curves.borrow_mut() = square(80.0);
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    assert_eq!(changed["error"], Value::Null);
    let check = |d: &Document<MockKernel>, what: &str| {
        let s = sketch(d, "F3");
        assert_eq!(s["error"], Value::Null, "{what}");
        let at = point_at(&s, &corner);
        assert!(close(at[0], 80.0) && close(at[1], 0.0), "{what}: {at:?}");
        let at = point_at(&s, &end);
        assert!(close(at[0], 95.0) && close(at[1], 0.0), "{what}: {at:?}");
        assert!(close(measured(&s, &k), 80.0), "{what}");
        let line = s["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == bottom)
            .unwrap();
        assert_eq!(line["geometry"]["end"], json!([80.0, 0.0]), "{what}");
        assert_areas(d, "F3", &[3200.0]);
    };
    check(&d, "after the change");
    // An edit of the sketch starts from the followed geometry.
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F3", "start": [0, -20], "end": [10, -30]}),
    );
    check(&d, "after an edit");
    // And it is what the file keeps (the extrude's distance changes too).
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d3", "value": 30}),
    );
    check(&d, "after another change");
}

#[test]
fn sketches_on_construction_planes_follow_them() {
    let mut d = doc();
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "construction_plane",
               "definition": {"type": "offset", "plane": "xz", "distance": 10}}}),
    );
    command(&mut d, json!({"cmd": "sketch.create", "plane": "F1"}));
    command(
        &mut d,
        json!({"cmd": "sketch.circle", "sketch": "F2", "mode": "center", "center": [0, 0], "radius": 5}),
    );
    let s = sketch(&d, "F2");
    assert_eq!(s["plane"], "F1");
    assert_eq!(s["frame"]["origin"], json!([0.0, 10.0, 0.0]));
    assert_eq!(s["frame"]["y_axis"], json!([0.0, 0.0, -1.0]));
    // The plane moves; the sketch follows.
    command(
        &mut d,
        json!({"cmd": "edit_feature", "uid": "F1", "def": {"type": "construction_plane",
               "definition": {"type": "offset", "plane": "xz", "distance": 25}}}),
    );
    assert_eq!(sketch(&d, "F2")["frame"]["origin"], json!([0.0, 25.0, 0.0]));
    // Only a construction plane before the sketch.
    let error = rejected(&mut d, json!({"cmd": "sketch.create", "plane": "F2"}));
    assert!(error.contains("not a construction plane"), "{error}");
    let error = rejected(&mut d, json!({"cmd": "sketch.create", "plane": "F99"}));
    assert!(error.contains("F99"), "{error}");
}

#[test]
fn a_tangent_distance_is_the_gap_between_a_circle_and_a_line() {
    let mut d = with_sketch();
    // A circle of 24 at the origin (fixed) and a free line below it.
    let circle = command(
        &mut d,
        json!({"cmd": "sketch.add_circle", "sketch": "F1", "center": [0, 0], "diameter": 24}),
    );
    let circle = circle["curves"][0].as_str().unwrap().to_owned();
    let line = command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [-30, -30], "end": [30, -30]}),
    );
    let line = line["entities"][2].as_str().unwrap().to_owned();
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "horizontal", "line": line}}),
    );
    let line_y = |s: &Value| {
        let l = s["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == line.as_str())
            .unwrap();
        l["geometry"]["start"][1].as_f64().unwrap()
    };
    assert!(close(line_y(&sketch(&d, "F1")), -30.0));
    let dim = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "tangent_distance", "circle": circle, "line": line},
               "value": 25}),
    );
    // The gap to the nearest point of the circle: 12 + 25 below the centre.
    let s = sketch(&d, "F1");
    assert!(close(line_y(&s), -37.0), "{s}");
    assert!(close(
        measured(&s, dim["dimensions"][0].as_str().unwrap()),
        25.0
    ));
    // A driven one measures the gap.
    let driven = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "tangent_distance", "circle": circle, "line": line},
               "driven": true}),
    );
    let s = sketch(&d, "F1");
    assert!(close(
        measured(&s, driven["dimensions"][0].as_str().unwrap()),
        25.0
    ));
    // Two lines are no circle.
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "tangent_distance", "circle": line, "line": line},
               "value": 3}),
    );
    assert!(!error.is_empty());
}

/// A trapezoid's lower left corner (mitcad#296): the fixed bottom line c1
/// from (0, 0) to (40, 0) and the side c4 from a free point at (10, 20)
/// down to the corner, 22.36 long. Their directions make 116.57 degrees,
/// the inner corner 63.43. Returns the document and the side's free point.
fn trapezoid_corner() -> (Document<MockKernel>, String) {
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [40, 0]}),
    );
    let side = command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [10, 20], "end": "p2"}),
    );
    assert_eq!(side["entities"], json!(["p5", "c4"]), "{side}");
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": ["c1"]}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "length", "line": "c4"}, "value": 500f64.sqrt()}),
    );
    (d, "p5".to_owned())
}

fn angle(reverse: Option<[bool; 2]>) -> Value {
    let mut dimension = json!({"type": "angle", "a": "c1", "b": "c4"});
    if let Some([a, b]) = reverse {
        dimension["reverse_a"] = json!(a);
        dimension["reverse_b"] = json!(b);
    }
    dimension
}

#[test]
fn an_angle_dimension_measures_on_its_side() {
    let (mut d, _) = trapezoid_corner();
    let inner = (2.0f64).atan();
    let mut ids = Vec::new();
    for reverse in [
        None,
        Some([false, false]),
        Some([false, true]),
        Some([true, false]),
        Some([true, true]),
    ] {
        let added = command(
            &mut d,
            json!({"cmd": "sketch.add_dimension", "sketch": "F1",
                   "dimension": angle(reverse), "driven": true}),
        );
        ids.push(added["dimensions"][0].as_str().unwrap().to_owned());
    }
    let s = sketch(&d, "F1");
    // Without a side (as stored before): between the directions, as before.
    let expected = [PI - inner, PI - inner, inner, inner, PI - inner];
    for (id, expected) in ids.iter().zip(expected) {
        assert!(close(measured(&s, id), expected), "{id}: {s}");
    }
    // The query gives the side as it was given.
    let dimension = |id: &str| {
        s["dimensions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == id)
            .unwrap()
            .clone()
    };
    let none = dimension(&ids[0]);
    assert!(
        none.get("reverse_a").is_none() && none.get("reverse_b").is_none(),
        "{none}"
    );
    let given = dimension(&ids[2]);
    assert_eq!(
        (&given["reverse_a"], &given["reverse_b"]),
        (&json!(false), &json!(true))
    );
}

#[test]
fn an_angle_dimension_keeps_its_side_when_solved_and_edited() {
    let length = 500f64.sqrt();
    let polar = |degrees: f64| {
        let r = degrees.to_radians();
        [length * r.cos(), length * r.sin()]
    };
    let near = |p: [f64; 2], q: [f64; 2]| {
        assert!(close(p[0], q[0]) && close(p[1], q[1]), "{p:?} != {q:?}");
    };
    // The inner corner at 100 degrees: the side opens beyond the vertical,
    // although 100 is closer to the directions' 116.57.
    let (mut d, free) = trapezoid_corner();
    let added = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": angle(Some([false, true])), "value": "100 deg"}),
    );
    near(point_at(&sketch(&d, "F1"), &free), polar(100.0));
    // A new value keeps the side, also one closer to the other angle.
    let name = added["parameters"][0].as_str().unwrap().to_owned();
    for degrees in [30.0, 150.0, 63.0] {
        command(
            &mut d,
            json!({"cmd": "set_parameter", "name": name, "value": format!("{degrees} deg")}),
        );
        near(point_at(&sketch(&d, "F1"), &free), polar(degrees));
    }
    // The other side of the side line: from the bottom line's extension.
    let (mut d, free) = trapezoid_corner();
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": angle(Some([true, true])), "value": "100 deg"}),
    );
    near(point_at(&sketch(&d, "F1"), &free), polar(80.0));
    // Without a side, as before: the angle closer to the value where the
    // lines are, here the directions' (the side at 80 degrees).
    let (mut d, free) = trapezoid_corner();
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": angle(None), "value": "100 deg"}),
    );
    near(point_at(&sketch(&d, "F1"), &free), polar(80.0));
}

// --- A partly implied constraint replaces the one it makes redundant (mitcad#287)

/// A projected line along the x axis from (0, 0) to (100, 0) (p1, p2 and
/// c3, fixed reference geometry) and a rectangle from (0, 0) to (100, 30)
/// standing on it, its left corner coincident with the line's left end.
/// With the rectangle's bottom line, its Horizontal and its two bottom
/// corners.
fn rectangle_on_projected_line() -> (Document<MockKernel>, String, String, String, String) {
    let mut d = doc();
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy", "entities": [
            {"id": "p1", "type": "point", "at": [0.0, 0.0], "fixed": true, "reference": true},
            {"id": "p2", "type": "point", "at": [100.0, 0.0], "fixed": true, "reference": true},
            {"id": "c3", "type": "line", "start": "p1", "end": "p2", "fixed": true,
             "reference": true}]}}),
    );
    // The projected line's own fixed ends and fixed curve: what the solver
    // reports as redundant before anything is drawn.
    let base = sketch(&d, "F1")["redundant"].clone();
    let r = command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point",
               "a": [0, 0], "b": [100, 30]}),
    );
    let bottom = made(&r)[0].clone();
    let horizontal = r["constraints"][0].as_str().unwrap().to_owned();
    let s = sketch(&d, "F1");
    let left = point_id_of(&s, [0.0, 0.0], false);
    let right = point_id_of(&s, [100.0, 0.0], false);
    assert_eq!(constraint(&s, &horizontal)["type"], "horizontal", "{s}");
    assert_eq!(constraint(&s, &horizontal)["line"], bottom.as_str(), "{s}");
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "coincident", "point": left, "entity": "p1"}}),
    );
    let s = sketch(&d, "F1");
    assert_eq!(s["redundant"], base, "{s}");
    (d, bottom, horizontal, left, right)
}

/// The id of the point at a position: a projected one or one of the
/// sketch's own.
fn point_id_of(s: &Value, at: [f64; 2], reference: bool) -> String {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["type"] == "point"
                && e["reference"].as_bool().unwrap_or(false) == reference
                && close(e["at"][0].as_f64().unwrap(), at[0])
                && close(e["at"][1].as_f64().unwrap(), at[1])
        })
        .unwrap_or_else(|| panic!("no point at {at:?} in {s}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn constraint(s: &Value, id: &str) -> Value {
    s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["id"] == id)
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn a_partly_implied_coincident_replaces_the_constraint_it_makes_redundant() {
    let (mut d, _, horizontal, _, right) = rectangle_on_projected_line();
    let before = sketch(&d, "F1");
    let dof = before["dof"].as_u64().unwrap();
    // The right corner onto the line's right end: its height already
    // follows from the left corner and the Horizontal, its x does not.
    let result = command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "coincident", "point": right, "entity": "p2"}}),
    );
    let k = result["constraints"][0].as_str().unwrap().to_owned();
    assert_eq!(
        result["implied_removed"],
        json!([{"constraint": horizontal, "type": "horizontal"}]),
        "{result}"
    );
    assert_eq!(result["removed"], json!([horizontal]), "{result}");
    let s = sketch(&d, "F1");
    assert_eq!(constraint(&s, &horizontal), Value::Null, "{s}");
    assert_eq!(constraint(&s, &k)["type"], "coincident", "{s}");
    assert_eq!(
        s["redundant"], before["redundant"],
        "nothing more redundant: {s}"
    );
    assert_eq!(s["conflicts"], json!([]), "{s}");
    assert_eq!(s["dof"].as_u64().unwrap(), dof - 1, "{s}");
    assert!(s["error"].is_null(), "{s}");
    let at = point_at(&s, &right);
    assert!(close(at[0], 100.0) && close(at[1], 0.0), "{at:?}");
    // One undo takes both back.
    command(&mut d, json!({"cmd": "undo"}));
    let s = sketch(&d, "F1");
    assert_eq!(constraint(&s, &horizontal)["type"], "horizontal", "{s}");
    assert_eq!(constraint(&s, &k), Value::Null, "{s}");
    assert_eq!(s["dof"], before["dof"], "{s}");
    assert_eq!(s["constraints"], before["constraints"], "{s}");
}

#[test]
fn a_coincident_against_a_dimension_is_still_refused() {
    // A width that contradicts the line's length: refused, nothing removed.
    let (mut d, bottom, horizontal, _, right) = rectangle_on_projected_line();
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "length", "line": bottom}, "value": 80}),
    );
    let before = sketch(&d, "F1");
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "coincident", "point": right, "entity": "p2"}}),
    );
    assert!(
        error.contains("over-constrain") || error.contains("cannot be solved"),
        "{error}"
    );
    let s = sketch(&d, "F1");
    assert_eq!(s["constraints"], before["constraints"], "{s}");
    assert_eq!(constraint(&s, &horizontal)["type"], "horizontal", "{s}");

    // A width that agrees with it: the coincident then follows from the
    // dimension too, which is never removed, so it is refused as well and
    // the Horizontal stays.
    let (mut d, bottom, horizontal, _, right) = rectangle_on_projected_line();
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "length", "line": bottom}, "value": 100}),
    );
    let before = sketch(&d, "F1");
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "coincident", "point": right, "entity": "p2"}}),
    );
    assert!(error.contains("would over-constrain"), "{error}");
    let s = sketch(&d, "F1");
    assert_eq!(s["constraints"], before["constraints"], "{s}");
    assert_eq!(constraint(&s, &horizontal)["type"], "horizontal", "{s}");
}

#[test]
fn a_constraint_the_others_imply_in_full_is_still_refused() {
    // Parallel bottom and top lines follow from their Horizontals and take
    // nothing away: no Horizontal is traded for it.
    let (mut d, bottom, _, _, _) = rectangle_on_projected_line();
    let before = sketch(&d, "F1");
    let top = before["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["type"] == "line"
                && e["id"] != bottom.as_str()
                && before["constraints"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|k| k["type"] == "horizontal" && k["line"] == e["id"])
        })
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "parallel", "a": bottom, "b": top}}),
    );
    assert!(error.contains("would over-constrain"), "{error}");
    assert_eq!(
        sketch(&d, "F1")["constraints"],
        before["constraints"],
        "nothing removed"
    );
}
