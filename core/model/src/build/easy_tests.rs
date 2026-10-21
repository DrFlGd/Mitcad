// SPDX-License-Identifier: MIT
//! Easy placement (mitcad#394) with the mock kernel: BUILD places parts in
//! the nearest component up from the active one that takes components (the
//! root while a Part1 set to the Part policy is active; a new design's
//! Part1 itself since mitcad#421), the first part at the origin
//! or on the root's XY plane along X, Y or Z, and a part on a side measured
//! to its near face: flush with either end within one section width of it,
//! the middle and the grid points snapped, and never past an end.

use serde_json::json;

use super::test_library::*;
use crate::Document;
use crate::features::FeatureDef;
use crate::ids::ComponentUid;
use crate::testing::MockKernel;
use crate::transform::Transform;

/// A new design as File › New makes it (the root and an active Part1, both
/// Hybrid since mitcad#421), with the test library.
fn hybrid_design() -> Document<MockKernel> {
    let mut doc = Document::new_design(MockKernel::default());
    doc.set_link_resolver(Some(TestLibrary::new()));
    doc
}

/// A new design whose Part1 takes no components (the Part policy, as File ›
/// New made it before mitcad#421): BUILD goes up to the root.
fn new_design() -> Document<MockKernel> {
    let mut doc = hybrid_design();
    command(
        &mut doc,
        json!({"cmd": "set_policy", "component": "Part1", "preset": "part"}),
    )
    .unwrap();
    doc
}

fn axis(t: &Transform) -> [f64; 3] {
    t.apply_vector([0.0, 0.0, 1.0])
}

fn parent(doc: &Document<MockKernel>, answer: &serde_json::Value) -> ComponentUid {
    doc.assembly()
        .occurrence(occurrence(answer))
        .unwrap()
        .parent
}

/// A 30 profile 1000 long standing at the origin of a design, and the slot
/// frame `slot_y+_1` of it as `at.geometry`.
fn target(doc: &mut Document<MockKernel>) -> (serde_json::Value, serde_json::Value) {
    let t = first(doc, "p30", [0.0; 3], 1000.0);
    let tp = derived(doc, component(&t));
    (t, json!({"mount": format!("{tp}:mount(slot_y+_1)")}))
}

/// A 30 profile 200 long on the target's y+ side, `at` merged into the
/// side's `at`; the answer.
fn on_side(
    doc: &mut Document<MockKernel>,
    t: &serde_json::Value,
    slot: &serde_json::Value,
    at: serde_json::Value,
) -> serde_json::Value {
    let mut fields = json!({"occurrence": t["occurrence"], "geometry": slot});
    for (key, value) in at.as_object().unwrap() {
        fields[key] = value.clone();
    }
    place(
        doc,
        json!({"part": {"source": "S1"}, "at": fields, "length": 200}),
    )
}

/// Where a part on the y+ side stands along the target (its centre's z).
fn along(doc: &Document<MockKernel>, answer: &serde_json::Value) -> f64 {
    placed(doc, occurrence(answer)).translation[2]
}

