// SPDX-License-Identifier: MIT
//! A timber frame is a component of its own (mitcad#522): `build_frame`
//! makes `Wall 1` in the BUILD host and builds the frame's members, their
//! fasteners and the fasteners' joints inside it; the joints between frames
//! stay in the host. Laid out again, edited, moved as one and removed
//! through its component; a flat frame saved before stays flat.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::features::FeatureDef;
use mitcad_model::ids::{ComponentUid, OccurrenceUid};
use mitcad_model::testing::MockKernel;
use mitcad_model::transform::Transform;
use mitcad_model_build::prelude::*;
use mitcad_model_tests_support::test_library::{TestLibrary, all_ok, command, design, part, query};

fn uid(v: &Value) -> ComponentUid {
    v.as_str()
        .unwrap_or_else(|| panic!("no component: {v}"))
        .parse()
        .unwrap()
}

fn wall(doc: &mut Document<MockKernel>, from: [f64; 3], to: [f64; 3], extra: Value) -> Value {
    let mut fields = json!({"cmd": "build_frame", "kind": "wall", "part": part("rows98"),
                            "start": from, "end": to, "height": 2400, "spacing": 600});
    for (k, v) in extra.as_object().unwrap() {
        fields[k] = v.clone();
    }
    command(doc, fields).unwrap_or_else(|e| panic!("build_frame: {e}"))
}

fn parts_of(doc: &Document<MockKernel>) -> Vec<Value> {
    query(doc, json!({"query": "build_parts"}))["parts"]
        .as_array()
        .unwrap()
        .clone()
}

/// The parent component of a component's (only) occurrence.
fn parent_of(doc: &Document<MockKernel>, c: ComponentUid) -> ComponentUid {
    let mut parents = doc.assembly().occurrences_of(c).map(|o| o.parent);
    let parent = parents
        .next()
        .unwrap_or_else(|| panic!("{c} is not placed"));
    assert!(parents.next().is_none(), "{c} is placed more than once");
    parent
}

fn occurrence_of(doc: &Document<MockKernel>, c: ComponentUid) -> OccurrenceUid {
    doc.assembly().occurrences_of(c).next().unwrap().uid
}

/// The members (`frame` this lead) and the fasteners driven at their
/// junctions.
fn of_frame(doc: &Document<MockKernel>, lead: &Value) -> (Vec<ComponentUid>, Vec<ComponentUid>) {
    let parts = parts_of(doc);
    let members: Vec<ComponentUid> = parts
        .iter()
        .filter(|p| p["frame"] == *lead)
        .map(|p| uid(&p["component"]))
        .collect();
    let set: BTreeSet<String> = members.iter().map(|c| c.to_string()).collect();
    let fasteners = parts
        .iter()
        .filter(|p| p["role"] == "fastener")
        .filter(|p| {
            p["junction"].as_str().is_some_and(|j| {
                j.split('|')
                    .all(|side| set.contains(side.split(':').next().unwrap_or_default()))
            })
        })
        .map(|p| uid(&p["component"]))
        .collect();
    (members, fasteners)
}

/// The components holding joints, with how many each.
fn joints_by_component(doc: &Document<MockKernel>) -> Vec<(ComponentUid, usize)> {
    let mut out: Vec<(ComponentUid, usize)> = Vec::new();
    for f in doc.features() {
        if !matches!(f.def, FeatureDef::Joint(_)) {
            continue;
        }
        match out.iter_mut().find(|(c, _)| *c == f.component) {
            Some((_, n)) => *n += 1,
            None => out.push((f.component, 1)),
        }
    }
    out
}

fn joints_in(doc: &Document<MockKernel>, c: ComponentUid) -> usize {
    joints_by_component(doc)
        .into_iter()
        .find(|(k, _)| *k == c)
        .map_or(0, |(_, n)| n)
}

/// A member's end A in the design (its path's placement).
fn end_a(doc: &Document<MockKernel>, c: ComponentUid) -> [f64; 3] {
    let path = doc.placed_in_host(c).expect("placed in the host");
    doc.path_transform(&path).translation
}

fn member_by_key(doc: &Document<MockKernel>, lead: &Value, key: &str) -> ComponentUid {
    parts_of(doc)
        .into_iter()
        .find(|p| p["frame"] == *lead && p["member"] == key)
        .map(|p| uid(&p["component"]))
        .unwrap_or_else(|| panic!("no member {key}"))
}

