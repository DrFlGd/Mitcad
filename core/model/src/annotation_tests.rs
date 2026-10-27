// SPDX-License-Identifier: MIT
//! Annotations (mitcad#702) on the mock kernel (`MockKernel::
//! measures_selections`: the closest points of the picks' boxes, planar
//! faces' planes): added with a group, a tolerance, a format and the
//! places a view suggests; their stored form, older files' measurements
//! read unchanged, the geometry drawn in their planes, ISO 286 fit codes,
//! several edited and deleted as one undo step, and leader notes.

use serde_json::{Value, json};

use crate::Document;
use crate::testing::MockKernel;

const R: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";

/// A command's own answer, without the recompute's status fields that every
/// command's answer carries.
fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    let mut answer: Value = serde_json::from_str(&result).unwrap();
    if let Some(fields) = answer.as_object_mut() {
        fields.remove("recomputed");
        if fields.get("error").is_some_and(Value::is_null) {
            fields.remove("error");
        }
    }
    answer
}

fn refused(doc: &mut Document<MockKernel>, command: Value) -> String {
    match doc.command(&command.to_string()) {
        Ok(result) => panic!("{command} was accepted: {result}"),
        Err(e) => e.to_string(),
    }
}

fn annotations(doc: &Document<MockKernel>) -> Vec<Value> {
    let answer: Value =
        serde_json::from_str(&doc.query(r#"{"query": "annotations"}"#).unwrap()).unwrap();
    answer.as_array().unwrap().clone()
}

fn annotation(doc: &Document<MockKernel>, name: &str) -> Value {
    annotations(doc)
        .into_iter()
        .find(|m| m["name"] == name)
        .unwrap_or_else(|| panic!("no annotation {name}"))
}

/// The annotation of this name measured even while hidden (mitcad#716).
fn measured(doc: &Document<MockKernel>, name: &str) -> Value {
    let answer: Value = serde_json::from_str(
        &doc.query(r#"{"query": "annotations", "measure_hidden": true}"#)
            .unwrap(),
    )
    .unwrap();
    answer
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == name)
        .cloned()
        .unwrap_or_else(|| panic!("no annotation {name}"))
}

fn undo_label(doc: &Document<MockKernel>) -> Value {
    let answer: Value =
        serde_json::from_str(&doc.query(r#"{"query": "document"}"#).unwrap()).unwrap();
    answer["undo"].clone()
}

fn near(actual: &Value, expected: f64) {
    let value = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{actual} is no number"));
    assert!(
        (value - expected).abs() < 1e-6,
        "{value} instead of {expected}"
    );
}

fn near_point(actual: &Value, expected: [f64; 3]) {
    for (i, e) in expected.into_iter().enumerate() {
        near(&actual[i], e);
    }
}

fn vector(value: &Value) -> [f64; 3] {
    std::array::from_fn(|i| value[i].as_f64().unwrap())
}

fn face(face: &str) -> Value {
    json!({"body": "F2.b0", "face": face})
}

fn top() -> Value {
    face(&format!("F2:end({R})"))
}

fn bottom() -> Value {
    face(&format!("F2:start({R})"))
}

fn side() -> Value {
    face("F2:side(c1[c4,c2])")
}

/// A 20 x 20 block (F2.b0) on XY, as high as the parameter `height`
/// (20 mm).
fn block() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    doc.kernel().measures_selections.set(true);
    command(
        &mut doc,
        json!([
            {"cmd": "add_parameter", "name": "height", "value": "20 mm"},
            {"cmd": "sketch.create"},
            {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 20, "height": 20},
            {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": R}],
              "extent": {"type": "distance", "distance": "height"}, "operation": "new_body"}},
        ]),
    );
    doc
}

#[test]
fn annotations_are_kept_with_their_format_tolerance_group_and_places() {
    let mut doc = block();
    let added = command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "group": " Shop ",
               "format": {"decimals": 1, "before": "2 x"},
               "tolerance": {"type": "symmetric", "value": 0.1},
               "line_offset": -4, "text_offset": [1, 2]}),
    );
    assert_eq!(added, json!({"name": "Offset1", "kind": "offset"}));
    assert_eq!(undo_label(&doc), "Add Offset1");
    let offset = annotation(&doc, "Offset1");
    assert_eq!(offset["group"], "Shop");
    near(&offset["value"], 20.0);
    assert_eq!(offset["label"]["text"], "2 x 20.0 ±0.1");
    assert_eq!(offset["label"]["tolerance"], "±0.1");
    near(&offset["deviations"]["upper"], 0.1);
    near(&offset["deviations"]["lower"], -0.1);

    // The stored form: only what is set.
    let file: Value = serde_json::from_str(&doc.to_json()).unwrap();
    assert_eq!(
        file["annotations"],
        json!([{"name": "Offset1", "kind": "offset", "color": crate::ANNOTATION_COLORS[0],
                "group": "Shop", "a": top(), "b": bottom(), "line_offset": -4.0,
                "text_offset": [1.0, 2.0],
                "format": {"decimals": 1, "before": "2 x"},
                "tolerance": {"type": "symmetric", "value": 0.1}}])
    );
    assert!(file.get("measurements").is_none());
    let saved = doc.to_json();
    let mut loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    loaded.kernel().measures_selections.set(true);
    assert_eq!(loaded.to_json(), saved);
    command(&mut loaded, json!({"cmd": "recompute"}));
    assert_eq!(annotations(&loaded), annotations(&doc));

    // Refused: a fit code on an angle, a note's text on a dimension, a
    // plane on a circle's kind, unknown fields.
    let error = refused(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": side(), "kind": "angle",
               "tolerance": {"type": "fit", "code": "H7"}}),
    );
    assert!(
        error.contains("a fit code is a length's tolerance"),
        "{error}"
    );
    let error = refused(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "kind": "distance",
               "tolerance": {"type": "fit", "code": "X7"}}),
    );
    assert!(error.contains("no fit code covered"), "{error}");
    let error = refused(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "depth": 1}),
    );
    assert!(error.contains("depth"), "{error}");
    assert_eq!(annotations(&doc).len(), 1);
    // The measurements query is the same list.
    let answer: Value =
        serde_json::from_str(&doc.query(r#"{"query": "measurements"}"#).unwrap()).unwrap();
    assert_eq!(answer.as_array().unwrap(), &annotations(&doc));
}

#[test]
fn older_files_measurements_open_as_annotations_unchanged_in_value() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_measurement", "a": top(), "b": bottom()}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_measurement", "a": top(), "b": {"datum": "xy"}, "kind": "distance",
               "color": "#112233"}),
    );
    command(
        &mut doc,
        json!({"cmd": "set_measurement_visible", "name": "Distance1", "visible": false}),
    );
    // With only what the measurement objects of mitcad#649 had, they are
    // written as those were, which older versions read.
    let saved = doc.to_json();
    assert!(saved.contains("\"measurements\""), "{saved}");
    assert!(!saved.contains("\"annotations\""), "{saved}");
    let mut loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    loaded.kernel().measures_selections.set(true);
    command(&mut loaded, json!({"cmd": "recompute"}));
    assert_eq!(annotations(&loaded), annotations(&doc));
    near(&annotation(&loaded, "Offset1")["value"], 20.0);
    assert_eq!(annotation(&loaded, "Offset1")["label"]["text"], "20");
    assert_eq!(annotation(&loaded, "Distance1")["visible"], false);
    assert_eq!(loaded.to_json(), saved);
    // One with a new field: all of them as annotations, read back the same.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1", "group": "Shop"}),
    );
    let newer = doc.to_json();
    assert!(newer.contains("\"annotations\""), "{newer}");
    assert!(!newer.contains("\"measurements\""), "{newer}");
    let mut loaded = Document::from_json(&newer, MockKernel::default()).unwrap();
    loaded.kernel().measures_selections.set(true);
    command(&mut loaded, json!({"cmd": "recompute"}));
    assert_eq!(annotations(&loaded), annotations(&doc));
    assert_eq!(loaded.to_json(), newer);
    // Taken away again: back to measurements.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1", "group": null}),
    );
    assert_eq!(doc.to_json(), saved);
    // Both lists are read; a name in both is refused.
    let both = saved.replace(
        "\"measurements\": [",
        "\"annotations\": [{\"name\": \"Offset1\", \"kind\": \"distance\", \
         \"color\": \"#000000\", \"a\": {\"datum\": \"xy\"}, \"b\": {\"datum\": \"yz\"}}],\n  \
         \"measurements\": [",
    );
    let Err(error) = Document::from_json(&both, MockKernel::default()) else {
        panic!("a name listed twice was read");
    };
    assert!(
        error.to_string().contains("listed more than once"),
        "{error}"
    );
    let other = both.replace(
        "\"name\": \"Offset1\", \"kind\": \"distance\"",
        "\"name\": \"Gap\", \"kind\": \"distance\"",
    );
    let read = Document::from_json(&other, MockKernel::default()).unwrap();
    let names: Vec<&str> = read.annotations().iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Offset1", "Distance1", "Gap"]);
}

