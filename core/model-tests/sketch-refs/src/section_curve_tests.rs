// SPDX-License-Identifier: MIT
//! Section curves (mitcad#389) with the mock kernel, whose section of a
//! face is where the plane cuts the body's bounds: `sketch.project` with
//! `section` adds the curves where the sketch's plane cuts a face, a
//! linked section is recomputed from the body when it changes, also
//! through an occurrence link, and the design file keeps it. A projection
//! of a chain of edges is one spline, the edges joined (mitcad#389).

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::features::SketchPlane;
use mitcad_model::ids::{BodyUid, ComponentUid, OccurrenceUid};
use mitcad_model::kernel::Curve3;
use mitcad_model::sketch::project::join_curves;
use mitcad_model::test_support::allow_attachments;
use mitcad_model::test_support::{extrude, num};
use mitcad_model::testing::MockKernel;
use mitcad_model::transform::Transform;
use mitcad_model_api::DocumentApi;

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    serde_json::from_str(&doc.command(&command.to_string()).unwrap()).unwrap()
}

fn rejected(doc: &mut Document<MockKernel>, command: Value) -> String {
    doc.command(&command.to_string()).unwrap_err().to_string()
}

/// A 10 x 10 x 5 block round the active component's Z axis: its body and
/// top face.
fn block(doc: &mut Document<MockKernel>) -> (BodyUid, String) {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [-5.0, -5.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let uid = doc
        .add_feature(&extrude(sketch, &[&region], 5.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    (BodyUid::new(uid, 0), format!("{uid}:end({region})"))
}

/// The sketch's points, sorted.
fn points(doc: &Document<MockKernel>, sketch: &Value) -> Vec<[f64; 2]> {
    let s = query(doc, json!({"query": "sketch", "uid": sketch}));
    let mut at: Vec<[f64; 2]> = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "point")
        .map(|e| [e["at"][0].as_f64().unwrap(), e["at"][1].as_f64().unwrap()])
        .collect();
    at.sort_by(|a, b| a.partial_cmp(b).unwrap());
    at
}

fn near(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(p, q)| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9)
}

#[test]
fn section_curves_follow_the_body_they_cut() {
    let mut doc = Document::new(MockKernel::default());
    let (body, face) = block(&mut doc);
    // On XZ (x = +X, y = -Z): the block's section is the rectangle from
    // (-5, 0) to (5, -5).
    let sketch = command(&mut doc, json!({"cmd": "sketch.create", "plane": "xz"}))["uid"].clone();
    command(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": sketch, "source": face, "body": body,
               "linked": true, "section": true}),
    );
    let s = query(&doc, json!({"query": "sketch", "uid": sketch}));
    let lines = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "line")
        .count();
    assert_eq!(lines, 4, "{s}");
    assert_eq!(s["projections"][0]["section"], true, "{s}");
    assert_eq!(s["projections"][0]["source"], json!(face));
    let rectangle = |depth: f64| vec![[-5.0, -depth], [-5.0, 0.0], [5.0, -depth], [5.0, 0.0]];
    assert!(near(&points(&doc, &sketch), &rectangle(5.0)), "{s}");

    // The block grows to 8: the section follows it.
    let d = doc
        .parameters()
        .iter()
        .find(|p| p.comment().ends_with("distance"))
        .unwrap()
        .name()
        .to_owned();
    doc.set_parameter(&d, 8.0).unwrap();
    assert!(near(&points(&doc, &sketch), &rectangle(8.0)));
    let uid = sketch.as_str().unwrap().parse().unwrap();
    assert!(doc.status(uid).unwrap().is_ok(), "{:?}", doc.status(uid));
    assert_eq!(doc.warnings(uid), Vec::<String>::new());

    // The design file keeps it a section.
    let saved = doc.to_json();
    assert!(saved.contains("\"section\": true"), "{saved}");
    let mut loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    assert_eq!(loaded.to_json(), saved);
    loaded.recompute();
    assert!(near(&points(&loaded, &sketch), &rectangle(8.0)));
}

