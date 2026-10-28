// SPDX-License-Identifier: MIT
//! A BUILD part's options (mitcad#801): its cut length is one of its
//! derived part's `parameters`, which `set_component_parameters` binds to
//! the design's parameters like an inserted component's options.

use serde_json::json;

use mitcad_model::ids::BodyUid;
use mitcad_model_tests_support::test_library::*;

#[test]
fn a_build_cut_length_works_through_parameters() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    all_ok(&doc);
    let profile = component(&t);
    command(
        &mut doc,
        json!({"cmd": "add_parameter", "name": "Span", "expression": "700 mm"}),
    )
    .unwrap();
    // Its length bound to the design's parameter.
    let changed = command(
        &mut doc,
        json!({"cmd": "set_component_parameters", "component": profile,
               "parameters": {"L": "Span"}}),
    )
    .unwrap();
    assert_eq!(changed["moves"], json!([]));
    assert_eq!(changed["parameters"]["L"], "Span");
    all_ok(&doc);
    let length = |doc: &mitcad_model::Document<mitcad_model::testing::MockKernel>| {
        let part = derived(doc, profile);
        let shape = doc.body_shape(BodyUid::new(part, 0)).unwrap();
        let bounds = shape.bounds.unwrap();
        bounds.max[2] - bounds.min[2]
    };
    assert!((length(&doc) - 700.0).abs() < 1e-6);
    assert!((doc.part_info(profile).unwrap().length.unwrap().1 - 700.0).abs() < 1e-6);
    // The design's parameter changes: the part's length and its frames
    // follow, and the parts list names it at its length.
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "Span", "expression": "650 mm"}),
    )
    .unwrap();
    all_ok(&doc);
    assert!((length(&doc) - 650.0).abs() < 1e-6);
    let frames = doc.part_info(profile).unwrap().mounts;
    assert!(close(frames["end_b"].translation, [0.0, 0.0, 650.0]));
    let rows = query(&doc, json!({"query": "parts_list"}))["rows"].clone();
    assert_eq!(rows[0]["designation"], "Profile 30x30 B8 L=650");
    assert_eq!(rows[0]["options"]["parameters"]["L"], "Span");
    // A BUILD part's row is build_edit's.
    let error = command(
        &mut doc,
        json!({"cmd": "set_component_parameters", "component": profile,
               "config": "x"}),
    )
    .unwrap_err();
    assert!(error.contains("is a BUILD part"), "{error}");
    // A number again: its own value.
    command(
        &mut doc,
        json!({"cmd": "set_component_parameters", "component": profile,
               "parameters": {"L": 480}}),
    )
    .unwrap();
    assert!((length(&doc) - 480.0).abs() < 1e-6);
}
