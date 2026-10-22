// SPDX-License-Identifier: MIT
//! Frames of members (mitcad#485), with the mock kernel: the last member of
//! a frame, its length bound to the first, closes the corner as its start
//! is made (a shared mitre cutting both after a mitred start, a square butt
//! after a square one), of slot profiles and of timber; a member on a
//! member's end lies as that member does; a member on the ground lies flat
//! or on edge. A member lying on a plane rests on it, one on a member's
//! side lies as that member does, and a typed length that ends on a
//! member's end closes the corner too (mitcad#492).

use serde_json::{Value, json};

use super::test_library::*;
use crate::Document;
use crate::features::FeatureDef;
use crate::features::bound_length::BoundMode;
use crate::testing::MockKernel;

/// A part placed on the end face `end_b` of `t`, turned to `direction`
/// (none: straight on), with `extra` merged into the command.
fn on_end(
    doc: &Document<MockKernel>,
    t: &Value,
    direction: Option<[f64; 3]>,
    extra: Value,
) -> Value {
    let tp = derived(doc, component(t));
    let mut fields = json!({"part": {"source": "S1"},
        "at": {"occurrence": t["occurrence"],
               "geometry": {"body": format!("{tp}.b0"), "face": format!("{tp}:end_b")},
               "point": placed(doc, occurrence(t)).apply_point([0.0, 0.0, 0.0])}});
    if let Some(d) = direction {
        fields["at"]["direction"] = json!(d);
    }
    for (key, value) in extra.as_object().unwrap() {
        fields[key.clone()] = value.clone();
    }
    fields
}

/// The angles (radians) of a part's end cuts, A and B; 0 square.
fn cuts(doc: &Document<MockKernel>, p: &Value) -> [f64; 2] {
    let FeatureDef::DerivedPart(d) = &doc.feature(derived(doc, component(p))).unwrap().def else {
        panic!("a derived part");
    };
    [false, true].map(|b| {
        d.ends
            .end(b)
            .map_or(0.0, |c| doc.parameters().value(c.angle).unwrap())
    })
}

/// A part's length as the design has it now.
fn length_of(doc: &Document<MockKernel>, p: &Value) -> f64 {
    doc.part_info(component(p)).unwrap().length.unwrap().1
}

/// The mode of a part's bound length.
fn bound_mode(doc: &Document<MockKernel>, p: &Value) -> BoundMode {
    let bound = doc
        .assembly()
        .component(component(p))
        .unwrap()
        .build
        .as_ref()
        .unwrap()
        .bound[0];
    let FeatureDef::BoundLength(b) = &doc.feature(bound).unwrap().def else {
        panic!("a bound length");
    };
    b.mode
}

/// Places the last member of a frame on `t`'s end, turned to `direction`,
/// its length bound to what the preview's ray meets in `mode` (as the app's
/// auto length binds it); returns the preview and the answer.
fn close(
    doc: &mut Document<MockKernel>,
    t: &Value,
    direction: [f64; 3],
    extra: Value,
    mode: &str,
) -> (Value, Value) {
    let fields = on_end(doc, t, Some(direction), extra);
    let mut probe = fields.clone();
    probe["query"] = json!("build_preview");
    let target = query(doc, probe)["length"]["target"].clone();
    assert!(!target.is_null(), "the ray meets the first member");
    let mut fields = fields;
    fields["length"] = json!({"bind": target, "mode": mode, "add": 0});
    let mut probe = fields.clone();
    probe["query"] = json!("build_preview");
    let shown = query(doc, probe);
    (shown, place(doc, fields))
}