#[test]
fn a_section_is_of_a_face_the_plane_cuts() {
    let mut doc = Document::new(MockKernel::default());
    let (body, face) = block(&mut doc);
    let sketch = command(&mut doc, json!({"cmd": "sketch.create", "plane": "xz"}))["uid"].clone();
    // A face, an edge or a body has a section, a vertex not.
    let vertex = json!(format!("V{{{face}|{face}|{face}}}"));
    let error = rejected(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": sketch, "source": vertex, "body": body,
               "section": true}),
    );
    assert!(
        error.contains("a section's source is a face, an edge or a body"),
        "{error}"
    );
    // A projection needs its source.
    let error = rejected(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": sketch, "body": body}),
    );
    assert!(error.contains("needs a source"), "{error}");
    // A plane that misses the body cuts nothing.
    let away = command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "construction_plane",
               "definition": {"type": "offset", "plane": "xz", "distance": 20}}}),
    )["uid"]
        .clone();
    let far = command(&mut doc, json!({"cmd": "sketch.create", "plane": away}))["uid"].clone();
    let error = rejected(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": far, "source": face, "body": body,
               "section": true}),
    );
    assert!(error.contains("does not cut it"), "{error}");
}

#[test]
fn a_section_of_another_components_body_follows_its_occurrence() {
    // A:1 (O1) at (100, 50, 10) holds the block; the sketch is on B:1's
    // (O2, at (0, 50, -3)) XZ plane, which is y = 50 in the design and
    // cuts the block through its middle.
    let mut doc = Document::new(MockKernel::default());
    doc.create_component(Some("A"), Transform::translation([100.0, 50.0, 10.0]), true)
        .unwrap();
    let (body, face) = block(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    doc.create_component(Some("B"), Transform::translation([0.0, 50.0, -3.0]), true)
        .unwrap();
    allow_attachments(&mut doc);
    let sketch = command(&mut doc, json!({"cmd": "sketch.create", "plane": "xz"}))["uid"].clone();
    command(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": sketch, "source": face, "body": body,
               "linked": true, "section": true, "occurrence": "O1"}),
    );
    let s = query(&doc, json!({"query": "sketch", "uid": sketch}));
    assert_eq!(
        s["projections"][0]["link"],
        json!({"source": "O1", "target": "O2"})
    );
    // The block from z = 10 to 15 in the design is 13 to 18 in B, x from
    // 95 to 105.
    let section = |z: f64| vec![[95.0, -z - 5.0], [95.0, -z], [105.0, -z - 5.0], [105.0, -z]];
    assert!(near(&points(&doc, &sketch), &section(13.0)), "{s}");
    // A moves 10 up: the section follows.
    doc.set_occurrence_transform(
        OccurrenceUid(1),
        Transform::translation([100.0, 50.0, 20.0]),
        false,
    )
    .unwrap();
    assert!(near(&points(&doc, &sketch), &section(23.0)));
}

fn quadratic(poles: [[f64; 3]; 3]) -> Curve3 {
    Curve3::BSpline {
        degree: 2,
        poles: poles.to_vec(),
        weights: Vec::new(),
        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    }
}

