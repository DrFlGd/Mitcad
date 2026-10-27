// SPDX-License-Identifier: MIT
//! Copy, Cut and Paste of sketch geometry (mitcad#788): the constraints and
//! dimensions a copy keeps, where a paste puts the geometry, expressions
//! and values, and pasting into another design, with the mock kernel.

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
    let answer = doc
        .query(&query.to_string())
        .unwrap_or_else(|e| panic!("{query}: {e}"));
    serde_json::from_str(&answer).unwrap()
}

fn sketch(doc: &Document<MockKernel>, uid: &str) -> Value {
    query(doc, json!({"query": "sketch", "uid": uid}))
}

fn copy(doc: &Document<MockKernel>, uid: &str, items: &[&str]) -> Value {
    query(
        doc,
        json!({"query": "sketch_fragment", "sketch": uid, "items": items}),
    )
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

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    close(a[0], b[0]) && close(a[1], b[1])
}

/// A point `[x, y]` of an answer.
fn xy(v: &Value) -> [f64; 2] {
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]
}

/// The curves of a sketch (no points).
fn curves(s: &Value) -> Vec<Value> {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] != "point")
        .cloned()
        .collect()
}

/// The dimension of a type, with its `measured` value.
fn dimension(s: &Value, kind: &str) -> Value {
    s["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["type"] == kind)
        .unwrap_or_else(|| panic!("no {kind} dimension in {s}"))
        .clone()
}

fn constraint_types(s: &Value) -> Vec<String> {
    s["constraints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["type"].as_str().unwrap().to_owned())
        .collect()
}

fn first(result: &Value, key: &str) -> String {
    result[key][0].as_str().unwrap().to_owned()
}

/// Sketch1 (F1) with its origin point and a line c from (10, 10) to
/// (40, 10): horizontal, 30 long and 10 above the origin. A second line
/// from (0, 30) to (40, 30) is parallel to it. Returns the line, its start
/// point and the second line.
fn line_sketch(d: &mut Document<MockKernel>) -> (String, String, String) {
    command(d, json!({"cmd": "sketch.create"}));
    let origin = first(
        &command(
            d,
            json!({"cmd": "sketch.add_point", "sketch": "F1", "at": [0, 0], "fixed": true}),
        ),
        "entities",
    );
    let line = first(
        &command(
            d,
            json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [10, 10], "end": [40, 10]}),
        ),
        "made",
    );
    let start = entity(&sketch(d, "F1"), &line)["start"]
        .as_str()
        .unwrap()
        .to_owned();
    command(
        d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "horizontal", "line": line}}),
    );
    command(
        d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1", "value": 30,
               "dimension": {"type": "length", "line": line}}),
    );
    command(
        d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1", "value": 10,
               "dimension": {"type": "vertical_distance", "a": origin, "b": start}}),
    );
    let other = first(
        &command(
            d,
            json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 30], "end": [40, 30]}),
        ),
        "made",
    );
    command(
        d,
        json!({"cmd": "sketch.add_constraint", "sketch": "F1",
               "constraint": {"type": "parallel", "a": line, "b": other}}),
    );
    (line, start, other)
}

#[test]
fn a_copy_keeps_the_constraints_on_copied_geometry_and_the_origin() {
    let mut d = Document::new(MockKernel::default());
    let (line, _, other) = line_sketch(&mut d);
    let copied = copy(&d, "F1", &[&line]);
    // Horizontal, the length and the distance from the origin; the
    // parallel to the line not copied is left out.
    assert_eq!(copied["kept"], 3, "{copied}");
    assert_eq!(copied["left_out"], 1, "{copied}");
    // The line and its two points; the origin is the target's own.
    assert_eq!(copied["entities"], 3, "{copied}");
    let fragment = &copied["fragment"];
    assert_eq!(fragment["references"][0]["is"], "origin", "{fragment}");
    assert_eq!(fragment["constraints"].as_array().unwrap().len(), 1);
    assert_eq!(fragment["constraints"][0]["type"], "horizontal");
    assert_eq!(fragment["dimensions"].as_array().unwrap().len(), 2);
    // The base point is the line's start; the distance from the origin
    // holds it in y.
    assert!(near(xy(&copied["base"]), [10.0, 10.0]), "{copied}");
    assert_eq!(copied["held"], json!([false, true]), "{copied}");
    // The preview: the line as a polyline from end to end.
    let outline = copied["outline"].as_array().unwrap();
    assert_eq!(outline.len(), 1, "{copied}");
    let polyline = outline[0].as_array().unwrap();
    assert!(near(xy(&polyline[0]), [10.0, 10.0]), "{copied}");
    let last = xy(polyline.last().unwrap());
    assert!(near(last, [40.0, 10.0]), "{copied}");

    // Both lines: the parallel is kept too.
    let both = copy(&d, "F1", &[&line, &other]);
    assert_eq!(both["kept"], 4, "{both}");
    assert_eq!(both["left_out"], 0, "{both}");
}

