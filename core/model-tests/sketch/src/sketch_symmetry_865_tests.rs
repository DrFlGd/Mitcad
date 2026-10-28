// SPDX-License-Identifier: MIT
//! Symmetry of circles and arcs whose radii other constraints fix already
//! (mitcad#865): the constraint takes what is not fixed yet (the positions)
//! instead of over-constraining the sketch, and a Symmetry the others imply
//! in full is refused with its reason.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_api::DocumentApi;

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

fn entity(s: &Value, id: &str) -> Value {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {s}"))
        .clone()
}

fn at(s: &Value, id: &str) -> [f64; 2] {
    let e = entity(s, id);
    [e["at"][0].as_f64().unwrap(), e["at"][1].as_f64().unwrap()]
}

fn assert_at(s: &Value, id: &str, expected: [f64; 2]) {
    let p = at(s, id);
    assert!(
        close(p[0], expected[0]) && close(p[1], expected[1]),
        "{id} at {p:?}, not {expected:?}: {s}"
    );
}

/// The id of the sketch's point at a position.
fn point_at(s: &Value, xy: [f64; 2]) -> String {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["type"] == "point"
                && close(e["at"][0].as_f64().unwrap(), xy[0])
                && close(e["at"][1].as_f64().unwrap(), xy[1])
        })
        .unwrap_or_else(|| panic!("no point at {xy:?} in {s}"))["id"]
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

/// The curve a drawing command made.
fn curve_made(result: &Value) -> String {
    result["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .find(|id| id.starts_with('c'))
        .unwrap_or_else(|| panic!("no curve in {result}"))
        .to_owned()
}

fn constrain(d: &mut Document<MockKernel>, constraint: Value) -> String {
    let added = command(
        d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1", "constraint": constraint}),
    );
    added["constraints"][0]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn dimension(d: &mut Document<MockKernel>, dimension: Value, value: f64) {
    command(
        d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": dimension, "value": value}),
    );
}

/// A circle with a free centre: the circle and its centre point.
fn circle(d: &mut Document<MockKernel>, center: [f64; 2], radius: f64) -> (String, String) {
    let r = command(
        d,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center",
               "center": center, "radius": radius}),
    );
    let c = curve_made(&r);
    let centre = entity(&sketch(d), &c)["center"]
        .as_str()
        .unwrap()
        .to_owned();
    (c, centre)
}

/// The dimensions' ids with what they measure.
fn dimensions(s: &Value) -> Vec<(String, f64)> {
    s["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["id"].as_str().unwrap().to_owned(),
                d["measured"].as_f64().unwrap_or(f64::NAN),
            )
        })
        .collect()
}

/// The reported sketch: the circles (each the circle and its centre), the
/// sketch's x axis and the bottom left centre's coincident with the
/// diagonal.
struct Reported {
    d: Document<MockKernel>,
    top_left: (String, String),
    bottom_left: (String, String),
    top_right: (String, String),
    bottom_right: (String, String),
    x: String,
    bottom_left_on_diagonal: String,
}