#[test]
fn a_new_design_places_on_the_root_s_xy_plane_and_on_a_side_without_activating_root() {
    let mut doc = new_design();
    let part1 = doc.assembly().active;
    assert!(!part1.is_root());
    // BUILD's host is the root, as the query tells the app.
    let host = query(&doc, json!({"query": "build_host"}));
    assert_eq!(host["component"], "C0", "{host}");
    assert_eq!(host["name"], "Root");
    assert_eq!(host["path"], "");
    assert_eq!(host["active"], json!(part1));
    // A profile on the root's XY plane, at the click, standing along +Z.
    let first = place(
        &mut doc,
        json!({"part": part("p30"),
               "at": {"geometry": "xy", "point": [100.0, 50.0, 0.0]},
               "length": 1000}),
    );
    all_ok(&doc);
    assert_eq!(first["place"], "geometry");
    assert!(first["warnings"].as_array().unwrap().is_empty(), "{first}");
    assert_eq!(parent(&doc, &first), ComponentUid::ROOT);
    let at = placed(&doc, occurrence(&first));
    assert!(close(at.translation, [100.0, 50.0, 0.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 0.0, 1.0]));
    // Its joint is the root's, to the root's XY plane.
    let joint = first["joint"].as_str().unwrap().parse().unwrap();
    assert_eq!(doc.feature(joint).unwrap().component, ComponentUid::ROOT);
    // A second one on its side, near end B: flush with that end.
    let fp = derived(&doc, component(&first));
    let second = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": first["occurrence"],
                      "geometry": {"body": format!("{fp}.b0"), "face": format!("{fp}:part(F2:side(c1))")},
                      "point": [100.0, 65.0, 990.0]},
               "length": 300}),
    );
    all_ok(&doc);
    assert_eq!(second["place"], "side");
    assert_eq!(parent(&doc, &second), ComponentUid::ROOT);
    let at = placed(&doc, occurrence(&second));
    assert!(close(at.translation, [100.0, 65.0, 985.0]), "{at:?}");
    assert!(close(axis(&at), [0.0, 1.0, 0.0]));
    // A third standing beside, and a fourth from the first's side with its
    // length bound to the third: the bound length goes into the root too
    // (an assembly feature, which the Assembly policy takes).
    let third = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [600.0, 50.0, 0.0]},
               "length": 1000}),
    );
    let xp = derived(&doc, component(&third));
    let fourth = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": first["occurrence"],
                      "geometry": {"mount": format!("{fp}:mount(slot_x+_1)")}, "offset": 400},
               "length": {"bind": {"occurrence": third["occurrence"],
                                   "geometry": {"mount": format!("{xp}:mount(slot_x-_1)")}}}}),
    );
    all_ok(&doc);
    let bound = fourth["bound"][0].as_str().unwrap().parse().unwrap();
    let FeatureDef::BoundLength(_) = &doc.feature(bound).unwrap().def else {
        panic!("a bound length");
    };
    assert_eq!(doc.feature(bound).unwrap().component, ComponentUid::ROOT);
    // From x = 115 (the first's x+ side) to x = 585 (the third's x- side).
    let length = doc.part_info(component(&fourth)).unwrap().length.unwrap().1;
    assert!((length - 470.0).abs() < 1e-9, "{length}");
    // Part1 stays the active component.
    assert_eq!(doc.assembly().active, part1);
}

#[test]
fn the_first_part_goes_to_the_origin_without_a_pick_along_x_y_or_z() {
    let mut doc = new_design();
    // Enter without a pick: grounded at the origin, along +Z.
    let z = place(&mut doc, json!({"part": part("p30"), "length": 500}));
    all_ok(&doc);
    assert_eq!(z["place"], "grounded");
    assert_eq!(parent(&doc, &z), ComponentUid::ROOT);
    let at = placed(&doc, occurrence(&z));
    assert!(close(at.translation, [0.0; 3]));
    assert!(close(axis(&at), [0.0, 0.0, 1.0]));
    // Along X and Y: lying on the XY plane, resting on it (mitcad#492),
    // its axis half its 30 above.
    for (direction, expected) in [
        ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 1.0, 0.0]),
    ] {
        let p = place(
            &mut doc,
            json!({"part": {"source": "S1"}, "at": {"direction": direction}, "length": 500}),
        );
        all_ok(&doc);
        let at = placed(&doc, occurrence(&p));
        assert!(close(at.translation, [0.0, 0.0, 15.0]), "{at:?}");
        assert!(close(axis(&at), expected), "{at:?}");
    }
    // On the root's XY plane, lying along X from the click, resting on it.
    let x = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xy", "point": [0.0, 200.0, 0.0], "direction": [1, 0, 0]},
               "length": 500}),
    );
    all_ok(&doc);
    assert_eq!(x["place"], "geometry");
    let at = placed(&doc, occurrence(&x));
    assert!(close(at.translation, [0.0, 200.0, 15.0]), "{at:?}");
    assert!(close(axis(&at), [1.0, 0.0, 0.0]), "{at:?}");
    // Its far end where the length puts it.
    assert!(close(
        at.apply_point([0.0, 0.0, 500.0]),
        [500.0, 200.0, 15.0]
    ));
}

