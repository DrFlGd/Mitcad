// SPDX-License-Identifier: MIT
//! Assembly constraints (mitcad#148) with the mock kernel: each type with
//! its flip, offset, limits and lock, several rows solved together and with
//! joints, degrees of freedom, drags, over-constrained and conflicting
//! sets named by rows, the static checks, the `constraint_row` query, and
//! suppress, delete and the design file.
//!
//! The mock's blocks are 10 x 10 x 5 at their component's origin: the
//! planar end face's middle is (5, 5, 5) with the normal +Z, the start
//! face's (5, 5, 0) with -Z. Circles, cylinders, spheres and side planes
//! are set on the mock per test.

use serde_json::{Value, json};

use crate::Document;
use crate::datum::{CurveGeometry, SurfaceGeometry, Vec3};
use crate::document_tests::{extrude, num};
use crate::features::SketchPlane;
use crate::ids::{BodyUid, ComponentUid, OccurrenceUid};
use crate::testing::MockKernel;
use crate::topo::EdgeName;
use crate::transform::Transform;

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let text = command.to_string();
    match doc.command(&text) {
        Ok(result) => serde_json::from_str(&result).unwrap(),
        Err(e) => panic!("{text}: {e}"),
    }
}

fn near(a: Vec3, b: Vec3) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-7)
}

fn assert_near(a: Vec3, b: Vec3) {
    assert!(near(a, b), "{a:?} != {b:?}");
}