/// The reported sketch, made synthetically: a dimensioned 100 x 100 square
/// about the origin and a construction diagonal from its bottom left to its
/// top right corner; four circles, each with its own Ø10 dimension; the top
/// left one 15 from the left and the top edge; the top right one level with
/// it and on the diagonal; the bottom left one on the diagonal, the bottom
/// right one below the top right one and level with the bottom left one.
/// One degree of freedom is left: the bottom left circle along the
/// diagonal. Also the sketch's x axis.
fn reported_sketch() -> Reported {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    let x = command(
        &mut d,
        json!({"cmd": "sketch.axis", "sketch": "F1", "axis": "x"}),
    )["entity"]
        .as_str()
        .unwrap()
        .to_owned();
    // Bottom, right, top and left, counter-clockwise from the fixed corner.
    let square = command(
        &mut d,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-50, -50],
               "width": 100, "height": 100}),
    );
    let lines: Vec<String> = square["curves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(lines.len(), 4, "{square}");
    let (top, left) = (lines[2].clone(), lines[3].clone());
    let s = sketch(&d);
    let (low, high) = (point_at(&s, [-50.0, -50.0]), point_at(&s, [50.0, 50.0]));
    let diagonal = curve_made(&command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": low, "end": high,
               "construction": true}),
    ));
    let top_left = circle(&mut d, [-35.0, 35.0], 5.0);
    let top_right = circle(&mut d, [35.0, 35.0], 5.0);
    let bottom_left = circle(&mut d, [-30.0, -30.0], 5.0);
    let bottom_right = circle(&mut d, [35.0, -30.0], 5.0);
    for (c, _) in [&top_left, &top_right, &bottom_left, &bottom_right] {
        dimension(&mut d, json!({"type": "diameter", "curve": c}), 10.0);
    }
    for line in [&left, &top] {
        dimension(
            &mut d,
            json!({"type": "point_line_distance", "point": top_left.1, "line": line}),
            15.0,
        );
    }
    constrain(
        &mut d,
        json!({"type": "coincident", "point": top_right.1, "entity": diagonal}),
    );
    constrain(
        &mut d,
        json!({"type": "horizontal_points", "a": top_left.1, "b": top_right.1}),
    );
    let bottom_left_on_diagonal = constrain(
        &mut d,
        json!({"type": "coincident", "point": bottom_left.1, "entity": diagonal}),
    );
    constrain(
        &mut d,
        json!({"type": "vertical_points", "a": top_right.1, "b": bottom_right.1}),
    );
    constrain(
        &mut d,
        json!({"type": "horizontal_points", "a": bottom_left.1, "b": bottom_right.1}),
    );
    let s = sketch(&d);
    assert_eq!(
        s["dof"], 1,
        "the bottom left circle along the diagonal: {s}"
    );
    assert_eq!(s["conflicts"], json!([]), "{s}");
    assert_at(&s, &top_left.1, [-35.0, 35.0]);
    assert_at(&s, &top_right.1, [35.0, 35.0]);
    Reported {
        d,
        top_left,
        bottom_left,
        top_right,
        bottom_right,
        x,
        bottom_left_on_diagonal,
    }
}

#[test]
fn symmetry_of_dimensioned_circles_takes_the_free_degree_of_freedom() {
    let Reported {
        mut d,
        top_left,
        bottom_left,
        x,
        bottom_left_on_diagonal,
        ..
    } = reported_sketch();
    let before = sketch(&d);
    // The two left circles about the x axis: both radii are fixed by their
    // dimensions, and the bottom left centre's x follows from the diagonal
    // once its y does.
    let result = command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1", "constraint":
               {"type": "symmetric", "a": top_left.0, "b": bottom_left.0, "axis": x}}),
    );
    let k = result["constraints"][0].as_str().unwrap().to_owned();
    let s = sketch(&d);
    assert_eq!(s["dof"], 0, "fully constrained: {s}");
    assert_eq!(
        s["redundant"], before["redundant"],
        "nothing more redundant: {s}"
    );
    assert_eq!(s["conflicts"], json!([]), "{s}");
    assert!(s["error"].is_null(), "{s}");
    // The positions only: the radii stay with their dimensions.
    let symmetric = constraint(&s, &k);
    assert_eq!(symmetric["type"], "symmetric", "{s}");
    assert_eq!(symmetric["radii"], false, "{s}");
    // The bottom left centre's coincident with the diagonal goes in its
    // favour, as the user's last intent; never a dimension.
    assert_eq!(
        result["implied_removed"],
        json!([{"constraint": bottom_left_on_diagonal, "type": "coincident"}]),
        "{result}"
    );
    let (kept, was) = (dimensions(&s), dimensions(&before));
    assert_eq!(
        kept.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        was.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        "the dimensions are kept: {s}"
    );
    for ((id, now), (_, then)) in kept.iter().zip(&was) {
        assert!(close(*now, *then), "{id}: {now} after, {then} before: {s}");
    }
    assert_at(&s, &top_left.1, [-35.0, 35.0]);
    assert_at(&s, &bottom_left.1, [-35.0, -35.0]);
    for (c, _) in [&top_left, &bottom_left] {
        assert!(close(entity(&s, c)["radius"].as_f64().unwrap(), 5.0), "{s}");
    }
    // One undo takes it back with the coincident.
    command(&mut d, json!({"cmd": "undo"}));
    let s = sketch(&d);
    assert_eq!(s["dof"], 1, "{s}");
    assert_eq!(s["constraints"], before["constraints"], "{s}");
}