#[test]
fn pieces_end_to_end_join_into_one_spline() {
    let a = quadratic([[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]]);
    // The second runs the other way round: it is turned.
    let b = quadratic([[4.0, 0.0, 0.0], [3.0, -1.0, 0.0], [2.0, 0.0, 0.0]]);
    let Some(Curve3::BSpline {
        degree,
        poles,
        knots,
        ..
    }) = join_curves(&[a.clone(), b.clone()])
    else {
        panic!("not joined");
    };
    assert_eq!(degree, 2);
    assert_eq!(
        poles,
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, -1.0, 0.0],
            [4.0, 0.0, 0.0]
        ]
    );
    assert_eq!(knots, vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
    // A line joins raised to the splines' degree.
    let line = Curve3::Line {
        start: [4.0, 0.0, 0.0],
        end: [6.0, 0.0, 0.0],
    };
    let Some(Curve3::BSpline { poles, knots, .. }) = join_curves(&[a.clone(), b.clone(), line])
    else {
        panic!("not joined");
    };
    assert_eq!(poles.len(), 7);
    assert_eq!(poles[5], [5.0, 0.0, 0.0]);
    assert_eq!(knots.len(), 10);
    // Pieces apart do not join; nor do conics.
    let far = quadratic([[2.1, 0.0, 0.0], [3.0, -1.0, 0.0], [4.0, 0.0, 0.0]]);
    assert!(join_curves(&[a.clone(), far]).is_none());
    let arc = Curve3::Conic {
        center: [3.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        x_axis: [-1.0, 0.0, 0.0],
        major: 1.0,
        minor: 1.0,
        start: 0.0,
        end: std::f64::consts::PI,
        closed: false,
    };
    assert!(join_curves(&[a, arc]).is_none());
}

/// A linked projection of a chain of edges (an import's projection of a
/// helix the model splits into a piece per turn): one spline, the edges'
/// projections joined, following them.
#[test]
fn a_projection_of_a_chain_of_edges_is_one_spline_following_them() {
    let mut doc = Document::new(MockKernel::default());
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let extrude = doc
        .add_feature(&extrude(sketch, &[&region], 5.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    let edge = |i: usize| mitcad_model::test_support::top_edge(extrude, 1, &region, i).to_string();
    let pieces = |lift: f64| {
        [
            (
                edge(0),
                quadratic([[0.0, 0.0, 5.0], [1.0, 1.0 + lift, 5.0], [2.0, 0.0, 5.0]]),
            ),
            (
                edge(1),
                quadratic([[2.0, 0.0, 5.0], [3.0, -1.0, 5.0], [4.0, 0.0, 5.0]]),
            ),
        ]
    };
    doc.kernel().named_curves.borrow_mut().extend(pieces(0.0));
    // The spline stored a little off its source, as a file's numbers are.
    let point = |id: &str, x: f64, y: f64| json!({"id": id, "type": "point", "at": [x, y], "fixed": true, "reference": true});
    let added = command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "plane": "xy",
               "entities": [point("p1", 0.0, 0.0), point("p2", 1.0, 1.1), point("p3", 2.0, 0.0),
                            point("p4", 3.0, -1.1), point("p5", 4.0, 0.0),
                            {"id": "c6", "type": "spline", "degree": 2,
                             "control": ["p1", "p2", "p3", "p4", "p5"],
                             "knots": [0, 0, 0, 1, 1, 2, 2, 2], "fixed": true, "reference": true}],
               "projections": [{"source": edge(0), "chain": [edge(1)], "body": "F2.b0",
                                "entities": ["p1", "p2", "p3", "p4", "p5", "c6"]}]}}),
    );
    let uid = added["uid"].clone();
    let control = |doc: &Document<MockKernel>| -> Vec<[f64; 2]> {
        let s = query(doc, json!({"query": "sketch", "uid": uid}));
        ["p2", "p4"]
            .iter()
            .map(|id| {
                let e = s["entities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|e| e["id"] == *id)
                    .unwrap()
                    .clone();
                [e["at"][0].as_f64().unwrap(), e["at"][1].as_f64().unwrap()]
            })
            .collect()
    };
    assert!(
        near(&control(&doc), &[[1.0, 1.0], [3.0, -1.0]]),
        "{:?}",
        control(&doc)
    );
    let s = query(&doc, json!({"query": "sketch", "uid": uid}));
    assert_eq!(s["projections"][0]["chain"], json!([edge(1)]), "{s}");
    // The first piece moves (with the body): the spline follows.
    doc.kernel().named_curves.borrow_mut().extend(pieces(0.5));
    let d = doc
        .parameters()
        .iter()
        .find(|p| p.comment().ends_with("distance"))
        .unwrap()
        .name()
        .to_owned();
    doc.set_parameter(&d, 5.5).unwrap();
    assert!(
        near(&control(&doc), &[[1.0, 1.5], [3.0, -1.0]]),
        "{:?}",
        control(&doc)
    );
    // A piece gone: the spline keeps its stored geometry and warns.
    doc.kernel().named_curves.borrow_mut().remove(&edge(1));
    doc.set_parameter(&d, 6.0).unwrap();
    assert!(
        near(&control(&doc), &[[1.0, 1.1], [3.0, -1.1]]),
        "{:?}",
        control(&doc)
    );
    let sketch_uid = uid.as_str().unwrap().parse().unwrap();
    assert!(
        doc.warnings(sketch_uid)
            .iter()
            .any(|w| w.contains("does not follow it")),
        "{:?}",
        doc.warnings(sketch_uid)
    );
}

/// A planar spline seen edge on projects to the line between its extreme
/// points, as a conic does (mitcad#389: a thread's end edge on a sketch
/// through the thread's axis).
#[test]
fn a_spline_seen_edge_on_is_a_line_between_its_extremes() {
    use mitcad_model::profile::SketchFrame;
    use mitcad_model::sketch::geometry::Curve2;
    use mitcad_model::sketch::project::{Projected, project};
    let line = |poles: [[f64; 3]; 3]| match project(&[quadratic(poles)], &SketchFrame::XY).pop() {
        Some(Projected::Curve(Curve2::Line { a, b })) => Some((a, b)),
        _ => None,
    };
    // In the plane x = 0, seen along z: from end to end.
    let (a, b) = line([[0.0, 0.0, 0.0], [0.0, 1.0, 2.0], [0.0, 3.0, 0.0]]).unwrap();
    assert!(near(&[a, b], &[[0.0, 0.0], [0.0, 3.0]]), "{a:?} {b:?}");
    // Turning back inside: to its farthest point, 16/7 at t = 4/7.
    let (a, b) = line([[0.0, 0.0, 0.0], [0.0, 4.0, 2.0], [0.0, 1.0, 0.0]]).unwrap();
    assert!(
        near(&[a, b], &[[0.0, 0.0], [0.0, 16.0 / 7.0]]),
        "{a:?} {b:?}"
    );
    // Not edge on: a spline.
    assert!(line([[0.0, 0.0, 0.0], [1.0, 1.0, 2.0], [0.0, 3.0, 0.0]]).is_none());
}