#[test]
fn a_side_click_near_either_end_is_flush_with_it() {
    let mut doc = design();
    let (t, slot) = target(&mut doc);
    // Within one section width (30) of end A: the near face on end A.
    for z in [10.0, 40.0, -50.0] {
        let p = on_side(&mut doc, &t, &slot, json!({"point": [0.0, 15.0, z]}));
        assert!(
            (along(&doc, &p) - 15.0).abs() < 1e-9,
            "click {z}: {}",
            along(&doc, &p)
        );
    }
    // Within one of end B, and past it: the far face on end B.
    for z in [990.0, 960.0, 1100.0] {
        let p = on_side(&mut doc, &t, &slot, json!({"point": [0.0, 15.0, z]}));
        assert!(
            (along(&doc, &p) - 985.0).abs() < 1e-9,
            "click {z}: {}",
            along(&doc, &p)
        );
    }
    all_ok(&doc);
    // The preview says which snap the click took.
    let preview = query(
        &doc,
        json!({"query": "build_preview", "part": {"source": "S1"}, "length": 200,
               "at": {"occurrence": t["occurrence"], "geometry": slot, "point": [0.0, 15.0, 995.0]}}),
    );
    // Bound to end B (mitcad#417): from B, offset 0.
    assert_eq!(preview["side"]["kind"], "end_b", "{preview}");
    assert_eq!(preview["side"]["from"], "b", "{preview}");
    assert!(preview["side"]["offset"].as_f64().unwrap().abs() < 1e-9);
    assert!((preview["side"]["width"].as_f64().unwrap() - 30.0).abs() < 1e-9);
}

#[test]
fn offset_zero_is_flush_from_either_end() {
    let mut doc = design();
    let (t, slot) = target(&mut doc);
    let a = on_side(&mut doc, &t, &slot, json!({"offset": 0}));
    assert!((along(&doc, &a) - 15.0).abs() < 1e-9, "{}", along(&doc, &a));
    let b = on_side(&mut doc, &t, &slot, json!({"offset": 0, "from": "b"}));
    assert!(
        (along(&doc, &b) - 985.0).abs() < 1e-9,
        "{}",
        along(&doc, &b)
    );
    all_ok(&doc);
    // The side joint's position.tx: the frame's distance from end A, the
    // near face's offset plus half the section (from B, an expression of
    // the target's length).
    let tx = |answer: &serde_json::Value| {
        let joint = answer["joint"].as_str().unwrap().parse().unwrap();
        let FeatureDef::Joint(j) = &doc.feature(joint).unwrap().def else {
            panic!("a joint");
        };
        let id = j.position[&crate::joints::Motion::Tx];
        (
            doc.parameters().value(id).unwrap(),
            doc.parameters().get(id).unwrap().expression().to_owned(),
        )
    };
    assert!((tx(&a).0 - 15.0).abs() < 1e-9);
    let (value, expression) = tx(&b);
    assert!((value - 985.0).abs() < 1e-9);
    let length = doc.part_info(component(&t)).unwrap().length.unwrap().0;
    assert_eq!(
        expression,
        format!("{} - 15", doc.parameters().name(length))
    );
    // An offset typed with a parameter keeps it, shifted by half the
    // section.
    let e = on_side(&mut doc, &t, &slot, json!({"offset": "100 mm + 0 mm"}));
    assert!((along(&doc, &e) - 115.0).abs() < 1e-9);
}

#[test]
fn no_placement_overhangs_the_target() {
    let mut doc = design();
    let (t, slot) = target(&mut doc);
    // A typed offset past end B is kept within the length, with a warning.
    let p = on_side(&mut doc, &t, &slot, json!({"offset": 990}));
    assert!(
        (along(&doc, &p) - 985.0).abs() < 1e-9,
        "{}",
        along(&doc, &p)
    );
    let warnings = p["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("flush with end B instead")),
        "{p}"
    );
    // From end B past end A likewise.
    let q = on_side(&mut doc, &t, &slot, json!({"offset": 2000, "from": "b"}));
    assert!((along(&doc, &q) - 15.0).abs() < 1e-9, "{}", along(&doc, &q));
    all_ok(&doc);
    // Every part on the side is within the target's length.
    for answer in [&p, &q] {
        let local = doc.part_bounds(component(answer)).unwrap();
        let b = super::moved_box(&local, &placed(&doc, occurrence(answer)));
        assert!(b.min[2] > -1e-9 && b.max[2] < 1000.0 + 1e-9, "{b:?}");
    }
    // A rail longer than the target stays flush with end A.
    let mut doc = design();
    let short = first(&mut doc, "p30", [0.0; 3], 300.0);
    let sp = derived(&doc, component(&short));
    let rail = place(
        &mut doc,
        json!({"part": part("rail15"),
               "at": {"occurrence": short["occurrence"],
                      "geometry": {"mount": format!("{sp}:mount(slot_x+_1)")}, "offset": 100},
               "length": 400}),
    );
    let local = doc.part_bounds(component(&rail)).unwrap();
    let b = super::moved_box(&local, &placed(&doc, occurrence(&rail)));
    assert!(b.min[2].abs() < 1e-9, "{b:?}");
}

