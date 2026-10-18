// SPDX-License-Identifier: MIT
//! Placing BUILD parts with the mock kernel: the first part grounded, a
//! profile on a side's slot and on an end for each system, the side or end
//! a pick decides, offsets from either end, rotation and flip, mitres
//! (new, existing, shared), the derived part's names and frames, and the
//! refusals.

use serde_json::json;

use super::test_library::*;
use crate::features::FeatureDef;
use crate::transform::Transform;

fn axis(t: &Transform) -> [f64; 3] {
    t.apply_vector([0.0, 0.0, 1.0])
}

#[test]
fn the_first_part_is_grounded_where_it_is_placed() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [10.0, 20.0, 0.0], 1000.0);
    all_ok(&doc);
    assert_eq!(t["place"], "grounded");
    assert_eq!(t["name"], "Profile 30x30 B8 L=1000");
    let o = occurrence(&t);
    assert!(doc.assembly().occurrence(o).unwrap().grounded);
    assert!(close(placed(&doc, o).translation, [10.0, 20.0, 0.0]));
    // Its source is kept once; its derived part names its faces after the
    // source's and its ends end_a and end_b.
    let source = &doc.state().library_sources;
    assert_eq!(source.0.len(), 1);
    assert_eq!(source.0[0].supplier, "Test supplier");
    let part = derived(&doc, component(&t));
    let body = crate::ids::BodyUid::new(part, 0);
    let shape = doc.body_shape(body).unwrap();
    let names: Vec<String> = shape.faces.iter().map(ToString::to_string).collect();
    assert!(names.contains(&format!("{part}:end_a")), "{names:?}");
    assert!(names.contains(&format!("{part}:end_b")), "{names:?}");
    assert!(
        names
            .iter()
            .any(|n| n.starts_with(&format!("{part}:part(")) && n.contains(":side(")),
        "{names:?}"
    );
    // Its frames are the source's mount frames.
    let frames = doc.part_info(component(&t)).unwrap().mounts;
    assert!(close(frames["end_b"].translation, [0.0, 0.0, 1000.0]));
    assert!(close(frames["slot_x+_1"].translation, [15.0, 0.0, 0.0]));
    // The design records what BUILD made.
    let record = doc
        .assembly()
        .component(component(&t))
        .unwrap()
        .build
        .clone()
        .unwrap();
    assert_eq!(record.role, super::BuildRole::Profile);
    assert_eq!(record.part, Some(part));
    assert_eq!(doc.undo_label(), Some("Place Profile 30x30 B8 L=1000"));
}

#[test]
fn a_profile_on_a_side_stands_on_the_slot_for_each_system() {
    for (component_id, w) in [("p20", 20.0), ("p30", 30.0), ("p40", 40.0)] {
        let mut doc = design();
        let t = first(&mut doc, component_id, [0.0; 3], 1000.0);
        let tp = derived(&doc, component(&t));
        let p = place(
            &mut doc,
            json!({"part": {"source": "S1"},
                   "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_x+_1)")},
                          "offset": 100},
                   "length": 500}),
        );
        all_ok(&doc);
        assert_eq!(p["place"], "side");
        let at = placed(&doc, occurrence(&p));
        // End A on the slot's centre line, its near face 100 from the
        // target's end A (its centre half a section further), the profile
        // out along +X.
        assert!(
            close(at.translation, [w / 2.0, 0.0, 100.0 + w / 2.0]),
            "{component_id}: {at:?}"
        );
        assert!(close(axis(&at), [1.0, 0.0, 0.0]), "{component_id}: {at:?}");
        // A slider along the slot.
        let joint = p["joint"].as_str().unwrap().parse().unwrap();
        let FeatureDef::Joint(j) = &doc.feature(joint).unwrap().def else {
            panic!("a joint");
        };
        assert_eq!(j.kind, crate::joints::JointKind::Slider);
        assert!(j.flip);
    }
}