// Intersect (mitcad#736): the section of a whole body and the points where
// the plane crosses edges.

/// The block's extrusion depth parameter.
fn depth(doc: &Document<MockKernel>) -> String {
    doc.parameters()
        .iter()
        .find(|p| p.comment().ends_with("distance"))
        .unwrap()
        .name()
        .to_owned()
}

/// A whole body: every face the plane cuts gives a linked section of its
/// own (the mock puts side k of its section on the body's face k), in one
/// undo step, and they all follow the body. The faces' lines share the
/// corners where they meet (mitcad#741): a closed outline.
#[test]
fn a_whole_bodys_section_is_each_faces_section_following_it() {
    let mut doc = Document::new(MockKernel::default());
    let (body, _) = block(&mut doc);
    let sketch = command(&mut doc, json!({"cmd": "sketch.create", "plane": "xz"}))["uid"].clone();
    let result = command(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": sketch, "body": body,
               "linked": true, "section": true}),
    );
    assert!(result["error"].is_null(), "{result}");
    let s = query(&doc, json!({"query": "sketch", "uid": sketch}));
    let lines = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "line")
        .count();
    assert_eq!(lines, 4, "{s}");
    let projections = s["projections"].as_array().unwrap();
    assert_eq!(projections.len(), 4, "{s}");
    let mut sources: Vec<String> = Vec::new();
    for p in projections {
        assert_eq!(p["section"], true, "{p}");
        assert_eq!(p["body"], json!(body.to_string()), "{p}");
        let source = p["source"].as_str().unwrap().to_owned();
        assert!(
            !source.starts_with("E{") && !source.starts_with("V{"),
            "{p}"
        );
        assert!(!sources.contains(&source), "{s}");
        sources.push(source);
        // A line and those of its ends no earlier face's line has.
        let entities = p["entities"].as_array().unwrap();
        let curves = entities
            .iter()
            .filter(|e| e.as_str().is_some_and(|e| e.starts_with('c')))
            .count();
        assert!(curves == 1 && entities.len() <= 3, "{p}");
    }
    // Each corner of the section is one point, the end of two faces' lines.
    let corners = |depth: f64| vec![[-5.0, -depth], [-5.0, 0.0], [5.0, -depth], [5.0, 0.0]];
    assert!(near(&points(&doc, &sketch), &corners(5.0)), "{s}");
    // One undo step takes it all back.
    assert!(
        doc.undo_label()
            .is_some_and(|l| l.starts_with("Section Curves to")),
        "{:?}",
        doc.undo_label()
    );

    // The block grows to 8: every face's section follows it.
    let d = depth(&doc);
    doc.set_parameter(&d, 8.0).unwrap();
    assert!(near(&points(&doc, &sketch), &corners(8.0)));
    let uid = sketch.as_str().unwrap().parse().unwrap();
    assert!(doc.status(uid).unwrap().is_ok(), "{:?}", doc.status(uid));
    assert_eq!(doc.warnings(uid), Vec::<String>::new());

    // Unlinked: the same curves without records.
    let other = command(&mut doc, json!({"cmd": "sketch.create", "plane": "xz"}))["uid"].clone();
    command(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": other, "body": body, "section": true}),
    );
    let s = query(&doc, json!({"query": "sketch", "uid": other}));
    assert!(
        s["projections"].as_array().is_none_or(|p| p.is_empty()),
        "{s}"
    );
    assert!(near(&points(&doc, &other), &corners(8.0)), "{s}");
}

#[test]
fn a_whole_body_the_plane_misses_is_refused() {
    let mut doc = Document::new(MockKernel::default());
    let (body, _) = block(&mut doc);
    let away = command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "construction_plane",
               "definition": {"type": "offset", "plane": "xz", "distance": 20}}}),
    )["uid"]
        .clone();
    let far = command(&mut doc, json!({"cmd": "sketch.create", "plane": away}))["uid"].clone();
    let error = rejected(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": far, "body": body, "section": true}),
    );
    assert!(error.contains("does not cut the body"), "{error}");
}