#[test]
fn a_paste_into_another_sketch_goes_where_its_base_point_is_put() {
    let mut d = Document::new(MockKernel::default());
    let (line, _, _) = line_sketch(&mut d);
    let fragment = copy(&d, "F1", &[&line])["fragment"].clone();
    let created = command(&mut d, json!({"cmd": "sketch.create", "plane": "xz"}));
    let target = created["uid"].as_str().unwrap().to_owned();
    let pasted = command(
        &mut d,
        json!({"cmd": "sketch.paste", "sketch": target, "fragment": fragment, "at": [30, 40]}),
    );
    assert_eq!(pasted["kept"], 3, "{pasted}");
    assert_eq!(pasted["left_out"], 1, "{pasted}");
    // The parallel was left out at the copy: the paste does not say so again.
    assert!(pasted["warnings"].is_null(), "{pasted}");
    // The line, its two points and the target's own origin point.
    let s = sketch(&d, &target);
    let made = curves(&s);
    assert_eq!(made.len(), 1, "{s}");
    let copy = &made[0];
    assert_eq!(copy["type"], "line");
    // Put 30 higher, but held 10 above the origin: moved by (20, 0) in the
    // 2D coordinates.
    let start = at(&s, copy["start"].as_str().unwrap());
    let end = at(&s, copy["end"].as_str().unwrap());
    assert!(near(start, [30.0, 10.0]), "{start:?} in {s}");
    assert!(near(end, [60.0, 10.0]), "{end:?} in {s}");
    assert_eq!(constraint_types(&s), ["horizontal"]);
    let length = dimension(&s, "length")["measured"].as_f64().unwrap();
    assert!(close(length, 30.0), "{s}");
    let distance = dimension(&s, "vertical_distance");
    assert!(close(distance["measured"].as_f64().unwrap(), 10.0));
    // The vertical distance is from the target's origin, a fixed point at
    // (0, 0) made for it.
    let origin = entity(&s, distance["a"].as_str().unwrap());
    assert_eq!(origin["fixed"], true, "{origin}");
    assert!(near(at(&s, distance["a"].as_str().unwrap()), [0.0, 0.0]));
    // The source is as it was.
    let source = sketch(&d, "F1");
    assert_eq!(curves(&source).len(), 2);
    assert_eq!(source["constraints"].as_array().unwrap().len(), 2);

    // One undo step.
    command(&mut d, json!({"cmd": "undo"}));
    let s = sketch(&d, &target);
    assert!(curves(&s).is_empty(), "{s}");
    assert!(s["dimensions"].as_array().unwrap_or(&Vec::new()).is_empty());
}

