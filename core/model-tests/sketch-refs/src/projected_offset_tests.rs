// SPDX-License-Identifier: MIT
//! Offsets of projected outlines (mitcad#741), with the mock kernel: a face
//! and one of its edges projected make one outline, an existing sketch with
//! an edge projected twice offsets the outline, ends a model's edges meet
//! at within its tolerance share a point, and an offset that cannot be made
//! says why.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::kernel::Curve3;
use mitcad_model::testing::MockKernel;
use mitcad_model::topo::TopoName;
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

fn entity(s: &Value, id: &str) -> Value {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no {id} in {s}"))
        .clone()
}

/// The ids of the sketch's entities of a type.
fn of_type(s: &Value, kind: &str) -> Vec<String> {
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == kind)
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect()
}

/// The id of the line from `a` to `b` (either way round).
fn line_id(s: &Value, a: [f64; 2], b: [f64; 2]) -> String {
    let near = |p: &Value, q: [f64; 2]| {
        close(p[0].as_f64().unwrap(), q[0]) && close(p[1].as_f64().unwrap(), q[1])
    };
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

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The top face of the block.
const FACE: &str = "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})";

/// The top face's edge over the block's side on `curve` (c1 bottom, c2
/// right, c3 top, c4 left), as the kernel names it.
fn edge(side: &str) -> String {
    let neighbours = match side {
        "c1" => "c1[c4,c2]",
        "c2" => "c2[c1,c3]",
        "c3" => "c3[c2,c4]",
        _ => "c4[c3,c1]",
    };
    format!("E{{{FACE}|F2:side({neighbours})}}")
        .parse::<TopoName>()
        .unwrap()
        .to_string()
}

fn line(a: [f64; 2], b: [f64; 2]) -> Curve3 {
    Curve3::Line {
        start: [a[0], a[1], 20.0],
        end: [b[0], b[1], 20.0],
    }
}

/// The top face's edges for a block `width` wide, counter-clockwise.
fn square(width: f64) -> Vec<Curve3> {
    let c = [[0.0, 0.0], [width, 0.0], [width, 40.0], [0.0, 40.0]];
    (0..4).map(|i| line(c[i], c[(i + 1) % 4])).collect()
}

/// A 60 x 40 block 20 high (F1, F2) and a sketch on its top (F3).
fn block() -> Document<MockKernel> {
    let mut d = body();
    command(
        &mut d,
        json!({"cmd": "sketch.create", "plane": {"face": FACE}}),
    );
    d
}

/// The block alone.
fn body() -> Document<MockKernel> {
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
    d
}

fn project(d: &mut Document<MockKernel>, source: &str, linked: bool) -> Value {
    command(
        d,
        json!({"cmd": "sketch.project", "sketch": "F3", "source": source, "body": "F2.b0",
               "linked": linked}),
    )
}

fn offset(d: &mut Document<MockKernel>, curves: &[&str], distance: f64) -> Value {
    command(
        d,
        json!({"cmd": "sketch.offset", "sketch": "F3", "curves": curves, "distance": distance}),
    )
}

/// The last offset of the sketch: its source chain and its results, and
/// whether the results close (the first one starts where the last ends).
fn last_offset(d: &Document<MockKernel>) -> (Vec<String>, Vec<String>, bool) {
    let s = sketch(d);
    let o = s["offsets"].as_array().unwrap().last().unwrap().clone();
    let (curves, results) = (strings(&o["curves"]), strings(&o["results"]));
    let first = entity(&s, &results[0]);
    let last = entity(&s, results.last().unwrap());
    let ends = |e: &Value| [e["start"].clone(), e["end"].clone()];
    let closed = ends(&first).iter().any(|p| ends(&last).contains(p));
    (curves, results, closed)
}

fn mentions(result: &Value, text: &str) -> bool {
    strings(&result["warnings"])
        .iter()
        .any(|w| w.contains(text))
}

#[test]
fn a_face_and_then_one_of_its_edges_make_one_outline() {
    for linked in [true, false] {
        let mut d = block();
        *d.kernel().curves.borrow_mut() = square(60.0);
        project(&mut d, FACE, linked);
        let right = edge("c2");
        d.kernel()
            .named_curves
            .borrow_mut()
            .insert(right.clone(), line([60.0, 0.0], [60.0, 40.0]));
        let again = project(&mut d, &right, linked);
        // Nothing new: the edge's line is the face's.
        assert_eq!(again["entities"], json!([]), "{again}");
        assert!(mentions(&again, "not projected again"), "{again}");
        let s = sketch(&d);
        assert_eq!(of_type(&s, "line").len(), 4, "{s}");
        assert_eq!(of_type(&s, "point").len(), 4, "{s}");
        let projections = s["projections"].as_array().unwrap().len();
        assert_eq!(projections, usize::from(linked), "{s}");
        // The outline offsets from any of its lines first.
        let lines = of_type(&s, "line");
        for k in 0..4 {
            let order: Vec<&str> = (0..4).map(|i| lines[(k + i) % 4].as_str()).collect();
            let made = offset(&mut d, &order, 2.0);
            assert_eq!(made["warnings"], Value::Null, "{made}");
            let (curves, results, closed) = last_offset(&d);
            assert_eq!(curves.len(), 4, "{curves:?}");
            assert_eq!(results.len(), 4);
            assert!(closed, "linked {linked}, from {}", order[0]);
            command(&mut d, json!({"cmd": "undo"}));
        }
    }
}

#[test]
fn an_edge_and_then_its_face_share_the_edge_and_follow_the_block() {
    let mut d = block();
    let bottom = edge("c1");
    d.kernel()
        .named_curves
        .borrow_mut()
        .insert(bottom.clone(), line([0.0, 0.0], [60.0, 0.0]));
    project(&mut d, &bottom, true);
    *d.kernel().curves.borrow_mut() = square(60.0);
    let face = project(&mut d, FACE, true);
    // Three lines and two corners of its own; the bottom line and its ends
    // are the edge's.
    assert_eq!(strings(&face["entities"]).len(), 5, "{face}");
    let s = sketch(&d);
    let lines = of_type(&s, "line");
    assert_eq!(lines.len(), 4, "{s}");
    assert_eq!(of_type(&s, "point").len(), 4, "{s}");
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    offset(&mut d, &refs, 2.0);
    let (_, results, closed) = last_offset(&d);
    assert!(results.len() == 4 && closed);
    command(&mut d, json!({"cmd": "undo"}));
    // Both follow the block: one outline, no warning.
    d.kernel()
        .named_curves
        .borrow_mut()
        .insert(bottom, line([0.0, 0.0], [80.0, 0.0]));
    *d.kernel().curves.borrow_mut() = square(80.0);
    let changed = command(
        &mut d,
        json!({"cmd": "set_parameter", "name": "d1", "value": 80}),
    );
    assert_eq!(changed["error"], Value::Null, "{changed}");
    let s = sketch(&d);
    assert_eq!(s["warnings"], json!([]), "{s}");
    line_id(&s, [0.0, 0.0], [80.0, 0.0]);
    line_id(&s, [80.0, 0.0], [80.0, 40.0]);
    let areas: Vec<f64> = s["regions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.get("area").and_then(Value::as_f64).unwrap_or(0.0))
        .collect();
    assert!(areas.len() == 1 && close(areas[0], 3200.0), "{areas:?}");
}

/// A sketch on the top face as an earlier version left it: the face
/// projected (four lines sharing their corners) after its right edge, the
/// edge's line (c3, the lowest id) with points of its own.
fn with_a_doubled_edge() -> Document<MockKernel> {
    let mut d = body();
    *d.kernel().curves.borrow_mut() = square(60.0);
    let fixed = |e: Value| {
        let mut e = e;
        e["fixed"] = json!(true);
        e["reference"] = json!(true);
        e
    };
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": {"face": FACE},
               "entities": [
                   fixed(json!({"id": "p1", "type": "point", "at": [60.0, 0.0]})),
                   fixed(json!({"id": "p2", "type": "point", "at": [60.0, 40.0]})),
                   fixed(json!({"id": "c3", "type": "line", "start": "p1", "end": "p2"})),
                   fixed(json!({"id": "p4", "type": "point", "at": [0.0, 0.0]})),
                   fixed(json!({"id": "p5", "type": "point", "at": [60.0, 0.0]})),
                   fixed(json!({"id": "p6", "type": "point", "at": [60.0, 40.0]})),
                   fixed(json!({"id": "p7", "type": "point", "at": [0.0, 40.0]})),
                   fixed(json!({"id": "c8", "type": "line", "start": "p4", "end": "p5"})),
                   fixed(json!({"id": "c9", "type": "line", "start": "p5", "end": "p6"})),
                   fixed(json!({"id": "c10", "type": "line", "start": "p6", "end": "p7"})),
                   fixed(json!({"id": "c11", "type": "line", "start": "p7", "end": "p4"}))],
               "projections": [
                   {"source": edge("c2"), "body": "F2.b0", "by_geometry": true,
                    "entities": ["p1", "p2", "c3"]},
                   {"source": FACE, "body": "F2.b0", "by_geometry": true,
                    "entities": ["p4", "p5", "p6", "p7", "c8", "c9", "c10", "c11"]}]}}),
    );
    assert_eq!(sketch(&d)["uid"], "F3");
    d
}

