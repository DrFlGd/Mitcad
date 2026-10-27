// SPDX-License-Identifier: MIT
//! The cut list and panels with the mock kernel: identical cut parts
//! grouped (source, length, ends, machining and own features the same),
//! their stock in cutting order, the part's own end machining, and panels
//! sized from a picked inner face of a frame or a fixed width that does
//! not fit, and a surface panel on a frame's outer face with its clamps.

use serde_json::json;

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_build::prelude::*;
use mitcad_model_tests_support::test_library::*;

#[test]
fn the_cut_list_groups_identical_parts_and_nests_the_stock() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let x = first(&mut doc, "p30", [500.0, 0.0, 0.0], 1000.0);
    let tp = derived(&doc, component(&t));
    let xp = derived(&doc, component(&x));
    let p = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_x+_1)")},
                      "offset": 100},
               "length": {"bind": {"occurrence": x["occurrence"],
                                   "geometry": {"mount": format!("{xp}:mount(slot_x-_1)")}}}}),
    );
    let q = place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount(slot_y+_1)")},
                      "offset": 700},
               "length": 470}),
    );
    all_ok(&doc);
    let list = doc.build_cut_list(mitcad_model_build::CutListOptions { kerf: 3.0 });
    assert_eq!(list.rows.len(), 2, "{:?}", list.rows);
    assert_eq!(list.rows[0].quantity, 2);
    assert!((list.rows[0].length - 1000.0).abs() < 1e-9);
    assert!((list.rows[0].mass.unwrap() - 0.9).abs() < 1e-9);
    assert_eq!(list.rows[0].part_number, "P30_N8_B");
    assert_eq!(list.rows[0].designation, "Profile 30x30 B8 L=1000");
    assert_eq!(list.rows[1].quantity, 2);
    assert!((list.rows[1].length - 470.0).abs() < 1e-9);
    assert_eq!(list.rows[1].components, vec![component(&p), component(&q)]);
    let stock = &list.stock[0];
    assert_eq!(stock.stock_length, 6000.0);
    assert_eq!(stock.pieces, vec![1000.0, 1000.0, 470.0, 470.0]);
    assert_eq!(stock.needed, 1);
    assert!((stock.offcut - (6000.0 - 1003.0 * 2.0 - 473.0 * 2.0)).abs() < 1e-9);
    // Q's own end machining makes it a row of its own.
    command(
        &mut doc,
        json!({"cmd": "build_edit", "component": q["component"],
               "machining": [{"end": "a", "hole": {"diameter": 6.8, "depth": 15}}]}),
    )
    .unwrap();
    all_ok(&doc);
    let list = doc.build_cut_list(mitcad_model_build::CutListOptions::default());
    assert_eq!(list.rows.len(), 3, "{:?}", list.rows);
    let machined = list.rows.iter().find(|r| !r.machining.is_empty()).unwrap();
    assert_eq!(machined.components, vec![component(&q)]);
    assert_eq!(machined.machining[0].at, "end_a");
    assert!((machined.machining[0].diameter - 6.8).abs() < 1e-9);
    assert!(machined.machining[0].connector.is_none());
    // The parts list the app's Ends command reads back.
    let parts = query(&doc, json!({"query": "build_parts"}));
    let qp = parts["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["component"] == q["component"])
        .unwrap();
    assert_eq!(
        qp["machining"],
        json!([{"end": "a", "hole": {"diameter": 6.8, "depth": 15.0, "thread": null}}])
    );
    assert_eq!(qp["supplier"], "Test supplier");
    // The query's form.
    let answer = query(&doc, json!({"query": "build_cut_list", "kerf": 3}));
    assert_eq!(answer["rows"].as_array().unwrap().len(), 3);
    assert_eq!(answer["stock"][0]["needed"], 1);
    assert!(answer["other"].as_array().unwrap().is_empty());
}

/// A frame of four profiles in the XZ plane: bottom and top along +X from
/// x = 0 (400 long, at z = 0 and 300), left and right along +Z beside them.
fn frame() -> (Document<MockKernel>, serde_json::Value) {
    let mut doc = design();
    let bottom = place(
        &mut doc,
        json!({"part": part("p30"), "at": {"geometry": "yz", "point": [0, 0, 0]}, "length": 400}),
    );
    place(
        &mut doc,
        json!({"part": {"source": "S1"}, "at": {"geometry": "yz", "point": [0, 0, 300]}, "length": 400}),
    );
    first(&mut doc, "p30", [-15.0, 0.0, 0.0], 300.0);
    first(&mut doc, "p30", [415.0, 0.0, 0.0], 300.0);
    all_ok(&doc);
    (doc, bottom)
}

