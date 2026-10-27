// SPDX-License-Identifier: MIT
//! Members by two points between members (mitcad#509), with the mock
//! kernel: the member runs between the points in 3D; an end whose point is
//! on or in another member is cut back to that member's face, so that a
//! member clicked between two studs fits between them; on a plane picked
//! (coplanar points) it lies on it, else it is free and turned by its lie.

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::kernel::BoundingBox;
use mitcad_model::testing::MockKernel;
use mitcad_model_build::prelude::*;
use mitcad_model_tests_support::test_library::*;

/// Two studs 48 x 98, 2400 high, standing along Z, 600 apart along X
/// (their 98 along X: their facing sides at x = 49 and x = 551).
fn two_studs(doc: &mut Document<MockKernel>) -> (Value, Value) {
    let s1 = first(doc, "rows98", [0.0, 0.0, 0.0], 2400.0);
    let s2 = place(
        doc,
        json!({"part": {"source": "S1"}, "at": {"point": [600.0, 0.0, 0.0]}, "length": 2400}),
    );
    (s1, s2)
}

/// The fields of a member by two points of the studs' timber.
fn between(from: [f64; 3], to: [f64; 3], lie: &str) -> Value {
    json!({"part": {"source": "S1"}, "at": {"point": from}, "to": {"point": to}, "lie": lie})
}

fn preview(doc: &Document<MockKernel>, fields: &Value) -> Value {
    let mut probe = fields.clone();
    probe["query"] = json!("build_preview");
    query(doc, probe)
}

/// A placed part's box in the design.
fn body(doc: &Document<MockKernel>, p: &Value) -> BoundingBox {
    let local = doc.part_bounds(component(p)).unwrap();
    mitcad_model::internal::build::moved_box(&local, &placed(doc, occurrence(p)))
}

