// SPDX-License-Identifier: MIT
//! A body's print bed face (mitcad#731): set, replaced and cleared as undo
//! steps, refused when it cannot lie on the bed (a surface body's too),
//! kept by its name through edits (one that renames it) and in the design
//! file; and the 3D print export's `on_bed`, which turns the bodies onto
//! the one bed face among them and centres them on the plate (a box on its
//! top, on a tilted face, in a turned component, in a component placed
//! twice), keeps the bodies as assembled with several bed faces or one
//! body placed twice turned differently, leaves a surface body's face out
//! (the `bodies` query says why; it can be cleared) and reports a face
//! that is gone or not planar.
//! The mock kernel's meshes are the bodies' boxes.

use std::f64::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::Document;
use crate::datum::SurfaceGeometry;
use crate::exchange::{BaseInput, ImportBody};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::testing::{MockKernel, MockShape};

const R: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";
const SIDE: &str = "F2:side(c1[c4,c2])";
const TOL: f64 = 1e-6;
const GONE: &str = "the print bed face is gone";
const NOT_PLANAR: &str = "the print bed face is not planar";
const NOT_PRINTED: &str = "a surface body is not printed";
const IDENTITY: mitcad_3mf::Placement = mitcad_3mf::Placement::IDENTITY;

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

fn refused(doc: &mut Document<MockKernel>, command: Value) -> String {
    match doc.command(&command.to_string()) {
        Ok(result) => panic!("{command} was accepted: {result}"),
        Err(e) => e.to_string(),
    }
}

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    let answer = doc
        .query(&query.to_string())
        .unwrap_or_else(|e| panic!("{query}: {e}"));
    serde_json::from_str(&answer).unwrap()
}

fn document(doc: &Document<MockKernel>) -> Value {
    query(doc, json!({"query": "document"}))
}

/// A 20 x 20 x 20 block (F2.b0, Body1) on a rectangle on XY.
fn block() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    command(
        &mut doc,
        json!([
            {"cmd": "sketch.create"},
            {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 20, "height": 20},
            {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": R}],
              "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
        ]),
    );
    doc
}

fn top() -> String {
    format!("F2:end({R})")
}

fn bottom() -> String {
    format!("F2:start({R})")
}

fn set_bed(doc: &mut Document<MockKernel>, body: &str, face: Value) -> Value {
    command(
        doc,
        json!({"cmd": "set_print_bed_face", "uid": body, "face": face}),
    )
}

/// The first body's `print_bed_face` in the `bodies` query, or null.
fn bed_face(doc: &Document<MockKernel>) -> Value {
    let bodies = query(doc, json!({"query": "bodies"}));
    bodies[0]
        .get("print_bed_face")
        .cloned()
        .unwrap_or(Value::Null)
}

/// The surface the mock kernel gives a face by its name.
fn give_surface(doc: &Document<MockKernel>, face: &str, geometry: SurfaceGeometry) {
    let mut surfaces = doc.kernel().surfaces.borrow_mut();
    surfaces.insert(face.to_owned(), geometry);
}

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("not a number: {value}"))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOL
}

/// A folder of the test's own for the files it writes.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mitcad-print-bed-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Exports the bodies to a 3MF file on the print bed: the result and the
/// file read back.
fn export_3mf(
    doc: &mut Document<MockKernel>,
    dir: &Path,
    bodies: Value,
) -> (Value, mitcad_3mf::Model) {
    let path = dir.join("print.3mf").to_str().unwrap().to_owned();
    let result = command(
        doc,
        json!({"cmd": "export", "path": path, "bodies": bodies, "on_bed": true}),
    );
    let model = mitcad_3mf::read(&std::fs::read(&path).unwrap()).unwrap();
    (result, model)
}

/// The box of a 3MF part's mesh at all its placements.
fn placed_box(part: &mitcad_3mf::Part) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for placement in &part.placements {
        for vertex in &part.mesh.vertices {
            let p = placement.apply(*vertex);
            min = std::array::from_fn(|i| min[i].min(p[i]));
            max = std::array::from_fn(|i| max[i].max(p[i]));
        }
    }
    (min, max)
}

