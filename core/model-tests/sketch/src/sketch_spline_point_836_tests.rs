// SPDX-License-Identifier: MIT
//! Insert Point on a spline (mitcad#836, `sketch.insert_spline_point`): a
//! fit point between its neighbours, or a control point by knot insertion
//! that keeps the shape; open and closed splines; constraints kept. With
//! the mock kernel.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::sketch::geometry::Nurbs;
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
    let answer = doc
        .query(&json!({"query": "sketch", "uid": "F1"}).to_string())
        .unwrap();
    let s: Value = serde_json::from_str(&answer).unwrap();
    assert!(s["error"].is_null(), "{s}");
    s
}

/// A new document with an empty Sketch1 (F1).
fn with_sketch() -> Document<MockKernel> {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    d
}

/// The sketch query's entry of an entity.
fn entity(s: &Value, id: &str) -> Value {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {s}"))
        .clone()
}

/// A spline through the points, or by them as control points with a
/// degree; returns its curve id.
fn spline(
    d: &mut Document<MockKernel>,
    points: Value,
    degree: Option<u32>,
    closed: bool,
) -> String {
    let mut cmd =
        json!({"cmd": "sketch.spline", "sketch": "F1", "points": points, "closed": closed});
    if let Some(degree) = degree {
        cmd["degree"] = json!(degree);
    }
    let made = command(d, cmd);
    made["made"][0].as_str().unwrap().to_owned()
}

/// Inserts a point on `curve` at `at`; returns the new point.
fn insert(d: &mut Document<MockKernel>, curve: &str, at: [f64; 2]) -> String {
    let made = command(
        d,
        json!({"cmd": "sketch.insert_spline_point", "sketch": "F1", "curve": curve, "at": at}),
    );
    made["made"][0].as_str().unwrap().to_owned()
}

/// The point ids a spline lists: its fit points or its control points.
fn ids(s: &Value, curve: &str) -> Vec<String> {
    let e = entity(s, curve);
    let list = if e["type"] == "fitted_spline" {
        &e["points"]
    } else {
        &e["control"]
    };
    list.as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect()
}

/// A position in the sketch query.
fn xy(v: &Value) -> [f64; 2] {
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]
}

/// A point's solved position.
fn at(s: &Value, point: &str) -> [f64; 2] {
    xy(&entity(s, point)["at"])
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The solved curve of a spline.
fn nurbs(s: &Value, curve: &str) -> Nurbs {
    let g = &entity(s, curve)["geometry"];
    let numbers = |v: &Value| -> Vec<f64> {
        v.as_array()
            .map(|a| a.iter().map(|x| x.as_f64().unwrap()).collect())
            .unwrap_or_default()
    };
    Nurbs {
        degree: g["degree"].as_u64().unwrap() as usize,
        control: g["control"].as_array().unwrap().iter().map(xy).collect(),
        weights: numbers(&g["weights"]),
        knots: numbers(&g["knots"]),
    }
}

/// The parameters of a fitted spline's fit points (chord lengths over the
/// domain; a closed one's first point again last).
fn fit_params(s: &Value, curve: &str, closed: bool) -> Vec<f64> {
    let mut fit: Vec<[f64; 2]> = ids(s, curve).iter().map(|p| at(s, p)).collect();
    if closed {
        fit.push(fit[0]);
    }
    let mut lengths = vec![0.0];
    let mut total = 0.0;
    for w in fit.windows(2) {
        total += dist(w[0], w[1]);
        lengths.push(total);
    }
    let (lo, hi) = nurbs(s, curve).domain();
    lengths.iter().map(|l| lo + (hi - lo) * l / total).collect()
}

/// The curve passes through every fit point, at its parameter.
fn passes_through_fit_points(s: &Value, curve: &str, closed: bool) {
    let n = nurbs(s, curve);
    let mut points = ids(s, curve);
    if closed {
        points.push(points[0].clone());
    }
    for (p, t) in points.iter().zip(fit_params(s, curve, closed)) {
        let d = dist(n.eval(t)[0], at(s, p));
        assert!(d < 1e-6, "{p} at {t}: {d}");
    }
}

/// The curve is the same at sampled parameters, within 1e-6 mm.
fn same_shape(before: &Nurbs, after: &Nurbs) {
    assert_eq!(before.domain(), after.domain());
    let (lo, hi) = before.domain();
    for i in 0..=100 {
        let t = lo + (hi - lo) * i as f64 / 100.0;
        let d = dist(before.eval(t)[0], after.eval(t)[0]);
        assert!(d < 1e-6, "at {t}: {d}");
    }
}

/// The ids of `before` in `after`, in the same order, with `new` at
/// `index`.
fn listed_with(before: &[String], new: &str, index: usize) -> Vec<String> {
    let mut list = before.to_vec();
    list.insert(index, new.to_owned());
    list
}

#[test]
fn a_fit_point_goes_between_its_neighbours_of_an_open_spline() {
    let mut d = with_sketch();
    let c = spline(
        &mut d,
        json!([[0, 0], [20, 10], [40, 0], [60, 10]]),
        None,
        false,
    );
    let s = sketch(&d);
    let before = ids(&s, &c);
    // Between the second and the third point, on the curve.
    let params = fit_params(&s, &c, false);
    let place = nurbs(&s, &c).eval((params[1] + params[2]) / 2.0)[0];
    let p = insert(&mut d, &c, place);
    assert_eq!(d.undo_label(), Some("Insert Spline Point in Sketch1"));
    let s = sketch(&d);
    assert_eq!(ids(&s, &c), listed_with(&before, &p, 2));
    assert!(dist(at(&s, &p), place) < 1e-6, "{:?}", at(&s, &p));
    passes_through_fit_points(&s, &c, false);
    // Before the second point, and at the ends' stretches.
    let params = fit_params(&s, &c, false);
    let n = nurbs(&s, &c);
    let q = insert(
        &mut d,
        &c,
        n.eval(params[0] + (params[1] - params[0]) / 3.0)[0],
    );
    let s = sketch(&d);
    assert_eq!(ids(&s, &c)[1], q);
    passes_through_fit_points(&s, &c, false);
    let count = ids(&s, &c).len();
    let params = fit_params(&s, &c, false);
    let last = nurbs(&s, &c).eval((params[count - 2] + params[count - 1]) / 2.0)[0];
    let r = insert(&mut d, &c, last);
    let s = sketch(&d);
    assert_eq!(ids(&s, &c)[count - 1], r);
    assert_eq!(ids(&s, &c).len(), count + 1);
    passes_through_fit_points(&s, &c, false);
    // On an existing fit point: refused.
    let on = at(&s, &ids(&s, &c)[2]);
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.insert_spline_point", "sketch": "F1", "curve": c, "at": on}),
    );
    assert!(error.contains("already"), "{error}");
}