/// Sets a part's length.
fn lengthen(doc: &mut Document<MockKernel>, p: &Value, length: f64) {
    let id = doc.part_info(component(p)).unwrap().length.unwrap().0;
    let name = doc.parameters().name(id);
    command(
        doc,
        json!({"cmd": "set_parameter", "name": name, "value": length}),
    )
    .unwrap();
    all_ok(doc);
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

const QUARTER: f64 = std::f64::consts::FRAC_PI_4;

#[test]
fn a_mitred_frame_of_profiles_closes_with_a_shared_mitre() {
    // Whatever the auto length's mode: before mitcad#485 "through" ran the
    // last profile 30 mm into the first, "centre" 15 mm, both square.
    for mode in ["face", "centre", "through"] {
        let mut doc = design();
        doc.kernel().half_space_cuts.set(true);
        let shared = json!({"mitre": {"cut": "shared"}});
        // 1000 up along Z, 600 along Y, 1000 down, and back along -Y.
        let m1 = first(&mut doc, "p30", [0.0; 3], 1000.0);
        let mut f = on_end(&doc, &m1, Some([0.0, 1.0, 0.0]), shared.clone());
        f["length"] = json!(600);
        let m2 = place(&mut doc, f);
        let mut f = on_end(&doc, &m2, Some([0.0, 0.0, -1.0]), shared.clone());
        f["length"] = json!(1000);
        let m3 = place(&mut doc, f);
        let (shown, m4) = close(&mut doc, &m3, [0.0, -1.0, 0.0], shared.clone(), mode);
        all_ok(&doc);
        assert_eq!(shown["closing"]["mitre"], "shared", "{mode}: {shown}");
        assert_eq!(shown["closing"]["end"], "a", "{mode}: {shown}");
        assert_eq!(shown["length"]["mode"], "through", "{mode}: {shown}");
        assert!(
            near(shown["length"]["value"].as_f64().unwrap(), 600.0),
            "{mode}: {shown}"
        );
        // The outer length, to the first profile's outer side; both ends of
        // the corner cut by 45 degrees.
        assert_eq!(bound_mode(&doc, &m4), BoundMode::Through);
        assert!(
            near(length_of(&doc, &m4), 600.0),
            "{mode}: {}",
            length_of(&doc, &m4)
        );
        let [a, b] = cuts(&doc, &m4);
        assert!(near(a, QUARTER) && near(b, QUARTER), "{mode}: {a} {b}");
        let [a, b] = cuts(&doc, &m1);
        assert!(near(a, QUARTER) && near(b, QUARTER), "{mode}: {a} {b}");
        // An L at the corner the frame closed.
        let junctions = doc.build_junctions(Some(&[component(&m4)]));
        assert!(
            junctions
                .iter()
                .any(|j| j.kind == "L" && [j.a.component, j.b.component].contains(&component(&m1))),
            "{mode}: {junctions:?}"
        );
        // The binding stays: the frame 100 higher, the last one follows.
        lengthen(&mut doc, &m2, 700.0);
        assert!(
            near(length_of(&doc, &m4), 700.0),
            "{}",
            length_of(&doc, &m4)
        );
    }
}

#[test]
fn a_square_frame_of_profiles_closes_with_a_square_butt() {
    for mode in ["face", "through"] {
        let mut doc = design();
        doc.kernel().half_space_cuts.set(true);
        let none = json!({"mitre": {"cut": "none"}});
        let m1 = first(&mut doc, "p30", [0.0; 3], 1000.0);
        let mut f = on_end(&doc, &m1, Some([0.0, 1.0, 0.0]), none.clone());
        f["length"] = json!(600);
        let m2 = place(&mut doc, f);
        let mut f = on_end(&doc, &m2, Some([0.0, 0.0, -1.0]), none.clone());
        f["length"] = json!(1000);
        let m3 = place(&mut doc, f);
        // On m3's end face, flush with its outer side (y = 615), over m1's
        // end A: to m1's face (y = 15), not into it.
        let (shown, m4) = close(&mut doc, &m3, [0.0, -1.0, 0.0], none.clone(), mode);
        all_ok(&doc);
        assert_eq!(shown["closing"]["mitre"], "none", "{mode}: {shown}");
        assert_eq!(bound_mode(&doc, &m4), BoundMode::Face);
        assert!(
            near(length_of(&doc, &m4), 600.0),
            "{mode}: {}",
            length_of(&doc, &m4)
        );
        assert_eq!(cuts(&doc, &m4), [0.0, 0.0]);
        assert_eq!(cuts(&doc, &m1), [0.0, 0.0]);
    }
}

#[test]
fn a_member_away_from_the_first_s_end_keeps_its_auto_length() {
    // A T: the last member meets the first's side far from its ends.
    let mut doc = design();
    let m1 = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let mut f = on_end(
        &doc,
        &m1,
        Some([0.0, 1.0, 0.0]),
        json!({"mitre": {"cut": "shared"}}),
    );
    f["length"] = json!(600);
    let m2 = place(&mut doc, f);
    let mut f = on_end(
        &doc,
        &m2,
        Some([0.0, 0.0, -1.0]),
        json!({"mitre": {"cut": "shared"}}),
    );
    f["length"] = json!(500);
    let m3 = place(&mut doc, f);
    let (shown, m4) = close(
        &mut doc,
        &m3,
        [0.0, -1.0, 0.0],
        json!({"mitre": {"cut": "shared"}}),
        "centre",
    );
    assert!(shown["closing"].is_null(), "{shown}");
    assert_eq!(bound_mode(&doc, &m4), BoundMode::Centre);
    assert_eq!(cuts(&doc, &m1)[0], 0.0);
}

/// The world axis a placed part's own axis runs along.
fn axis_of(doc: &Document<MockKernel>, p: &Value, own: [f64; 3]) -> [f64; 3] {
    placed(doc, occurrence(p)).apply_vector(own)
}

fn along(v: [f64; 3], w: [f64; 3]) -> bool {
    (v[0] * w[0] + v[1] * w[1] + v[2] * w[2]).abs() > 1.0 - 1e-6
}

#[test]
fn a_member_on_the_ground_lies_flat_or_on_edge() {
    let mut doc = design();
    // 48 x 98: its thickness (48) along its Y.
    let flat = place(
        &mut doc,
        json!({"part": part("rows98"), "at": {"point": [0.0, 0.0, 0.0], "direction": [1.0, 0.0, 0.0]},
               "lie": "flat", "length": 1000}),
    );
    assert!(along(
        axis_of(&doc, &flat, [0.0, 1.0, 0.0]),
        [0.0, 0.0, 1.0]
    ));
    let edge = place(
        &mut doc,
        json!({"part": {"source": "S1"}, "at": {"point": [0.0, 500.0, 0.0], "direction": [0.0, 1.0, 0.0]},
               "lie": "edge", "length": 1000}),
    );
    assert!(along(
        axis_of(&doc, &edge, [1.0, 0.0, 0.0]),
        [0.0, 0.0, 1.0]
    ));
    // On the XY plane, along Y, flat.
    let on_plane = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [2000.0, 0.0, 0.0], "direction": [0.0, 1.0, 0.0]},
               "lie": "flat", "length": 1000}),
    );
    assert!(along(
        axis_of(&doc, &on_plane, [0.0, 1.0, 0.0]),
        [0.0, 0.0, 1.0]
    ));
    assert!(along(
        axis_of(&doc, &on_plane, [0.0, 0.0, 1.0]),
        [0.0, 1.0, 0.0]
    ));
    let on_edge = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [3000.0, 0.0, 0.0], "direction": [0.0, 1.0, 0.0]},
               "lie": "edge", "length": 1000}),
    );
    assert!(along(
        axis_of(&doc, &on_edge, [1.0, 0.0, 0.0]),
        [0.0, 0.0, 1.0]
    ));
}