fn block_in(doc: &mut Document<MockKernel>) -> BodyUid {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let uid = doc
        .add_feature(&extrude(sketch, &[&region], 5.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    BodyUid::new(uid, 0)
}

/// A plate (`Plate:1`, O1, at the origin, grounded) and a pin (`Pin:1`,
/// O2, at x = 100), each a block.
struct Rig {
    doc: Document<MockKernel>,
    o1: OccurrenceUid,
    o2: OccurrenceUid,
    plate: BodyUid,
    pin: BodyUid,
}

fn rig() -> Rig {
    let mut doc = Document::new(MockKernel::default());
    let (_, o1) = doc
        .create_component(Some("Plate"), Transform::IDENTITY, true)
        .unwrap();
    let plate = block_in(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let (_, o2) = doc
        .create_component(Some("Pin"), Transform::translation([100.0, 0.0, 0.0]), true)
        .unwrap();
    let pin = block_in(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    doc.set_occurrence_grounded(o1, true).unwrap();
    Rig {
        doc,
        o1,
        o2,
        plate,
        pin,
    }
}

impl Rig {
    fn run(&mut self, cmd: Value) -> Value {
        command(&mut self.doc, cmd)
    }

    fn refuse(&mut self, cmd: Value) -> String {
        self.doc.command(&cmd.to_string()).unwrap_err().to_string()
    }

    /// A command that adds or edits a feature that fails: its error.
    fn fails(&mut self, cmd: Value) -> String {
        let result = self.run(cmd);
        result["error"]
            .as_str()
            .unwrap_or_else(|| panic!("no error: {result}"))
            .to_owned()
    }

    fn faces(&self, body: BodyUid, role: &str) -> Vec<String> {
        self.doc
            .body_shape(body)
            .unwrap()
            .faces
            .iter()
            .filter(|f| f.role == role)
            .map(ToString::to_string)
            .collect()
    }

    fn face(&self, body: BodyUid, role: &str) -> String {
        self.faces(body, role).remove(0)
    }

    /// An edge between the face of `role` and a side face, made a circle.
    fn circle(&self, body: BodyUid, role: &str, center: Vec3, radius: f64) -> String {
        let shape = self.doc.body_shape(body).unwrap().clone();
        let (a, b) = shape
            .edges
            .iter()
            .find(|(a, b)| {
                (a.role == role) != (b.role == role) && (a.role == "side" || b.role == "side")
            })
            .expect("a rim edge");
        let edge = format!("E{{{a}|{b}}}")
            .parse::<EdgeName>()
            .unwrap()
            .to_string();
        self.doc.kernel().edge_curves.borrow_mut().insert(
            edge.clone(),
            CurveGeometry::Circle {
                center,
                normal: [0.0, 0.0, 1.0],
                radius,
                start: [center[0] + radius, center[1], center[2]],
                end: [center[0] + radius, center[1], center[2]],
            },
        );
        edge
    }

    /// The `i`-th side face, given a surface.
    fn side(&self, body: BodyUid, i: usize, surface: SurfaceGeometry) -> String {
        let face = self.faces(body, "side")[i].clone();
        self.doc
            .kernel()
            .surfaces
            .borrow_mut()
            .insert(face.clone(), surface);
        face
    }

    fn on_plate(&self, geometry: Value) -> Value {
        json!({"occurrence": self.o1.to_string(), "geometry": geometry})
    }

    fn on_pin(&self, geometry: Value) -> Value {
        json!({"occurrence": self.o2.to_string(), "geometry": geometry})
    }

    fn pin_face(&self, role: &str) -> Value {
        self.on_pin(json!({"body": self.pin, "face": self.face(self.pin, role)}))
    }

    fn plate_face(&self, role: &str) -> Value {
        self.on_plate(json!({"body": self.plate, "face": self.face(self.plate, role)}))
    }

    /// Adds a constraint set of `rows`; returns its uid.
    fn constrain(&mut self, rows: Value) -> String {
        let result = self.run(json!({"cmd": "add_constraint_set",
                                     "rows": rows}));
        result["uid"].as_str().unwrap().to_owned()
    }

    fn set_json(&self, uid: &str) -> Value {
        query(&self.doc, json!({"query": "joints"}))["constraints"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["uid"] == uid)
            .unwrap()
            .clone()
    }

    fn placement(&self, o: OccurrenceUid) -> Transform {
        self.doc.placement(o).unwrap()
    }

    fn pin_dof(&self) -> u64 {
        query(
            &self.doc,
            json!({"query": "joint_dof", "occurrence": "Pin:1"}),
        )["dof"]
            .as_u64()
            .unwrap()
    }
}

#[test]
fn planes_align_facing_the_same_way_or_offset() {
    let mut rig = rig();
    let uid = rig.constrain(json!([{"type": "align", "a": rig.pin_face("start"),
                                    "b": rig.plate_face("end")}]));
    let set = rig.set_json(&uid);
    assert_eq!(set["name"], "Constraint1");
    assert_eq!(set["state"], "placed", "{set}");
    assert_eq!(set["moved"], json!([rig.o2.to_string()]));
    assert_eq!(set["rows"][0]["holds"], true);
    // Facing: the pin stands on the plate where it was in x and y.
    let placed = rig.placement(rig.o2);
    assert_near(placed.translation, [100.0, 0.0, 5.0]);
    assert_near(placed.apply_vector([0.0, 0.0, 1.0]), [0.0, 0.0, 1.0]);
    // A plane on a plane leaves three motions.
    assert_eq!(rig.pin_dof(), 3);

    // Offset 2: 2 mm above; flipped: the same way, the pin turned over
    // below the plate's top.
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][0]["offset"] = json!(2);
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    assert_near(
        rig.placement(rig.o2).apply_point([5.0, 5.0, 0.0]),
        [105.0, 5.0, 7.0],
    );
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][0]["flip"] = json!(true);
    def["rows"][0]["offset"] = json!(0);
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    let placed = rig.placement(rig.o2);
    assert_near(placed.apply_vector([0.0, 0.0, -1.0]), [0.0, 0.0, 1.0]);
    assert!((placed.apply_point([5.0, 5.0, 0.0])[2] - 5.0).abs() < 1e-9);
}

#[test]
fn an_offset_between_limits_is_free_or_held_at_rest() {
    let mut rig = rig();
    // The pin starts at z = 0: its bottom 5 below the plate's top, outside
    // 1..8, so it comes up to 1; inside them it stays.
    let uid = rig.constrain(json!([{"type": "align", "a": rig.pin_face("start"),
                                    "b": rig.plate_face("end"),
                                    "limits": {"min": 1, "max": 8}}]));
    let set = rig.set_json(&uid);
    assert!(
        (set["rows"][0]["value"].as_f64().unwrap() - 1.0).abs() < 1e-9,
        "{set}"
    );
    assert!((rig.placement(rig.o2).translation[2] - 6.0).abs() < 1e-9);
    // Free between its limits: four motions.
    assert_eq!(rig.pin_dof(), 4);
    // Held at a rest value.
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][0]["limits"]["rest"] = json!("3 mm");
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    assert!((rig.placement(rig.o2).translation[2] - 8.0).abs() < 1e-9);
    // Limits in the wrong order, or a rest outside them, are refused.
    let error = rig.refuse(json!({"cmd": "add_constraint_set",
        "rows": [{"type": "align", "a": rig.pin_face("start"), "b": rig.plate_face("end"),
                  "limits": {"min": 8, "max": 1}}]}));
    assert!(
        error.contains("row 1 (align): limits: the minimum"),
        "{error}"
    );
}

#[test]
fn a_pin_inserts_into_a_hole_turning_freely_or_locked() {
    let mut rig = rig();
    let hole = rig.circle(rig.plate, "end", [5.0, 5.0, 5.0], 2.0);
    let rim = rig.circle(rig.pin, "start", [5.0, 5.0, 0.0], 2.0);
    let a = rig.on_pin(json!({"body": rig.pin, "edge": rim}));
    let b = rig.on_plate(json!({"body": rig.plate, "edge": hole}));
    // The circles' sides are their faces': the pin's rim looks down, the
    // hole's up: an insert.
    let row = query(&rig.doc, json!({"query": "constraint_row", "a": a, "b": b}));
    assert_eq!(row["kinds"], json!({"a": "circle", "b": "circle"}));
    assert_eq!(row["type"], "insert");
    assert_eq!(row["types"][0], "insert");
    assert!(
        row["types"]
            .as_array()
            .unwrap()
            .contains(&json!("insert_axis"))
    );
    assert!((row["measures"]["angle"].as_f64().unwrap() - std::f64::consts::PI).abs() < 1e-9);

    let uid = rig.constrain(json!([{"type": "insert", "a": a, "b": b, "offset": -5}]));
    assert_eq!(rig.set_json(&uid)["state"], "placed");
    // The pin's bottom is at the plate's bottom, its centre on the hole's.
    assert_near(rig.placement(rig.o2).translation, [0.0, 0.0, 0.0]);
    assert_eq!(rig.pin_dof(), 1);

    // Locked at 90 degrees about the axis: no motion left.
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][0]["lock_rotation"] = json!(true);
    def["rows"][0]["angle"] = json!("90 deg");
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    let placed = rig.placement(rig.o2);
    assert_near(placed.apply_vector([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    assert_near(placed.apply_point([5.0, 5.0, 0.0]), [5.0, 5.0, 0.0]);
    assert_eq!(rig.pin_dof(), 0);

    // Flipped: the rim looks the hole's way, the pin turned over.
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][0] = json!({"type": "insert", "a": a, "b": b, "flip": true});
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    let placed = rig.placement(rig.o2);
    assert_near(placed.apply_vector([0.0, 0.0, -1.0]), [0.0, 0.0, 1.0]);
    assert_near(placed.apply_point([5.0, 5.0, 0.0]), [5.0, 5.0, 5.0]);
}

#[test]
fn an_axis_insert_leaves_the_depth_free_and_a_drag_moves_it() {
    let mut rig = rig();
    let hole = rig.side(
        rig.plate,
        0,
        SurfaceGeometry::Cylinder {
            origin: [5.0, 5.0, 2.5],
            axis: [0.0, 0.0, 1.0],
            radius: 2.0,
        },
    );
    let rim = rig.circle(rig.pin, "start", [5.0, 5.0, 0.0], 2.0);
    let a = rig.on_pin(json!({"body": rig.pin, "edge": rim}));
    let b = rig.on_plate(json!({"body": rig.plate, "face": hole}));
    let row = query(&rig.doc, json!({"query": "constraint_row", "a": a, "b": b}));
    assert_eq!(row["type"], "insert_axis", "{row}");
    rig.constrain(json!([{"type": "insert_axis", "a": a, "b": b}]));
    // On the axis, at the height it had.
    assert_near(rig.placement(rig.o2).translation, [0.0, 0.0, 0.0]);
    assert_eq!(rig.pin_dof(), 2);
    // A drag along the axis moves it; across it, it stays on the axis.
    let dragged = query(
        &rig.doc,
        json!({"query": "joint_drag", "occurrence": "Pin:1",
                                         "point": [5, 5, 0], "target": [5, 5, 7]}),
    );
    let t = &dragged["placements"][0]["transform"];
    assert!((t[2][3].as_f64().unwrap() - 7.0).abs() < 1e-6, "{dragged}");
    rig.run(json!({"cmd": "drag_occurrence", "occurrence": "Pin:1",
                   "point": [5, 5, 0], "target": [9, 5, 3]}));
    let placed = rig.placement(rig.o2);
    let bottom = placed.apply_point([5.0, 5.0, 0.0]);
    assert!(
        (bottom[0] - 5.0).abs() < 1e-6 && (bottom[1] - 5.0).abs() < 1e-6,
        "{bottom:?}"
    );
    assert!((bottom[2] - 3.0).abs() < 1e-6, "{bottom:?}");
}

#[test]
fn an_angle_turns_the_pin_and_limits_hold_it() {
    let mut rig = rig();
    let uid = rig.constrain(json!([{"type": "angle", "a": rig.pin_face("end"),
                                    "b": rig.plate_face("end"), "angle": "90 deg"}]));
    assert_eq!(rig.set_json(&uid)["state"], "placed");
    let up = rig.placement(rig.o2).apply_vector([0.0, 0.0, 1.0]);
    assert!(up[2].abs() < 1e-9, "{up:?}");
    assert_eq!(rig.pin_dof(), 5);
    // Between 30 and 60 degrees from parallel: it comes to 30.
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][0] = json!({"type": "angle", "a": rig.pin_face("end"), "b": rig.plate_face("end"),
                            "limits": {"min": "30 deg", "max": "60 deg"}});
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    let set = rig.set_json(&uid);
    assert!(
        (set["rows"][0]["value"].as_f64().unwrap() - 30f64.to_radians()).abs() < 1e-9,
        "{set}"
    );
    // The limits are angles: parameters made for them are in degrees.
    let def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    let min = def["rows"][0]["limits"]["min"].as_str().unwrap().to_owned();
    let params = query(&rig.doc, json!({"query": "parameters"}));
    let p = params
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == min.as_str())
        .unwrap()
        .clone();
    assert_eq!(p["unit"], "deg", "{p}");
    // An angle outside 0..180 degrees is refused.
    let error = rig.refuse(json!({"cmd": "add_constraint_set",
        "rows": [{"type": "angle", "a": rig.pin_face("end"), "b": rig.plate_face("end"),
                  "angle": "200 deg"}]}));
    assert!(error.contains("between 0 and 180 degrees"), "{error}");
}