#[test]
fn a_fit_point_goes_between_its_neighbours_of_a_closed_spline() {
    let mut d = with_sketch();
    let section = json!([[0, 0], [40, -10], [80, 0], [40, 12]]);
    let c = spline(&mut d, section, None, true);
    let s = sketch(&d);
    let before = ids(&s, &c);
    // On the stretch from the last point back to the first: listed last.
    let params = fit_params(&s, &c, true);
    let place = nurbs(&s, &c).eval((params[3] + params[4]) / 2.0)[0];
    let p = insert(&mut d, &c, place);
    let s = sketch(&d);
    assert_eq!(entity(&s, &c)["closed"], true);
    assert_eq!(ids(&s, &c), listed_with(&before, &p, 4));
    passes_through_fit_points(&s, &c, true);
    // Between the first and the second point.
    let before = ids(&s, &c);
    let params = fit_params(&s, &c, true);
    let place = nurbs(&s, &c).eval((params[0] + params[1]) / 2.0)[0];
    let q = insert(&mut d, &c, place);
    let s = sketch(&d);
    assert_eq!(ids(&s, &c), listed_with(&before, &q, 1));
    assert!(dist(at(&s, &q), place) < 1e-6);
    passes_through_fit_points(&s, &c, true);
    // Still closed: it starts and ends at the first point.
    let n = nurbs(&s, &c);
    let (lo, hi) = n.domain();
    assert!(dist(n.eval(lo)[0], n.eval(hi)[0]) < 1e-9);
}

#[test]
fn a_control_point_keeps_the_shape_of_an_open_spline() {
    let mut d = with_sketch();
    let c = spline(
        &mut d,
        json!([[0, 0], [10, 25], [30, 30], [45, -5], [60, 10]]),
        Some(3),
        false,
    );
    let s = sketch(&d);
    let before = nurbs(&s, &c);
    let old = ids(&s, &c);
    for t in [0.37, 0.5, 0.9] {
        let s = sketch(&d);
        let shape = nurbs(&s, &c);
        let listed = ids(&s, &c);
        let p = insert(&mut d, &c, shape.eval(t)[0]);
        assert_eq!(d.undo_label(), Some("Insert Spline Point in Sketch1"));
        let s = sketch(&d);
        let now = ids(&s, &c);
        assert_eq!(now.len(), listed.len() + 1);
        let index = now
            .iter()
            .position(|q| *q == p)
            .expect("the new point listed");
        assert_eq!(now, listed_with(&listed, &p, index));
        let after = nurbs(&s, &c);
        assert_eq!(after.control.len(), shape.control.len() + 1);
        same_shape(&shape, &after);
        same_shape(&before, &after);
        // The knots are written out: one more.
        let knots = entity(&s, &c)["knots"].as_array().unwrap().len();
        assert_eq!(knots, after.control.len() + 4);
    }
    // The ends stay where they were and every old point is still listed.
    let s = sketch(&d);
    let now = ids(&s, &c);
    assert_eq!(now.first(), old.first());
    assert_eq!(now.last(), old.last());
    assert!(old.iter().all(|p| now.contains(p)));
    // One undo step takes the last point back.
    command(&mut d, json!({"cmd": "undo"}));
    assert_eq!(ids(&sketch(&d), &c).len(), now.len() - 1);
    // At an end: refused; nothing changes.
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.insert_spline_point", "sketch": "F1", "curve": c, "at": [0, 0]}),
    );
    assert!(error.contains("end"), "{error}");
}