/// A root block F2.b0 and a Block (F4.b0, 20 mm cubed) inside an
/// Assembly: the Assembly at (100, 20, 30) turned 90 degrees about Z, the
/// Block at (5, 7, 11) in it. In the design the Block spans x 73 to 93, y
/// 25 to 45 and z 41 to 61 (as `api::measure_tests` places it).
fn nested() -> Document<MockKernel> {
    let mut doc = block();
    command(
        &mut doc,
        json!([
            {"cmd": "create_component", "name": "Assembly", "transform": {
                "translation": [100, 20, 30], "rotation": {"axis": [0, 0, 1], "angle": std::f64::consts::FRAC_PI_2}}},
            {"cmd": "create_component", "name": "Block", "transform": {"translation": [5, 7, 11]}},
            {"cmd": "sketch.create"},
            {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [0, 0], "width": 20, "height": 20},
            {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": R}],
              "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
            {"cmd": "activate_component", "component": "C0"}
        ]),
    );
    doc
}

#[test]
fn a_note_on_a_component_follows_its_occurrence() {
    let mut doc = nested();
    let block_top = json!({"body": "F4.b0", "face": format!("F4:end({R})"), "occurrence": "O1/O2"});
    // Picked at (3, 4, 25) in the Block's coordinates, seen from above:
    // its point and its plane are kept in them.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": block_top, "text": "Deburr", "at": [3, 4, 25],
               "view": {"direction": [0, 0, -1], "up": [0, 1, 0], "pixel": 0.5}}),
    );
    let note = annotation(&doc, "Note1");
    // The arrow on the Block's top in the design: (3, 4) of the Block is
    // (89, 28), its top at 61.
    near_point(&note["drawing"]["arrows"][0]["tip"], [89.0, 28.0, 61.0]);
    near_point(&note["plane"]["normal"], [0.0, 0.0, 1.0]);
    // The view's right (X) in the Block's coordinates, turned with it.
    near_point(&note["plane"]["x"], [0.0, -1.0, 0.0]);
    near_point(&note["drawing"]["plane"]["x"], [1.0, 0.0, 0.0]);
    near_point(&note["drawing"]["text"]["at"], [109.0, 43.0, 61.0]);
    // The Block moved in its Assembly: the note goes with it.
    command(
        &mut doc,
        json!({"cmd": "set_occurrence_transform", "occurrence": "O2",
               "transform": {"translation": [5, 17, 11]}}),
    );
    let note = annotation(&doc, "Note1");
    near_point(&note["drawing"]["arrows"][0]["tip"], [79.0, 28.0, 61.0]);
    // Pointed at a root face instead: the plane and point kept in the
    // Block's coordinates go; a body without a point is refused.
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Note1", "a": {"body": "F2.b0"}}),
    );
    assert!(error.contains("no point to point at"), "{error}");
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Note1", "a": top(), "at": [5, 6, 30]}),
    );
    let note = annotation(&doc, "Note1");
    assert!(note.get("plane").is_none(), "{note}");
    near_point(&note["at"], [5.0, 6.0, 30.0]);
    near_point(&note["drawing"]["arrows"][0]["tip"], [5.0, 6.0, 20.0]);
    // A body picked without a point has nothing to point at.
    let error = refused(
        &mut doc,
        json!({"cmd": "add_annotation", "a": {"body": "F2.b0"}, "text": "Steel"}),
    );
    assert!(error.contains("no point to point at"), "{error}");
}