#[test]
fn center_and_tangent() {
    let mut rig = rig();
    let left = rig.side(
        rig.plate,
        0,
        SurfaceGeometry::Plane {
            origin: [0.0, 5.0, 2.5],
            normal: [-1.0, 0.0, 0.0],
        },
    );
    let right = rig.side(
        rig.plate,
        1,
        SurfaceGeometry::Plane {
            origin: [10.0, 5.0, 2.5],
            normal: [1.0, 0.0, 0.0],
        },
    );
    let ball = rig.side(
        rig.pin,
        0,
        SurfaceGeometry::Sphere {
            center: [5.0, 5.0, 2.5],
            radius: 2.0,
        },
    );
    // The pin's top centred between the plate's sides; the pin's ball on
    // the plate's top: centre 2 above it.
    let uid = rig.constrain(json!([
        {"type": "center", "a": rig.pin_face("end"),
         "b": rig.on_plate(json!({"body": rig.plate, "face": left})),
         "b2": rig.on_plate(json!({"body": rig.plate, "face": right}))},
        {"type": "tangent", "a": rig.on_pin(json!({"body": rig.pin, "face": ball})),
         "b": rig.plate_face("end")}
    ]));
    let set = rig.set_json(&uid);
    assert_eq!(set["rows"][0]["holds"], true, "{set}");
    assert_eq!(set["rows"][1]["holds"], true, "{set}");
    let placed = rig.placement(rig.o2);
    assert_near(placed.apply_point([5.0, 5.0, 2.5]), [5.0, 5.0, 7.0]);
    // The ball takes one motion, the center one.
    assert_eq!(rig.pin_dof(), 4);
    // Inside (flip): the ball's centre 2 below the plate's top.
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][1]["flip"] = json!(true);
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    assert!((rig.placement(rig.o2).apply_point([5.0, 5.0, 2.5])[2] - 3.0).abs() < 1e-9);
    // A tangent of a point is refused at recompute, naming the row.
    let mut def = query(&rig.doc, json!({"query": "feature", "uid": uid}))["def"].clone();
    def["rows"][1]["a"] = rig.on_pin(json!("origin"));
    rig.run(json!({"cmd": "edit_feature", "uid": uid, "def": def}));
    let set = rig.set_json(&uid);
    let error = set["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("row 2 (tangent): a tangent row takes"),
        "{set}"
    );
    assert_eq!(set["state"], "failed");
}

