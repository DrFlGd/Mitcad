// SPDX-License-Identifier: MIT
//! Junctions and connectors with the mock kernel: where profiles meet
//! (T, L near an end or at two ends, inline, cross), a connector placed at
//! a T junction with its nut, its screw and the hole it machines, listed
//! at the junction, and removed with all it added.

use serde_json::json;

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_build::prelude::*;
use mitcad_model_tests_support::test_library::*;

fn on_side(
    doc: &mut Document<MockKernel>,
    t: &serde_json::Value,
    slot: &str,
    offset: f64,
) -> serde_json::Value {
    let tp = derived(doc, component(t));
    place(
        doc,
        json!({"part": {"source": "S1"},
               "at": {"occurrence": t["occurrence"], "geometry": {"mount": format!("{tp}:mount({slot})")},
                      "offset": offset},
               "length": 300}),
    )
}

fn kinds(doc: &Document<MockKernel>) -> Vec<&'static str> {
    doc.build_junctions(None).iter().map(|j| j.kind).collect()
}

#[test]
fn an_end_on_a_side_is_a_t_or_near_an_end_an_l() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let p = on_side(&mut doc, &t, "slot_x+_1", 500.0);
    all_ok(&doc);
    let junctions = doc.build_junctions(None);
    assert_eq!(junctions.len(), 1, "{junctions:?}");
    let j = &junctions[0];
    assert_eq!(j.kind, "T");
    assert_eq!(j.a.component, component(&p));
    assert_eq!(j.a.at, "end_a");
    assert_eq!(j.b.component, component(&t));
    assert_eq!(j.b.at, "slot_x+_1");
    assert!((j.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert_eq!(
        j.id,
        format!("{}:end_a|{}:slot_x+_1", component(&p), component(&t))
    );
    // Near the end: a corner.
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    on_side(&mut doc, &t, "slot_x+_1", 10.0);
    assert_eq!(kinds(&doc), vec!["L"]);
    // The query's form.
    let answer = query(&doc, json!({"query": "build_junctions"}));
    let first_junction = &answer["junctions"][0];
    assert_eq!(first_junction["kind"], "L");
    assert_eq!(first_junction["systems"], json!(["B8/30", "B8/30"]));
    assert!(first_junction["a"]["frame"]["origin"].is_array());
}

#[test]
fn two_ends_meet_inline_or_at_a_corner_and_sides_cross() {
    let end = |direction: Option<[f64; 3]>, mitre: Option<&str>| {
        let mut doc = design();
        let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
        let tp = derived(&doc, component(&t));
        let mut at = json!({"occurrence": t["occurrence"],
                            "geometry": {"mount": format!("{tp}:mount(end_b)")}});
        if let Some(d) = direction {
            at["direction"] = json!(d);
        }
        let mut fields = json!({"part": {"source": "S1"}, "at": at, "length": 300});
        if let Some(cut) = mitre {
            fields["mitre"] = json!({"cut": cut});
        }
        place(&mut doc, fields);
        all_ok(&doc);
        doc
    };
    assert_eq!(kinds(&end(None, None)), vec!["inline"]);
    // A square corner by default lies on the end face (mitcad#427): T's end
    // on the new part's side near its end, an L.
    let corner = end(Some([0.0, 1.0, 0.0]), None);
    assert_eq!(kinds(&corner), vec!["L"]);
    let j = &corner.build_junctions(None)[0];
    assert_eq!(j.a.at.as_str(), "end_b", "{:?}", j.a.at);
    assert!((j.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // Mitred: the two ends meet.
    let mitred = end(Some([0.0, 1.0, 0.0]), Some("shared"));
    assert_eq!(kinds(&mitred), vec!["L"]);
    let j = &mitred.build_junctions(None)[0];
    assert_eq!((j.a.at.as_str(), j.b.at.as_str()), ("end_a", "end_b"));
    assert!((j.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    // A profile beside T, across it.
    let mut doc = design();
    first(&mut doc, "p30", [0.0; 3], 1000.0);
    place(
        &mut doc,
        json!({"part": {"source": "S1"},
               "at": {"geometry": "xz", "point": [30, 0, 500]}, "length": 300}),
    );
    all_ok(&doc);
    assert_eq!(kinds(&doc), vec!["cross"]);
}

#[test]
fn a_connector_brings_its_fasteners_and_machining_and_takes_them_away() {
    let mut doc = design();
    let t = first(&mut doc, "p30", [0.0; 3], 1000.0);
    let p = on_side(&mut doc, &t, "slot_x+_1", 500.0);
    let id = doc.build_junctions(None)[0].id.clone();
    // A preview first: the connector's frame, its fasteners and holes.
    let preview = query(
        &doc,
        json!({"query": "build_preview", "junction": id, "part": part("bracket30")}),
    );
    assert_eq!(preview["problems"], json!([]), "{preview}");
    assert_eq!(preview["fasteners"].as_array().unwrap().len(), 1);
    assert_eq!(preview["machining"].as_array().unwrap().len(), 1);
    let answer = command(
        &mut doc,
        json!({"cmd": "build_connect", "junction": id, "part": part("bracket30")}),
    )
    .unwrap();
    all_ok(&doc);
    let connector = component(&answer);
    let adds = answer["adds"].as_array().unwrap();
    assert_eq!(adds.len(), 3, "{answer}");
    // The hole is a feature of the profile whose end meets.
    let hole: mitcad_model::ids::FeatureUid = adds
        .iter()
        .find_map(|a| a.as_str().unwrap().parse().ok())
        .unwrap();
    assert_eq!(doc.feature(hole).unwrap().component, component(&p));
    assert!(matches!(
        doc.feature(hole).unwrap().def,
        mitcad_model::features::FeatureDef::Hole(_)
    ));
    // The nut and the screw were added by the connector.
    let added: Vec<_> = doc
        .assembly()
        .components
        .iter()
        .filter(|c| {
            c.build
                .as_ref()
                .is_some_and(|b| b.added_by == Some(connector))
        })
        .map(|c| c.uid)
        .collect();
    assert_eq!(added.len(), 2);
    assert_eq!(doc.undo_label(), Some("Connect Bracket 30 B8"));
    // The junction lists it.
    assert_eq!(doc.build_junctions(None)[0].connectors, vec![connector]);
    // A connector that does not fit a junction is refused.
    let error = command(
        &mut doc,
        json!({"cmd": "build_connect", "junction": "C9:end_a|C1:slot_x+_1", "part": part("bracket30")}),
    )
    .unwrap_err();
    assert!(error.contains("no junction"), "{error}");
    // Removing the connector removes all it added.
    let removed = command(
        &mut doc,
        json!({"cmd": "build_remove", "component": connector.to_string()}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(
        removed["removed"]["components"].as_array().unwrap().len(),
        3
    );
    assert!(doc.feature(hole).is_none());
    assert!(doc.assembly().component(connector).is_none());
    for c in added {
        assert!(doc.assembly().component(c).is_none());
    }
    assert!(doc.part_info(component(&p)).is_some());
}

#[test]
fn removing_a_target_detaches_the_lengths_bound_to_it() {
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
    let removed = command(
        &mut doc,
        json!({"cmd": "build_remove", "component": x["component"]}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(removed["removed"]["detached"].as_array().unwrap().len(), 1);
    let info = doc.part_info(component(&p)).unwrap();
    assert!(info.record.bound.is_empty());
    assert!((info.length.unwrap().1 - 470.0).abs() < 1e-9);
    // Removing T: the part on it stays where it is.
    let before = placed(&doc, occurrence(&p));
    command(
        &mut doc,
        json!({"cmd": "build_remove", "component": t["component"]}),
    )
    .unwrap();
    all_ok(&doc);
    assert!(close(
        placed(&doc, occurrence(&p)).translation,
        before.translation
    ));
}
