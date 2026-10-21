// SPDX-License-Identifier: MIT
//! Frames of members (mitcad#485), with the mock kernel: the last member of
//! a frame, its length bound to the first, closes the corner as its start
//! is made (a shared mitre cutting both after a mitred start, a square butt
//! after a square one), of slot profiles and of timber; a member on a
//! member's end lies as that member does; a member on the ground lies flat
//! or on edge.

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