#[test]
fn seen_along_its_ends_a_length_takes_the_plane_of_the_views_up() {
    for direction in [json!([0, 0, -1]), json!([1e-9, 0, -1])] {
        let mut doc = block();
        // Looking down the top-to-bottom offset: no plane square to the
        // view holds it; the view's up decides.
        command(
            &mut doc,
            json!({"cmd": "add_annotation", "a": top(), "b": bottom(),
                   "view": {"direction": direction, "up": [0, 1, 0], "pixel": 0.5}}),
        );
        let offset = annotation(&doc, "Offset1");
        near_point(&offset["plane"]["normal"], [0.0, 1.0, 0.0]);
        near_point(&offset["drawing"]["plane"]["normal"], [0.0, 1.0, 0.0]);
    }
}

#[test]
fn a_dimension_is_drawn_in_its_plane_relative_to_its_picks() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom()}),
    );
    let offset = annotation(&doc, "Offset1");
    let drawing = &offset["drawing"];
    let [p, q] = [vector(&offset["points"][0]), vector(&offset["points"][1])];
    // Along -Z, so in the plane square to Y (the axis least along it), the
    // line a quarter of the length off along -X.
    near_point(&drawing["plane"]["normal"], [0.0, 1.0, 0.0]);
    near_point(&drawing["plane"]["x"], [0.0, 0.0, -1.0]);
    near_point(&drawing["plane"]["y"], [-1.0, 0.0, 0.0]);
    near(&drawing["line_offset"], 5.0);
    near_point(&drawing["extension_lines"][0][0], p);
    near_point(&drawing["extension_lines"][0][1], [p[0] - 5.0, p[1], p[2]]);
    near_point(&drawing["dimension_lines"][0][1], [q[0] - 5.0, q[1], q[2]]);
    near_point(&drawing["arrows"][1]["direction"], [0.0, 0.0, -1.0]);
    near_point(&drawing["text"]["at"], [p[0] - 5.0, p[1], 10.0]);
    near_point(&drawing["text"]["direction"], [0.0, 0.0, -1.0]);

    // The text dragged along and off the line, the line to the other side.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1", "text_offset": [3, 1],
               "line_offset": 7}),
    );
    assert_eq!(undo_label(&doc), "Edit Offset1");
    let drawing = annotation(&doc, "Offset1")["drawing"].clone();
    near_point(&drawing["text"]["at"], [p[0] - 7.0 - 1.0, p[1], 10.0 - 3.0]);
    // The block made higher: the places follow the picks.
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "height", "value": 30}),
    );
    let drawing = annotation(&doc, "Offset1")["drawing"].clone();
    near_point(&drawing["text"]["at"], [p[0] - 8.0, p[1], 15.0 - 3.0]);
    // Back to the defaults with null.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1", "text_offset": null,
               "line_offset": null}),
    );
    let stored = annotation(&doc, "Offset1");
    assert!(stored.get("text_offset").is_none(), "{stored}");
    near(&stored["drawing"]["line_offset"], 7.5);

    // A plane picked: the XZ plane, which the length lies in already.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1",
               "plane": {"pick": {"datum": "xz"}}}),
    );
    let normal = vector(&annotation(&doc, "Offset1")["drawing"]["plane"]["normal"]);
    near(&json!(normal[1].abs()), 1.0);
    // A plane whose pick is no plane breaks it with the reason.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1",
               "plane": {"pick": {"datum": "x"}}}),
    );
    let broken = annotation(&doc, "Offset1");
    let error = broken["error"]
        .as_str()
        .unwrap_or_else(|| panic!("{broken}"));
    assert!(error.starts_with("the plane: "), "{error}");
    assert!(broken.get("drawing").is_none());
}

#[test]
fn a_view_suggests_the_plane_and_the_places() {
    let mut doc = block();
    // Seen from the front (looking along +Y), Z up, half a millimetre a
    // pixel.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(),
               "view": {"direction": [0, 1, 0], "up": [0, 0, 1], "pixel": 0.5}}),
    );
    let offset = annotation(&doc, "Offset1");
    assert_eq!(offset["plane"], json!({"normal": [0.0, -1.0, 0.0]}));
    near(&offset["line_offset"], 11.0);
    near_point(&offset["drawing"]["plane"]["y"], [1.0, 0.0, 0.0]);
    // A note there: its text along the view's right, 40 and 30 pixels off.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "at": [3, 4, 25], "text": "Deburr\nall edges",
               "view": {"direction": [0, 1, 0], "up": [0, 0, 1], "pixel": 0.5}}),
    );
    let note = annotation(&doc, "Note1");
    assert_eq!(note["kind"], "note");
    assert_eq!(note["text_offset"], json!([20.0, 15.0]));
    near_point(&note["drawing"]["plane"]["x"], [1.0, 0.0, 0.0]);
    near_point(&note["drawing"]["plane"]["y"], [0.0, 0.0, 1.0]);
    near_point(&note["drawing"]["arrows"][0]["tip"], [3.0, 4.0, 20.0]);
    near_point(&note["drawing"]["text"]["at"], [23.0, 4.0, 35.0]);
}