#[test]
fn a_member_on_a_member_s_end_lies_as_it_does_and_a_timber_frame_closes() {
    let mut doc = design();
    doc.kernel().half_space_cuts.set(true);
    // A board lying flat along X: its thickness (its Y) along Z.
    let m1 = place(
        &mut doc,
        json!({"part": part("rows98"), "at": {"point": [0.0, 0.0, 0.0], "direction": [1.0, 0.0, 0.0]},
               "lie": "flat", "length": 1000}),
    );
    let up = [0.0, 0.0, 1.0];
    assert!(along(axis_of(&doc, &m1, [0.0, 1.0, 0.0]), up));
    // Straight on: the same lie, the ends the same width.
    let mut f = on_end(&doc, &m1, None, json!({}));
    f["length"] = json!(500);
    let straight = place(&mut doc, f);
    assert!(along(axis_of(&doc, &straight, [0.0, 1.0, 0.0]), up));
    // A corner in the plane the boards lie in: flat too (it came on edge,
    // turned 90 degrees about its axis), its faces flush with the first's.
    let shared = json!({"mitre": {"cut": "shared"}});
    let mut f = on_end(&doc, &m1, Some([0.0, 1.0, 0.0]), shared.clone());
    f["length"] = json!(600);
    let m2 = place(&mut doc, f);
    all_ok(&doc);
    assert!(along(axis_of(&doc, &m2, [0.0, 1.0, 0.0]), up));
    let (b1, b2) = (body(&doc, &m1), body(&doc, &m2));
    assert!(
        near(b1.min[2], b2.min[2]) && near(b1.max[2], b2.max[2]),
        "{b1:?} {b2:?}"
    );
    // Rotate still turns it: on edge.
    let mut f = on_end(&doc, &m2, Some([-1.0, 0.0, 0.0]), shared.clone());
    f["length"] = json!(1000);
    f["rotate"] = json!("90 deg");
    let mut probe = f.clone();
    probe["query"] = json!("build_preview");
    let turned = query(&doc, probe);
    let frame_x = turned["frame"]["x_axis"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect::<Vec<_>>();
    assert!(along([frame_x[0], frame_x[1], frame_x[2]], up), "{turned}");
    f.as_object_mut().unwrap().remove("rotate");
    let m3 = place(&mut doc, f);
    assert!(along(axis_of(&doc, &m3, [0.0, 1.0, 0.0]), up));
    // The last one bound to the first closes the frame: a shared mitre.
    let (shown, m4) = close(&mut doc, &m3, [0.0, -1.0, 0.0], shared, "face");
    all_ok(&doc);
    assert!(along(axis_of(&doc, &m4, [0.0, 1.0, 0.0]), up));
    assert_eq!(shown["closing"]["mitre"], "shared", "{shown}");
    assert!(
        near(length_of(&doc, &m4), 600.0),
        "{}",
        length_of(&doc, &m4)
    );
    let [a, b] = cuts(&doc, &m4);
    assert!(near(a, QUARTER) && near(b, QUARTER), "{a} {b}");
    assert!(near(cuts(&doc, &m1)[0], QUARTER));
}

/// A placed part's box in the design.
fn body(doc: &Document<MockKernel>, p: &Value) -> crate::kernel::BoundingBox {
    let local = doc.part_bounds(component(p)).unwrap();
    super::moved_box(&local, &placed(doc, occurrence(p)))
}

/// A member's mount frame reference.
fn mount_of(doc: &Document<MockKernel>, p: &Value, frame: &str) -> Value {
    json!({"mount": format!("{}:mount({frame})", derived(doc, component(p)))})
}

/// The preview of a placing.
fn preview(doc: &Document<MockKernel>, fields: &Value) -> Value {
    let mut probe = fields.clone();
    probe["query"] = json!("build_preview");
    query(doc, probe)
}

/// Points the same within 1e-6 mm.
fn same(a: [f64; 3], b: [f64; 3]) -> bool {
    super::test_library::close(a, b)
}

/// A preview frame's origin.
fn preview_origin(shown: &Value) -> [f64; 3] {
    let o = &shown["frame"]["origin"];
    [0, 1, 2].map(|i| o[i].as_f64().unwrap())
}

#[test]
fn a_member_lying_on_a_plane_rests_on_it_and_one_standing_is_unchanged() {
    let mut doc = design();
    // 48 x 98 along X on the ground (no plane picked: the XY plane), flat
    // and on edge: its lowest point at z = 0, not half below (mitcad#492).
    for (lie, high, y) in [("flat", 48.0, 0.0), ("edge", 98.0, 500.0)] {
        let fields = json!({"part": part("rows98"),
            "at": {"point": [0.0, y, 0.0], "direction": [1.0, 0.0, 0.0]},
            "lie": lie, "length": 1000});
        let shown = preview(&doc, &fields);
        let m = place(&mut doc, fields);
        let b = body(&doc, &m);
        assert!(near(b.min[2], 0.0) && near(b.max[2], high), "{lie}: {b:?}");
        // The preview shows it where it goes: end A's middle.
        let end_a = placed(&doc, occurrence(&m)).apply_point([0.0; 3]);
        assert!(same(preview_origin(&shown), end_a), "{lie}: {shown}");
    }
    // On the XY plane picked, from the click, the same.
    for (lie, high, y) in [("flat", 48.0, 1000.0), ("edge", 98.0, 1500.0)] {
        let fields = json!({"part": {"source": "S1"},
            "at": {"geometry": "xy", "point": [0.0, y, 0.0], "direction": [1.0, 0.0, 0.0]},
            "lie": lie, "length": 1000});
        let shown = preview(&doc, &fields);
        let m = place(&mut doc, fields);
        assert_eq!(m["place"], "geometry");
        all_ok(&doc);
        let b = body(&doc, &m);
        assert!(near(b.min[2], 0.0) && near(b.max[2], high), "{lie}: {b:?}");
        assert!(
            near(b.min[0], 0.0) && near(b.max[0], 1000.0),
            "{lie}: {b:?}"
        );
        let end_a = placed(&doc, occurrence(&m)).apply_point([0.0; 3]);
        assert!(same(preview_origin(&shown), end_a), "{lie}: {shown}");
    }
    // Rotate still turns it (flat to on edge), Flip puts its end B at the
    // click: both resting on the plane.
    let turned = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [0.0, 2000.0, 0.0], "direction": [1.0, 0.0, 0.0]},
               "lie": "flat", "rotate": "90 deg", "length": 1000}),
    );
    let b = body(&doc, &turned);
    assert!(near(b.min[2], 0.0) && near(b.max[2], 98.0), "{b:?}");
    let flipped = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [0.0, 2500.0, 0.0], "direction": [1.0, 0.0, 0.0]},
               "lie": "flat", "flip": true, "length": 1000}),
    );
    let b = body(&doc, &flipped);
    assert!(near(b.min[2], 0.0) && near(b.max[2], 48.0), "{b:?}");
    assert!(near(b.min[0], 0.0) && near(b.max[0], 1000.0), "{b:?}");
    let end_a = placed(&doc, occurrence(&flipped)).apply_point([0.0; 3]);
    assert!(near(end_a[0], 1000.0), "{end_a:?}");
    // Turned and flipped by edits (MODIFY's Rotate and Flip): resting still.
    let edited = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [0.0, 4000.0, 0.0], "direction": [1.0, 0.0, 0.0]},
               "lie": "flat", "length": 1000}),
    );
    for (edit, high) in [
        (json!({"rotate": "90 deg"}), 98.0),
        (json!({"flip": true}), 98.0),
        (json!({"rotate": "0 deg"}), 48.0),
    ] {
        let mut c = edit.clone();
        c["cmd"] = json!("build_edit");
        c["component"] = edited["component"].clone();
        command(&mut doc, c).unwrap();
        all_ok(&doc);
        let b = body(&doc, &edited);
        assert!(near(b.min[2], 0.0) && near(b.max[2], high), "{edit}: {b:?}");
        assert!(
            near(b.min[0], 0.0) && near(b.max[0], 1000.0),
            "{edit}: {b:?}"
        );
    }
    // Standing along the plane's normal: on its end at the click, as
    // before.
    let standing = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [3000.0, 0.0, 0.0], "direction": [0.0, 0.0, 1.0]},
               "length": 1000}),
    );
    let t = placed(&doc, occurrence(&standing));
    assert!(same(t.translation, [3000.0, 0.0, 0.0]), "{t:?}");
    let b = body(&doc, &standing);
    assert!(near(b.min[2], 0.0) && near(b.max[2], 1000.0), "{b:?}");
    all_ok(&doc);
}