#[test]
fn a_cut_and_a_paste_at_the_same_place_keep_the_geometry() {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    let circle = first(
        &command(
            &mut d,
            json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center",
                   "center": [15, 25], "radius": 5}),
        ),
        "made",
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1", "value": 12,
               "dimension": {"type": "diameter", "curve": circle}, "text": [25, 25]}),
    );
    let copied = copy(&d, "F1", &[&circle]);
    // Not held on to the origin: the base point is the circle's centre.
    assert!(near(xy(&copied["base"]), [15.0, 25.0]), "{copied}");
    assert_eq!(copied["held"], json!([false, false]), "{copied}");
    assert_eq!(copied["kept"], 1);
    // Cut: the copy, then the circle removed.
    command(
        &mut d,
        json!({"cmd": "sketch.remove", "sketch": "F1", "items": [circle]}),
    );
    assert!(curves(&sketch(&d, "F1")).is_empty());
    // Pasted without a place: where it was.
    let pasted = command(
        &mut d,
        json!({"cmd": "sketch.paste", "sketch": "F1", "fragment": copied["fragment"]}),
    );
    assert_eq!(pasted["left_out"], 0, "{pasted}");
    let s = sketch(&d, "F1");
    let made = curves(&s);
    assert_eq!(made.len(), 1, "{s}");
    let center = at(&s, made[0]["center"].as_str().unwrap());
    assert!(near(center, [15.0, 25.0]), "{s}");
    let diameter = dimension(&s, "diameter");
    assert!(close(diameter["measured"].as_f64().unwrap(), 12.0));
    assert!(near(xy(&diameter["text"]), [25.0, 25.0]), "{diameter}");
    // Pasted again at (50, 50): the centre there, the value beside it.
    command(
        &mut d,
        json!({"cmd": "sketch.paste", "sketch": "F1", "fragment": copied["fragment"],
               "at": [50, 50]}),
    );
    let s = sketch(&d, "F1");
    let centers: Vec<[f64; 2]> = curves(&s)
        .iter()
        .map(|c| at(&s, c["center"].as_str().unwrap()))
        .collect();
    assert_eq!(centers.len(), 2);
    let moved = centers.iter().any(|c| near(*c, [50.0, 50.0]));
    assert!(moved, "{centers:?}");
    let texts: Vec<[f64; 2]> = s["dimensions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| xy(&k["text"]))
        .collect();
    assert!(texts.iter().any(|t| near(*t, [60.0, 50.0])), "{texts:?}");
}

#[test]
fn a_paste_keeps_expressions_where_their_parameters_exist() {
    let mut d = Document::new(MockKernel::default());
    command(
        &mut d,
        json!({"cmd": "add_parameter", "name": "width", "expression": "15 mm"}),
    );
    command(&mut d, json!({"cmd": "sketch.create"}));
    let line = first(
        &command(
            &mut d,
            json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [0, 5], "end": [20, 5]}),
        ),
        "made",
    );
    command(
        &mut d,
        json!({"cmd": "sketch.add_dimension", "sketch": "F1", "value": "width * 2",
               "dimension": {"type": "length", "line": line}}),
    );
    let fragment = copy(&d, "F1", &[&line])["fragment"].clone();
    let value = &fragment["dimensions"][0]["value"];
    let expression = value["expression"].as_str().unwrap();
    assert!(expression.contains("width"), "{fragment}");
    assert_eq!(value["parameters"], json!(["width"]));
    assert!(close(value["value"].as_f64().unwrap(), 30.0), "{fragment}");
    // In this design: the expression, which follows width.
    command(&mut d, json!({"cmd": "sketch.create"}));
    let pasted = command(
        &mut d,
        json!({"cmd": "sketch.paste", "sketch": "F2", "fragment": fragment}),
    );
    assert!(pasted["values"].is_null(), "{pasted}");
    let length = dimension(&sketch(&d, "F2"), "length");
    let expression = length["expression"].as_str().unwrap();
    assert!(expression.contains("width"), "{length}");
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "width", "expression": "20 mm"}),
    );
    let length = dimension(&sketch(&d, "F2"), "length");
    assert!(
        close(length["measured"].as_f64().unwrap(), 40.0),
        "{length}"
    );

    // In another design without width: the value, and the paste says so.
    let mut other = Document::new(MockKernel::default());
    command(&mut other, json!({"cmd": "sketch.create"}));
    let pasted = command(
        &mut other,
        json!({"cmd": "sketch.paste", "sketch": "F1", "fragment": fragment, "at": [5, 5]}),
    );
    assert_eq!(pasted["values"].as_array().unwrap().len(), 1, "{pasted}");
    assert!(
        pasted["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("took their values")),
        "{pasted}"
    );
    let s = sketch(&other, "F1");
    let length = dimension(&s, "length");
    assert!(
        close(length["measured"].as_f64().unwrap(), 30.0),
        "{length}"
    );
    let copy = &curves(&s)[0];
    let start = at(&s, copy["start"].as_str().unwrap());
    assert!(near(start, [5.0, 5.0]), "{s}");
}