#[test]
fn notes_point_at_their_picks() {
    let mut doc = block();
    let added = command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "at": [3, 4, 25],
               "text": "Deburr\nall edges "}),
    );
    assert_eq!(added, json!({"name": "Note1", "kind": "note"}));
    let note = annotation(&doc, "Note1");
    assert_eq!(note["label"]["lines"], json!(["Deburr", "all edges"]));
    assert_eq!(note["label"]["text"], "Deburr / all edges");
    assert!(note.get("value").is_none());
    // In the top face's plane, the point brought onto it.
    let drawing = &note["drawing"];
    near_point(&drawing["plane"]["normal"], [0.0, 0.0, 1.0]);
    near_point(&drawing["arrows"][0]["tip"], [3.0, 4.0, 20.0]);
    near_point(&drawing["text"]["at"], [13.0, 14.0, 20.0]);
    near_point(&drawing["dimension_lines"][0][1], [3.0, 4.0, 20.0]);
    // Its text edited; it follows the face.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Note1", "text": "Polish"}),
    );
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "height", "value": 25}),
    );
    let note = annotation(&doc, "Note1");
    assert_eq!(note["label"]["text"], "Polish");
    near_point(&note["drawing"]["arrows"][0]["tip"], [3.0, 4.0, 25.0]);
    // Refused: a note without text, a tolerance, a dimension's kind.
    let error = refused(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "kind": "note"}),
    );
    assert!(error.contains("a note needs a text"), "{error}");
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Note1",
               "tolerance": {"type": "symmetric", "value": 1}}),
    );
    assert!(error.contains("a note has a text"), "{error}");
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Note1", "kind": "distance"}),
    );
    assert!(error.contains("a note stays a note"), "{error}");
    // Its pick gone (rolled back): broken, its text still listed.
    command(&mut doc, json!({"cmd": "set_marker", "position": 1}));
    let broken = annotation(&doc, "Note1");
    assert!(
        broken["error"]
            .as_str()
            .unwrap()
            .starts_with("a pick is not there")
    );
    assert_eq!(broken["label"]["text"], "Polish");
}

#[test]
fn fit_codes_give_their_deviations() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(),
               "tolerance": {"type": "fit", "code": "H7", "deviations": true}}),
    );
    let offset = annotation(&doc, "Offset1");
    assert_eq!(offset["label"]["text"], "20 H7 (+0.021/0)");
    assert_eq!(offset["label"]["main"], "20 H7");
    assert_eq!(offset["label"]["upper"], "+0.021");
    assert_eq!(offset["label"]["lower"], "0");
    assert_eq!(offset["label"]["brackets"], true);
    near(&offset["deviations"]["upper"], 0.021);
    near(&offset["deviations"]["lower"], 0.0);
    // A shaft's, and the size grows into the next range.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1",
               "tolerance": {"type": "fit", "code": "g6"}}),
    );
    assert_eq!(annotation(&doc, "Offset1")["label"]["text"], "20 g6");
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "height", "value": 40}),
    );
    let offset = annotation(&doc, "Offset1");
    near(&offset["deviations"]["upper"], -0.009);
    near(&offset["deviations"]["lower"], -0.025);
    // Over 500 mm: the code, with why there are no deviations.
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "height", "value": 600}),
    );
    let offset = annotation(&doc, "Offset1");
    assert_eq!(offset["label"]["text"], "600 g6");
    assert!(
        offset["label"]["warning"]
            .as_str()
            .unwrap()
            .contains("500 mm")
    );
    assert!(offset.get("deviations").is_none());
}

#[test]
fn several_are_edited_and_deleted_as_one_step() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom()}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": side()}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "kind": "distance",
               "group": "Assembly"}),
    );
    let before = annotations(&doc);
    let edited = command(
        &mut doc,
        json!({"cmd": "edit_annotation", "names": ["Offset1", "Angle1"], "color": "#00AA00",
               "group": "Shop", "visible": false, "format": {"decimals": 2},
               "tolerance": {"type": "symmetric", "value": 0.5}}),
    );
    assert_eq!(edited, json!({"changed": true}));
    assert_eq!(undo_label(&doc), "Edit 2 annotations");
    for name in ["Offset1", "Angle1"] {
        let a = annotation(&doc, name);
        assert_eq!(a["color"], "#00aa00");
        assert_eq!(a["group"], "Shop");
        assert_eq!(a["visible"], false);
        assert_eq!(a["format"], json!({"decimals": 2}));
        assert_eq!(a["measured"], false);
        assert!(a.get("label").is_none() && a.get("values").is_none(), "{a}");
    }
    // Hidden: measured only when asked for (mitcad#716).
    assert_eq!(measured(&doc, "Offset1")["label"]["text"], "20.00 ±0.5");
    assert_eq!(measured(&doc, "Angle1")["label"]["text"], "90.00° ±0.5°");
    assert_eq!(annotation(&doc, "Distance1")["group"], "Assembly");
    // Nothing changed is no step.
    let again = command(
        &mut doc,
        json!({"cmd": "edit_annotation", "names": ["Offset1", "Angle1"], "visible": false}),
    );
    assert_eq!(again, json!({"changed": false}));
    // One undo brings both back.
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(annotations(&doc), before);
    // A group taken away, the decimals back to the default.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Distance1", "group": null,
               "format": {"decimals": 3}}),
    );
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Distance1", "format": {"decimals": null},
               "new_name": "Height"}),
    );
    let height = annotation(&doc, "Height");
    assert!(height.get("group").is_none() && height.get("format").is_none());
    // Refused: a new name or picks for several, a name taken, a name twice.
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "names": ["Offset1", "Angle1"], "new_name": "X"}),
    );
    assert!(error.contains("without a new name or picks"), "{error}");
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1", "new_name": "Angle1"}),
    );
    assert!(error.contains("an annotation 'Angle1' exists"), "{error}");
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "names": ["Offset1", "Offset1"], "visible": true}),
    );
    assert!(error.contains("named twice"), "{error}");
    // Other picks: the kind they have first when the old one is gone.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Offset1", "b": side()}),
    );
    assert_eq!(annotation(&doc, "Offset1")["kind"], "angle");

    let before = annotations(&doc);
    command(
        &mut doc,
        json!({"cmd": "delete_annotation", "names": ["Offset1", "Height"]}),
    );
    assert_eq!(undo_label(&doc), "Delete 2 annotations");
    let left: Vec<Value> = annotations(&doc)
        .iter()
        .map(|a| a["name"].clone())
        .collect();
    assert_eq!(left, [json!("Angle1")]);
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(annotations(&doc), before);
    command(
        &mut doc,
        json!({"cmd": "delete_annotation", "name": "Angle1"}),
    );
    assert_eq!(undo_label(&doc), "Delete Angle1");
    let error = refused(&mut doc, json!({"cmd": "delete_annotation"}));
    assert!(error.contains("`names`"), "{error}");
}