#[test]
fn a_profile_lying_on_the_xz_plane_rests_on_its_plus_y_side() {
    let mut doc = design();
    // A 30 x 30 profile along X and along Z on XZ: on +Y, its lowest point
    // at y = 0.
    for (direction, x) in [([1.0, 0.0, 0.0], 0.0), ([0.0, 0.0, 1.0], 2000.0)] {
        let fields = json!({"part": part("p30"),
            "at": {"geometry": "xz", "point": [x, 0.0, 0.0], "direction": direction},
            "length": 1000});
        let shown = preview(&doc, &fields);
        let p = place(&mut doc, fields);
        all_ok(&doc);
        let b = body(&doc, &p);
        assert!(near(b.min[1], 0.0) && near(b.max[1], 30.0), "{b:?}");
        let end_a = placed(&doc, occurrence(&p)).apply_point([0.0; 3]);
        assert!(same(preview_origin(&shown), end_a), "{shown}");
    }
    // Along Y, the plane's normal: standing on its end, unchanged.
    let standing = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xz", "point": [5000.0, 0.0, 0.0], "direction": [0.0, 1.0, 0.0]},
               "length": 1000}),
    );
    let t = placed(&doc, occurrence(&standing));
    assert!(same(t.translation, [5000.0, 0.0, 0.0]), "{t:?}");
    let b = body(&doc, &standing);
    assert!(near(b.min[1], 0.0) && near(b.max[1], 1000.0), "{b:?}");
}