/// The turn of a result's `bed.transform` (its rows' first three numbers)
/// applied to a vector.
fn turned(result: &Value, v: [f64; 3]) -> [f64; 3] {
    let rows = &result["bed"]["transform"];
    std::array::from_fn(|r| (0..3).map(|c| number(&rows[r][c]) * v[c]).sum())
}

/// The whole of a result's `bed.transform` applied to a point.
fn moved(result: &Value, p: [f64; 3]) -> [f64; 3] {
    let rows = &result["bed"]["transform"];
    let turn = turned(result, p);
    std::array::from_fn(|r| turn[r] + number(&rows[r][3]))
}

/// The box's lowest point is on the bed and its middle over `plate`.
fn assert_on_plate(found: ([f64; 3], [f64; 3]), plate: [f64; 2]) {
    let (min, max) = found;
    assert!(near(min[2], 0.0), "lowest z {}", min[2]);
    assert!(near((min[0] + max[0]) / 2.0, plate[0]), "{min:?} {max:?}");
    assert!(near((min[1] + max[1]) / 2.0, plate[1]), "{min:?} {max:?}");
}

#[test]
fn a_planar_face_is_set_replaced_and_cleared() {
    let mut doc = block();
    let result = set_bed(&mut doc, "F2.b0", json!(top()));
    assert_eq!(result["recomputed"], 0, "no geometry changes");
    assert_eq!(document(&doc)["undo"], "Set Print Bed Face of Body1");
    assert_eq!(bed_face(&doc), json!({"face": top(), "faces": [top()]}));
    // The same again is no undo step.
    let depth = document(&doc)["undo_depth"].clone();
    set_bed(&mut doc, "F2.b0", json!(top()));
    assert_eq!(document(&doc)["undo_depth"], depth);
    // One face per body: another replaces it.
    set_bed(&mut doc, "F2.b0", json!(bottom()));
    assert_eq!(bed_face(&doc)["face"], bottom());
    set_bed(&mut doc, "F2.b0", Value::Null);
    assert_eq!(document(&doc)["undo"], "Clear Print Bed Face of Body1");
    assert_eq!(bed_face(&doc), Value::Null);
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(bed_face(&doc)["face"], bottom());
}

#[test]
fn a_face_that_cannot_lie_on_the_bed_is_refused() {
    let mut doc = block();
    doc.kernel().surfaces.borrow_mut().insert(
        SIDE.to_owned(),
        SurfaceGeometry::Cylinder {
            origin: [0.0, 10.0, 10.0],
            axis: [0.0, 0.0, 1.0],
            radius: 5.0,
        },
    );
    for (body, face, says) in [
        ("F2.b0", SIDE, "planar"),
        ("F2.b0", "F2:end(r{c9})", "has no face"),
        ("F2.b0", "no face at all", "no face name"),
        ("F9.b0", "F9:end(r{c1})", "F9.b0"),
    ] {
        let error = refused(
            &mut doc,
            json!({"cmd": "set_print_bed_face", "uid": body, "face": face}),
        );
        assert!(error.contains(says), "{face}: {error}");
    }
    assert_eq!(bed_face(&doc), Value::Null);
}

#[test]
fn the_bed_face_follows_edits_and_is_saved() {
    let mut doc = block();
    let plain = doc.to_json();
    set_bed(&mut doc, "F2.b0", json!(top()));
    // A taller block keeps its top as the bed face.
    command(
        &mut doc,
        json!({"cmd": "edit_feature", "uid": "F2", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": R}],
               "extent": {"type": "distance", "distance": 30}, "operation": "new_body"}}),
    );
    assert_eq!(bed_face(&doc), json!({"face": top(), "faces": [top()]}));

    let saved = doc.to_json();
    let file: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(file["bodies"][0]["print_bed_face"], top());
    let mut loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    assert_eq!(loaded.to_json(), saved);
    command(&mut loaded, json!({"cmd": "recompute"}));
    assert_eq!(bed_face(&loaded), bed_face(&doc));

    // A file of an older version has none, and is saved without one.
    assert!(!plain.contains("print_bed_face"));
    let mut old = Document::from_json(&plain, MockKernel::default()).unwrap();
    assert_eq!(old.to_json(), plain);
    command(&mut old, json!({"cmd": "recompute"}));
    assert_eq!(bed_face(&old), Value::Null);

    // A name that is no face name is refused.
    let broken = saved.replace(&top(), "no face at all");
    assert_ne!(broken, saved);
    let Err(error) = Document::from_json(&broken, MockKernel::default()) else {
        panic!("a broken face name was read");
    };
    assert!(error.to_string().contains("print_bed_face"), "{error}");
}