#[test]
fn datums_and_feature_control_frames_are_kept_on_their_picks() {
    let mut doc = block();
    // A datum on the bottom face: the first free letter, named by it, its
    // leader ending in a datum triangle.
    let added = command(
        &mut doc,
        json!({"cmd": "add_annotation", "kind": "datum", "a": bottom(), "at": [5, 5, -1]}),
    );
    assert_eq!(added, json!({"name": "DatumA", "kind": "datum"}));
    let datum = annotation(&doc, "DatumA");
    assert_eq!(datum["letter"], "A");
    assert_eq!(datum["label"], json!({"text": "A", "datum": "A"}));
    assert_eq!(datum["drawing"]["terminator"], "datum");
    near_point(&datum["drawing"]["arrows"][0]["tip"], [5.0, 5.0, 0.0]);
    assert!(datum.get("value").is_none());
    // A letter given (a datum B on the side), and refused letters.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": side(), "letter": "B"}),
    );
    for (letter, why) in [
        ("A", "datum A exists"),
        ("I", "I, O and Q"),
        ("ab", "capital"),
    ] {
        let error = refused(
            &mut doc,
            json!({"cmd": "add_annotation", "a": top(), "letter": letter}),
        );
        assert!(error.contains(why), "{letter}: {error}");
    }

    // The top perpendicular to A within Ø0.05 at maximum material, on a
    // leader to the top.
    let added = command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "at": [10, 10, 20],
               "frame": {"characteristic": "perpendicularity", "value": 0.05,
                         "diameter": true, "modifier": "M", "datums": ["A", "B"]}}),
    );
    assert_eq!(added, json!({"name": "Frame1", "kind": "frame"}));
    let frame = annotation(&doc, "Frame1");
    assert_eq!(
        frame["label"],
        json!({"text": "⟂ | Ø0.05 Ⓜ | A | B", "frame": ["⟂", "Ø0.05 Ⓜ", "A", "B"],
               "tolerance": "Ø0.05 Ⓜ"})
    );
    near_point(&frame["drawing"]["arrows"][0]["tip"], [10.0, 10.0, 20.0]);
    assert!(frame["drawing"].get("terminator").is_none());
    // Refused: the rules of ISO 1101.
    for (tolerance, why) in [
        (
            json!({"characteristic": "flatness", "value": 0.1, "datums": ["A"]}),
            "refers to no datum",
        ),
        (
            json!({"characteristic": "parallelism", "value": 0.1}),
            "refers to a datum",
        ),
        (
            json!({"characteristic": "flatness", "value": 0.1, "diameter": true}),
            "no cylinder",
        ),
        (
            json!({"characteristic": "circular_runout", "value": 0.1, "modifier": "L",
                "datums": ["A"]}),
            "no material condition",
        ),
        (
            json!({"characteristic": "position", "value": 0.0}),
            "more than 0",
        ),
        (
            json!({"characteristic": "position", "value": 0.1, "datums": ["A", "A"]}),
            "twice",
        ),
        (
            json!({"characteristic": "position", "value": 0.1, "datums": ["A", "B", "C", "D"]}),
            "three datums",
        ),
    ] {
        let error = refused(
            &mut doc,
            json!({"cmd": "add_annotation", "a": top(), "frame": tolerance}),
        );
        assert!(error.contains(why), "{tolerance}: {error}");
    }
    // A flatness of the top: no datum; one referred to that has no label
    // says so.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "at": [3, 3, 20],
               "frame": {"characteristic": "flatness", "value": 0.02}}),
    );
    assert_eq!(annotation(&doc, "Frame2")["label"]["text"], "⏥ | 0.02");
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "at": [3, 3, 20],
               "frame": {"characteristic": "position", "value": 0.1, "datums": ["C"]}}),
    );
    assert_eq!(
        annotation(&doc, "Frame3")["label"]["warning"],
        "no datum label C"
    );

    // A frame under a dimension: in that dimension's text's place.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "name": "Height"}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "on": "Height", "name": "HeightFrame",
               "frame": {"characteristic": "parallelism", "value": 0.03, "datums": ["A"]}}),
    );
    let height = annotation(&doc, "Height");
    let under = annotation(&doc, "HeightFrame");
    assert_eq!(under["drawing"]["attached"], "Height");
    assert_eq!(under["drawing"]["text"], height["drawing"]["text"]);
    assert_eq!(under["drawing"]["arrows"], json!([]));
    // Its dimension deleted: a leader to its own pick again, in the same
    // step, so that a later Height does not take it.
    command(
        &mut doc,
        json!({"cmd": "delete_annotation", "name": "Height"}),
    );
    let leader = annotation(&doc, "HeightFrame");
    assert!(
        leader.get("on").is_none() && leader.get("error").is_none(),
        "{leader}"
    );
    assert_eq!(
        leader["drawing"]["arrows"].as_array().map(Vec::len),
        Some(1)
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "name": "Height"}),
    );
    assert!(annotation(&doc, "HeightFrame").get("on").is_none());
    command(&mut doc, json!({"cmd": "undo"}));
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(annotation(&doc, "HeightFrame")["on"], "Height");
    // A frame named under a dimension that is not there (added so by the
    // document, an older file): on its leader instead.
    let mut orphan = doc.annotation("HeightFrame").unwrap().clone();
    orphan.name = "Orphan".to_owned();
    orphan.on = Some("Gone".to_owned());
    doc.add_annotation(orphan).unwrap();
    let orphan = annotation(&doc, "Orphan");
    assert!(
        orphan.get("on").is_none() && orphan.get("error").is_none(),
        "{orphan}"
    );
    command(&mut doc, json!({"cmd": "undo"}));

    // Edited: another value and datum order; a frame stays a frame, a
    // datum's letter is its own.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1",
               "frame": {"characteristic": "perpendicularity", "value": 0.1, "datums": ["B", "A"]}}),
    );
    assert_eq!(
        annotation(&doc, "Frame1")["label"]["text"],
        "⟂ | 0.1 | B | A"
    );
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1", "kind": "distance"}),
    );
    assert!(error.contains("a frame stays a frame"), "{error}");
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "DatumB", "letter": "A"}),
    );
    assert!(error.contains("datum A exists"), "{error}");
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "DatumB", "letter": "C"}),
    );
    assert!(annotation(&doc, "Frame3")["label"].get("warning").is_none());

    // Saved and read back the same, as annotations.
    let saved = doc.to_json();
    assert!(saved.contains("\"annotations\""), "{saved}");
    let mut loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    loaded.kernel().measures_selections.set(true);
    command(&mut loaded, json!({"cmd": "recompute"}));
    assert_eq!(annotations(&loaded), annotations(&doc));
    assert_eq!(loaded.to_json(), saved);
}