#[test]
fn a_member_on_a_member_s_side_lies_as_it_does() {
    let mut doc = design();
    // A 48 x 98 board flat along X: 98 across (Y), 48 up, resting on z = 0.
    let host = place(
        &mut doc,
        json!({"part": part("rows98"), "at": {"point": [0.0, 0.0, 0.0], "direction": [1.0, 0.0, 0.0]},
               "lie": "flat", "length": 3000}),
    );
    let (yp, xp) = (
        mount_of(&doc, &host, "face_yp"),
        mount_of(&doc, &host, "face_xp"),
    );
    let h = body(&doc, &host);
    assert!(
        near(h.max[2], 48.0) && near(h.min[1], -49.0) && near(h.max[1], 49.0),
        "{h:?}"
    );
    // Standing on its top face from end A at offset 0, not rotated: its 98
    // along the board's 98, its sides flush with the board's (it came
    // turned a quarter, 48 across, before mitcad#492).
    let fields = json!({"part": {"source": "S1"},
        "at": {"occurrence": host["occurrence"], "geometry": yp,
               "from": "a", "offset": 0},
        "length": 500});
    let shown = preview(&doc, &fields);
    let standing = place(&mut doc, fields);
    assert_eq!(standing["place"], "side");
    all_ok(&doc);
    let s = body(&doc, &standing);
    assert!(
        near(s.min[1], h.min[1]) && near(s.max[1], h.max[1]),
        "{s:?} {h:?}"
    );
    assert!(near(s.min[0], 0.0) && near(s.max[0], 48.0), "{s:?}");
    assert!(near(s.min[2], 48.0) && near(s.max[2], 548.0), "{s:?}");
    let end_a = placed(&doc, occurrence(&standing)).apply_point([0.0; 3]);
    assert!(same(preview_origin(&shown), end_a), "{shown}");
    // Rotate still turns it: 48 across, 98 along the board.
    let turned = place(
        &mut doc,
        json!({"part": {"source": "S1"}, "rotate": "90 deg",
               "at": {"occurrence": host["occurrence"], "geometry": yp,
                      "from": "a", "offset": 1000},
               "length": 500}),
    );
    let t = body(&doc, &turned);
    assert!(near(t.max[1] - t.min[1], 48.0), "{t:?}");
    assert!(near(t.min[0], 1000.0) && near(t.max[0], 1098.0), "{t:?}");
    // Its offset edited: still measured to its faces as it stands.
    command(
        &mut doc,
        json!({"cmd": "build_edit", "component": standing["component"], "at": {"offset": 200}}),
    )
    .unwrap();
    all_ok(&doc);
    let s = body(&doc, &standing);
    assert!(near(s.min[0], 200.0) && near(s.max[0], 248.0), "{s:?}");
    // Flipped by an edit: its end B on the board, lying as a fresh
    // placement with Flip does (its 98 across, flush).
    let fresh = place(
        &mut doc,
        json!({"part": {"source": "S1"}, "flip": true,
               "at": {"occurrence": host["occurrence"], "geometry": yp, "from": "a", "offset": 1500},
               "length": 500}),
    );
    let f = body(&doc, &fresh);
    assert!(
        near(f.min[1], h.min[1]) && near(f.max[1], h.max[1]),
        "{f:?}"
    );
    for flip in [true, false] {
        command(
            &mut doc,
            json!({"cmd": "build_edit", "component": standing["component"], "flip": flip}),
        )
        .unwrap();
        all_ok(&doc);
        let s = body(&doc, &standing);
        assert!(
            near(s.min[1], h.min[1]) && near(s.max[1], h.max[1]),
            "flip {flip}: {s:?}"
        );
        assert!(near(s.max[0] - s.min[0], 48.0), "flip {flip}: {s:?}");
        assert!(
            near(s.min[2], 48.0) && near(s.max[2], 548.0),
            "flip {flip}: {s:?}"
        );
    }
    // On the board's narrow side (+Y): its 48 already along the board's
    // 48, flush top and bottom.
    let narrow = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": host["occurrence"], "geometry": xp,
                      "from": "a", "offset": 2000},
               "length": 500}),
    );
    let n = body(&doc, &narrow);
    assert!(near(n.min[2], 0.0) && near(n.max[2], 48.0), "{n:?}");
    // A member lying across its top face by two points: flat on it.
    let across = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": host["occurrence"], "geometry": yp,
                      "point": [2600.0, -400.0, 48.0]},
               "to": {"point": [2600.0, 400.0, 48.0]}, "lie": "flat"}),
    );
    all_ok(&doc);
    let a = body(&doc, &across);
    assert!(near(a.min[2], 48.0) && near(a.max[2], 96.0), "{a:?}");
    assert!(near(a.max[0] - a.min[0], 98.0), "{a:?}");
}

