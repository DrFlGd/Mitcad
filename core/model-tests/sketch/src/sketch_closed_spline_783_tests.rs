// SPDX-License-Identifier: MIT
//! Closed fitted splines and tangents at a chosen end (mitcad#783), with
//! the mock kernel.

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

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    let answer = doc.query(&query.to_string()).unwrap();
    serde_json::from_str(&answer).unwrap()
}

fn sketch(doc: &Document<MockKernel>) -> Value {
    query(doc, json!({"query": "sketch", "uid": "F1"}))
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

/// The sketch query's entry of a constraint.
fn constraint(s: &Value, id: &str) -> Value {
    s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {s}"))
        .clone()
}

/// A spline through the points; returns its curve id.
fn spline(d: &mut Document<MockKernel>, points: Value, closed: bool) -> String {
    let made = command(
        d,
        json!({"cmd": "sketch.spline", "sketch": "F1", "points": points, "closed": closed}),
    );
    made["made"][0].as_str().unwrap().to_owned()
}

/// The fit points of a fitted spline.
fn fit_points(s: &Value, curve: &str) -> Vec<String> {
    entity(s, curve)["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect()
}

/// A constraint added to the sketch; returns its id.
fn constrain(d: &mut Document<MockKernel>, constraint: Value) -> String {
    let made = command(
        d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1", "constraint": constraint}),
    );
    made["constraints"][0].as_str().unwrap().to_owned()
}

/// The solved control points of a spline.
fn control(s: &Value, curve: &str) -> Vec<[f64; 2]> {
    entity(s, curve)["geometry"]["control"]
        .as_array()
        .unwrap()
        .iter()
        .map(xy)
        .collect()
}

/// A position in the sketch query.
fn xy(v: &Value) -> [f64; 2] {
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]
}

/// The leg of a clamped spline at an end: from the end into the curve.
fn leg(control: &[[f64; 2]], end: usize) -> [f64; 2] {
    let n = control.len() - 1;
    let (p, q) = if end == 0 {
        (control[0], control[1])
    } else {
        (control[n], control[n - 1])
    };
    [q[0] - p[0], q[1] - p[1]]
}

/// The sine of the angle between two directions.
fn sine(u: [f64; 2], v: [f64; 2]) -> f64 {
    (u[0] * v[1] - u[1] * v[0]) / (u[0].hypot(u[1]) * v[0].hypot(v[1]))
}

/// A body-like closed outline: a fuselage section through four points.
const SECTION: [[f64; 2]; 4] = [[0.0, 0.0], [40.0, -10.0], [80.0, 0.0], [40.0, 12.0]];

#[test]
fn a_closed_spline_is_one_curve_one_profile_and_extrudes() {
    let mut d = with_sketch();
    let c = spline(&mut d, json!(SECTION), true);
    let s = sketch(&d);
    assert!(s["error"].is_null(), "{s}");
    let e = entity(&s, &c);
    assert_eq!(e["type"], "fitted_spline");
    assert_eq!(e["closed"], true);
    assert_eq!(fit_points(&s, &c).len(), 4, "the points listed once: {e}");
    // The solved curve starts and ends at the first point.
    let control = e["geometry"]["control"].as_array().unwrap();
    assert_eq!(control.first(), control.last(), "{e}");
    let start = xy(&control[0]);
    assert!(start[0].hypot(start[1]) < 1e-9, "at the first point: {e}");
    // Only the points are free.
    assert_eq!(s["dof"], 8, "{s}");
    assert!(s["redundant"].as_array().unwrap().is_empty(), "{s}");
    // One region, extruded as one profile.
    let regions = s["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1, "{s}");
    let area = regions[0]["area"].as_f64().unwrap();
    assert!(area > 1000.0 && area < 2500.0, "area {area}");
    let key = regions[0]["key"].as_str().unwrap().to_owned();
    let extrude = command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": key}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}}),
    );
    assert!(extrude["error"].is_null(), "{extrude}");
    let bodies = query(&d, json!({"query": "bodies"}));
    assert_eq!(bodies.as_array().unwrap().len(), 1);
}

#[test]
fn a_closed_spline_needs_three_fit_points_and_no_degree() {
    let mut d = with_sketch();
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.spline", "sketch": "F1", "points": [[0, 0], [10, 5]],
               "closed": true}),
    );
    assert!(error.contains("three"), "{error}");
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.spline", "sketch": "F1", "points": SECTION, "degree": 3,
               "closed": true}),
    );
    assert!(error.contains("fit point"), "{error}");
}

#[test]
fn a_closed_spline_saves_and_loads_and_open_ones_are_saved_as_before() {
    let mut d = with_sketch();
    let closed = spline(&mut d, json!(SECTION), true);
    let open = spline(&mut d, json!([[0, 30], [20, 40], [40, 30]]), false);
    let json = d.to_json();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entities = value["features"][0]["entities"].as_array().unwrap();
    let saved = |id: &str| entities.iter().find(|e| e["id"] == id).unwrap().clone();
    assert_eq!(saved(&closed)["closed"], true);
    assert!(saved(&open).get("closed").is_none(), "{}", saved(&open));
    let mut loaded = Document::from_json(&json, MockKernel::default()).unwrap();
    assert_eq!(loaded.to_json(), json, "saving again gives the same file");
    loaded.recompute();
    assert_eq!(sketch(&loaded), sketch(&d));
}

