// SPDX-License-Identifier: MIT
//! A selection dragged together (mitcad#837): `sketch.drag` with
//! `entities` pulls every point of them by one offset in one solve.

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

fn entity(s: &Value, id: &str) -> Value {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {s}"))
        .clone()
}

/// Where a point is.
fn at(s: &Value, point: &str) -> [f64; 2] {
    let p = &entity(s, point)["at"];
    [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]
}

/// A line's start and end point ids.
fn ends(s: &Value, line: &str) -> [String; 2] {
    let e = entity(s, line);
    [
        e["start"].as_str().unwrap().to_owned(),
        e["end"].as_str().unwrap().to_owned(),
    ]
}

/// The baseline tolerance of geometric comparisons.
fn assert_near(a: [f64; 2], b: [f64; 2], what: &str) {
    assert!(
        (a[0] - b[0]).abs() <= 1e-6 && (a[1] - b[1]).abs() <= 1e-6,
        "{what}: {a:?}, expected {b:?}"
    );
}

fn plus(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

/// A line's direction from its start to its end.
fn direction(s: &Value, line: &str) -> [f64; 2] {
    let [a, b] = ends(s, line);
    let (a, b) = (at(s, &a), at(s, &b));
    [b[0] - a[0], b[1] - a[1]]
}

fn length(v: [f64; 2]) -> f64 {
    v[0].hypot(v[1])
}

fn angle_between(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1])
}

/// A new document with an empty Sketch1 (F1).
fn with_sketch() -> Document<MockKernel> {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    d
}

/// A line; returns its curve id.
fn line(d: &mut Document<MockKernel>, start: Value, end: Value) -> String {
    let made = command(
        d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": start, "end": end}),
    );
    made["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .find(|id| id.starts_with('c'))
        .unwrap_or_else(|| panic!("no curve in {made}"))
        .to_owned()
}

fn drag_together(d: &mut Document<MockKernel>, entities: &[&str], by: [f64; 2]) -> Value {
    command(
        d,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entities": entities, "by": by}),
    )
}

#[test]
fn two_lines_of_a_closed_profile_drag_together_and_the_third_follows() {
    // A closed triangle of three lines with shared corners.
    let mut d = with_sketch();
    let c1 = line(&mut d, json!([0, 0]), json!([40, 0]));
    let s = sketch(&d);
    let [p_a, p_b] = ends(&s, &c1);
    let c2 = line(&mut d, json!(p_b), json!([20, 30]));
    let p_c = ends(&sketch(&d), &c2)[1].clone();
    let c3 = line(&mut d, json!(p_c), json!(p_a));
    // A dimension on a selected line.
    let dim = command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "length", "line": c1}, "value": 40}),
    );
    let k = dim["dimensions"][0].as_str().unwrap().to_owned();

    let before = sketch(&d);
    let corners: Vec<[f64; 2]> = [&p_a, &p_b, &p_c].iter().map(|p| at(&before, p)).collect();
    let (d1, d2, d3) = (
        direction(&before, &c1),
        direction(&before, &c2),
        direction(&before, &c3),
    );

    let by = [5.0, 3.0];
    drag_together(&mut d, &[&c1, &c2], by);

    let after = sketch(&d);
    for (p, was) in [&p_a, &p_b, &p_c].iter().zip(&corners) {
        assert_near(at(&after, p), plus(*was, by), p);
    }
    // The selected lines keep their lengths and the angle between them.
    let (e1, e2) = (direction(&after, &c1), direction(&after, &c2));
    assert!((length(e1) - length(d1)).abs() <= 1e-6, "{c1}: {after}");
    assert!((length(e2) - length(d2)).abs() <= 1e-6, "{c2}: {after}");
    assert!(
        (angle_between(e1, e2) - angle_between(d1, d2)).abs() <= 1e-6,
        "{after}"
    );
    // The third line still joins them at their shared corners.
    assert_eq!(ends(&after, &c3), [p_c.clone(), p_a.clone()]);
    assert!((length(direction(&after, &c3)) - length(d3)).abs() <= 1e-6);
    // The dimension between selected entities keeps its value.
    let kept = after["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == k.as_str())
        .unwrap_or_else(|| panic!("no {k} in {after}"))
        .clone();
    assert!(
        (kept["measured"].as_f64().unwrap() - 40.0).abs() <= 1e-6,
        "{kept}"
    );

    // One undo step takes the whole drag back.
    command(&mut d, json!({"cmd": "undo"}));
    let undone = sketch(&d);
    for (p, was) in [&p_a, &p_b, &p_c].iter().zip(&corners) {
        assert_near(at(&undone, p), *was, p);
    }
}