/// An edge: the points where the plane crosses it, linked, following the
/// edge; an edge the plane misses is refused.
#[test]
fn an_edges_crossing_points_follow_it() {
    let mut doc = Document::new(MockKernel::default());
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [-5.0, -5.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let extrude = doc
        .add_feature(&extrude(sketch, &[&region], 5.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    let body = BodyUid::new(extrude, 0);
    let edge = |i: usize| mitcad_model::test_support::top_edge(extrude, 1, &region, i).to_string();
    // Edge 0 runs across the XZ plane (y = 0) at height z; edge 1 stays
    // on one side of it.
    let across = |z: f64| Curve3::Line {
        start: [5.0, -5.0, z],
        end: [5.0, 5.0, z],
    };
    doc.kernel()
        .named_curves
        .borrow_mut()
        .insert(edge(0), across(5.0));
    doc.kernel().named_curves.borrow_mut().insert(
        edge(1),
        Curve3::Line {
            start: [-5.0, 5.0, 5.0],
            end: [5.0, 5.0, 5.0],
        },
    );
    let on_xz = command(&mut doc, json!({"cmd": "sketch.create", "plane": "xz"}))["uid"].clone();
    command(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": on_xz, "source": edge(0), "body": body,
               "linked": true, "section": true}),
    );
    let s = query(&doc, json!({"query": "sketch", "uid": on_xz}));
    // On XZ, x = +X and y = -Z: the crossing at (5, 0, 5) is (5, -5).
    assert!(near(&points(&doc, &on_xz), &[[5.0, -5.0]]), "{s}");
    assert_eq!(s["projections"][0]["section"], true, "{s}");
    assert_eq!(s["projections"][0]["source"], json!(edge(0)), "{s}");
    let curves = s["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] != "point")
        .count();
    assert_eq!(curves, 0, "{s}");

    // The edge moves (the body changes): its crossing point follows.
    doc.kernel()
        .named_curves
        .borrow_mut()
        .insert(edge(0), across(7.0));
    let d = depth(&doc);
    doc.set_parameter(&d, 7.0).unwrap();
    assert!(near(&points(&doc, &on_xz), &[[5.0, -7.0]]));
    let uid = on_xz.as_str().unwrap().parse().unwrap();
    assert_eq!(doc.warnings(uid), Vec::<String>::new());

    // An edge the plane does not cross is refused.
    let error = rejected(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": on_xz, "source": edge(1), "body": body,
               "section": true}),
    );
    assert!(error.contains("does not cut it"), "{error}");
}

#[test]
fn a_bodys_pieces_group_by_their_faces() {
    use mitcad_model::kernel::SectionCurve;
    use mitcad_model::sketch::project::section_groups;
    use mitcad_model::topo::{FaceName, TopoName};
    let face = |n: &str| -> FaceName { n.parse().unwrap() };
    let line = |x: f64| Curve3::Line {
        start: [x, 0.0, 0.0],
        end: [x, 1.0, 0.0],
    };
    let spline = quadratic([[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]]);
    let groups = section_groups(vec![
        SectionCurve {
            curve: line(0.0),
            faces: vec![face("F2:side(c1)")],
        },
        SectionCurve {
            curve: spline.clone(),
            faces: vec![face("F2:side(c2)"), face("F2:side(c3)")],
        },
        SectionCurve {
            curve: line(1.0),
            faces: vec![],
        },
        SectionCurve {
            curve: line(2.0),
            faces: vec![face("F2:side(c1)")],
        },
    ]);
    assert_eq!(groups.len(), 3, "{groups:?}");
    // Two lines on one face: one section of that face.
    assert_eq!(groups[0].source, Some(TopoName::Face(face("F2:side(c1)"))));
    assert_eq!(groups[0].curves, vec![line(0.0), line(2.0)]);
    assert!(groups[0].chain.is_empty());
    // A spline joined across two faces: the first's section with the
    // second as its chain.
    assert_eq!(groups[1].source, Some(TopoName::Face(face("F2:side(c2)"))));
    assert_eq!(groups[1].chain, vec![TopoName::Face(face("F2:side(c3)"))]);
    assert_eq!(groups[1].curves, vec![spline]);
    // Pieces on faces without names: unlinked.
    assert_eq!(groups[2].source, None);
    assert_eq!(groups[2].curves, vec![line(1.0)]);
}