#[test]
fn a_rail_lies_along_a_slot_on_its_base_frame() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    let r = place(
        &mut doc,
        json!({"part": part("rail15"),
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_x+_1)")},
                      "offset": 100},
               "length": 400}),
    );
    all_ok(&doc);
    assert_eq!(r["place"], "side");
    // A slider on the slot by its face frame, not its end.
    let rp = derived(&doc, component(&r));
    let joint = r["joint"].as_str().unwrap().parse().unwrap();
    let FeatureDef::Joint(j) = &doc.feature(joint).unwrap().def else {
        panic!("a joint");
    };
    assert_eq!(j.kind, crate::joints::JointKind::Slider);
    assert_eq!(
        j.a.geometry,
        crate::features::GeomRef::Mount {
            feature: rp,
            name: "base".to_owned()
        }
    );
    // Flat on the slot's face (x = 15), its length along the profile from
    // 100 to 500.
    let local = doc.part_bounds(component(&r)).unwrap();
    let b = super::moved_box(&local, &placed(&doc, occurrence(&r)));
    assert!(close(b.min, [15.0, -7.5, 100.0]), "{b:?}");
    assert!(close(b.max, [30.0, 7.5, 500.0]), "{b:?}");
}

#[test]
fn a_pick_on_a_face_decides_side_or_end() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    // An end face: on the end, inline.
    let e = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"],
                      "geometry": {"body": format!("{tp}.b0"), "face": format!("{tp}:end_b")}},
               "length": 300}),
    );
    all_ok(&doc);
    assert_eq!(e["place"], "end");
    let at = placed(&doc, occurrence(&e));
    assert!(close(at.translation, [0.0, 0.0, 1000.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 0.0, 1.0]));
    // A side face near a point: the nearest slot, centred on the point's
    // distance along the profile (between the snaps: its near face 407
    // from end A, not near a grid point of 30).
    let s = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"],
                      "geometry": {"body": format!("{tp}.b0"), "face": format!("{tp}:part(F2:side(c1))")},
                      "point": [0.0, 15.0, 422.0]},
               "length": 200}),
    );
    all_ok(&doc);
    assert_eq!(s["place"], "side");
    let at = placed(&doc, occurrence(&s));
    assert!(close(at.translation, [0.0, 15.0, 422.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 1.0, 0.0]));
    // `place: end` on a side pick takes the nearest end.
    let n = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"],
                      "geometry": {"body": format!("{tp}.b0"), "face": format!("{tp}:part(F2:side(c1))")},
                      "point": [0.0, 15.0, 20.0], "place": "end"},
               "length": 200}),
    );
    assert_eq!(n["place"], "end");
    let at = placed(&doc, occurrence(&n));
    assert!(close(at.translation, [0.0, 0.0, 0.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 0.0, -1.0]));
}

#[test]
fn offsets_from_end_b_follow_the_target_s_length() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    let p = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_y+_1)")},
                      "offset": 50, "from": "b"},
               "length": 200, "rotate": "90 deg"}),
    );
    all_ok(&doc);
    let at = placed(&doc, occurrence(&p));
    assert!(close(at.translation, [0.0, 15.0, 935.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 1.0, 0.0]));
    // Rotated a quarter turn about its axis.
    let x = at.apply_vector([1.0, 0.0, 0.0]);
    assert!((x[0].abs() - 1.0).abs() < 1e-6, "{x:?}");
    // The target gets longer: the part keeps 50 from end B.
    let t_length = doc.part_info(component(&t)).unwrap().length.unwrap().0;
    let name = doc.parameters().name(t_length);
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": name, "value": 1200}),
    )
    .unwrap();
    all_ok(&doc);
    assert!(close(
        placed(&doc, occurrence(&p)).translation,
        [0.0, 15.0, 1135.0]
    ));
    // An offset alone keeps its reference (mitcad#417): 80 from end B.
    let c = p["component"].clone();
    command(
        &mut doc,
        json!({"cmd": "build_edit", "component": c, "at": {"offset": 80}}),
    )
    .unwrap();
    assert!(close(
        placed(&doc, occurrence(&p)).translation,
        [0.0, 15.0, 1105.0]
    ));
    // Edited to 80 from end A, then flipped: end B on the slot.
    command(
        &mut doc,
        json!({"cmd": "build_edit", "component": c, "at": {"offset": 80, "from": "a"}}),
    )
    .unwrap();
    assert!(close(
        placed(&doc, occurrence(&p)).translation,
        [0.0, 15.0, 95.0]
    ));
    command(
        &mut doc,
        json!({"cmd": "build_edit", "component": c, "flip": true}),
    )
    .unwrap();
    all_ok(&doc);
    let at = placed(&doc, occurrence(&p));
    // End B on the slot: end A 200 out along +Y.
    assert!(
        close(at.apply_point([0.0, 0.0, 200.0]), [0.0, 15.0, 95.0]),
        "{at:?}"
    );
    assert!(close(at.translation, [0.0, 215.0, 95.0]), "{at:?}");
}