#[test]
fn rows_solve_with_joints_and_conflicts_name_them() {
    let mut rig = rig();
    // A planar joint holds the pin's bottom on the plate's top; a center
    // row then centres it in x, leaving two motions.
    let joint = rig.run(json!({"cmd": "add_joint", "kind": "planar", "flip": true,
                               "a": rig.pin_face("start"), "b": rig.plate_face("end")}));
    assert_eq!(joint["state"], "placed");
    let left = rig.side(
        rig.plate,
        0,
        SurfaceGeometry::Plane {
            origin: [0.0, 5.0, 2.5],
            normal: [-1.0, 0.0, 0.0],
        },
    );
    let right = rig.side(
        rig.plate,
        1,
        SurfaceGeometry::Plane {
            origin: [10.0, 5.0, 2.5],
            normal: [1.0, 0.0, 0.0],
        },
    );
    let center = json!({"type": "center", "a": rig.pin_face("end"),
                        "b": rig.on_plate(json!({"body": rig.plate, "face": left})),
                        "b2": rig.on_plate(json!({"body": rig.plate, "face": right}))});
    let uid = rig.constrain(json!([center]));
    assert_eq!(rig.set_json(&uid)["state"], "placed");
    let placed = rig.placement(rig.o2);
    assert!((placed.apply_point([5.0, 5.0, 5.0])[0] - 5.0).abs() < 1e-9);
    assert!((placed.translation[2] - 5.0).abs() < 1e-9);
    assert_eq!(rig.pin_dof(), 2);
    let joints = query(&rig.doc, json!({"query": "joints"}));
    let dof = &joints["dof"][0];
    assert_eq!(dof["overconstrained"], false, "{dof}");

    // A row against the joint: the pin's top on the plate's top.
    let error = rig.fails(json!({"cmd": "add_constraint_set",
        "rows": [{"type": "align", "a": rig.pin_face("end"), "b": rig.plate_face("end")}]}));
    assert!(
        error.contains("over-constrained: Joint1 and Constraint2 row 1 (align) cannot all hold"),
        "{error}"
    );
    // Two rows of one set that contradict each other.
    rig.run(json!({"cmd": "undo"}));
    rig.run(json!({"cmd": "delete_feature", "uid": joint["uid"]}));
    let error = rig.fails(json!({"cmd": "add_constraint_set",
        "rows": [{"type": "align", "a": rig.pin_face("start"), "b": rig.plate_face("end")},
                 {"type": "align", "a": rig.pin_face("end"), "b": rig.plate_face("end")}]}));
    assert!(error.contains("Constraint2 row 2 (align)"), "{error}");
    assert!(error.contains("Constraint2 row 1 (align)"), "{error}");
}