#[test]
fn a_panel_fills_the_frame_in_its_slots() {
    let (mut doc, bottom) = frame();
    let bp = derived(&doc, component(&bottom));
    let at = json!({"occurrence": bottom["occurrence"],
                    "geometry": {"mount": format!("{bp}:mount(slot_x-_1)")},
                    "point": [200, 0, 15]});
    let preview = query(
        &doc,
        json!({"query": "build_preview", "part": part("panel"), "at": at}),
    );
    assert_eq!(preview["fits"], true, "{preview}");
    assert!(
        (preview["width"].as_f64().unwrap() - 417.0).abs() < 1e-9,
        "{preview}"
    );
    assert!((preview["height"].as_f64().unwrap() - 287.0).abs() < 1e-9);
    let answer = command(
        &mut doc,
        json!({"cmd": "build_panel", "part": part("panel"), "at": at}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(answer["fits"], true);
    assert_eq!(answer["bound"].as_array().unwrap().len(), 2);
    let info = doc.part_info(component(&answer)).unwrap();
    let values =
        mitcad_model::internal::build::part_values(info.def.as_ref().unwrap(), doc.parameters());
    assert!((values["W"] - 417.0).abs() < 1e-9, "{values:?}");
    assert!((values["H"] - 287.0).abs() < 1e-9, "{values:?}");
    // In the cut list's other parts, not its rows.
    let list = query(&doc, json!({"query": "build_cut_list"}));
    assert!(
        list["other"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["component"] == answer["component"])
    );
}

#[test]
fn a_fixed_width_that_does_not_reach_the_slots_does_not_fit() {
    let (mut doc, bottom) = frame();
    let bp = derived(&doc, component(&bottom));
    let at = json!({"occurrence": bottom["occurrence"],
                    "geometry": {"mount": format!("{bp}:mount(slot_x-_1)")},
                    "point": [200, 0, 15]});
    let answer = command(
        &mut doc,
        json!({"cmd": "build_panel", "part": part("sheet300"), "at": at}),
    )
    .unwrap();
    assert_eq!(answer["fits"], false);
    assert!(
        answer["reason"]
            .as_str()
            .unwrap()
            .contains("does not reach into the slots"),
        "{answer}"
    );
    // Its height is still bound to the frame.
    assert_eq!(answer["bound"].as_array().unwrap().len(), 1);
    // An outer face has no frame around it.
    let error = command(
        &mut doc,
        json!({"cmd": "build_panel", "part": part("panel"),
               "at": {"occurrence": bottom["occurrence"],
                      "geometry": {"mount": format!("{bp}:mount(slot_x+_1)")},
                      "point": [200, 0, -15]}}),
    )
    .unwrap_err();
    assert!(error.contains("not inside a frame"), "{error}");
}

/// The box of a BUILD part's bodies in the design.
fn world_box(
    doc: &Document<MockKernel>,
    part: &serde_json::Value,
) -> mitcad_model::kernel::BoundingBox {
    let local = doc.part_bounds(component(part)).unwrap();
    mitcad_model::internal::build::moved_box(&local, &placed(doc, occurrence(part)))
}

#[test]
fn a_surface_panel_covers_the_frame_on_its_outer_face_with_its_clamps() {
    let (mut doc, bottom) = frame();
    let bp = derived(&doc, component(&bottom));
    // The bottom's slot on the frame's front (-Y), and its face.
    let info = doc.part_info(component(&bottom)).unwrap();
    let at_bottom = placed(&doc, occurrence(&bottom));
    let front = ["slot_y+_1", "slot_y-_1"]
        .into_iter()
        .find(|n| {
            let f = at_bottom.after(info.mount(n).unwrap());
            close(mitcad_model::internal::build::z_axis(&f), [0.0, -1.0, 0.0])
        })
        .expect("a slot on the front");
    let local = *info.mount(front).unwrap();
    let face = format!("{bp}:part(F2:side(c1))");
    doc.kernel().surfaces.borrow_mut().insert(
        face.clone(),
        mitcad_model::datum::SurfaceGeometry::Plane {
            origin: local.translation,
            normal: mitcad_model::internal::build::z_axis(&local),
        },
    );
    let at = json!({"occurrence": bottom["occurrence"],
                    "geometry": {"body": format!("{bp}.b0"), "face": face},
                    "point": [200, -15, 0]});
    let preview = query(
        &doc,
        json!({"query": "build_preview", "part": part("surface-panel"), "at": at}),
    );
    assert_eq!(preview["problems"], json!([]), "{preview}");
    // From the left's outer side (x = -30) to the right's (430), from the
    // bottom's (z = -15) to the top's (315).
    assert!(
        (preview["width"].as_f64().unwrap() - 460.0).abs() < 1e-9,
        "{preview}"
    );
    assert!(
        (preview["height"].as_f64().unwrap() - 330.0).abs() < 1e-9,
        "{preview}"
    );
    let answer = command(
        &mut doc,
        json!({"cmd": "build_panel", "part": part("surface-panel"), "at": at}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(answer["fits"], true);
    assert_eq!(answer["bound"].as_array().unwrap().len(), 2, "{answer}");
    let panel = component(&answer);
    let info = doc.part_info(panel).unwrap();
    let values =
        mitcad_model::internal::build::part_values(info.def.as_ref().unwrap(), doc.parameters());
    assert!((values["W"] - 460.0).abs() < 1e-9, "{values:?}");
    assert!((values["H"] - 330.0).abs() < 1e-9, "{values:?}");
    // It lies on the front face, 8 mm thick, over the whole frame.
    let b = world_box(&doc, &answer);
    assert!(close(b.min, [-30.0, -23.0, -15.0]), "{b:?}");
    assert!(close(b.max, [430.0, -15.0, 315.0]), "{b:?}");
    // Its two clamps, added by it, slide in the bottom's front slot 10 mm
    // (their reach) inside the bottom's ends.
    let adds = answer["adds"].as_array().unwrap();
    assert_eq!(adds.len(), 2, "{answer}");
    let clamps: Vec<_> = adds
        .iter()
        .map(|a| {
            a.as_str()
                .unwrap()
                .parse::<mitcad_model::ids::ComponentUid>()
                .unwrap()
        })
        .collect();
    let mut at_x = Vec::new();
    for clamp in &clamps {
        let c = doc.part_info(*clamp).unwrap();
        assert_eq!(c.record.added_by, Some(panel));
        assert_eq!(c.meta.role, mitcad_model_build::BuildRole::Connector);
        let joint = c.record.joint.expect("a clamp's joint");
        assert!(doc.status(joint).is_some_and(|s| s.is_ok()));
        let path = doc.placed_in_host(*clamp).unwrap();
        let t = doc.path_transform(&path).translation;
        assert!((t[1] + 15.0).abs() < 1e-6 && t[2].abs() < 1e-6, "{t:?}");
        at_x.push(t[0]);
    }
    at_x.sort_by(f64::total_cmp);
    assert!(
        close([at_x[0], at_x[1], 0.0], [10.0, 390.0, 0.0]),
        "{at_x:?}"
    );
    let parts = query(&doc, json!({"query": "build_parts"}));
    let listed = parts["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["added_by"] == answer["component"])
        .count();
    assert_eq!(listed, 2, "{parts}");
    // Its size follows the frame: the right profile 100 further out.
    let right = doc
        .build_parts_info()
        .into_iter()
        .filter(|p| p.meta.role == mitcad_model_build::BuildRole::Profile)
        .nth(3)
        .unwrap()
        .component;
    let right_path = doc.placed_in_host(right).unwrap();
    let right_occurrence = right_path[0].to_string();
    command(
        &mut doc,
        json!({"cmd": "ground_occurrence", "occurrence": right_occurrence, "grounded": false}),
    )
    .unwrap();
    command(
        &mut doc,
        json!({"cmd": "set_occurrence_transform", "occurrence": right_occurrence,
               "transform": {"translation": [515, 0, 0]}}),
    )
    .unwrap();
    all_ok(&doc);
    let info = doc.part_info(panel).unwrap();
    let values =
        mitcad_model::internal::build::part_values(info.def.as_ref().unwrap(), doc.parameters());
    assert!((values["W"] - 560.0).abs() < 1e-9, "{values:?}");
    assert!((values["H"] - 330.0).abs() < 1e-9, "{values:?}");
    // Removed, it takes its clamps with it.
    let removed = command(
        &mut doc,
        json!({"cmd": "build_remove", "component": answer["component"]}),
    )
    .unwrap();
    let gone = removed["removed"]["components"].as_array().unwrap();
    assert_eq!(gone.len(), 3, "{removed}");
    for clamp in &clamps {
        assert!(doc.assembly().component(*clamp).is_none());
    }
    all_ok(&doc);
}
