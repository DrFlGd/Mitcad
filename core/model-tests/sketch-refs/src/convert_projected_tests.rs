// SPDX-License-Identifier: MIT
//! Convert to Sketch Geometry (mitcad#257): projected curves and points into
//! ordinary sketch geometry, with the mock kernel.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::kernel::Curve3;
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
        &doc.query(&json!({"query": "sketch", "uid": "F3"}).to_string())
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

fn entity(s: &Value, id: &str) -> Value {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {s}"))
        .clone()
}

/// Whether an entity is projected: reference geometry, fixed.
fn projected(s: &Value, id: &str) -> bool {
    let e = entity(s, id);
    e["reference"] == true && e["fixed"] == true
}

/// Whether an entity is ordinary free geometry.
fn free(s: &Value, id: &str) -> bool {
    let e = entity(s, id);
    e.get("reference").is_none() && e.get("fixed").is_none()
}

fn linked(s: &Value) -> Vec<String> {
    s["projections"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["entities"].as_array().unwrap().clone())
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

fn ids(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

/// The top face of the block.
const FACE: &str = "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";

fn line(a: [f64; 2], b: [f64; 2]) -> Curve3 {
    Curve3::Line {
        start: [a[0], a[1], 20.0],
        end: [b[0], b[1], 20.0],
    }
}

/// The top face's edges for a block `width` wide.
fn square(width: f64) -> Vec<Curve3> {
    let c = [[0.0, 0.0], [width, 0.0], [width, 40.0], [0.0, 40.0]];
    (0..4).map(|i| line(c[i], c[(i + 1) % 4])).collect()
}

/// A 60 x 40 block 20 high (F1, F2) and a sketch on its top (F3) with
/// `curves` as the source's, projected (`linked` or not).
fn projected_block(curves: Vec<Curve3>, linked: bool) -> Document<MockKernel> {
    let mut d = Document::new(MockKernel::default());
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
    command(
        &mut d,
        json!({"cmd": "sketch.create", "plane": {"face": FACE}}),
    );
    *d.kernel().curves.borrow_mut() = curves;
    command(
        &mut d,
        json!({"cmd": "sketch.project", "sketch": "F3", "source": FACE, "body": "F2.b0",
               "linked": linked}),
    );
    d
}

/// The id of the line from `a` to `b` (either way round).
fn line_id(s: &Value, a: [f64; 2], b: [f64; 2]) -> String {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            let g = &e["geometry"];
            e["type"] == "line"
                && ((near(&g["start"], a) && near(&g["end"], b))
                    || (near(&g["start"], b) && near(&g["end"], a)))
        })
        .unwrap_or_else(|| panic!("no line {a:?}-{b:?} in {s}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

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

fn convert(d: &mut Document<MockKernel>, entities: &[&str]) -> Value {
    command(
        d,
        json!({"cmd": "sketch.convert_projected", "sketch": "F3", "entities": entities}),
    )
}

#[test]
fn one_line_of_a_projected_rectangle_converts_and_its_corners_stay_projected() {
    let mut d = projected_block(square(60.0), true);
    let s = sketch(&d);
    let bottom = line_id(&s, [0.0, 0.0], [60.0, 0.0]);
    let (a, b) = (point_id(&s, [0.0, 0.0]), point_id(&s, [60.0, 0.0]));
    let result = convert(&mut d, &[&bottom]);
    assert_eq!(
        ids(&result["converted"]),
        std::slice::from_ref(&bottom),
        "{result}"
    );
    let mut kept = ids(&result["kept"]);
    kept.sort();
    let mut corners = vec![a.clone(), b.clone()];
    corners.sort();
    assert_eq!(kept, corners, "{result}");
    assert_eq!(result["warnings"].as_array().unwrap().len(), 1, "{result}");
    let s = sketch(&d);
    assert!(free(&s, &bottom), "{s}");
    assert!(projected(&s, &a) && projected(&s, &b), "{s}");
    let still = linked(&s);
    assert!(!still.contains(&bottom), "{s}");
    assert_eq!(
        still.len(),
        7,
        "three lines and four corners stay linked: {s}"
    );
    // Its region is still there, under the same name.
    assert_eq!(s["regions"].as_array().unwrap().len(), 1);
}

#[test]
fn a_whole_rectangle_converts_and_its_projection_goes() {
    let mut d = projected_block(square(60.0), true);
    let s = sketch(&d);
    let lines: Vec<String> = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "line")
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let result = convert(&mut d, &refs);
    assert_eq!(result["converted"].as_array().unwrap().len(), 8, "{result}");
    assert_eq!(result["kept"], json!([]));
    let s = sketch(&d);
    assert_eq!(s["projections"], json!([]), "{s}");
    for e in s["entities"].as_array().unwrap() {
        assert!(free(&s, e["id"].as_str().unwrap()), "{s}");
    }
    // Free: the rectangle can be dragged.
    let corner = point_id(&s, [60.0, 40.0]);
    command(
        &mut d,
        json!({"cmd": "sketch.drag", "sketch": "F3", "entity": corner, "to": [70, 50]}),
    );
    assert!(near(&entity(&sketch(&d), &corner)["at"], [70.0, 50.0]));
    // One undo step back, and redo.
    command(&mut d, json!({"cmd": "undo"}));
    command(&mut d, json!({"cmd": "undo"}));
    let s = sketch(&d);
    assert_eq!(linked(&s).len(), 8, "{s}");
    assert!(projected(&s, &corner));
    command(&mut d, json!({"cmd": "redo"}));
    let s = sketch(&d);
    assert_eq!(s["projections"], json!([]));
    assert!(free(&s, &corner));
    // Saved and opened again.
    let text = d.to_json();
    let opened = Document::from_json(&text, MockKernel::default()).unwrap();
    let s = sketch(&opened);
    assert_eq!(s["projections"], json!([]));
    assert!(free(&s, &corner), "{s}");
}

#[test]
fn converted_lines_stop_following_while_the_rest_follows() {
    // The bottom and right lines converted: the corner between them goes
    // with them; the top line and its corners follow the block.
    let mut d = projected_block(square(60.0), true);
    let s = sketch(&d);
    let bottom = line_id(&s, [0.0, 0.0], [60.0, 0.0]);
    let right = line_id(&s, [60.0, 0.0], [60.0, 40.0]);
    let top = line_id(&s, [60.0, 40.0], [0.0, 40.0]);
    let between = point_id(&s, [60.0, 0.0]);
    let top_right = point_id(&s, [60.0, 40.0]);
    let result = convert(&mut d, &[&bottom, &right]);
    assert!(ids(&result["converted"]).contains(&between), "{result}");
    assert!(ids(&result["kept"]).contains(&top_right), "{result}");
    *d.kernel().curves.borrow_mut() = square(80.0);
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    let s = sketch(&d);
    assert!(
        near(&entity(&s, &top)["geometry"]["start"], [80.0, 40.0]),
        "{s}"
    );
    assert!(near(&entity(&s, &top_right)["at"], [80.0, 40.0]), "{s}");
    assert!(near(&entity(&s, &between)["at"], [60.0, 0.0]), "{s}");
    // The right line, joined to the corner that follows, stretches to it.
    assert!(
        near(&entity(&s, &right)["geometry"]["end"], [80.0, 40.0]),
        "{s}"
    );
    assert_eq!(s["warnings"], json!([]), "{s}");
}

#[test]
fn a_projected_circle_arc_and_point_convert() {
    let circle = Curve3::Conic {
        center: [20.0, 20.0, 20.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        major: 5.0,
        minor: 5.0,
        start: 0.0,
        end: std::f64::consts::TAU,
        closed: true,
    };
    let arc = Curve3::Conic {
        center: [45.0, 20.0, 20.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        major: 5.0,
        minor: 5.0,
        start: 0.0,
        end: std::f64::consts::FRAC_PI_2,
        closed: false,
    };
    let mut d = projected_block(vec![circle, arc], true);
    let s = sketch(&d);
    let find = |kind: &str| {
        s["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["type"] == kind)
            .unwrap_or_else(|| panic!("no {kind} in {s}"))
            .clone()
    };
    let (c, a) = (find("circle"), find("arc"));
    let (cid, aid) = (c["id"].as_str().unwrap(), a["id"].as_str().unwrap());
    let result = convert(&mut d, &[cid, aid]);
    assert_eq!(result["converted"].as_array().unwrap().len(), 6, "{result}");
    let s = sketch(&d);
    for id in [
        cid,
        aid,
        c["center"].as_str().unwrap(),
        a["start"].as_str().unwrap(),
    ] {
        assert!(free(&s, id), "{id}: {s}");
    }
    assert_eq!(s["projections"], json!([]));
    // A projected vertex alone.
    let mut d = Document::new(MockKernel::default());
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
               "length": 10, "width": 10, "height": 10, "operation": "new_body"}}),
    );
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy", "entities": [
            {"id": "p1", "type": "point", "at": [5.0, 5.0], "fixed": true, "reference": true}],
            "projections": [{"source": "F1:top", "body": "F1.b0", "by_geometry": true,
                             "entities": ["p1"]}]}}),
    );
    let s: Value = serde_json::from_str(
        &d.query(&json!({"query": "sketch", "uid": "F2"}).to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(entity(&s, "p1")["reference"], true);
    let result = command(
        &mut d,
        json!({"cmd": "sketch.convert_projected", "sketch": "F2", "entities": ["p1"]}),
    );
    assert_eq!(result["converted"], json!(["p1"]), "{result}");
}

#[test]
fn an_unlinked_projection_converts() {
    let mut d = projected_block(square(60.0), false);
    let s = sketch(&d);
    assert_eq!(s["projections"], json!([]));
    let bottom = line_id(&s, [0.0, 0.0], [60.0, 0.0]);
    assert!(projected(&s, &bottom));
    convert(&mut d, &[&bottom]);
    assert!(free(&sketch(&d), &bottom));
}

#[test]
fn a_stale_curve_converted_is_no_longer_stale() {
    // A projected circle whose source loses it: it keeps its geometry, and
    // the sketch warns, until it is converted.
    let mut d = Document::new(MockKernel::default());
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
    *d.kernel().curves.borrow_mut() = vec![Curve3::Conic {
        center: [30.0, 20.0, 20.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        major: 5.0,
        minor: 5.0,
        start: 0.0,
        end: std::f64::consts::TAU,
        closed: true,
    }];
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": {"face": FACE},
               "entities": [
                   {"id": "p1", "type": "point", "at": [30.0, 20.0], "fixed": true, "reference": true},
                   {"id": "c2", "type": "circle", "center": "p1", "radius": 5.0,
                    "fixed": true, "reference": true}],
               "projections": [{"source": FACE, "body": "F2.b0", "by_geometry": true,
                                "entities": ["p1", "c2"]}]}}),
    );
    *d.kernel().curves.borrow_mut() = square(80.0);
    command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    let s = sketch(&d);
    assert_eq!(s["projections"][0]["stale"], json!(["c2"]), "{s}");
    assert_eq!(s["warnings"].as_array().unwrap().len(), 1, "{s}");
    convert(&mut d, &["c2"]);
    let s = sketch(&d);
    assert_eq!(s["warnings"], json!([]), "{s}");
    assert_eq!(s["projections"], json!([]), "{s}");
    assert!(free(&s, "c2") && free(&s, "p1"), "{s}");
}