#[test]
fn the_middle_and_the_grid_points_snap() {
    let mut doc = design();
    let (t, slot) = target(&mut doc);
    // Near the middle: centred on it.
    let m = on_side(&mut doc, &t, &slot, json!({"point": [0.0, 15.0, 503.0]}));
    assert!(
        (along(&doc, &m) - 500.0).abs() < 1e-9,
        "{}",
        along(&doc, &m)
    );
    // Near a grid point (30): the near face on it (near face 237 -> 240).
    let g = on_side(&mut doc, &t, &slot, json!({"point": [0.0, 15.0, 252.0]}));
    assert!(
        (along(&doc, &g) - 255.0).abs() < 1e-9,
        "{}",
        along(&doc, &g)
    );
    // Between them: where the click was.
    let free = on_side(&mut doc, &t, &slot, json!({"point": [0.0, 15.0, 422.0]}));
    assert!((along(&doc, &free) - 422.0).abs() < 1e-9);
    all_ok(&doc);
    // The preview lists the snap points and says which one was taken.
    let preview = |z: f64| {
        query(
            &doc,
            json!({"query": "build_preview", "part": {"source": "S1"}, "length": 200,
                   "at": {"occurrence": t["occurrence"], "geometry": slot, "point": [0.0, 15.0, z]}}),
        )
    };
    let shown = preview(503.0);
    assert_eq!(shown["side"]["kind"], "middle", "{shown}");
    assert_eq!(preview(252.0)["side"]["kind"], "grid");
    assert!(preview(422.0)["side"]["kind"].is_null());
    let snaps = shown["snaps"].as_array().unwrap();
    let offsets = |kind: &str| -> Vec<f64> {
        snaps
            .iter()
            .filter(|s| s["kind"] == kind)
            .map(|s| s["offset"].as_f64().unwrap())
            .collect()
    };
    assert_eq!(offsets("end_a"), vec![0.0]);
    assert_eq!(offsets("end_b"), vec![970.0]);
    assert_eq!(offsets("middle"), vec![485.0]);
    assert!(offsets("grid").contains(&240.0));
    // Each snap's point: where the new part's centre then stands on the
    // slot, in the design.
    let end_b = snaps.iter().find(|s| s["kind"] == "end_b").unwrap();
    let point: Vec<f64> = end_b["point"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert!(
        close([point[0], point[1], point[2]], [0.0, 15.0, 985.0]),
        "{point:?}"
    );
}

#[test]
fn a_part_in_a_component_that_takes_components_stays_there() {
    // An existing design: the root active (the general policy): BUILD's
    // host is the active component, as before.
    let doc = design();
    let host = query(&doc, json!({"query": "build_host"}));
    assert_eq!(host["component"], "C0");
    // A component that takes components, active: the parts go into it.
    let mut doc = new_design();
    command(
        &mut doc,
        json!({"cmd": "create_component", "name": "Frame", "policy": "assembly"}),
    )
    .unwrap();
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "Frame"}),
    )
    .unwrap();
    let frame = doc.assembly().active;
    let host = query(&doc, json!({"query": "build_host"}));
    assert_eq!(host["component"], json!(frame), "{host}");
    assert_eq!(host["name"], "Frame");
    let p = place(&mut doc, json!({"part": part("p30"), "length": 500}));
    all_ok(&doc);
    assert_eq!(parent(&doc, &p), frame);
}

#[test]
fn a_new_hybrid_design_places_in_its_active_part1() {
    // mitcad#421: Part1 takes components, so BUILD's host is Part1 itself.
    let mut doc = hybrid_design();
    let part1 = doc.assembly().active;
    assert!(!part1.is_root());
    let host = query(&doc, json!({"query": "build_host"}));
    assert_eq!(host["component"], json!(part1), "{host}");
    let first = place(&mut doc, json!({"part": part("p30"), "length": 500}));
    all_ok(&doc);
    assert!(first["warnings"].as_array().unwrap().is_empty(), "{first}");
    assert_eq!(parent(&doc, &first), part1);
    assert_eq!(doc.assembly().active, part1);
}