#[test]
fn an_edge_projected_twice_offsets_the_outline_however_its_lines_are_picked() {
    let mut d = with_a_doubled_edge();
    // All five lines: the repeat counts once.
    let made = offset(&mut d, &["c3", "c8", "c9", "c10", "c11"], 2.0);
    assert!(mentions(&made, "c3 lies on c9"), "{made}");
    assert!(mentions(&made, "Clean Up Projections"), "{made}");
    let (curves, results, closed) = last_offset(&d);
    assert_eq!(curves.len(), 4, "{curves:?}");
    assert!(!curves.contains(&"c3".to_owned()));
    assert!(results.len() == 4 && closed);
    command(&mut d, json!({"cmd": "undo"}));
    // The repeat picked in place of the face's line: the face's line is
    // offset, as it joins the others.
    let made = offset(&mut d, &["c8", "c3", "c10", "c11"], 2.0);
    assert!(mentions(&made, "c3 lies on c9"), "{made}");
    let (curves, results, closed) = last_offset(&d);
    assert!(curves.contains(&"c9".to_owned()) && !curves.contains(&"c3".to_owned()));
    assert!(results.len() == 4 && closed, "{curves:?}");
    command(&mut d, json!({"cmd": "undo"}));
    // The repeat alone is the one line picked; the panel says that its
    // ends are not the outline's.
    let made = offset(&mut d, &["c3"], 2.0);
    assert!(
        mentions(
            &made,
            "the chain ends at p1, where c8 ends at the same place"
        ),
        "{made}"
    );
    let (curves, results, _) = last_offset(&d);
    assert_eq!((curves.len(), results.len()), (1, 1));
}