/// Two posts 1000 high, 1500 apart along X, and the fields of a member on
/// the first's end, turned to +X, its start a shared mitre.
fn two_posts(doc: &mut Document<MockKernel>, post: &str) -> (Value, Value, Value) {
    let p1 = place(
        doc,
        json!({"part": part(post), "at": {"point": [0.0, 0.0, 0.0]}, "length": 1000}),
    );
    let p2 = place(
        doc,
        json!({"part": {"source": "S1"}, "at": {"point": [1500.0, 0.0, 0.0]}, "length": 1000}),
    );
    let fields = on_end(
        doc,
        &p1,
        Some([1.0, 0.0, 0.0]),
        json!({"mitre": {"cut": "shared"}}),
    );
    (p1, p2, fields)
}

#[test]
fn a_typed_length_that_ends_on_a_member_s_end_closes_the_corner() {
    // The posts' width along X.
    for (post, width) in [("rows98", 98.0), ("p30", 30.0)] {
        let mut doc = design();
        doc.kernel().half_space_cuts.set(true);
        let (p1, p2, fields) = two_posts(&mut doc, post);
        // The length an auto length runs: to the second post's outer side
        // (from the first's, 1500 + width), its face a width nearer.
        let target = preview(&doc, &fields)["length"]["target"].clone();
        assert!(!target.is_null(), "{post}: the ray meets the second post");
        let mut f = fields.clone();
        f["length"] = json!({"bind": target, "mode": "face", "add": 0});
        let through = preview(&doc, &f)["length"]["value"].as_f64().unwrap();
        assert!(near(through, 1500.0 + width), "{post}: {through}");
        let face = through - width;
        // Typed to its face: a square butt there, no overlap.
        let mut f = fields.clone();
        f["length"] = json!(face);
        let shown = preview(&doc, &f);
        assert_eq!(shown["closing"]["mitre"], "none", "{post}: {shown}");
        assert_eq!(shown["warnings"], json!([]), "{post}: {shown}");
        // Typed between: it would overlap, a warning says so.
        f["length"] = json!((face + through) / 2.0);
        let shown = preview(&doc, &f);
        assert!(shown["closing"].is_null(), "{post}: {shown}");
        assert!(
            shown["warnings"][0]
                .as_str()
                .unwrap()
                .contains("ends inside"),
            "{post}: {shown}"
        );
        // Typed to its outer side, as in the owner's frame: the far end
        // mitred as the start is, shared, cutting both (mitcad#492).
        f["length"] = json!(through);
        let shown = preview(&doc, &f);
        assert_eq!(shown["closing"]["mitre"], "shared", "{post}: {shown}");
        assert_eq!(
            shown["closing"]["component"], p2["component"],
            "{post}: {shown}"
        );
        assert_eq!(shown["closing"]["end"], "b", "{post}: {shown}");
        let top = place(&mut doc, f);
        all_ok(&doc);
        assert!(near(length_of(&doc, &top), through), "{post}");
        let [a, b] = cuts(&doc, &top);
        assert!(near(a, QUARTER) && near(b, QUARTER), "{post}: {a} {b}");
        assert!(near(cuts(&doc, &p1)[1], QUARTER), "{post}");
        assert!(near(cuts(&doc, &p2)[1], QUARTER), "{post}");
        assert!(near(cuts(&doc, &p2)[0], 0.0), "{post}");
    }
}