#[test]
fn a_closed_spline_offsets_and_trims() {
    let mut d = with_sketch();
    let c = spline(&mut d, json!(SECTION), true);
    // An offset out of it: a closed curve around it.
    command(
        &mut d,
        json!({"cmd": "sketch.offset", "sketch": "F1", "curves": [c], "distance": 2}),
    );
    let s = sketch(&d);
    assert!(s["error"].is_null(), "{s}");
    assert_eq!(
        s["regions"].as_array().unwrap().len(),
        2,
        "inside and the ring: {s}"
    );
    // Trim: a line across the right end cuts it twice; the piece right of
    // the line goes, the rest is a spline from crossing to crossing.
    let mut d = with_sketch();
    let c = spline(&mut d, json!(SECTION), true);
    command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [60, -20], "end": [60, 20]}),
    );
    assert_eq!(sketch(&d)["regions"].as_array().unwrap().len(), 2);
    command(
        &mut d,
        json!({"cmd": "sketch.trim", "sketch": "F1", "curve": c, "at": [80, 0]}),
    );
    let s = sketch(&d);
    assert!(s["error"].is_null(), "{s}");
    let e = entity(&s, &c);
    assert_eq!(e["type"], "spline", "{e}");
    let control = e["geometry"]["control"].as_array().unwrap();
    for end in [control.first().unwrap(), control.last().unwrap()] {
        let x = end[0].as_f64().unwrap();
        assert!((x - 60.0).abs() < 1e-6, "ends on the line: {e}");
    }
    assert_eq!(s["regions"].as_array().unwrap().len(), 1, "{s}");
}

#[test]
fn two_curves_are_tangent_to_a_spline_at_its_two_ends() {
    let mut d = with_sketch();
    // Two splines from p.. to p..: a lens, tangent at both of its corners.
    let upper = spline(&mut d, json!([[0, 0], [20, 8], [40, 0]]), false);
    let ends = fit_points(&sketch(&d), &upper);
    let (a, b) = (ends[0].clone(), ends[2].clone());
    let lower = spline(&mut d, json!([a, [20, -6], b]), false);
    let first = constrain(
        &mut d,
        json!({"type": "tangent", "a": upper, "b": lower, "at": a}),
    );
    let second = constrain(
        &mut d,
        json!({"type": "tangent", "a": lower, "b": upper, "at": b}),
    );
    let s = sketch(&d);
    assert!(s["error"].is_null(), "{s}");
    assert!(s["redundant"].as_array().unwrap().is_empty(), "{s}");
    assert!(s["conflicts"].as_array().unwrap().is_empty(), "{s}");
    assert_eq!(constraint(&s, &first)["at"], json!(a));
    assert_eq!(constraint(&s, &second)["at"], json!(b));
    // The legs at each end are parallel: the curves leave each corner
    // along one line.
    let (cu, cl) = (control(&s, &upper), control(&s, &lower));
    for end in 0..2 {
        let sine = sine(leg(&cu, end), leg(&cl, end));
        assert!(sine.abs() < 1e-6, "end {end}: {sine}");
    }
}

#[test]
fn a_spline_whose_ends_meet_is_made_smooth_there_by_a_tangent_to_itself() {
    let mut d = with_sketch();
    let point = command(
        &mut d,
        json!({"cmd": "sketch.add_point", "sketch": "F1", "at": [0, 0]}),
    );
    let p = point["entities"][0].as_str().unwrap().to_owned();
    // An open spline that ends where it starts.
    let c = spline(&mut d, json!([p, [30, -5], [40, 20], [5, 18], p]), false);
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "tangent", "a": c, "b": c}}),
    );
    assert!(error.contains("itself"), "{error}");
    let k = constrain(&mut d, json!({"type": "tangent", "a": c, "b": c, "at": p}));
    let s = sketch(&d);
    assert!(s["error"].is_null(), "{s}");
    assert_eq!(constraint(&s, &k)["at"], json!(p));
    let control = control(&s, &c);
    let sine = sine(leg(&control, 0), leg(&control, 1));
    assert!(sine.abs() < 1e-6, "smooth where it closes: {sine}");
}

#[test]
fn tangents_without_a_point_keep_their_meaning_and_file_form() {
    // A line from a spline's end, tangent to it as older designs say it:
    // no `at` is written, and the tangent is at the shared end.
    let mut d = with_sketch();
    let c = spline(&mut d, json!([[0, 0], [20, 10], [40, 0]]), false);
    let ends = fit_points(&sketch(&d), &c);
    let line = command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": ends[2], "end": [60, -5]}),
    );
    let l = line["made"][0].as_str().unwrap().to_owned();
    let k = constrain(&mut d, json!({"type": "tangent", "a": l, "b": c}));
    let s = sketch(&d);
    assert!(s["error"].is_null(), "{s}");
    assert!(constraint(&s, &k).get("at").is_none(), "{s}");
    let saved: Value = serde_json::from_str(&d.to_json()).unwrap();
    let saved = saved["features"][0]["constraints"].as_array().unwrap();
    let saved = saved.iter().find(|x| x["id"] == k).unwrap();
    assert!(saved.get("at").is_none(), "saved as before: {saved}");
    let end = leg(&control(&s, &c), 1);
    let line = entity(&s, &l);
    let (a, b) = (xy(&line["geometry"]["start"]), xy(&line["geometry"]["end"]));
    let sine = sine(end, [b[0] - a[0], b[1] - a[1]]);
    assert!(sine.abs() < 1e-6, "tangent at the shared end: {sine}");
}