#[test]
fn a_bent_end_and_the_shared_mitre() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    let p = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(end_b)")},
                      "direction": [0, 1, 0]},
               "length": 400, "mitre": {"cut": "shared"}}),
    );
    all_ok(&doc);
    assert_eq!(p["place"], "end");
    let at = placed(&doc, occurrence(&p));
    assert!(close(at.translation, [0.0, 0.0, 1000.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 1.0, 0.0]), "{at:?}");
    let quarter = std::f64::consts::FRAC_PI_4;
    let end = |part: crate::ids::FeatureUid, b: bool| {
        let FeatureDef::DerivedPart(d) = &doc.feature(part).unwrap().def else {
            panic!("a derived part");
        };
        let cut = if b {
            d.ends.b.clone()
        } else {
            d.ends.a.clone()
        };
        cut.map(|c| (doc.parameters().value(c.angle).unwrap(), c.tilt))
    };
    let pp = derived(&doc, component(&p));
    let (angle, tilt) = end(pp, false).unwrap();
    assert!((angle - quarter).abs() < 1e-9);
    assert_eq!(tilt, Some(crate::features::derived_part::Tilt::X));
    let (angle, tilt) = end(tp, true).unwrap();
    assert!((angle - quarter).abs() < 1e-9);
    assert_eq!(tilt, Some(crate::features::derived_part::Tilt::X));
    assert!(end(tp, false).is_none());
    // Both ends are cut, their faces keep the end names.
    for part in [pp, tp] {
        let shape = doc.body_shape(crate::ids::BodyUid::new(part, 0)).unwrap();
        assert!(shape.history.contains("cut("), "{}", shape.history);
    }
    let shape = doc.body_shape(crate::ids::BodyUid::new(tp, 0)).unwrap();
    assert!(
        shape
            .faces
            .iter()
            .any(|f| f.to_string() == format!("{tp}:end_b"))
    );
}

#[test]
fn mitres_new_and_existing_on_an_end_and_new_on_a_side() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    let existing = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(end_b)")},
                      "direction": [1, 0, 0]},
               "length": 300, "mitre": {"cut": "existing", "angle": "30 deg"}}),
    );
    all_ok(&doc);
    let FeatureDef::DerivedPart(target) = &doc.feature(tp).unwrap().def else {
        panic!();
    };
    let cut = target.ends.b.clone().unwrap();
    assert!((doc.parameters().value(cut.angle).unwrap() - 30f64.to_radians()).abs() < 1e-9);
    assert_eq!(cut.tilt, Some(crate::features::derived_part::Tilt::Y));
    let ep = derived(&doc, component(&existing));
    let FeatureDef::DerivedPart(new) = &doc.feature(ep).unwrap().def else {
        panic!();
    };
    assert!(new.ends.is_square());
    // New on a side: only the new part's end.
    let side = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_x-_1)")},
                      "offset": 500},
               "length": 300, "mitre": {"cut": "new", "angle": "45 deg"}}),
    );
    all_ok(&doc);
    let sp = derived(&doc, component(&side));
    let FeatureDef::DerivedPart(d) = &doc.feature(sp).unwrap().def else {
        panic!();
    };
    assert!(d.ends.a.is_some() && d.ends.b.is_none());
    let error = command(
        &mut doc,
        json!({"cmd": "build_place", "part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_y-_1)")}},
               "length": 300, "mitre": {"cut": "shared"}}),
    )
    .unwrap_err();
    assert!(error.contains("only the new part's end"), "{error}");
}