#[test]
fn rows_are_checked_before_they_are_solved() {
    let mut rig = rig();
    let a = rig.pin_face("start");
    let b = rig.plate_face("end");
    let cases = [
        (
            json!({"type": "align", "a": a, "b": b, "lock_rotation": true}),
            "lock_rotation is for insert rows",
        ),
        (
            json!({"type": "insert", "a": a, "b": b, "limits": {"min": 1}}),
            "limits are for align and angle rows",
        ),
        (
            json!({"type": "center", "a": a, "b": b, "offset": 1}),
            "offset is for align and insert rows",
        ),
        (
            json!({"type": "align", "a": a, "b": b, "a2": a}),
            "a2 and b2 are for center rows",
        ),
        (
            json!({"type": "align", "a": a, "b": a}),
            "a and b are on the same occurrence",
        ),
        (
            json!({"type": "align", "a": a, "b": b, "offset": 1, "limits": {"min": 0, "max": 2}}),
            "with limits",
        ),
    ];
    for (row, message) in cases {
        let error = rig.refuse(json!({"cmd": "add_constraint_set",
                                      "rows": [row]}));
        assert!(error.contains(message), "{message}: {error}");
    }
    let error = rig.refuse(json!({"cmd": "add_constraint_set",
                                  "rows": []}));
    assert!(error.contains("at least one row"), "{error}");
    // A plane is no insert's circle.
    let error = rig.fails(json!({"cmd": "add_constraint_set",
        "rows": [{"type": "insert", "a": a, "b": b}]}));
    assert!(
        error.contains("row 1 (insert): an insert's a is a circle"),
        "{error}"
    );
    let row = query(
        &rig.doc,
        json!({"query": "constraint_row", "a": a, "b": b, "type": "insert"}),
    );
    assert_eq!(row["fits"], false);
    assert_eq!(row["type"], "align");
}