#[test]
fn a_control_point_keeps_the_shape_of_a_closed_spline() {
    let mut d = with_sketch();
    let point = command(
        &mut d,
        json!({"cmd": "sketch.add_point", "sketch": "F1", "at": [0, 0]}),
    );
    let p = point["entities"][0].as_str().unwrap().to_owned();
    // A spline by control points whose ends are one point, smooth there.
    let c = spline(
        &mut d,
        json!([p, [30, -5], [40, 20], [5, 18], [-10, 8], p]),
        Some(3),
        false,
    );
    let tangent = command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "tangent", "a": c, "b": c, "at": p}}),
    );
    let k = tangent["constraints"][0].as_str().unwrap().to_owned();
    let s = sketch(&d);
    let before = nurbs(&s, &c);
    let constraints = s["constraints"].clone();
    for t in [0.1, 0.5, 0.85] {
        let s = sketch(&d);
        let shape = nurbs(&s, &c);
        insert(&mut d, &c, shape.eval(t)[0]);
        let s = sketch(&d);
        let after = nurbs(&s, &c);
        assert_eq!(after.control.len(), shape.control.len() + 1);
        same_shape(&before, &after);
        // Still closed at the one point, with its tangent constraint.
        let now = ids(&s, &c);
        assert_eq!((now.first(), now.last()), (Some(&p), Some(&p)));
        assert_eq!(s["constraints"], constraints, "{k} kept");
    }
    // Where it closes: refused. The tangent constraint's solve moved the
    // shared point away from where it was added, so pick it where it is.
    let seam = at(&sketch(&d), &p);
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.insert_spline_point", "sketch": "F1", "curve": c, "at": seam}),
    );
    assert!(error.contains("end"), "{error}");
}

#[test]
fn constraints_and_dimensions_on_the_old_points_are_kept() {
    // A fitted spline: a constraint and a dimension between fit points.
    let mut d = with_sketch();
    let c = spline(
        &mut d,
        json!([[0, 0], [20, 10], [40, 0], [60, 10]]),
        None,
        false,
    );
    let points = ids(&sketch(&d), &c);
    command(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "horizontal_points", "a": points[0], "b": points[2]}}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "distance", "a": points[0], "b": points[3]}, "value": 65}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": [points[0]]}),
    );
    let s = sketch(&d);
    let (constraints, dimensions) = (s["constraints"].clone(), s["dimensions"].clone());
    let params = fit_params(&s, &c, false);
    let place = nurbs(&s, &c).eval((params[1] + params[2]) / 2.0)[0];
    insert(&mut d, &c, place);
    let s = sketch(&d);
    assert_eq!(s["constraints"], constraints);
    assert_eq!(s["dimensions"], dimensions);
    assert_eq!(entity(&s, &points[0])["fixed"], true);
    assert!((dist(at(&s, &points[0]), at(&s, &points[3])) - 65.0).abs() < 1e-6);
    assert!((at(&s, &points[0])[1] - at(&s, &points[2])[1]).abs() < 1e-6);
    passes_through_fit_points(&s, &c, false);

    // A spline by control points: a dimension between its ends and the
    // first one fixed.
    let mut d = with_sketch();
    let c = spline(
        &mut d,
        json!([[0, 0], [10, 25], [30, 30], [45, -5], [60, 10]]),
        Some(3),
        false,
    );
    let points = ids(&sketch(&d), &c);
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1",
               "dimension": {"type": "distance", "a": points[0], "b": points[4]}, "value": 60}),
    );
    command(
        &mut d,
        json!({"cmd": "sketch.set_fixed", "sketch": "F1", "entities": [points[0]]}),
    );
    let s = sketch(&d);
    let before = nurbs(&s, &c);
    let (constraints, dimensions) = (s["constraints"].clone(), s["dimensions"].clone());
    insert(&mut d, &c, before.eval(0.4)[0]);
    let s = sketch(&d);
    assert_eq!(s["constraints"], constraints);
    assert_eq!(s["dimensions"], dimensions);
    assert_eq!(entity(&s, &points[0])["fixed"], true);
    same_shape(&before, &nurbs(&s, &c));
}

#[test]
fn only_splines_take_an_inserted_point() {
    let mut d = with_sketch();
    let line = command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 0], "end": [10, 0]}),
    );
    let l = line["made"][0].as_str().unwrap().to_owned();
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.insert_spline_point", "sketch": "F1", "curve": l, "at": [5, 0]}),
    );
    assert!(error.contains("splines only"), "{error}");
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.insert_spline_point", "sketch": "F1", "curve": "c99", "at": [5, 0]}),
    );
    assert!(error.contains("c99"), "{error}");
}