#[test]
fn the_characteristics_of_frames_are_listed_with_their_rules() {
    let doc = block();
    let answer: Value = serde_json::from_str(
        &doc.query(r#"{"query": "geometric_characteristics"}"#)
            .unwrap(),
    )
    .unwrap();
    let list = answer.as_array().unwrap();
    assert_eq!(list.len(), 14);
    assert_eq!(
        list[1],
        json!({"key": "flatness", "symbol": "⏥", "datums": "none",
               "diameter": false, "modifier": false})
    );
    let position = list.iter().find(|c| c["key"] == "position").unwrap();
    assert_eq!(position["symbol"], "⌖");
    assert_eq!(position["datums"], "optional");
    assert_eq!(position["diameter"], true);
    assert_eq!(position["modifier"], true);
    let runout = list.iter().find(|c| c["key"] == "total_runout").unwrap();
    assert_eq!(runout["datums"], "required");
    assert_eq!(runout["diameter"], false);
}

#[test]
fn frames_follow_their_dimension_and_datums_their_letters() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "kind": "datum", "a": bottom()}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "name": "Height"}),
    );
    // Datum letters trimmed, as they are stored.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "on": "Height", "name": "HeightFrame",
               "frame": {"characteristic": "parallelism", "value": 0.03, "datums": [" A "]}}),
    );
    assert_eq!(
        annotation(&doc, "HeightFrame")["frame"]["datums"],
        json!(["A"])
    );

    // Under a leader, under itself or under nothing: refused.
    for (on, why) in [
        ("DatumA", "DatumA is no dimension"),
        ("HeightFrame", "HeightFrame is no dimension"),
        ("Width", "there is no dimension Width"),
    ] {
        let error = refused(
            &mut doc,
            json!({"cmd": "edit_annotation", "name": "HeightFrame", "on": on}),
        );
        assert!(error.contains(why), "{on}: {error}");
    }

    // Height renamed: its frame stays under it, in the same undo step.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Height", "new_name": "Thickness"}),
    );
    let under = annotation(&doc, "HeightFrame");
    assert_eq!(under["on"], "Thickness");
    assert_eq!(under["drawing"]["attached"], "Thickness");
    assert!(under.get("error").is_none(), "{under}");
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(annotation(&doc, "HeightFrame")["on"], "Height");

    // Its dimension hidden: the frame is not drawn either.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Height", "visible": false}),
    );
    let hidden = annotation(&doc, "HeightFrame");
    assert!(
        hidden.get("drawing").is_none() && hidden.get("error").is_none(),
        "{hidden}"
    );
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Height", "visible": true}),
    );

    // `on: null`: a leader to its pick again.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "HeightFrame", "on": null}),
    );
    let leader = annotation(&doc, "HeightFrame");
    assert!(leader.get("on").is_none(), "{leader}");
    assert!(leader["drawing"].get("attached").is_none());
    assert_eq!(
        leader["drawing"]["arrows"].as_array().map(Vec::len),
        Some(1)
    );

    // A datum still named by its letter follows a new letter; one named
    // otherwise keeps its name.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "DatumA", "letter": "D"}),
    );
    assert_eq!(annotation(&doc, "DatumD")["letter"], "D");
    // The frame referring to it follows its letter.
    assert_eq!(
        annotation(&doc, "HeightFrame")["frame"]["datums"],
        json!(["D"])
    );
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "DatumD", "new_name": "Base"}),
    );
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Base", "letter": "E"}),
    );
    assert_eq!(annotation(&doc, "Base")["letter"], "E");
    assert_eq!(
        annotation(&doc, "HeightFrame")["frame"]["datums"],
        json!(["E"])
    );
    assert_eq!(
        annotation(&doc, "HeightFrame")["label"]["text"],
        "∥ | 0.03 | E"
    );

    // On a point (a datum point): no datum label, no frame.
    for new in [
        json!({"cmd": "add_annotation", "kind": "datum", "a": {"datum": "origin"}}),
        json!({"cmd": "add_annotation", "a": {"datum": "origin"},
               "frame": {"characteristic": "flatness", "value": 0.1}}),
    ] {
        let error = refused(&mut doc, new);
        assert!(error.contains("not on a point"), "{error}");
    }
}

#[test]
fn datum_letters_go_on_in_pairs_after_z() {
    let mut doc = block();
    // A to Z without I, O and Q: 23 letters, then AA.
    for _ in 0..23 {
        command(
            &mut doc,
            json!({"cmd": "add_annotation", "kind": "datum", "a": top()}),
        );
    }
    assert_eq!(annotation(&doc, "DatumZ")["letter"], "Z");
    let added = command(
        &mut doc,
        json!({"cmd": "add_annotation", "kind": "datum", "a": top()}),
    );
    assert_eq!(added, json!({"name": "DatumAA", "kind": "datum"}));
}