#[test]
fn a_member_by_two_points_ending_at_a_member_s_end_butts_square() {
    // The posts' half width along X.
    for (post, half) in [("rows98", 49.0), ("p30", 15.0)] {
        let mut doc = design();
        let p1 = place(
            &mut doc,
            json!({"part": part(post), "at": {"point": [0.0, 0.0, 0.0]}, "length": 1000}),
        );
        let p2 = place(
            &mut doc,
            json!({"part": {"source": "S1"}, "at": {"point": [1500.0, 0.0, 0.0]}, "length": 1000}),
        );
        // From the first post's side near its top (within its section of
        // the end: a corner) to the second's: on its face a square butt,
        // no cut, no warning (mitcad#492).
        let z = 1000.0 - half;
        let from = [half, 0.0, z];
        let points = |to_x: f64| {
            json!({"part": {"source": "S1"}, "at": {"point": from},
                   "to": {"point": [to_x, 0.0, z]}})
        };
        let face = preview(&doc, &points(1500.0 - half));
        assert_eq!(face["place"], "points", "{post}: {face}");
        assert_eq!(face["closing"]["mitre"], "none", "{post}: {face}");
        assert_eq!(
            face["closing"]["component"], p2["component"],
            "{post}: {face}"
        );
        assert_eq!(face["warnings"], json!([]), "{post}: {face}");
        // Into it (its centre line) or through it (its outer side): cut
        // back to its face, the same square butt (mitcad#509; before it, a
        // warning of the overlap).
        for to_x in [1500.0, 1500.0 + half] {
            let inside = preview(&doc, &points(to_x));
            assert_eq!(
                inside["closing"]["mitre"], "none",
                "{post} {to_x}: {inside}"
            );
            assert_eq!(
                inside["closing"]["component"], p2["component"],
                "{post} {to_x}: {inside}"
            );
            assert_eq!(inside["warnings"], json!([]), "{post} {to_x}: {inside}");
            assert_eq!(
                inside["points"]["end"]["cut"], true,
                "{post} {to_x}: {inside}"
            );
            assert!(
                near(
                    inside["length"]["value"].as_f64().unwrap(),
                    1500.0 - 2.0 * half
                ),
                "{post} {to_x}: {inside}"
            );
        }
        let butt = place(&mut doc, points(1500.0 - half));
        all_ok(&doc);
        assert!(near(length_of(&doc, &butt), 1500.0 - 2.0 * half), "{post}");
        assert_eq!(cuts(&doc, &butt), [0.0, 0.0], "{post}");
        assert_eq!(cuts(&doc, &p2), [0.0, 0.0], "{post}");
        assert_eq!(cuts(&doc, &p1), [0.0, 0.0], "{post}");
    }
}