fn length_of(doc: &Document<MockKernel>, p: &Value) -> f64 {
    doc.part_info(component(p)).unwrap().length.unwrap().1
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn along(v: [f64; 3], w: [f64; 3]) -> bool {
    (v[0] * w[0] + v[1] * w[1] + v[2] * w[2]).abs() > 1.0 - 1e-6
}

#[test]
fn a_member_between_two_studs_butts_both_whatever_their_points() {
    // On the studs' centre lines (as the snaps give them), on the sides
    // facing each other (the case of mitcad#509: no longer 19.5 mm), one
    // of each, and on the far side of the second (through it): always the
    // 502 mm between the facing sides, its ends on them.
    let cases = [
        ([0.0, 0.0, 1200.0], [600.0, 0.0, 1200.0], [true, true]),
        ([49.0, 0.0, 1200.0], [551.0, 0.0, 1200.0], [false, false]),
        ([0.0, 0.0, 1200.0], [551.0, 0.0, 1200.0], [true, false]),
        ([49.0, 0.0, 1200.0], [649.0, 0.0, 1200.0], [false, true]),
    ];
    for (from, to, cut) in cases {
        let mut doc = design();
        let (s1, s2) = two_studs(&mut doc);
        let fields = between(from, to, "flat");
        let shown = preview(&doc, &fields);
        assert_eq!(shown["place"], "points", "{from:?}: {shown}");
        assert!(
            near(shown["length"]["value"].as_f64().unwrap(), 502.0),
            "{from:?}: {shown}"
        );
        assert_eq!(shown["warnings"], json!([]), "{from:?}: {shown}");
        let points = &shown["points"];
        assert_eq!(points["lies"], "free", "{from:?}: {shown}");
        assert_eq!(points["start"]["on"], s1["component"], "{from:?}: {shown}");
        assert_eq!(points["end"]["on"], s2["component"], "{from:?}: {shown}");
        assert_eq!(points["start"]["cut"], cut[0], "{from:?}: {shown}");
        assert_eq!(points["end"]["cut"], cut[1], "{from:?}: {shown}");
        let m = place(&mut doc, fields);
        all_ok(&doc);
        assert!(near(length_of(&doc, &m), 502.0), "{from:?}");
        let b = body(&doc, &m);
        assert!(
            near(b.min[0], 49.0) && near(b.max[0], 551.0),
            "{from:?}: {b:?}"
        );
        // Flat, free: its 48 up (the host's +Z), centred on the points.
        assert!(
            near(b.min[2], 1176.0) && near(b.max[2], 1224.0),
            "{from:?}: {b:?}"
        );
        assert!(
            near(b.min[1], -49.0) && near(b.max[1], 49.0),
            "{from:?}: {b:?}"
        );
        // Placed anew on edge from the points it was given: the same cut.
        command(
            &mut doc,
            json!({"cmd": "build_edit", "component": m["component"], "lie": "edge"}),
        )
        .unwrap();
        all_ok(&doc);
        assert!(near(length_of(&doc, &m), 502.0), "{from:?}");
        let b = body(&doc, &m);
        assert!(
            near(b.min[0], 49.0) && near(b.max[0], 551.0),
            "{from:?}: {b:?}"
        );
        assert!(
            near(b.min[2], 1151.0) && near(b.max[2], 1249.0),
            "{from:?}: {b:?}"
        );
    }
}

#[test]
fn a_member_between_two_studs_by_flipped_points_butts_both_too() {
    let mut doc = design();
    let (s1, s2) = two_studs(&mut doc);
    let mut fields = between([0.0, 0.0, 1200.0], [600.0, 0.0, 1200.0], "flat");
    fields["flip"] = json!(true);
    let shown = preview(&doc, &fields);
    assert_eq!(shown["points"]["start"]["on"], s1["component"], "{shown}");
    assert_eq!(shown["points"]["end"]["on"], s2["component"], "{shown}");
    let m = place(&mut doc, fields);
    all_ok(&doc);
    assert!(near(length_of(&doc, &m), 502.0));
    let b = body(&doc, &m);
    assert!(near(b.min[0], 49.0) && near(b.max[0], 551.0), "{b:?}");
}

#[test]
fn a_brace_on_the_studs_front_faces_lies_on_their_plane() {
    let mut doc = design();
    let (s1, s2) = two_studs(&mut doc);
    // The studs' fronts (their y- sides, y = -24) are one plane: picked
    // on the first one's, the brace lies on it, in front of both.
    let front = json!({"mount": format!("{}:mount(face_yn)", derived(&doc, component(&s1)))});
    let from = [0.0, -24.0, 300.0];
    let to = [600.0, -24.0, 2100.0];
    let fields = json!({"part": {"source": "S1"},
                        "at": {"occurrence": s1["occurrence"], "geometry": front, "point": from},
                        "to": {"point": to}, "lie": "flat"});
    let shown = preview(&doc, &fields);
    let points = &shown["points"];
    assert_eq!(points["lies"], "plane", "{shown}");
    assert_eq!(points["start"]["on"], s1["component"], "{shown}");
    assert_eq!(points["end"]["on"], s2["component"], "{shown}");
    assert_eq!(points["start"]["cut"], false, "{shown}");
    assert_eq!(points["end"]["cut"], false, "{shown}");
    let diagonal = (600.0_f64 * 600.0 + 1800.0 * 1800.0).sqrt();
    let m = place(&mut doc, fields);
    all_ok(&doc);
    assert!(
        (length_of(&doc, &m) - diagonal).abs() < 1e-3,
        "{}",
        length_of(&doc, &m)
    );
    // Flat on the fronts: its 48 across the plane, in front of it.
    let b = body(&doc, &m);
    assert!(near(b.max[1], -24.0) && near(b.min[1], -72.0), "{b:?}");
    // Its axis along the diagonal.
    let d = [600.0 / diagonal, 0.0, 1800.0 / diagonal];
    assert!(along(
        placed(&doc, occurrence(&m)).apply_vector([0.0, 0.0, 1.0]),
        d
    ));
}

#[test]
fn a_member_between_points_on_faces_not_in_one_plane_is_free_and_turned_by_its_lie() {
    // From the first stud's outer side (x = -49) down to the ground: a
    // strut.
    let from = [-49.0, 0.0, 1200.0];
    let to = [-1500.0, 0.0, 0.0];
    for lie in ["flat", "edge"] {
        let mut doc = design();
        let (s1, _) = two_studs(&mut doc);
        let fields = between(from, to, lie);
        let shown = preview(&doc, &fields);
        let points = &shown["points"];
        assert_eq!(points["lies"], "free", "{lie}: {shown}");
        assert_eq!(points["start"]["on"], s1["component"], "{lie}: {shown}");
        assert!(points["end"]["on"].is_null(), "{lie}: {shown}");
        assert_eq!(points["start"]["cut"], false, "{lie}: {shown}");
        let m = place(&mut doc, fields);
        all_ok(&doc);
        let distance = (1451.0_f64 * 1451.0 + 1200.0 * 1200.0).sqrt();
        assert!((length_of(&doc, &m) - distance).abs() < 1e-3, "{lie}");
        let t = placed(&doc, occurrence(&m));
        // Its axis through the points, from the start.
        assert!(close(t.translation, from), "{lie}: {:?}", t.translation);
        assert!(along(
            t.apply_vector([0.0, 0.0, 1.0]),
            [-1451.0 / distance, 0.0, -1200.0 / distance]
        ));
        // "Up" the host's +Z: flat its 48 (Y) in the vertical plane of
        // its axis, its 98 (X) level; on edge the other way round.
        let level = if lie == "flat" {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        assert!(along(t.apply_vector(level), [0.0, 1.0, 0.0]), "{lie}");
    }
}

#[test]
fn members_whose_points_are_in_members_that_meet_are_refused() {
    let mut doc = design();
    // Two studs side by side, touching at x = 49.
    first(&mut doc, "rows98", [0.0, 0.0, 0.0], 2400.0);
    place(
        &mut doc,
        json!({"part": {"source": "S1"}, "at": {"point": [98.0, 0.0, 0.0]}, "length": 2400}),
    );
    let refused = command(
        &mut doc,
        json!({"cmd": "build_place", "part": {"source": "S1"},
               "at": {"point": [-20.0, 0.0, 1200.0]}, "to": {"point": [120.0, 0.0, 1200.0]}}),
    )
    .unwrap_err();
    assert!(
        refused.contains("nothing is left between them"),
        "{refused}"
    );
}