#[test]
fn a_frame_stands_under_a_dimension_from_a_point() {
    let mut doc = block();
    // A dimension whose first pick is a point (the origin): a frame under
    // it is no frame on a point; its pick is only an anchor there.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": {"datum": "origin"}, "b": top(), "name": "Rise"}),
    );
    let added = command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": {"datum": "origin"}, "on": "Rise",
               "frame": {"characteristic": "flatness", "value": 0.1}}),
    );
    assert_eq!(added["name"], "Frame1");
    let under = annotation(&doc, "Frame1");
    assert!(under.get("error").is_none(), "{under}");
    assert_eq!(under["drawing"]["attached"], "Rise");
    // Taken from under it, it would stand on the point: refused.
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1", "on": null}),
    );
    assert!(error.contains("not on a point"), "{error}");

    // The letter a new datum label takes, from the model.
    let next: Value =
        serde_json::from_str(&doc.query(r#"{"query": "next_datum_letter"}"#).unwrap()).unwrap();
    assert_eq!(next, json!({"letter": "A"}));
}

#[test]
fn a_new_letter_never_leaves_a_frame_invalid_and_files_open_again() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "kind": "datum", "a": bottom()}),
    );
    // Frame1 refers to A and to C, which no label has yet.
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(),
               "frame": {"characteristic": "perpendicularity", "value": 0.05, "datums": ["A", "C"]}}),
    );
    // C is not offered for a new label while a frame refers to it.
    let next: Value =
        serde_json::from_str(&doc.query(r#"{"query": "next_datum_letter"}"#).unwrap()).unwrap();
    assert_eq!(next, json!({"letter": "B"}));
    // A lettered C would leave Frame1 referring to C twice: refused, named.
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "DatumA", "letter": "C"}),
    );
    assert!(
        error.contains("Frame1: datum C is referred to twice"),
        "{error}"
    );
    assert_eq!(
        annotation(&doc, "Frame1")["frame"]["datums"],
        json!(["A", "C"])
    );

    // What the commands allow opens again: lettered, renamed, deleted.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "DatumA", "letter": "D"}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom(), "name": "Height"}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "on": "Height", "name": "Under",
               "frame": {"characteristic": "parallelism", "value": 0.02, "datums": ["D"]}}),
    );
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Height", "new_name": "Thickness"}),
    );
    command(
        &mut doc,
        json!({"cmd": "delete_annotation", "name": "Thickness"}),
    );
    let saved = doc.to_json();
    let mut loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    loaded.kernel().measures_selections.set(true);
    command(&mut loaded, json!({"cmd": "recompute"}));
    assert_eq!(annotations(&loaded), annotations(&doc));
}

#[test]
fn a_frame_stands_under_a_dimension_from_a_body() {
    let mut doc = block();
    // A dimension whose first pick is a whole body (no point of its own).
    let body: crate::document::MeasurePick =
        serde_json::from_value(json!({"body": "F2.b0"})).unwrap();
    let mut between = crate::document::Annotation::new(
        "Between",
        crate::document::MeasureKind::Distance,
        "",
        body,
    );
    between.b = Some(serde_json::from_value(bottom()).unwrap());
    doc.add_annotation(between).unwrap();
    // A frame under it: its pick is only an anchor, so no point is needed.
    let added = command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": {"body": "F2.b0"}, "on": "Between",
               "frame": {"characteristic": "flatness", "value": 0.1}}),
    );
    assert_eq!(added["name"], "Frame1");
    assert_eq!(annotation(&doc, "Frame1")["on"], "Between");
    // Edited under it, its pick is not checked either.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1", "a": {"body": "F2.b0"}}),
    );
    // On a leader it would need a point: refused.
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1", "on": null}),
    );
    assert!(error.contains("no point"), "{error}");
    // Moved onto a leader with a face picked in the same edit: accepted,
    // as the editor sends it (mitcad#713).
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1", "on": null, "a": top(),
               "at": [5, 6, 20]}),
    );
    let leader = annotation(&doc, "Frame1");
    assert!(leader.get("on").is_none(), "{leader}");
    assert!(leader.get("error").is_none(), "{leader}");
    assert_eq!(leader["a"]["face"], top()["face"]);
}

#[test]
fn a_frame_under_a_dimension_from_a_point_moves_onto_a_leader_with_a_new_pick() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": {"datum": "origin"}, "b": top(), "name": "Rise"}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": {"datum": "origin"}, "on": "Rise",
               "frame": {"characteristic": "flatness", "value": 0.1}}),
    );
    // A point picked again is still refused on a leader.
    let error = refused(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1", "on": null,
               "a": {"datum": "origin"}}),
    );
    assert!(error.contains("not on a point"), "{error}");
    // `on: null` with a face in one edit: the check takes the new pick.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Frame1", "on": null, "a": top(),
               "at": [5, 6, 20]}),
    );
    let leader = annotation(&doc, "Frame1");
    assert!(leader.get("on").is_none(), "{leader}");
    assert!(leader.get("error").is_none(), "{leader}");
    assert_eq!(
        leader["drawing"]["arrows"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn a_file_with_a_datum_letter_twice_is_refused() {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "kind": "datum", "a": top()}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "kind": "datum", "a": bottom()}),
    );
    let saved = doc.to_json();
    assert!(Document::from_json(&saved, MockKernel::default()).is_ok());
    // DatumB lettered A by hand: two labels of A.
    let mut file: Value = serde_json::from_str(&saved).unwrap();
    let list = file["annotations"].as_array_mut().expect("annotations");
    let second = list
        .iter_mut()
        .find(|a| a["letter"] == "B")
        .expect("datum B");
    second["letter"] = json!("A");
    let edited = serde_json::to_string(&file).unwrap();
    let Err(error) = Document::from_json(&edited, MockKernel::default()) else {
        panic!("a datum letter on two labels was read");
    };
    assert!(
        error
            .to_string()
            .contains("annotations[1]: datum A is lettered on 'DatumA' and on 'DatumB'"),
        "{error}"
    );
}