#[test]
fn a_symmetry_the_others_imply_in_full_is_refused_with_its_reason() {
    let Reported {
        mut d,
        top_left,
        bottom_left,
        top_right,
        bottom_right,
        x,
        ..
    } = reported_sketch();
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1", "constraint":
               {"type": "symmetric", "a": top_left.0, "b": bottom_left.0, "axis": x}}),
    );
    let before = sketch(&d);
    assert_eq!(before["dof"], 0, "{before}");
    // The right circles: their radii, their x and their heights are all
    // fixed already.
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1", "constraint":
               {"type": "symmetric", "a": top_right.0, "b": bottom_right.0, "axis": x}}),
    );
    assert!(
        error.contains("would over-constrain the sketch"),
        "the reason: {error}"
    );
    let s = sketch(&d);
    assert_eq!(
        s["constraints"], before["constraints"],
        "nothing removed: {s}"
    );
    assert_eq!(s["dimensions"], before["dimensions"], "{s}");
}

#[test]
fn a_symmetry_of_circles_with_free_radii_makes_them_equal() {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    let x = command(
        &mut d,
        json!({"cmd": "sketch.axis", "sketch": "F1", "axis": "x"}),
    )["entity"]
        .as_str()
        .unwrap()
        .to_owned();
    let (a, ca) = circle(&mut d, [20.0, 10.0], 4.0);
    let (b, cb) = circle(&mut d, [22.0, -12.0], 6.0);
    let dof = sketch(&d)["dof"].as_u64().unwrap();
    let k = constrain(
        &mut d,
        json!({"type": "symmetric", "a": a, "b": b, "axis": x}),
    );
    let s = sketch(&d);
    // As before mitcad#865: the radii are made equal too ("radii" left out).
    let symmetric = constraint(&s, &k);
    assert_eq!(symmetric["type"], "symmetric", "{s}");
    assert!(symmetric.get("radii").is_none(), "{s}");
    assert_eq!(s["dof"].as_u64().unwrap(), dof - 3, "{s}");
    let (pa, pb) = (at(&s, &ca), at(&s, &cb));
    assert!(close(pa[0], pb[0]) && close(pa[1], -pb[1]), "{s}");
    let (ra, rb) = (
        entity(&s, &a)["radius"].as_f64().unwrap(),
        entity(&s, &b)["radius"].as_f64().unwrap(),
    );
    assert!(close(ra, rb), "{ra} and {rb}: {s}");
}

#[test]
fn a_symmetry_of_dimensioned_arcs_mirrors_their_centres_and_ends() {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    let y = command(
        &mut d,
        json!({"cmd": "sketch.axis", "sketch": "F1", "axis": "y"}),
    )["entity"]
        .as_str()
        .unwrap()
        .to_owned();
    let h = 10.0 * std::f64::consts::FRAC_1_SQRT_2;
    // A quarter about (20, 5) from 0 to 90 degrees, and one about (-22, 6)
    // from 90 to 180 degrees, both of radius 10.
    let a = curve_made(&command(
        &mut d,
        json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "three_point",
               "start": [30, 5], "through": [20.0 + h, 5.0 + h], "end": [20, 15]}),
    ));
    let b = curve_made(&command(
        &mut d,
        json!({"cmd": "sketch.arc", "sketch": "F1", "mode": "three_point",
               "start": [-22, 16], "through": [-22.0 - h, 6.0 + h], "end": [-32, 6]}),
    ));
    for arc in [&a, &b] {
        dimension(&mut d, json!({"type": "radius", "curve": arc}), 10.0);
    }
    let dof = sketch(&d)["dof"].as_u64().unwrap();
    let k = constrain(
        &mut d,
        json!({"type": "symmetric", "a": a, "b": b, "axis": y}),
    );
    let s = sketch(&d);
    assert_eq!(constraint(&s, &k)["radii"], false, "{s}");
    // The centre and the two end directions of b follow from a.
    assert_eq!(s["dof"].as_u64().unwrap(), dof - 4, "{s}");
    assert_eq!(s["conflicts"], json!([]), "{s}");
    let ends = |arc: &str| {
        let e = entity(&s, arc);
        ["center", "start", "end"].map(|key| e[key].as_str().unwrap().to_owned())
    };
    let ([ac, as_, ae], [bc, bs, be]) = (ends(&a), ends(&b));
    let mirrored = |p: [f64; 2]| [-p[0], p[1]];
    let near = |p: [f64; 2], q: [f64; 2]| close(p[0], q[0]) && close(p[1], q[1]);
    assert!(near(at(&s, &bc), mirrored(at(&s, &ac))), "{s}");
    // Mirroring reverses the direction: b's start mirrors a's end.
    assert!(near(at(&s, &bs), mirrored(at(&s, &ae))), "{s}");
    assert!(near(at(&s, &be), mirrored(at(&s, &as_))), "{s}");
}
