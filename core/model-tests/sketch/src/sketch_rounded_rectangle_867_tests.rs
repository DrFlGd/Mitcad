// SPDX-License-Identifier: MIT
//! Rounded rectangles and Sketch Fillet on several corners at once
//! (mitcad#867): `sketch.rectangle` with `radius`, and `sketch.fillet`
//! with `curves` or `points`.

use std::f64::consts::PI;

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_api::DocumentApi;

fn with_sketch() -> Document<MockKernel> {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    d
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

fn sketch(doc: &Document<MockKernel>) -> Value {
    serde_json::from_str(
        &doc.query(&json!({"query": "sketch", "uid": "F1"}).to_string())
            .unwrap(),
    )
    .unwrap()
}

fn made(result: &Value) -> Vec<String> {
    result["made"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap().to_owned()).collect())
        .unwrap_or_default()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(1.0)
}

/// The radii of the sketch's arcs.
fn arc_radii(s: &Value) -> Vec<f64> {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "arc")
        .map(|e| e["geometry"]["radius"].as_f64().unwrap())
        .collect()
}

fn assert_radii(s: &Value, r: f64) {
    let radii = arc_radii(s);
    assert_eq!(radii.len(), 4, "{radii:?}");
    assert!(radii.iter().all(|x| close(*x, r)), "{radii:?} != {r}");
}

/// How many constraints of a type the sketch has.
fn count(s: &Value, kind: &str) -> usize {
    s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["type"] == kind)
        .count()
}

/// The area of the sketch's one region.
fn area(s: &Value) -> f64 {
    let regions = s["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1, "{regions:?}");
    regions[0]["area"].as_f64().unwrap()
}

/// A 40 x 20 rectangle with corners of radius `r`.
fn rounded(r: f64) -> f64 {
    40.0 * 20.0 - r * r * (4.0 - PI)
}

/// The id of the point at a position.
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
        .unwrap_or_else(|| panic!("no point at {at:?} in {s}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Width and height between the kept corners, and the first corner fixed.
fn dimension_and_place(d: &mut Document<MockKernel>) {
    let s = sketch(d);
    let corners = [[0.0, 0.0], [40.0, 0.0], [40.0, 20.0]].map(|at| point_id(&s, at));
    for (a, b, value) in [(0, 1, 40), (1, 2, 20)] {
        command(
            d,
            json!({"cmd": "sketch.add_dimension", "sketch": "F1",
                   "dimension": {"type": "distance", "a": corners[a], "b": corners[b]},
                   "value": value}),
        );
    }
    command(
        d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": [corners[0]]}),
    );
}

/// Four arcs of r, Equal on three of them to the first, the sketch fully
/// constrained, and the one radius dimension moving all four.
fn check_rounded(d: &mut Document<MockKernel>, radius: &str) {
    let s = sketch(d);
    assert_radii(&s, 5.0);
    assert_eq!(count(&s, "tangent"), 8, "{}", s["constraints"]);
    assert_eq!(count(&s, "equal"), 3, "{}", s["constraints"]);
    let radius_dimensions = s["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|k| k["type"] == "radius")
        .count();
    assert_eq!(radius_dimensions, 1, "{}", s["dimensions"]);
    assert_eq!(s["dof"], 0, "{}", s["fully_constrained"]);
    assert!(close(area(&s), rounded(5.0)), "{}", area(&s));
    command(
        d,
        json!({"cmd": "set_parameter", "name": radius, "value": 8}),
    );
    let s = sketch(d);
    assert_radii(&s, 8.0);
    assert!(close(area(&s), rounded(8.0)), "{}", area(&s));
    assert_eq!(s["dof"], 0);
    // Width and height still measure the whole rectangle: its far corner
    // stays.
    point_id(&s, [40.0, 20.0]);
}

#[test]
fn rounded_rectangle_fully_constrained_by_width_height_radius_and_position() {
    let mut d = with_sketch();
    let result = command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point",
               "a": [0, 0], "b": [40, 20], "radius": 5}),
    );
    // The four lines, then the four arcs; one radius parameter.
    assert_eq!(made(&result).len(), 8, "{result}");
    let parameters = result["parameters"].as_array().unwrap();
    assert_eq!(parameters.len(), 1, "{result}");
    let radius = parameters[0].as_str().unwrap().to_owned();
    let s = sketch(&d);
    assert_eq!(count(&s, "horizontal"), 2);
    assert_eq!(count(&s, "vertical"), 2);
    // Only the width, the height and the position are left.
    assert_eq!(s["dof"], 4, "{}", s["fully_constrained"]);
    dimension_and_place(&mut d);
    check_rounded(&mut d, &radius);
}

#[test]
fn center_rounded_rectangle() {
    let mut d = with_sketch();
    let result = command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "center",
               "center": [20, 10], "corner": [40, 20], "radius": 5}),
    );
    assert_eq!(made(&result).len(), 8, "{result}");
    let s = sketch(&d);
    assert_radii(&s, 5.0);
    assert!(close(area(&s), rounded(5.0)), "{}", area(&s));
    // Sketch Fillet on a whole Center Rectangle, picked with its
    // construction diagonal and its corner points (as a window selection
    // gives them): the diagonal does not spoil the corners.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "center",
               "center": [20, 10], "corner": [40, 20]}),
    );
    let s = sketch(&d);
    let ids = |kind: &str| -> Vec<String> {
        s["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == kind)
            .map(|e| e["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let lines = ids("line");
    assert_eq!(lines.len(), 5, "four sides and the diagonal");
    let corners: Vec<String> = [[0.0, 0.0], [40.0, 0.0], [40.0, 20.0], [0.0, 20.0]]
        .iter()
        .map(|at| point_id(&s, *at))
        .collect();
    let fillet = command(
        &mut d,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "curves": lines, "points": corners,
               "radius": 5}),
    );
    assert_eq!(made(&fillet).len(), 4, "{fillet}");
    let s = sketch(&d);
    assert_radii(&s, 5.0);
    assert!(close(area(&s), rounded(5.0)), "{}", area(&s));
}

#[test]
fn sketch_fillet_on_a_whole_rectangle_gives_the_same() {
    // A plain rectangle with its width and height as length dimensions
    // (as the drawing tool adds typed values), placed, then all four
    // corners rounded at once.
    let mut d = with_sketch();
    let lines = made(&command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point",
               "a": [0, 0], "b": [40, 20]}),
    ));
    for (line, value) in [(&lines[0], 40), (&lines[1], 20)] {
        command(
            &mut d,
            json!({"cmd": "sketch.add_dimension", "sketch": "F1",
                   "dimension": {"type": "length", "line": line}, "value": value}),
        );
    }
    let s = sketch(&d);
    let origin = point_id(&s, [0.0, 0.0]);
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": [origin]}),
    );
    let fillet = command(
        &mut d,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "curves": lines, "radius": 5}),
    );
    assert_eq!(made(&fillet).len(), 4, "{fillet}");
    let parameters = fillet["parameters"].as_array().unwrap();
    assert_eq!(parameters.len(), 1, "{fillet}");
    let radius = parameters[0].as_str().unwrap().to_owned();
    // The length dimensions became distances between the kept corners.
    let s = sketch(&d);
    let lengths = s["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|k| k["type"] == "length")
        .count();
    assert_eq!(lengths, 0, "{}", s["dimensions"]);
    check_rounded(&mut d, &radius);
    // The first undo takes back the radius change of check_rounded: four
    // arcs of r 5 again. Then one undo step takes all four back.
    command(&mut d, json!({"cmd": "undo"}));
    assert_radii(&sketch(&d), 5.0);
    command(&mut d, json!({"cmd": "undo"}));
    let s = sketch(&d);
    assert!(arc_radii(&s).is_empty(), "{:?}", arc_radii(&s));
    assert!(close(area(&s), 800.0));
}