#[test]
fn ends_a_model_meets_within_its_tolerance_share_a_point() {
    // The right edge starts 1e-6 mm beside where the bottom edge ends (a
    // body's edges meet within its tolerance).
    let mut d = block();
    let mut curves = square(60.0);
    curves[1] = line([60.0 + 1e-6, 0.0], [60.0, 40.0]);
    *d.kernel().curves.borrow_mut() = curves;
    project(&mut d, FACE, true);
    let s = sketch(&d);
    assert_eq!(of_type(&s, "point").len(), 4, "{s}");
    let lines = of_type(&s, "line");
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let made = offset(&mut d, &refs, 2.0);
    assert_eq!(made["warnings"], Value::Null, "{made}");
    let (_, results, closed) = last_offset(&d);
    assert!(results.len() == 4 && closed);
}

#[test]
fn an_outline_with_arcs_and_a_spline_and_its_arc_projected_again_offsets() {
    // A rounded corner (a quarter circle from (50, 0) to (60, 10)) and a
    // curved top edge (a B-spline from (60, 40) to (0, 40)).
    let arc = Curve3::Conic {
        center: [50.0, 10.0, 20.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [1.0, 0.0, 0.0],
        major: 10.0,
        minor: 10.0,
        start: -std::f64::consts::FRAC_PI_2,
        end: 0.0,
        closed: false,
    };
    let top = Curve3::BSpline {
        degree: 3,
        poles: vec![
            [60.0, 40.0, 20.0],
            [40.0, 50.0, 20.0],
            [20.0, 50.0, 20.0],
            [0.0, 40.0, 20.0],
        ],
        weights: Vec::new(),
        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    };
    let outline = vec![
        line([0.0, 0.0], [50.0, 0.0]),
        arc.clone(),
        line([60.0, 10.0], [60.0, 40.0]),
        top,
        line([0.0, 40.0], [0.0, 0.0]),
    ];
    let mut d = block();
    *d.kernel().curves.borrow_mut() = outline;
    project(&mut d, FACE, true);
    let corner = edge("c2");
    d.kernel()
        .named_curves
        .borrow_mut()
        .insert(corner.clone(), arc);
    let again = project(&mut d, &corner, true);
    assert_eq!(again["entities"], json!([]), "{again}");
    let s = sketch(&d);
    assert_eq!(of_type(&s, "arc").len(), 1, "{s}");
    let mut chain = of_type(&s, "line");
    chain.extend(of_type(&s, "arc"));
    chain.extend(of_type(&s, "spline"));
    assert_eq!(chain.len(), 5, "{s}");
    let refs: Vec<&str> = chain.iter().map(String::as_str).collect();
    // With the spline the offset is derived; its curves close the outline.
    let made = offset(&mut d, &refs, 2.0);
    assert_eq!(made["warnings"], Value::Null, "{made}");
    let s = sketch(&d);
    let o = s["offsets"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(o["derived"], true, "{o}");
    assert_eq!(strings(&o["curves"]).len(), 5, "{o}");
    assert_eq!(strings(&o["results"]).len(), 5, "{o}");
}

#[test]
fn an_offset_that_cannot_be_made_says_why() {
    // c3 from p1 to p2; c6 from p4, at p2's place but a point of its own;
    // c8 and c9 from p2.
    let mut d = Document::new(MockKernel::default());
    command(
        &mut d,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy", "entities": [
            {"id": "p1", "type": "point", "at": [0.0, 0.0]},
            {"id": "p2", "type": "point", "at": [10.0, 0.0]},
            {"id": "c3", "type": "line", "start": "p1", "end": "p2"},
            {"id": "p4", "type": "point", "at": [10.0, 0.0]},
            {"id": "p5", "type": "point", "at": [10.0, 10.0]},
            {"id": "c6", "type": "line", "start": "p4", "end": "p5"},
            {"id": "p7", "type": "point", "at": [0.0, 10.0]},
            {"id": "c8", "type": "line", "start": "p2", "end": "p7"},
            {"id": "p10", "type": "point", "at": [20.0, 5.0]},
            {"id": "c9", "type": "line", "start": "p2", "end": "p10"}]}}),
    );
    let offset = |curves: &[&str]| json!({"cmd": "sketch.offset", "sketch": "F1", "curves": curves, "distance": 1});
    // Apart: the reason names the ends and the constraint that joins them.
    let error = rejected(&mut d, offset(&["c3", "c6"]));
    assert!(
        error.contains("c6 ends at p2 of c3 but on a point of its own (p4)")
            && error.contains("a Coincident constraint joins them"),
        "{error}"
    );
    // Three of them end at p2.
    let error = rejected(&mut d, offset(&["c3", "c8", "c9"]));
    assert!(
        error.contains("3 of them end at p2 (it branches there)"),
        "{error}"
    );
    // One line alone is offset, and the panel says where it stops short.
    let made = command(&mut d, offset(&["c3"]));
    assert!(
        mentions(
            &made,
            "the chain ends at p2, where c6 ends at the same place on a point of its own (p4)"
        ),
        "{made}"
    );
}