/// Everything a frame made is inside its own component, nothing of it in
/// the host.
fn all_inside(doc: &Document<MockKernel>, lead: &Value, frame: ComponentUid) {
    let (members, fasteners) = of_frame(doc, lead);
    assert!(!members.is_empty());
    for c in members.iter().chain(&fasteners) {
        assert_eq!(parent_of(doc, *c), frame, "{c}");
    }
    // Each fastener's joint with its member: in the frame's component.
    for c in &fasteners {
        let joint = doc
            .assembly()
            .component(*c)
            .and_then(|d| d.build.as_ref())
            .and_then(|b| b.joint)
            .expect("a fastener's joint");
        assert_eq!(doc.feature(joint).unwrap().component, frame, "{c}'s joint");
    }
}

#[test]
fn a_wall_frame_is_one_component_in_the_host_with_its_members_fasteners_and_joints() {
    let mut doc = design();
    let host = doc.build_host();
    let made = wall(
        &mut doc,
        [0.0, 0.0, 0.0],
        [1800.0, 0.0, 0.0],
        json!({"fastener": part("wood-screw")}),
    );
    all_ok(&doc);
    let lead = made["frame"].clone();
    let frame = uid(&made["frame_component"]);
    assert_eq!(doc.assembly().name(frame), "Wall 1");
    assert_eq!(parent_of(&doc, frame), host);
    // Its record: its lead keeps the component with the definition.
    let lead_part = parts_of(&doc)
        .into_iter()
        .find(|p| p["component"] == lead)
        .unwrap();
    assert_eq!(lead_part["frame_component"], made["frame_component"]);
    assert_eq!(made["fasteners"], 8, "{made}");
    all_inside(&doc, &lead, frame);
    let (members, fasteners) = of_frame(&doc, &lead);
    assert_eq!(members.len(), 6);
    assert_eq!(fasteners.len(), 16);
    // The host holds only the frame's component: no member, fastener or
    // joint of it.
    let in_host: Vec<ComponentUid> = doc.assembly().children(host).map(|o| o.component).collect();
    assert_eq!(in_host, vec![frame]);
    assert_eq!(joints_in(&doc, host), 0);
    assert_eq!(joints_in(&doc, frame), 16);
    // Members are where the frame wants them in the design (its component
    // at the host's origin): the end stud's foot.
    let stud = member_by_key(&doc, &lead, "stud.end");
    let at = end_a(&doc, stud);
    assert!(
        (at[0] - 1776.0).abs() < 1e-6 && at[1].abs() < 1e-6 && (at[2] - 48.0).abs() < 1e-6,
        "{at:?}"
    );
    // One undo step takes the component with it.
    assert_eq!(doc.undo_label(), Some("Place wall frame Wall 1"));
    doc.undo();
    assert!(doc.assembly().component(frame).is_none());
    assert!(parts_of(&doc).is_empty());
    doc.redo();
    all_inside(&doc, &lead, frame);
}

#[test]
fn walls_at_a_corner_keep_their_own_joints_and_the_host_those_between_them() {
    let mut doc = design();
    let host = doc.build_host();
    let screw = part("wood-screw");
    let w1 = wall(
        &mut doc,
        [0.0, 0.0, 0.0],
        [3600.0, 0.0, 0.0],
        json!({"fastener": screw, "corner": "butt"}),
    );
    let lead1 = w1["frame"].clone();
    let w2 = wall(
        &mut doc,
        [3600.0, 0.0, 0.0],
        [3600.0, 3000.0, 0.0],
        json!({"fastener": screw, "corner": "butt", "joins": {"start": lead1}}),
    );
    all_ok(&doc);
    let lead2 = w2["frame"].clone();
    let (f1, f2) = (uid(&w1["frame_component"]), uid(&w2["frame_component"]));
    assert_ne!(f1, f2);
    assert_eq!(doc.assembly().name(f2), "Wall 2");
    // Wall 1 laid out again with Wall 2 (its backing studs): still in its
    // own component.
    assert_eq!(w2["relaid"], json!([lead1]), "{w2}");
    all_inside(&doc, &lead1, f1);
    all_inside(&doc, &lead2, f2);
    assert!(joints_in(&doc, f1) > 0 && joints_in(&doc, f2) > 0);
    assert_eq!(joints_in(&doc, host), 0);
    // A junction between the two frames, fastened by hand: in the host,
    // the common parent, its joint there too.
    let lead_of = |c: ComponentUid| doc.frame_lead(c);
    let between = doc
        .build_junctions(None)
        .into_iter()
        .find(|j| {
            let (a, b) = (lead_of(j.a.component), lead_of(j.b.component));
            a.is_some() && b.is_some() && a != b && j.connectors.is_empty()
        })
        .expect("a junction of Wall 1 and Wall 2");
    let connected = command(
        &mut doc,
        json!({"cmd": "build_connect", "junction": between.id, "part": part("wood-screw")}),
    )
    .unwrap();
    all_ok(&doc);
    let screw = uid(&connected["component"]);
    assert_eq!(parent_of(&doc, screw), host);
    assert_eq!(joints_in(&doc, host), 2, "{:?}", joints_by_component(&doc));
    // The frames' own joints did not move.
    all_inside(&doc, &lead1, f1);
    all_inside(&doc, &lead2, f2);
}