#[test]
fn splines_construction_and_texts_paste_with_new_ids() {
    let mut d = Document::new(MockKernel::default());
    command(&mut d, json!({"cmd": "sketch.create"}));
    let spline = first(
        &command(
            &mut d,
            json!({"cmd": "sketch.spline", "sketch": "F1",
                   "points": [[0, 0], [10, 5], [20, 0]], "construction": true}),
        ),
        "made",
    );
    let text = first(
        &command(
            &mut d,
            json!({"cmd": "sketch.add_text", "sketch": "F1", "text": "A", "at": [5, -10],
                   "height": 5, "font": "Mock Sans"}),
        ),
        "made",
    );
    // The spline first: the base point is its first fit point.
    let copied = copy(&d, "F1", &[&spline, &text]);
    assert!(near(xy(&copied["base"]), [0.0, 0.0]), "{copied}");
    assert_eq!(copied["texts"], 1);
    assert!(copied["fragment"]["references"].is_null(), "{copied}");
    let pasted = command(
        &mut d,
        json!({"cmd": "sketch.paste", "sketch": "F1", "fragment": copied["fragment"],
               "at": [100, 0]}),
    );
    let made: Vec<&str> = pasted["made"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    assert!(!made.contains(&spline.as_str()), "{pasted}");
    assert!(!made.contains(&text.as_str()), "{pasted}");
    let s = sketch(&d, "F1");
    let copy = curves(&s)
        .into_iter()
        .find(|c| made.contains(&c["id"].as_str().unwrap()))
        .unwrap_or_else(|| panic!("no pasted spline in {s}"));
    assert_eq!(copy["type"], "fitted_spline", "{copy}");
    assert_eq!(copy["construction"], true, "{copy}");
    let points: Vec<[f64; 2]> = copy["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| at(&s, p.as_str().unwrap()))
        .collect();
    assert!(near(points[0], [100.0, 0.0]), "{points:?}");
    assert!(near(points[2], [120.0, 0.0]), "{points:?}");
    let texts = s["texts"].as_array().unwrap();
    assert_eq!(texts.len(), 2);
    let pasted_text = texts
        .iter()
        .find(|t| made.contains(&t["id"].as_str().unwrap()))
        .unwrap_or_else(|| panic!("no pasted text in {s}"));
    assert_eq!(pasted_text["text"], "A");
    let placed = xy(&pasted_text["at"]);
    assert!(near(placed, [105.0, -10.0]), "{pasted_text}");
}

#[test]
fn copies_and_pastes_that_cannot_be_are_refused() {
    let mut d = Document::new(MockKernel::default());
    let (line, _, _) = line_sketch(&mut d);
    // Nothing selected but what does not exist.
    let missing = d
        .query(&json!({"query": "sketch_fragment", "sketch": "F1", "items": ["c99"]}).to_string())
        .unwrap_err()
        .to_string();
    assert!(missing.contains("does not exist"), "{missing}");
    let empty = d
        .query(&json!({"query": "sketch_fragment", "sketch": "F1", "items": []}).to_string())
        .unwrap_err()
        .to_string();
    assert!(empty.contains("nothing to copy"), "{empty}");
    // A curve on a point the fragment lacks.
    let mut fragment = copy(&d, "F1", &[&line])["fragment"].clone();
    let curves_only: Vec<Value> = fragment["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] != "point")
        .cloned()
        .collect();
    fragment["entities"] = json!(curves_only);
    let refused = rejected(
        &mut d,
        json!({"cmd": "sketch.paste", "sketch": "F1", "fragment": fragment}),
    );
    assert!(refused.contains("geometry lacks"), "{refused}");
    // A newer format.
    let mut newer = copy(&d, "F1", &[&line])["fragment"].clone();
    newer["version"] = json!(99);
    let refused = rejected(
        &mut d,
        json!({"cmd": "sketch.paste", "sketch": "F1", "fragment": newer}),
    );
    assert!(refused.contains("newer version"), "{refused}");
}