#[test]
fn kept_annotations_are_measured_again_only_when_their_bodies_change() {
    // mitcad#716: the application asks for the annotations after every
    // command; picks whose bodies kept their shapes are not measured again.
    let mut doc = block();
    command(
        &mut doc,
        json!([
            {"cmd": "add_parameter", "name": "width", "value": "10 mm"},
            {"cmd": "sketch.create"},
            {"cmd": "sketch.add_rectangle", "sketch": "F3", "corner": [40, 0], "width": 10, "height": 10},
            {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F3", "region": R}],
              "extent": {"type": "distance", "distance": "width"}, "operation": "new_body"}},
        ]),
    );
    assert!(doc.body_shape("F4.b0".parse().unwrap()).is_some());
    command(
        &mut doc,
        json!({"cmd": "add_annotation", "a": top(), "b": bottom()}),
    );
    let measures = |doc: &Document<MockKernel>| doc.kernel().count("measure_between");
    near(&annotation(&doc, "Offset1")["value"], 20.0);
    let once = measures(&doc);
    assert!(once >= 1);
    near(&annotation(&doc, "Offset1")["value"], 20.0);
    assert_eq!(measures(&doc), once, "asked again");
    for change in [
        json!({"cmd": "edit_annotation", "name": "Offset1", "new_name": "Height"}),
        json!({"cmd": "edit_annotation", "name": "Height", "color": "#00AA00"}),
        json!({"cmd": "edit_annotation", "name": "Height", "visible": false}),
        json!({"cmd": "edit_annotation", "name": "Height", "visible": true}),
        json!({"cmd": "set_body_visible", "uid": "F2.b0", "visible": false}),
        json!({"cmd": "set_body_visible", "uid": "F2.b0", "visible": true}),
        json!({"cmd": "set_parameter", "name": "width", "value": 15}),
    ] {
        command(&mut doc, change.clone());
        let height = annotation(&doc, "Height");
        assert!(height.get("error").is_none(), "{change}: {height}");
        if height["visible"] == true {
            near(&height["value"], 20.0);
        }
        assert_eq!(measures(&doc), once, "{change} measured it again");
    }

    // The picked body changed: measured again, once.
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "height", "value": 12}),
    );
    near(&annotation(&doc, "Height")["value"], 12.0);
    assert_eq!(measures(&doc), once + 1);
    near(&annotation(&doc, "Height")["value"], 12.0);
    assert_eq!(measures(&doc), once + 1);
    // Undone: the earlier shape's measure is still kept.
    command(&mut doc, json!({"cmd": "undo"}));
    near(&annotation(&doc, "Height")["value"], 20.0);
    assert_eq!(measures(&doc), once + 1);

    // Hidden: not measured for the answer, measured when asked for.
    command(
        &mut doc,
        json!({"cmd": "edit_annotation", "name": "Height", "visible": false}),
    );
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "height", "value": 14}),
    );
    let hidden = annotation(&doc, "Height");
    assert_eq!(hidden["measured"], false);
    assert_eq!(hidden["visible"], false);
    assert!(
        hidden.get("value").is_none()
            && hidden.get("values").is_none()
            && hidden.get("error").is_none(),
        "{hidden}"
    );
    assert_eq!(measures(&doc), once + 1);
    let asked = measured(&doc, "Height");
    near(&asked["value"], 14.0);
    assert!(asked.get("measured").is_none(), "{asked}");
    assert_eq!(asked["values"]["offset"], asked["value"]);
    assert_eq!(measures(&doc), once + 2);
    // Its body gone: broken all the same, without measuring.
    command(&mut doc, json!({"cmd": "delete_feature", "uid": "F2"}));
    let broken = annotation(&doc, "Height");
    assert!(broken["error"].is_string(), "{broken}");
    assert_eq!(measures(&doc), once + 2);
}

#[test]
fn more_annotations_than_the_measure_cache_holds_are_not_measured_again() {
    // mitcad#758: the annotations are asked for in the same order after
    // every command; a cache smaller than the keys they use would evict
    // each before it is asked for again and measure all of them each time.
    let mut doc = Document::new(MockKernel::default());
    doc.kernel().measures_selections.set(true);
    let mut faces = Vec::new();
    for i in 0..12 {
        let sketch = format!("F{}", 2 * i + 1);
        let extrude = format!("F{}", 2 * i + 2);
        command(
            &mut doc,
            json!([
                {"cmd": "sketch.create"},
                {"cmd": "sketch.add_rectangle", "sketch": sketch, "corner": [40 * i, 0],
                 "width": 20, "height": 20},
                {"cmd": "add_feature", "def": {"type": "extrude",
                  "profiles": [{"sketch": sketch, "region": R}],
                  "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
            ]),
        );
        let body = format!("{extrude}.b0");
        faces.push(json!({"body": body, "face": format!("{extrude}:end({R})")}));
        faces.push(json!({"body": body, "face": format!("{extrude}:start({R})")}));
    }
    // 300 distances between distinct ordered pairs of faces apart: as many
    // distinct measure keys.
    let pairs: Vec<(&Value, &Value)> = faces
        .iter()
        .flat_map(|a| faces.iter().map(move |b| (a, b)))
        .filter(|(a, b)| a != b)
        .take(300)
        .collect();
    assert_eq!(pairs.len(), 300);
    for (a, b) in pairs {
        command(
            &mut doc,
            json!({"cmd": "add_annotation", "a": a, "b": b, "kind": "distance"}),
        );
    }
    let measures = |doc: &Document<MockKernel>| doc.kernel().count("measure_between");
    let listed = annotations(&doc);
    assert_eq!(listed.len(), 300);
    for item in &listed {
        assert!(item.get("error").is_none(), "{item}");
        assert!(item["value"].as_f64().is_some(), "{item}");
    }
    let first = measures(&doc);
    assert_eq!(annotations(&doc).len(), 300);
    assert_eq!(measures(&doc), first, "measured again");
}