/// The ids of the lines from `a` to `b` (either way round).
fn line_ids(s: &Value, a: [f64; 2], b: [f64; 2]) -> Vec<String> {
    let near = |p: &Value, q: [f64; 2]| {
        close(p[0].as_f64().unwrap(), q[0]) && close(p[1].as_f64().unwrap(), q[1])
    };
    s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| {
            let g = &e["geometry"];
            e["type"] == "line"
                && ((near(&g["start"], a) && near(&g["end"], b))
                    || (near(&g["start"], b) && near(&g["end"], a)))
        })
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn two_faces_that_share_an_edge_keep_their_outlines() {
    // The top face, then a face beside it (unlinked: the mock kernel gives
    // every face the same curves) whose left edge is the top face's right
    // edge, the other way round. Sharing the edge's line would make three
    // lines end at its corners, and Chain Selection would stop there: each
    // face keeps its outline.
    let mut d = block();
    *d.kernel().curves.borrow_mut() = square(60.0);
    project(&mut d, FACE, false);
    let c = [[60.0, 0.0], [90.0, 0.0], [90.0, 40.0], [60.0, 40.0]];
    *d.kernel().curves.borrow_mut() = (0..4).map(|i| line(c[i], c[(i + 1) % 4])).collect();
    let beside = project(&mut d, "F2:side(c2[c1,c3])", false);
    assert_eq!(beside["warnings"], Value::Null, "{beside}");
    let s = sketch(&d);
    assert_eq!(of_type(&s, "line").len(), 8, "{s}");
    assert_eq!(of_type(&s, "point").len(), 8, "{s}");
    let shared = line_ids(&s, [60.0, 0.0], [60.0, 40.0]);
    assert_eq!(shared.len(), 2, "{s}");
    // Each outline offsets, whichever of the two lines on the edge is
    // listed: the one that joins the others is taken.
    let first = |shared: &str| {
        vec![
            line_id(&s, [0.0, 0.0], [60.0, 0.0]),
            shared.to_owned(),
            line_id(&s, [60.0, 40.0], [0.0, 40.0]),
            line_id(&s, [0.0, 40.0], [0.0, 0.0]),
        ]
    };
    let second = |shared: &str| {
        vec![
            line_id(&s, [60.0, 0.0], [90.0, 0.0]),
            line_id(&s, [90.0, 0.0], [90.0, 40.0]),
            line_id(&s, [90.0, 40.0], [60.0, 40.0]),
            shared.to_owned(),
        ]
    };
    for outline in [
        first(&shared[0]),
        first(&shared[1]),
        second(&shared[0]),
        second(&shared[1]),
    ] {
        let refs: Vec<&str> = outline.iter().map(String::as_str).collect();
        offset(&mut d, &refs, 2.0);
        let (curves, results, closed) = last_offset(&d);
        assert!(
            curves.len() == 4 && results.len() == 4 && closed,
            "{outline:?}"
        );
        command(&mut d, json!({"cmd": "undo"}));
    }
}