#[test]
fn an_edit_lays_the_frame_out_again_inside_its_component_through_any_member_or_it() {
    let mut doc = design();
    let host = doc.build_host();
    let made = wall(
        &mut doc,
        [0.0, 0.0, 0.0],
        [1800.0, 0.0, 0.0],
        json!({"fastener": part("wood-screw")}),
    );
    let lead = made["frame"].clone();
    let frame = uid(&made["frame_component"]);
    let components = doc.assembly().components.len();
    // Through a member.
    let stud = member_by_key(&doc, &lead, "stud.start");
    let edited = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": stud.to_string(), "frame": {"spacing": 400}}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(edited["count"], 8, "{edited}");
    assert_eq!(edited["frame_component"], made["frame_component"]);
    all_inside(&doc, &lead, frame);
    let (members, fasteners) = of_frame(&doc, &lead);
    // Two more studs and their screws; the component is the same one.
    assert_eq!(members.len(), 8);
    assert_eq!(fasteners.len(), 24);
    assert_eq!(doc.assembly().components.len(), components + 2 + 8);
    let named: Vec<&str> = doc
        .assembly()
        .components
        .iter()
        .filter(|c| c.name == "Wall 1" || c.name.starts_with("Wall 1 ("))
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(named, vec!["Wall 1"]);
    assert_eq!(doc.undo_label(), Some("Edit Wall 1"));
    // Through its component: the preview, an edit, the walls' joins.
    let preview = query(
        &doc,
        json!({"query": "build_preview", "frame": {"spacing": 600}, "component": frame.to_string()}),
    );
    assert_eq!(preview["count"], 6, "{preview}");
    let edited = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": frame.to_string(), "frame": {"spacing": 600}}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(edited["count"], 6, "{edited}");
    assert_eq!(edited["frame"], lead);
    all_inside(&doc, &lead, frame);
    let w2 = wall(
        &mut doc,
        [1800.0, 0.0, 0.0],
        [1800.0, 2000.0, 0.0],
        json!({"joins": {"start": frame.to_string()}}),
    );
    all_ok(&doc);
    assert_eq!(w2["relaid"], json!([lead]), "{w2}");
    // Removed as a set through its component: everything of it, the
    // component too; the other frame stays.
    command(
        &mut doc,
        json!({"cmd": "build_remove", "component": frame.to_string(), "set": true}),
    )
    .unwrap();
    assert!(doc.assembly().component(frame).is_none());
    assert!(of_frame(&doc, &lead).0.is_empty());
    let f2 = uid(&w2["frame_component"]);
    let in_host: Vec<ComponentUid> = doc.assembly().children(host).map(|o| o.component).collect();
    assert_eq!(in_host, vec![f2]);
}

#[test]
fn a_frame_moved_as_one_is_laid_out_again_where_it_is() {
    let mut doc = design();
    let made = wall(&mut doc, [0.0, 0.0, 0.0], [1800.0, 0.0, 0.0], json!({}));
    let lead = made["frame"].clone();
    let frame = uid(&made["frame_component"]);
    // Moved 1000 mm along +Y: its members with it.
    doc.set_occurrence_transform(
        occurrence_of(&doc, frame),
        Transform::translation([0.0, 1000.0, 0.0]),
        false,
    )
    .unwrap();
    doc.recompute();
    let stud = member_by_key(&doc, &lead, "stud.start");
    let at = end_a(&doc, stud);
    assert!(
        (at[0] - 24.0).abs() < 1e-6 && (at[1] - 1000.0).abs() < 1e-6,
        "{at:?}"
    );
    // Laid out again: where it is now, not back at the origin.
    command(
        &mut doc,
        json!({"cmd": "build_edit", "component": lead, "frame": {"spacing": 400}}),
    )
    .unwrap();
    all_ok(&doc);
    let stud = member_by_key(&doc, &lead, "stud.end");
    let at = end_a(&doc, stud);
    assert!(
        (at[0] - 1776.0).abs() < 1e-6
            && (at[1] - 1000.0).abs() < 1e-6
            && (at[2] - 48.0).abs() < 1e-6,
        "{at:?}"
    );
    // A wall started on its side there: square to it where it is.
    let w2 = wall(
        &mut doc,
        [900.0, 1049.0, 0.0],
        [900.0, 3049.0, 0.0],
        json!({"square": true, "corner": "butt", "joins": {"start": lead}}),
    );
    all_ok(&doc);
    let lead2 = w2["frame"].clone();
    let plate = member_by_key(&doc, &lead2, "plate.bottom.0");
    let at = end_a(&doc, plate);
    assert!((at[0] - 900.0).abs() < 1e-6, "{at:?}");
    assert!((at[1] - 1049.0).abs() < 1e-6, "{at:?}");
}