#[test]
fn a_selected_fixed_point_holds_the_set_as_a_single_drag_does() {
    // A horizontal line 10 long whose start is fixed: neither a single
    // drag of the line nor a drag of the line with its fixed start moves
    // it.
    let build = || {
        let mut d = with_sketch();
        let c = line(&mut d, json!([0, 0]), json!([10, 0]));
        let [start, end] = ends(&sketch(&d), &c);
        command(
            &mut d,
            json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": [start]}),
        );
        command(
            &mut d,
            json!({"cmd": "sketch.add_constraint", "sketch": "F1",
                   "constraint": {"type": "horizontal", "line": c}}),
        );
        command(
            &mut d,
            json!({"cmd": "sketch.add_dimension", "sketch": "F1",
                   "dimension": {"type": "length", "line": c}, "value": 10}),
        );
        (d, c, start, end)
    };
    let by = [5.0, 3.0];

    let (mut single, c, start, end) = build();
    let single_ok = single
        .command(&json!({"cmd": "sketch.drag", "sketch": "F1", "entity": c, "by": by}).to_string())
        .is_ok();

    let (mut together, c, start2, end2) = build();
    assert_eq!((&start, &end), (&start2, &end2));
    let together_ok = together
        .command(
            &json!({"cmd": "sketch.drag", "sketch": "F1", "entities": [c, start], "by": by})
                .to_string(),
        )
        .is_ok();

    // Refused or held, the same as the single drag.
    assert_eq!(single_ok, together_ok);
    for d in [&single, &together] {
        let s = sketch(d);
        assert_near(at(&s, &start), [0.0, 0.0], &start);
        assert_near(at(&s, &end), [10.0, 0.0], &end);
    }
}

#[test]
fn shared_endpoints_of_a_selection_move_once() {
    // A closed rectangle; two adjacent sides and their three corners are
    // selected: each corner moves by the offset once, the fourth corner
    // stays, and the other two sides follow their shared corners.
    let mut d = with_sketch();
    let c1 = line(&mut d, json!([0, 0]), json!([20, 0]));
    let [p_a, p_b] = ends(&sketch(&d), &c1);
    let c2 = line(&mut d, json!(p_b), json!([20, 10]));
    let p_c = ends(&sketch(&d), &c2)[1].clone();
    let c3 = line(&mut d, json!(p_c), json!([0, 10]));
    let p_d = ends(&sketch(&d), &c3)[1].clone();
    let c4 = line(&mut d, json!(p_d), json!(p_a));

    let before = sketch(&d);
    let by = [5.0, 3.0];
    drag_together(&mut d, &[&c1, &c2, &p_a, &p_b, &p_c], by);

    let after = sketch(&d);
    for p in [&p_a, &p_b, &p_c] {
        assert_near(at(&after, p), plus(at(&before, p), by), p);
    }
    assert_near(at(&after, &p_d), at(&before, &p_d), &p_d);
    assert_eq!(ends(&after, &c3), [p_c.clone(), p_d.clone()]);
    assert_eq!(ends(&after, &c4), [p_d.clone(), p_a.clone()]);
}

#[test]
fn a_selection_drag_takes_an_offset_only() {
    let mut d = with_sketch();
    let c = line(&mut d, json!([0, 0]), json!([10, 0]));
    let [start, _] = ends(&sketch(&d), &c);
    let message = rejected(
        &mut d,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entities": [c], "to": [1, 1]}),
    );
    assert!(message.contains("by"), "{message}");
    let message = rejected(
        &mut d,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entity": start, "entities": [c],
               "by": [1, 1]}),
    );
    assert!(message.contains("not both"), "{message}");
    let message = rejected(
        &mut d,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entities": [], "by": [1, 1]}),
    );
    assert!(message.contains("empty"), "{message}");
    rejected(
        &mut d,
        json!({"cmd": "sketch.drag", "sketch": "F1", "by": [1, 1]}),
    );
}