#[test]
fn nothing_projected_is_refused_and_ordinary_geometry_is_left_alone() {
    let mut d = projected_block(square(60.0), true);
    let mine = command(
        &mut d,
        json!({"cmd": "sketch.add_line", "sketch": "F3", "start": [0, -10], "end": [10, -20]}),
    );
    let user = mine["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .find(|v| v.starts_with('c'))
        .unwrap();
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.convert_projected", "sketch": "F3", "entities": [user]}),
    );
    assert!(error.contains("none of the selected entities"), "{error}");
    let s = sketch(&d);
    let bottom = line_id(&s, [0.0, 0.0], [60.0, 0.0]);
    let result = convert(&mut d, &[&user, &bottom]);
    let warnings = result["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .unwrap()
            .contains(&format!("left as it is: {user}"))),
        "{result}"
    );
    // A corner that projected lines that stay use is not converted alone.
    let corner = point_id(&sketch(&d), [0.0, 40.0]);
    let error = rejected(
        &mut d,
        json!({"cmd": "sketch.convert_projected", "sketch": "F3", "entities": [corner]}),
    );
    assert!(error.contains("convert them too"), "{error}");
}

// --- Projected curves keep their shape (mitcad#357)

#[test]
fn trim_extend_fillet_and_chamfer_refuse_projected_curves_until_converted() {
    for linked in [true, false] {
        let mut d = projected_block(square(60.0), linked);
        let s = sketch(&d);
        let bottom = line_id(&s, [0.0, 0.0], [60.0, 0.0]);
        let right = line_id(&s, [60.0, 0.0], [60.0, 40.0]);
        // A line of the user's across the bottom edge.
        command(
            &mut d,
            json!({"cmd": "sketch.add_line", "sketch": "F3", "start": [30, -10], "end": [30, 10]}),
        );
        let attempts = [
            json!({"cmd": "sketch.trim", "sketch": "F3", "curve": bottom, "at": [10, 0]}),
            json!({"cmd": "sketch.extend", "sketch": "F3", "curve": bottom, "at": [59, 0]}),
            json!({"cmd": "sketch.fillet", "sketch": "F3", "a": bottom, "b": right, "radius": 5}),
            json!({"cmd": "sketch.chamfer", "sketch": "F3", "a": bottom, "b": right,
                   "distance": 5}),
        ];
        let before = sketch(&d);
        for attempt in attempts {
            let error = rejected(&mut d, attempt.clone());
            assert!(
                error.contains("sketch.convert_projected"),
                "{attempt}: {error}"
            );
        }
        assert_eq!(
            sketch(&d)["entities"],
            before["entities"],
            "nothing changed"
        );
        // Converted, the line is free geometry and trims.
        convert(&mut d, &[&bottom]);
        command(
            &mut d,
            json!({"cmd": "sketch.trim", "sketch": "F3", "curve": bottom, "at": [10, 0]}),
        );
    }
}