#[test]
fn a_flat_frame_saved_before_loads_and_edits_in_its_host() {
    let mut doc = design();
    let host = doc.build_host();
    let made = wall(&mut doc, [0.0, 0.0, 0.0], [1800.0, 0.0, 0.0], json!({}));
    let lead = made["frame"].clone();
    let frame = uid(&made["frame_component"]);
    let lead_uid = uid(&lead);
    // As a design saved before mitcad#522 has it: the members in the host,
    // no component of the frame's own, its definition without one.
    doc.apply(|state| {
        for o in state.assembly.occurrences.iter_mut() {
            if o.parent == frame {
                o.parent = host;
            }
        }
        state.assembly.remove(frame);
        let record = state
            .assembly
            .component_mut(lead_uid)
            .and_then(|c| c.build.as_mut())
            .expect("the lead");
        record
            .framing
            .as_mut()
            .and_then(Value::as_object_mut)
            .expect("its definition")
            .remove("component");
        Ok(("Flatten".to_owned(), ()))
    })
    .unwrap();
    let saved = doc.to_json();
    let mut doc = Document::from_json(&saved, MockKernel::default()).unwrap();
    doc.set_link_resolver(Some(TestLibrary::new()));
    doc.recompute();
    all_ok(&doc);
    let (members, _) = of_frame(&doc, &lead);
    assert_eq!(members.len(), 6);
    assert!(members.iter().all(|c| parent_of(&doc, *c) == host));
    assert!(
        parts_of(&doc)
            .iter()
            .all(|p| p["frame_component"].is_null())
    );
    // Laid out again with MODIFY: it stays flat in its host (no component
    // is made, nothing re-parented), its fasteners there.
    let components = doc.assembly().components.len();
    let edited = command(
        &mut doc,
        json!({"cmd": "build_edit", "component": lead,
               "frame": {"spacing": 400, "fastener": part("wood-screw")}}),
    )
    .unwrap();
    all_ok(&doc);
    assert_eq!(edited["count"], 8, "{edited}");
    assert!(edited["frame_component"].is_null(), "{edited}");
    let (members, fasteners) = of_frame(&doc, &lead);
    assert_eq!(members.len(), 8);
    assert_eq!(fasteners.len(), 24);
    assert!(
        members
            .iter()
            .chain(&fasteners)
            .all(|c| parent_of(&doc, *c) == host)
    );
    assert_eq!(doc.assembly().components.len(), components + 2 + 24);
    assert_eq!(joints_in(&doc, host), 24);
    // A new frame beside it is a component of its own.
    let w2 = wall(
        &mut doc,
        [0.0, 2000.0, 0.0],
        [1800.0, 2000.0, 0.0],
        json!({}),
    );
    assert!(w2["frame_component"].is_string(), "{w2}");
}

#[test]
fn the_cut_list_goes_by_frame() {
    let mut doc = design();
    let w1 = wall(&mut doc, [0.0, 0.0, 0.0], [1800.0, 0.0, 0.0], json!({}));
    let w2 = wall(
        &mut doc,
        [0.0, 3000.0, 0.0],
        [1800.0, 3000.0, 0.0],
        json!({}),
    );
    all_ok(&doc);
    let list = query(&doc, json!({"query": "build_cut_list"}));
    let rows = list["rows"].as_array().unwrap();
    // The 2304 studs: a row of four for each wall, not one of eight.
    let studs: Vec<&Value> = rows
        .iter()
        .filter(|r| (r["length"].as_f64().unwrap() - 2304.0).abs() < 1e-6)
        .collect();
    assert_eq!(studs.len(), 2, "{rows:?}");
    for (row, name) in studs.iter().zip(["Wall 1", "Wall 2"]) {
        assert_eq!(row["frame"], name, "{row}");
        assert_eq!(row["quantity"], 4, "{row}");
    }
    // Every member's row names its frame; the rows of one frame together.
    let frames: Vec<&str> = rows.iter().map(|r| r["frame"].as_str().unwrap()).collect();
    let first_of_2 = frames.iter().position(|f| *f == "Wall 2").unwrap();
    assert!(
        frames[..first_of_2].iter().all(|f| *f == "Wall 1"),
        "{frames:?}"
    );
    assert!(
        frames[first_of_2..].iter().all(|f| *f == "Wall 2"),
        "{frames:?}"
    );
    let _ = (w1, w2);
}