#[test]
fn sketch_fillet_at_corner_points() {
    // Two opposite corners of a rectangle by their points.
    let mut d = with_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0],
               "width": 40, "height": 20}),
    );
    let s = sketch(&d);
    let corners = [[0.0, 0.0], [40.0, 20.0]].map(|at| point_id(&s, at));
    let fillet = command(
        &mut d,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "points": corners, "radius": 5}),
    );
    assert_eq!(made(&fillet).len(), 2, "{fillet}");
    let s = sketch(&d);
    assert_eq!(arc_radii(&s).len(), 2);
    assert_eq!(count(&s, "equal"), 1);
    assert_eq!(s["dof"], 0, "{}", s["fully_constrained"]);
    assert!(close(area(&s), 800.0 - 2.0 * 25.0 * (1.0 - PI / 4.0)));
    // A point that is no corner of two lines is refused.
    let middle = command(
        &mut d,
        json!({"cmd": "sketch.add_point", "sketch": "F1", "at": [20, 10]}),
    );
    let lone = middle["entities"][0].as_str().unwrap().to_owned();
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "points": [lone], "radius": 2}),
    );
    assert!(error.contains("not the corner of two lines"), "{error}");
}

#[test]
fn too_large_a_radius_is_refused_with_a_reason() {
    let mut d = with_sketch();
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point",
               "a": [0, 0], "b": [40, 20], "radius": 10}),
    );
    assert!(
        error.contains("less than half the shorter side (10 mm)"),
        "{error}"
    );
    assert!(sketch(&d)["entities"].as_array().unwrap().is_empty());
    // Sketch Fillet on the whole rectangle names the line and the limit.
    let lines = made(&command(
        &mut d,
        json!({"cmd": "sketch.rectangle", "sketch": "F1", "mode": "two_point",
               "a": [0, 0], "b": [40, 20]}),
    ));
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "curves": lines, "radius": 12}),
    );
    assert!(error.contains("must be less than 10 mm"), "{error}");
    assert!(arc_radii(&sketch(&d)).is_empty());
    // "a" and "b" and "curves" together are refused.
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "a": lines[0], "b": lines[1],
               "curves": lines, "radius": 2}),
    );
    assert!(error.contains("\"curves\""), "{error}");
}