#[test]
fn refusals_change_nothing() {
    let mut doc = design();
    let t = first(&mut doc, "p20", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    let revision = doc.revision();
    let refused = |doc: &mut crate::Document<crate::testing::MockKernel>,
                   fields: serde_json::Value| {
        let mut fields = fields;
        fields["cmd"] = json!("build_place");
        command(doc, fields).unwrap_err()
    };
    // A panel through build_place.
    let e = refused(&mut doc, json!({"part": part("panel")}));
    assert!(e.contains("build_panel"), "{e}");
    // A cap of another system on the end.
    let e = refused(
        &mut doc,
        json!({"part": part("cap30"),
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(end_b)")}}}),
    );
    assert!(e.contains("the systems do not fit"), "{e}");
    // A length beyond the part's limits.
    let e = refused(&mut doc, json!({"part": {"source": "S1"}, "length": 7000}));
    assert!(e.contains("longer than the part's longest"), "{e}");
    // A rotation that is not a quarter turn.
    let e = refused(
        &mut doc,
        json!({"part": {"source": "S1"}, "rotate": "30 deg"}),
    );
    assert!(e.contains("0, 90, 180 or 270"), "{e}");
    // A part without build.
    let e = refused(&mut doc, json!({"part": part("screw6")}));
    assert!(e.contains("has no build"), "{e}");
    assert_eq!(doc.revision(), revision);
    // A closed side.
    let mut doc = design();
    let c = first(&mut doc, "p30-2n", [0.0; 3], 1000.0);
    let cp = derived(&doc, component(&c));
    let face = format!("{cp}:part(F2:side(c2))");
    doc.kernel().surfaces.borrow_mut().insert(
        face.clone(),
        crate::datum::SurfaceGeometry::Plane {
            origin: [0.0, 15.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        },
    );
    let e = refused(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": c["occurrence"],
                      "geometry": {"body": format!("{cp}.b0"), "face": face},
                      "point": [0.0, 15.0, 100.0]}}),
    );
    assert!(e.contains("closed side"), "{e}");
}

#[test]
fn a_cap_goes_on_an_end_of_its_system() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    let cap = place(
        &mut doc,
        json!({"part": part("cap30"),
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(end_b)")}}}),
    );
    all_ok(&doc);
    assert_eq!(cap["place"], "end");
    let at = placed(&doc, occurrence(&cap));
    // Its frame (z out of the cap, −Z) against end B (z +Z): the cap on
    // top.
    assert!(close(at.translation, [0.0, 0.0, 1000.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 0.0, 1.0]), "{at:?}");
}

#[test]
fn derived_parts_save_reopen_and_undo() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(&doc, component(&t));
    place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_x+_1)")},
                      "offset": 100},
               "length": 500}),
    );
    let text = doc.to_json();
    assert!(text.contains("\"library_sources\""), "{text}");
    assert!(text.contains("\"derived_part\""));
    assert!(text.contains("\"build\""));
    let mut reopened =
        crate::Document::from_json(&text, crate::testing::MockKernel::default()).unwrap();
    reopened.recompute();
    all_ok(&reopened);
    assert_eq!(reopened.to_json(), text);
    assert_eq!(
        reopened.state().library_sources,
        doc.state().library_sources
    );
    // Undo takes the part away; undoing the first one takes the source too.
    doc.undo();
    doc.undo();
    assert!(doc.state().library_sources.is_empty());
    doc.redo();
    assert_eq!(doc.state().library_sources.0.len(), 1);
}