#[test]
fn suppress_delete_and_the_design_file() {
    let mut rig = rig();
    let uid = rig.constrain(json!([{"type": "align", "a": rig.pin_face("start"),
                                    "b": rig.plate_face("end"), "offset": "2 mm"}]));
    assert_near(rig.placement(rig.o2).translation, [100.0, 0.0, 7.0]);
    rig.run(json!({"cmd": "suppress_feature", "uid": uid}));
    assert_near(rig.placement(rig.o2).translation, [100.0, 0.0, 0.0]);
    assert_eq!(rig.set_json(&uid)["state"], "suppressed");
    rig.run(json!({"cmd": "suppress_feature", "uid": uid, "suppressed": false}));
    assert_near(rig.placement(rig.o2).translation, [100.0, 0.0, 7.0]);

    let text = rig.doc.to_json();
    assert!(text.contains("\"constraint_set\""), "{text}");
    let mut copy = Document::from_json(&text, MockKernel::default()).unwrap();
    copy.recompute();
    assert_eq!(
        query(&rig.doc, json!({"query": "joints"})),
        query(&copy, json!({"query": "joints"}))
    );
    assert_eq!(copy.to_json(), text);
    assert_eq!(copy.placement(rig.o2), rig.doc.placement(rig.o2));

    // Deleting the pin deletes the set.
    let deleted = rig.run(json!({"cmd": "delete_occurrence", "occurrence": "Pin:1"}));
    assert!(
        deleted["deleted"].as_array().unwrap().contains(&json!(uid)),
        "{deleted}"
    );
    rig.run(json!({"cmd": "undo"}));
    assert_eq!(rig.set_json(&uid)["state"], "placed");
    let _ = rig.o1;
}