#[test]
fn a_box_on_its_top_is_turned_over_and_centred_on_the_plate() {
    let mut doc = block();
    let dir = folder("top");
    set_bed(&mut doc, "F2.b0", json!(top()));
    let bodies = query(&doc, json!({"query": "bodies"}));
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    let bed = &result["bed"];
    assert_eq!(bed["body"], "F2.b0");
    assert_eq!(bed["name"], "Body1");
    assert_eq!(bed["face"], top());
    assert_eq!(bed["plate"], json!([128.0, 128.0]));
    assert_eq!(bed["kept"], json!([]));
    assert_eq!(bed["problems"], json!([]));
    assert_eq!(result["warnings"], json!([]));
    // The top's outward normal straight down, the top on the bed.
    let down = turned(&result, [0.0, 0.0, 1.0]);
    assert!(near(down[2], -1.0), "{down:?}");
    let middle = moved(&result, [10.0, 10.0, 20.0]);
    assert!(near(middle[2], 0.0), "{middle:?}");
    assert_on_plate(placed_box(&model.parts[0]), [128.0, 128.0]);
    // The placement is the transform; the mesh is the body's own.
    let placement = model.parts[0].placements[0];
    let rows = &bed["transform"];
    for (r, shift) in placement.translation.iter().enumerate() {
        assert!(near(*shift, number(&rows[r][3])), "{placement:?}");
    }
    assert!(near(model.parts[0].mesh.volume(), 8000.0));

    // The design does not move.
    assert_eq!(query(&doc, json!({"query": "bodies"})), bodies);

    // The plate of the design's printer: the middle of its bed shape.
    command(
        &mut doc,
        json!({"cmd": "create_print_profile", "kind": "printer", "name": "Bench printer", "choose": true,
               "values": {"bed_shape": [[0, 0], [250, 0], [250, 210], [0, 210]]}}),
    );
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    assert_eq!(result["bed"]["plate"], json!([125.0, 105.0]));
    assert_on_plate(placed_box(&model.parts[0]), [125.0, 105.0]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_box_on_a_tilted_face_lies_on_it() {
    let mut doc = block();
    let dir = folder("tilted");
    // The side as a plane tilted 36.87° from the vertical, facing out and
    // down.
    let normal = [0.6, 0.0, -0.8];
    doc.kernel().surfaces.borrow_mut().insert(
        SIDE.to_owned(),
        SurfaceGeometry::Plane {
            origin: [20.0, 10.0, 0.0],
            normal,
        },
    );
    set_bed(&mut doc, "F2.b0", json!(SIDE));
    assert_eq!(bed_face(&doc), json!({"face": SIDE, "faces": [SIDE]}));
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    assert_eq!(result["bed"]["face"], SIDE);
    let down = turned(&result, normal);
    assert!(near(down[0], 0.0) && near(down[1], 0.0), "{down:?}");
    assert!(near(down[2], -1.0), "{down:?}");
    assert_on_plate(placed_box(&model.parts[0]), [128.0, 128.0]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_body_in_a_turned_component_lies_on_its_face_where_it_is_placed() {
    let mut doc = block();
    let dir = folder("component");
    let body: BodyUid = "F2.b0".parse().unwrap();
    doc.components_from_bodies(&[body]).unwrap();
    command(
        &mut doc,
        json!({"cmd": "set_occurrence_transform", "occurrence": "O1", "transform":
               {"translation": [100, 0, 0], "rotation": {"axis": [1, 0, 0], "angle": FRAC_PI_2}}}),
    );
    set_bed(&mut doc, "F2.b0", json!(top()));
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    // The top faces -Y in the design: that is turned down.
    let down = turned(&result, [0.0, -1.0, 0.0]);
    assert!(near(down[2], -1.0), "{down:?}");
    assert_on_plate(placed_box(&model.parts[0]), [128.0, 128.0]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The block and a second body under it, 10 high (F3.b0, Body2).
fn two_bodies() -> Document<MockKernel> {
    let mut doc = block();
    command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": R}],
               "extent": {"type": "distance", "distance": -10}, "operation": "new_body"}}),
    );
    doc
}

#[test]
fn several_bed_faces_keep_the_bodies_as_assembled() {
    let mut doc = two_bodies();
    let dir = folder("several");
    set_bed(&mut doc, "F2.b0", json!(top()));
    set_bed(&mut doc, "F3.b0", json!(format!("F3:start({R})")));
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0", "F3.b0"]));
    let bed = &result["bed"];
    assert_eq!(bed["kept"], json!(["F2.b0", "F3.b0"]));
    assert_eq!(bed["body"], Value::Null);
    assert_eq!(bed["transform"], Value::Null);
    for part in &model.parts {
        assert_eq!(part.placements, [IDENTITY]);
    }

    // One bed face among them: both turn onto it as one piece.
    set_bed(&mut doc, "F3.b0", Value::Null);
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0", "F3.b0"]));
    assert_eq!(result["bed"]["body"], "F2.b0");
    assert_eq!(result["bed"]["kept"], json!([]));
    assert_eq!(model.parts[0].placements, model.parts[1].placements);
    assert_ne!(model.parts[0].placements[0], IDENTITY);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_bed_face_that_is_gone_or_not_planar_is_reported() {
    let mut doc = block();
    let dir = folder("gone");
    set_bed(&mut doc, "F2.b0", json!(top()));
    // No longer planar: reported, the body as the design has it.
    doc.kernel().surfaces.borrow_mut().insert(
        top(),
        SurfaceGeometry::Cylinder {
            origin: [10.0, 10.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            radius: 10.0,
        },
    );
    assert_eq!(bed_face(&doc)["error"], NOT_PLANAR);
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    let bed = &result["bed"];
    assert_eq!(bed["transform"], Value::Null);
    assert_eq!(
        bed["problems"],
        json!([{"body": "F2.b0", "name": "Body1", "reason": NOT_PLANAR}])
    );
    assert_eq!(
        result["warnings"],
        json!(["Body1: the print bed face is not planar; printed as in the design"])
    );
    assert_eq!(model.parts[0].placements, [IDENTITY]);

    // Gone: a name the body has no face of, as an edit leaves it.
    let saved = doc.to_json().replace(&top(), "F2:end(r{c9})");
    let mut gone = Document::from_json(&saved, MockKernel::default()).unwrap();
    command(&mut gone, json!({"cmd": "recompute"}));
    assert_eq!(
        bed_face(&gone),
        json!({"face": "F2:end(r{c9})", "faces": [], "error": GONE})
    );
    let (result, _) = export_3mf(&mut gone, &dir, json!(["F2.b0"]));
    assert_eq!(
        result["warnings"],
        json!(["Body1: the print bed face is gone; printed as in the design"])
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_stl_file_lies_on_the_bed_too() {
    let mut doc = block();
    let dir = folder("stl");
    set_bed(&mut doc, "F2.b0", json!(top()));
    let path = dir.join("print.stl").to_str().unwrap().to_owned();
    let result = command(
        &mut doc,
        json!({"cmd": "export", "path": path, "bodies": ["F2.b0"], "ascii": true, "on_bed": true}),
    );
    assert_eq!(result["bed"]["face"], top());
    assert_eq!(result["warnings"], json!([]));
    let text = std::fs::read_to_string(&path).unwrap();
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for line in text.lines() {
        let Some(numbers) = line.trim().strip_prefix("vertex ") else {
            continue;
        };
        let p: Vec<f64> = numbers
            .split_whitespace()
            .map(|n| n.parse().unwrap())
            .collect();
        min = std::array::from_fn(|i| min[i].min(p[i]));
        max = std::array::from_fn(|i| max[i].max(p[i]));
    }
    assert_on_plate((min, max), [128.0, 128.0]);
    assert!(near(max[2], 20.0), "{max:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn only_3mf_and_stl_files_go_on_the_bed() {
    let mut doc = block();
    let dir = folder("formats");
    let step = dir.join("print.step").to_str().unwrap().to_owned();
    let error = refused(
        &mut doc,
        json!({"cmd": "export", "path": step, "on_bed": true}),
    );
    assert!(error.contains("only 3MF, STL and OBJ"), "{error}");
    let three = dir.join("print.3mf").to_str().unwrap().to_owned();
    let error = refused(
        &mut doc,
        json!({"cmd": "export", "path": three, "on_bed": true, "coordinates": "component"}),
    );
    assert!(error.contains("design's coordinates"), "{error}");
    // Without a bed face the bodies stay where the design has them.
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    assert_eq!(result["bed"]["body"], Value::Null);
    assert_eq!(result["bed"]["kept"], json!([]));
    assert_eq!(model.parts[0].placements, [IDENTITY]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_bed_face_follows_a_sketch_edit_that_renames_it() {
    let mut doc = block();
    let dir = folder("renamed");
    // The side y = 0 as a plane facing -Y.
    let plane = SurfaceGeometry::Plane {
        origin: [10.0, 0.0, 10.0],
        normal: [0.0, -1.0, 0.0],
    };
    give_surface(&doc, SIDE, plane.clone());
    set_bed(&mut doc, "F2.b0", json!(SIDE));
    // A fillet between c1 and c2 in the sketch: c1's side face now ends on
    // the fillet's arc, so its name changes.
    command(
        &mut doc,
        json!({"cmd": "sketch.fillet", "sketch": "F1", "a": "c1", "b": "c2", "radius": 3}),
    );
    let bed = bed_face(&doc);
    assert_eq!(bed["face"], SIDE, "the assigned name is kept");
    assert_eq!(bed["faces"].as_array().unwrap().len(), 1, "{bed}");
    let renamed = bed["faces"][0].as_str().unwrap().to_owned();
    assert_ne!(renamed, SIDE);
    assert!(renamed.starts_with("F2:side(c1["), "{renamed}");
    // The mock kernel knows no plane of the renamed face yet: the reason is
    // the resolution's own, not that the surface is no plane.
    assert!(bed["error"].is_string(), "{bed}");
    assert_ne!(bed["error"], NOT_PLANAR);
    // Its plane known, the renamed face lies on the bed.
    give_surface(&doc, &renamed, plane);
    assert_eq!(bed_face(&doc), json!({"face": SIDE, "faces": [renamed]}));
    let (result, _) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    let down = turned(&result, [0.0, -1.0, 0.0]);
    assert!(near(down[2], -1.0), "{down:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_component_placed_twice_lies_on_its_face_in_both_places() {
    let mut doc = block();
    let dir = folder("twice");
    let body: BodyUid = "F2.b0".parse().unwrap();
    doc.components_from_bodies(&[body]).unwrap();
    command(
        &mut doc,
        json!({"cmd": "copy_occurrence", "occurrence": "O1",
               "transform": {"translation": [100, 0, 0]}}),
    );
    set_bed(&mut doc, "F2.b0", json!(top()));
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    // The face applies to every occurrence: both turn over as one piece.
    assert_eq!(result["bed"]["body"], "F2.b0");
    assert_eq!(result["bed"]["kept"], json!([]));
    let placements = &model.parts[0].placements;
    assert_eq!(placements.len(), 2);
    for placement in placements {
        assert!(near(placement.linear[2][2], -1.0), "{placement:?}");
    }
    assert_on_plate(placed_box(&model.parts[0]), [128.0, 128.0]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_component_placed_twice_turned_differently_keeps_its_placement() {
    let mut doc = block();
    let dir = folder("twice-turned");
    let body: BodyUid = "F2.b0".parse().unwrap();
    doc.components_from_bodies(&[body]).unwrap();
    command(
        &mut doc,
        json!({"cmd": "copy_occurrence", "occurrence": "O1",
               "transform": {"translation": [100, 0, 0],
                             "rotation": {"axis": [1, 0, 0], "angle": FRAC_PI_2}}}),
    );
    set_bed(&mut doc, "F2.b0", json!(top()));
    let (result, model) = export_3mf(&mut doc, &dir, json!(["F2.b0"]));
    // The top faces +Z in one place and -Y in the other: no one turn lays
    // it on the bed in both, so the body stays as it is assembled (mitcad#737).
    let bed = &result["bed"];
    assert_eq!(bed["kept"], json!(["F2.b0"]));
    assert_eq!(bed["body"], Value::Null);
    assert_eq!(bed["face"], Value::Null);
    assert_eq!(bed["transform"], Value::Null);
    assert_eq!(bed["problems"], json!([]));
    assert_eq!(result["warnings"], json!([]));
    // The 3MF part's placements are the occurrences' own.
    let placements = &model.parts[0].placements;
    assert_eq!(placements.len(), 2);
    let (first, copy) = if placements[0].translation[0] < 50.0 {
        (placements[0], placements[1])
    } else {
        (placements[1], placements[0])
    };
    let numbers = |p: &mitcad_3mf::Placement| -> Vec<f64> {
        p.linear
            .iter()
            .flatten()
            .chain(&p.translation)
            .copied()
            .collect()
    };
    for (a, b) in numbers(&first).iter().zip(numbers(&IDENTITY)) {
        assert!(near(*a, b), "{first:?}");
    }
    assert!(near(copy.translation[0], 100.0), "{copy:?}");
    // The copy turned a quarter about X: its Y goes to Z, its Z to -Y.
    assert!(near(copy.linear[2][1], 1.0), "{copy:?}");
    assert!(near(copy.linear[1][2], -1.0), "{copy:?}");
    assert!(near(copy.linear[2][2], 0.0), "{copy:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_surface_body_has_no_print_bed_face() {
    let mut doc = block();
    let sheet = ImportBody {
        name: Some("Surface".to_owned()),
        color: None,
        shape: MockShape::imported("sheet", 4),
    };
    let added = doc.add_base_feature(BaseInput::new(vec![sheet])).unwrap();
    let surface = added.bodies[0].uid;
    let shape = doc.body_shape(surface).unwrap();
    let face = doc.kernel().faces(shape).unwrap()[0].names[0].clone();
    let plane = SurfaceGeometry::Plane {
        origin: [0.0; 3],
        normal: [1.0, 0.0, 0.0],
    };
    give_surface(&doc, &face, plane.clone());
    let error = refused(
        &mut doc,
        json!({"cmd": "set_print_bed_face", "uid": surface, "face": face}),
    );
    assert!(error.contains("no solid or mesh body"), "{error}");

    // One kept in a file anyway counts for nothing: the 3MF file leaves the
    // surface body out, and its face turns nothing.
    let mut file: Value = serde_json::from_str(&doc.to_json()).unwrap();
    for entry in file["bodies"].as_array_mut().unwrap() {
        if entry["uid"] == json!(surface) {
            entry["print_bed_face"] = json!(face);
        }
    }
    let text = file.to_string();
    let mut loaded = Document::from_json(&text, MockKernel::default()).unwrap();
    give_surface(&loaded, &face, plane);
    command(&mut loaded, json!({"cmd": "recompute"}));
    // The `bodies` query says why it is not used (mitcad#737), and the
    // face can still be cleared.
    let surface_bed = |doc: &Document<MockKernel>| {
        let bodies = query(doc, json!({"query": "bodies"}));
        let entry = bodies
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["uid"] == json!(surface))
            .unwrap_or_else(|| panic!("no {surface} in {bodies}"))
            .clone();
        entry.get("print_bed_face").cloned().unwrap_or(Value::Null)
    };
    let stored = surface_bed(&loaded);
    assert_eq!(stored["face"], json!(face), "{stored}");
    assert_eq!(stored["faces"].as_array().unwrap().len(), 1, "{stored}");
    assert_eq!(stored["error"], NOT_PRINTED, "{stored}");
    let dir = folder("surface");
    let path = dir.join("print.3mf").to_str().unwrap().to_owned();
    let result = command(
        &mut loaded,
        json!({"cmd": "export", "path": path, "on_bed": true}),
    );
    assert_eq!(result["skipped"][0]["body"], json!(surface));
    assert_eq!(result["bed"]["body"], Value::Null);
    assert_eq!(result["bed"]["transform"], Value::Null);
    assert_eq!(result["bed"]["problems"], json!([]));
    std::fs::remove_dir_all(&dir).unwrap();

    // Cleared as any body's face (the context menu's Clear only).
    command(
        &mut loaded,
        json!({"cmd": "set_print_bed_face", "uid": surface, "face": null}),
    );
    assert_eq!(surface_bed(&loaded), Value::Null);
    let label = format!("Clear Print Bed Face of {}", loaded.body_name(surface));
    assert_eq!(document(&loaded)["undo"], label);
}
