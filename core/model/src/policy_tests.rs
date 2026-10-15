// SPDX-License-Identifier: MIT
//! Design policy (mitcad#220) with the mock kernel: the new design's
//! Assembly root and Part1, creation refused where an owner does not
//! allow it, attachment and reference permissions in both directions,
//! violations of existing content counted and listed, moves validated
//! before anything changes, and the policy kept in the design file and in
//! undo and redo. Older designs load without a policy and unchanged.

use serde_json::{Value, json};

use crate::document_tests::{extrude, num};
use crate::features::SketchPlane;
use crate::ids::{BodyUid, ComponentUid, FeatureUid};
use crate::kernel::Curve3;
use crate::policy::Policy;
use crate::testing::MockKernel;
use crate::transform::Transform;
use crate::{Document, FeatureStatus};

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    serde_json::from_str(&doc.command(&command.to_string()).unwrap()).unwrap()
}

fn rejected(doc: &mut Document<MockKernel>, command: Value) -> String {
    doc.command(&command.to_string()).unwrap_err().to_string()
}

/// Lets every owner's sketches attach to every other owner's faces and
/// planes (fixtures of linked sketches, made before the policy).
pub(crate) fn allow_attachments(doc: &mut Document<MockKernel>) {
    let owners: Vec<ComponentUid> = std::iter::once(ComponentUid::ROOT)
        .chain(doc.assembly().components.iter().map(|c| c.uid))
        .collect();
    for owner in owners {
        let policy = Policy {
            attach_to_others: true,
            attachments_from_others: true,
            ..doc.policy(owner)
        };
        doc.set_policy(owner, policy).unwrap();
    }
}

/// A 10 x 10 x 5 block in the active component: its body and top face.
fn block(doc: &mut Document<MockKernel>) -> (BodyUid, String) {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let uid = doc
        .add_feature(&extrude(sketch, &[&region], 5.0, "new_body", &[]), None)
        .unwrap()
        .uid;
    (BodyUid::new(uid, 0), format!("{uid}:end({region})"))
}

/// Root with A:1 (O1) holding a block and an empty B:1 (O2); B active.
fn siblings() -> (
    Document<MockKernel>,
    BodyUid,
    String,
    ComponentUid,
    ComponentUid,
) {
    let mut doc = Document::new(MockKernel::default());
    let (a, _) = doc
        .create_component(Some("A"), Transform::translation([100.0, 0.0, 10.0]), true)
        .unwrap();
    let (body, face) = block(&mut doc);
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let (b, _) = doc
        .create_component(Some("B"), Transform::translation([0.0, 50.0, -3.0]), true)
        .unwrap();
    (doc, body, face, a, b)
}

fn set(doc: &mut Document<MockKernel>, component: &str, policy: Value) -> Value {
    command(
        doc,
        json!({"cmd": "set_policy", "component": component, "policy": policy}),
    )
}

fn uid(value: &Value) -> FeatureUid {
    value["uid"].as_str().unwrap().parse().unwrap()
}

/// The count and the uids of the objects that break a setting.
fn breaking(doc: &Document<MockKernel>, component: &str, setting: &str) -> Vec<String> {
    let report = query(doc, json!({"query": "policy", "component": component}));
    let row = report["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["setting"] == setting)
        .unwrap()
        .clone();
    let items: Vec<String> = row["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["uid"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(row["count"], json!(items.len()));
    items
}

#[test]
fn a_new_design_has_an_assembly_root_and_an_active_part() {
    let mut doc = Document::new_design(MockKernel::default());
    assert!(!doc.is_modified());
    let policies = query(&doc, json!({"query": "policies"}));
    assert_eq!(policies[0]["name"], "Root");
    assert_eq!(policies[0]["policy"]["preset"], "assembly");
    assert_eq!(policies[1]["name"], "Part1");
    assert_eq!(policies[1]["policy"]["preset"], "part");
    let part = doc.active_component();
    assert_eq!(doc.assembly().name(part), "Part1");

    // The first sketch and feature go into Part1.
    let sketch = uid(&command(&mut doc, json!({"cmd": "sketch.create"})));
    assert_eq!(doc.feature(sketch).unwrap().component, part);
    let plane = command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "construction_plane",
               "definition": {"type": "offset", "plane": "xy", "distance": 4}}}),
    );
    assert_eq!(doc.feature(uid(&plane)).unwrap().component, part);
    // Part1 has no child components: a new one goes beside it
    // (mitcad#292, `a_new_component_refused_by_a_part_goes_beside_it`).

    // Activating the root is allowed and redirects nothing: the root
    // refuses sketches and features with the reason.
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "Root"}),
    );
    let depth = doc.undo_depth();
    let error = rejected(&mut doc, json!({"cmd": "sketch.create"}));
    assert!(error.contains("Root does not allow sketches"), "{error}");
    assert!(error.contains("Activate a component"), "{error}");
    let error = rejected(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "construction_plane",
               "definition": {"type": "offset", "plane": "xy", "distance": 4}}}),
    );
    assert!(
        error.contains("Root does not allow modelling features"),
        "{error}"
    );
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.features().count(), 2);
    let report = query(&doc, json!({"query": "policy"}));
    assert_eq!(report["name"], "Root");
    assert!(report["refusals"]["sketches"].is_string());
    assert!(report["refusals"].get("components").is_none());
    // The root takes components: Part2, which the application makes a part.
    let made = command(
        &mut doc,
        json!({"cmd": "create_component", "name": "Part2", "policy": "part"}),
    );
    assert_eq!(made["name"], "Part2");
    let part2 = doc.active_component();
    assert_eq!(doc.policy(part2), Policy::PART);
}

#[test]
fn an_older_design_loads_with_the_general_defaults_and_unchanged() {
    let (mut doc, body, face, _, b) = siblings();
    // B's own sketch first: the attached one is not its first (mitcad#293).
    doc.add_sketch(SketchPlane::Xy).unwrap();
    // A sketch of B on A's face made without checks (as an importer
    // replays a file): kept and evaluated, shown as a violation.
    let sketch = doc
        .without_policy_checks(|doc| {
            doc.command(
                &json!({"cmd": "sketch.create", "plane": {"face": face, "body": body},
                        "occurrence": "O1"})
                .to_string(),
            )
        })
        .unwrap();
    let sketch: Value = serde_json::from_str(&sketch).unwrap();
    let text = doc.to_json();
    assert!(!text.contains("policy"), "{text}");
    let mut loaded = Document::from_json(&text, MockKernel::default()).unwrap();
    loaded.recompute();
    assert_eq!(loaded.assembly().components.len(), 2);
    assert!(
        loaded
            .assembly()
            .components
            .iter()
            .all(|c| c.name != "Part1")
    );
    assert_eq!(loaded.active_component(), b);
    for owner in ["C0", "C1", "C2"] {
        let report = query(&loaded, json!({"query": "policy", "component": owner}));
        assert_eq!(report["policy"]["preset"], "defaults", "{owner}");
    }
    let uid = uid(&sketch);
    assert_eq!(loaded.status(uid), Some(&FeatureStatus::Ok));
    assert_eq!(
        breaking(&loaded, "B", "attach_to_others"),
        [uid.to_string()]
    );
    assert_eq!(
        breaking(&loaded, "A", "attachments_from_others"),
        [uid.to_string()]
    );
    // It stays editable; an unrelated edit of the design passes.
    command(
        &mut loaded,
        json!({"cmd": "sketch.add_rectangle", "sketch": uid, "corner": [0, 0],
               "width": 2, "height": 2}),
    );
    command(
        &mut loaded,
        json!({"cmd": "rename_feature", "uid": uid, "name": "Kept"}),
    );
}

#[test]
fn either_side_of_an_attachment_can_refuse_it() {
    let (mut doc, body, face, _, _) = siblings();
    // B's own sketch first: the attached one is not its first, which the
    // first-sketch settings would allow (mitcad#293).
    doc.add_sketch(SketchPlane::Xy).unwrap();
    let create = json!({"cmd": "sketch.create", "plane": {"face": face, "body": body},
                        "occurrence": "O1"});
    // The general defaults: both sides refuse.
    let error = rejected(&mut doc, create.clone());
    assert!(
        error.contains("cannot be attached to a face or plane of A"),
        "{error}"
    );
    assert!(error.contains("B's policy"), "{error}");
    let supports = query(&doc, json!({"query": "sketch_supports"}));
    assert!(supports["denied_paths"]["O1"].is_string(), "{supports}");
    assert!(supports["denied_paths"].get("O2").is_none());

    // B may attach, A does not take attachments.
    set(&mut doc, "B", json!({"attach_to_others": true}));
    let error = rejected(&mut doc, create.clone());
    assert!(
        error.contains("A's policy does not allow sketches of other"),
        "{error}"
    );
    // A takes them, B may not attach.
    set(&mut doc, "B", json!({"attach_to_others": false}));
    set(&mut doc, "A", json!({"attachments_from_others": true}));
    let error = rejected(&mut doc, create.clone());
    assert!(
        error.contains("B's policy does not allow its sketches"),
        "{error}"
    );
    // Both allow it.
    set(&mut doc, "B", json!({"attach_to_others": true}));
    let supports = query(&doc, json!({"query": "sketch_supports"}));
    assert!(supports["denied_paths"].get("O1").is_none(), "{supports}");
    let sketch = uid(&command(&mut doc, create));
    assert_eq!(doc.status(sketch), Some(&FeatureStatus::Ok));
}

#[test]
fn references_are_checked_in_both_directions_and_apart_from_attachments() {
    let (mut doc, body, face, _, _) = siblings();
    *doc.kernel().curves.borrow_mut() = vec![Curve3::Line {
        start: [0.0, 0.0, 5.0],
        end: [10.0, 0.0, 5.0],
    }];
    // B's own sketch; attachments refused both ways do not stop a linked
    // projection of A's face.
    let sketch = uid(&command(&mut doc, json!({"cmd": "sketch.create"})));
    let project = |linked: bool| {
        json!({"cmd": "sketch.project", "sketch": sketch, "source": face, "body": body,
               "linked": linked, "occurrence": "O1"})
    };
    command(&mut doc, project(true));
    // A refuses references from others: a new linked projection is
    // refused, an unlinked one (no reference) is not.
    set(&mut doc, "A", json!({"references_from_others": false}));
    let error = rejected(&mut doc, project(true));
    assert!(error.contains("cannot reference geometry of A"), "{error}");
    assert!(error.contains("Projection Link"), "{error}");
    command(&mut doc, project(false));
    // The existing projection is one violation of A's, listed once.
    assert_eq!(
        breaking(&doc, "A", "references_from_others"),
        [sketch.to_string()]
    );
    // B refuses references to others instead.
    set(&mut doc, "A", json!({"references_from_others": true}));
    set(&mut doc, "B", json!({"reference_others": false}));
    let error = rejected(&mut doc, project(true));
    assert!(
        error.contains("B's policy does not allow references to other"),
        "{error}"
    );
    command(&mut doc, project(false));
    // A query with a proposed policy counts without changing anything.
    let report = query(
        &doc,
        json!({"query": "policy", "component": "A", "policy": {"references_from_others": false}}),
    );
    assert_eq!(report["settings"][8]["setting"], "references_from_others");
    assert_eq!(report["settings"][8]["count"], json!(1));
    assert_eq!(report["policy"]["references_from_others"], json!(false));
    assert!(doc.policy(ComponentUid(1)).references_from_others);
    // Same-owner references are never governed: A's own projection.
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "A"}),
    );
    set(
        &mut doc,
        "A",
        json!({"reference_others": false, "references_from_others": false}),
    );
    let own = uid(&command(&mut doc, json!({"cmd": "sketch.create"})));
    command(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": own, "source": face, "body": body,
               "linked": true}),
    );
}

#[test]
fn violations_count_direct_objects_and_consumers_once() {
    let (mut doc, body, face, a, b) = siblings();
    allow_attachments(&mut doc);
    // B's own sketch first: the attached one is not its first (mitcad#293).
    doc.add_sketch(SketchPlane::Xy).unwrap();
    *doc.kernel().curves.borrow_mut() = vec![Curve3::Line {
        start: [0.0, 0.0, 5.0],
        end: [10.0, 0.0, 5.0],
    }];
    // B: a sketch on A's face with two linked projections of it.
    let sketch = uid(&command(
        &mut doc,
        json!({"cmd": "sketch.create", "plane": {"face": face, "body": body},
               "occurrence": "O1"}),
    ));
    for _ in 0..2 {
        command(
            &mut doc,
            json!({"cmd": "sketch.project", "sketch": sketch, "source": face,
                   "body": body, "linked": true, "occurrence": "O1"}),
        );
    }
    let features = doc.features().count();
    let before = doc.to_json();
    // Changing policies changes no content.
    command(
        &mut doc,
        json!({"cmd": "set_policy", "component": "A", "preset": "part",
               "policy": {"references_from_others": false}}),
    );
    set(
        &mut doc,
        "C0",
        json!({"sketches": false, "features": false, "components": false}),
    );
    assert_eq!(doc.features().count(), features);
    assert!(
        doc.features()
            .all(|f| doc.status(f.uid) == Some(&FeatureStatus::Ok))
    );
    // The consumer once per setting, however many links it has.
    assert_eq!(
        breaking(&doc, "A", "attachments_from_others"),
        [sketch.to_string()]
    );
    assert_eq!(
        breaking(&doc, "A", "references_from_others"),
        [sketch.to_string()]
    );
    assert!(breaking(&doc, "B", "attach_to_others").is_empty());
    // Containment counts what is directly in the owner: the root's two
    // occurrences, not A's sketch and extrude or B's sketch.
    assert_eq!(breaking(&doc, "C0", "components"), ["O1", "O2"]);
    assert!(breaking(&doc, "C0", "sketches").is_empty());
    assert!(breaking(&doc, "C0", "features").is_empty());
    let report = query(&doc, json!({"query": "policy", "component": "A"}));
    let item = &report["settings"][4]["items"][0];
    assert_eq!(item["type"], "sketch");
    assert_eq!(item["component"], json!(b));
    assert_eq!(item["others"][0]["name"], "B");
    assert_eq!(report["policy"]["preset"], Value::Null);
    let _ = a;

    // Existing objects stay editable; a new link that breaks a setting
    // is refused without changing anything, removing violations is not.
    command(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": sketch, "corner": [0, 0],
               "width": 2, "height": 2}),
    );
    let depth = doc.undo_depth();
    let error = rejected(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": sketch, "source": face,
               "body": body, "linked": true, "occurrence": "O1"}),
    );
    assert!(error.contains("cannot reference geometry of A"), "{error}");
    assert_eq!(doc.undo_depth(), depth);
    command(&mut doc, json!({"cmd": "delete_feature", "uid": sketch}));
    assert!(breaking(&doc, "A", "references_from_others").is_empty());

    // Policy changes are undo steps and saved with the design.
    command(&mut doc, json!({"cmd": "undo"}));
    let saved = doc.to_json();
    assert!(saved.contains("\"root_policy\""), "{saved}");
    let loaded = Document::from_json(&saved, MockKernel::default()).unwrap();
    assert_eq!(
        loaded.policy(ComponentUid::ROOT),
        doc.policy(ComponentUid::ROOT)
    );
    assert!(!loaded.policy(ComponentUid(1)).components);
    assert!(!loaded.policy(ComponentUid(1)).references_from_others);
    for _ in 0..3 {
        command(&mut doc, json!({"cmd": "undo"}));
    }
    assert_eq!(doc.to_json(), before);
    command(&mut doc, json!({"cmd": "redo"}));
    assert_eq!(doc.policy(ComponentUid(1)).preset_name(), None);
    assert!(!doc.policy(ComponentUid(1)).components);
}

#[test]
fn moves_and_pastes_follow_the_destination_policy() {
    let (mut doc, _, _, a, _) = siblings();
    // The root's own block, and a sketch of B on its top face.
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let (root_body, root_face) = block(&mut doc);
    let root_sketch = doc
        .features()
        .find(|f| f.def.type_name() == "sketch" && f.component == ComponentUid::ROOT)
        .map(|f| f.uid)
        .unwrap();
    allow_attachments(&mut doc);
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "B"}),
    );
    let attached = uid(&command(
        &mut doc,
        json!({"cmd": "sketch.create", "plane": {"face": root_face, "body": root_body},
               "occurrence": ""}),
    ));

    // The root takes no more components: moving its features into a new
    // component is refused before anything changes.
    set(&mut doc, "Root", json!({"components": false}));
    let depth = doc.undo_depth();
    let json = doc.to_json();
    let error = rejected(
        &mut doc,
        json!({"cmd": "move_to_component", "name": "Base"}),
    );
    assert!(error.contains("Root does not allow components"), "{error}");
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.to_json(), json);
    // Nor pasting an occurrence into it.
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "Root"}),
    );
    let error = rejected(&mut doc, json!({"cmd": "paste_new", "occurrence": "O1"}));
    assert!(error.contains("Root does not allow components"), "{error}");
    let error = rejected(
        &mut doc,
        json!({"cmd": "copy_occurrence", "occurrence": "O1"}),
    );
    assert!(error.contains("Root does not allow components"), "{error}");

    // A takes no sketches: moving the root's sketch into it is refused.
    set(&mut doc, "Root", json!({"components": true}));
    set(&mut doc, "A", json!({"features": false, "sketches": false}));
    let depth = doc.undo_depth();
    let error = rejected(
        &mut doc,
        json!({"cmd": "move_to_component", "features": [root_sketch],
               "destination": {"kind": "existing", "component": a, "occurrence": "O1"}}),
    );
    assert!(error.contains("A does not allow"), "{error}");
    assert_eq!(doc.undo_depth(), depth);

    // A move into a new component keeps the links as allowed as they were:
    // B's sketch stays on the face, which goes into Base.
    command(
        &mut doc,
        json!({"cmd": "move_to_component", "name": "Base"}),
    );
    let base = doc.feature(root_sketch).unwrap().component;
    assert_eq!(doc.assembly().name(base), "Base");
    assert!(doc.policy(base).attachments_from_others);
    assert!(doc.policy(base).sketches && doc.policy(base).components);
    assert_eq!(doc.status(attached), Some(&FeatureStatus::Ok));
}

#[test]
fn checks_are_off_outside_commands_and_when_suspended() {
    let mut doc = Document::new_design(MockKernel::default());
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "Root"}),
    );
    // A Rust caller (an importer) is not checked.
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    assert_eq!(doc.feature(sketch).unwrap().component, ComponentUid::ROOT);
    // Nor a command run without checks.
    doc.without_policy_checks(|doc| doc.command(r#"{"cmd": "sketch.create"}"#))
        .unwrap();
    // The checks are back afterwards.
    assert!(doc.command(r#"{"cmd": "sketch.create"}"#).is_err());
    assert_eq!(breaking(&doc, "Root", "sketches").len(), 2);
}

#[test]
fn reparenting_into_a_component_that_refuses_components_changes_nothing() {
    let mut doc = Document::new(MockKernel::default());
    let (holder, _) = doc
        .create_component(Some("Holder"), Transform::IDENTITY, false)
        .unwrap();
    doc.create_component(Some("Pin"), Transform::translation([10.0, 0.0, 0.0]), false)
        .unwrap();
    set(&mut doc, "Holder", json!({"components": false}));
    let depth = doc.undo_depth();
    let before = doc.to_json();
    // The destination picker's plan says why before anything runs.
    let answer = query(
        &doc,
        json!({"query": "reparent_occurrence", "occurrence": "O2", "parent": "O1"}),
    );
    assert_eq!(answer["plan"]["eligible"], false, "{answer}");
    let problem = answer["plan"]["problems"][0].as_str().unwrap_or_default();
    assert!(
        problem.contains("Holder does not allow components"),
        "{answer}"
    );
    let error = rejected(
        &mut doc,
        json!({"cmd": "reparent_occurrence", "occurrence": "O2", "parent": "O1"}),
    );
    assert!(
        error.contains("Holder does not allow components"),
        "{error}"
    );
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.to_json(), before);
    // Allowed, it moves.
    set(&mut doc, "Holder", json!({"components": true}));
    command(
        &mut doc,
        json!({"cmd": "reparent_occurrence", "occurrence": "O2", "parent": "O1"}),
    );
    let pin = doc
        .assembly()
        .occurrences
        .iter()
        .find(|o| o.uid.0 == 2)
        .unwrap()
        .parent;
    assert_eq!(pin, holder);
}

#[test]
fn a_sketch_moved_into_a_component_that_refuses_sketches_changes_nothing() {
    let mut doc = Document::new(MockKernel::default());
    let (holder, _) = doc
        .create_component(Some("Holder"), Transform::IDENTITY, false)
        .unwrap();
    // A sketch of the root, as the browser's sketch row moves it (mitcad#218).
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    doc.add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap();
    set(&mut doc, "Holder", json!({"sketches": false}));
    let selection = json!({"component": "C0", "features": [sketch], "source_occurrence": "",
                           "destination": {"kind": "existing", "component": holder,
                                           "occurrence": "O1"}});
    let mut picker = selection.clone();
    picker["query"] = json!("move_to_component");
    let answer = query(&doc, picker);
    let problems = answer["problems"].to_string();
    assert!(
        problems.contains("Holder does not allow sketches"),
        "{answer}"
    );
    let depth = doc.undo_depth();
    let before = doc.to_json();
    let mut move_it = selection.clone();
    move_it["cmd"] = json!("move_to_component");
    let error = rejected(&mut doc, move_it.clone());
    assert!(error.contains("Holder does not allow sketches"), "{error}");
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.to_json(), before);
    // Allowed, it moves.
    set(&mut doc, "Holder", json!({"sketches": true}));
    command(&mut doc, move_it);
    assert_eq!(doc.feature(sketch).unwrap().component, holder);
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
}

/// A rectangle sketch in the active component, with checks, and the
/// extrusion of it as a new component (mitcad#292).
fn new_component_extrude(doc: &mut Document<MockKernel>) -> Value {
    let sketch = uid(&command(doc, json!({"cmd": "sketch.create"})));
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    json!({"cmd": "add_feature", "def": {"type": "extrude",
           "profiles": [{"sketch": sketch, "region": region}],
           "extent": {"type": "distance", "distance": 5}, "operation": "new_component"}})
}

#[test]
fn a_new_component_refused_by_a_part_goes_beside_it() {
    // mitcad#292: Part1 (a part) placed 100 mm along X in the root.
    let mut doc = Document::new_design(MockKernel::default());
    let part = doc.active_component();
    command(
        &mut doc,
        json!({"cmd": "set_occurrence_transform", "occurrence": "Part1:1",
               "transform": {"translation": [100, 0, 0]}}),
    );

    // New Component in Part1: made in the root, where Part1 shows.
    let before = doc.to_json();
    let depth = doc.undo_depth();
    let made = command(
        &mut doc,
        json!({"cmd": "create_component", "name": "Inner", "policy": "part"}),
    );
    assert_eq!(made["placed_in"], "C0", "{made}");
    assert_eq!(made["instead_of"], json!(part));
    assert_eq!(
        made["placement_warning"],
        "Part1 does not take components (its policy): New Component made in Root instead"
    );
    let inner: ComponentUid = made["component"].as_str().unwrap().parse().unwrap();
    let occurrence: crate::ids::OccurrenceUid =
        made["occurrence"].as_str().unwrap().parse().unwrap();
    let placed = doc.assembly().occurrence(occurrence).unwrap();
    assert_eq!(placed.parent, ComponentUid::ROOT);
    assert!(near(placed.transform.translation, [100.0, 0.0, 0.0]));
    // Activated as New Component activates; one undo step.
    assert_eq!(doc.active_component(), inner);
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.undo().unwrap(), "New Component Inner");
    assert_eq!(doc.to_json(), before);
    assert_eq!(doc.active_component(), part);

    // Extrude with New Component in Part1: the feature stays in Part1, its
    // component goes into the root and the body is where it would have
    // been.
    let extrude = new_component_extrude(&mut doc);
    let before = doc.to_json();
    let added = command(&mut doc, extrude);
    let feature = uid(&added);
    assert_eq!(added["placed_in"], "C0", "{added}");
    assert_eq!(added["instead_of"], json!(part));
    assert_eq!(
        added["placement_warning"],
        "Part1 does not take components (its policy): Extrude1's new component made in Root \
         instead"
    );
    assert_eq!(doc.feature(feature).unwrap().component, part);
    assert_eq!(doc.status(feature), Some(&FeatureStatus::Ok));
    let made = doc.assembly().created_by(feature).unwrap();
    let placed = doc.assembly().occurrences_of(made).next().unwrap().clone();
    assert_eq!(placed.parent, ComponentUid::ROOT);
    let body = BodyUid::new(feature, 0);
    let instance = doc
        .instances()
        .into_iter()
        .find(|i| i.body == body)
        .unwrap();
    assert_eq!(instance.path, [placed.uid]);
    assert!(near(instance.transform.translation, [100.0, 0.0, 0.0]));
    assert_eq!(doc.active_component(), part);
    assert!(breaking(&doc, "Part1", "components").is_empty());
    doc.undo().unwrap();
    assert_eq!(doc.to_json(), before);
    assert!(doc.assembly().created_by(feature).is_none());
}

#[test]
fn a_new_component_goes_into_the_nearest_owner_that_takes_components() {
    // Root > Frame:1 (general, 0, 50, 0) > Plate:1 (a part, 10, 0, 0).
    let mut doc = Document::new(MockKernel::default());
    command(
        &mut doc,
        json!({"cmd": "create_component", "name": "Frame",
               "transform": {"translation": [0, 50, 0]}}),
    );
    command(
        &mut doc,
        json!({"cmd": "create_component", "name": "Plate", "policy": "part",
               "transform": {"translation": [10, 0, 0]}}),
    );
    let made = command(&mut doc, json!({"cmd": "create_component", "name": "Pin"}));
    assert_eq!(made["placed_in"], "C1", "{made}");
    assert_eq!(made["instead_of"], "C2");
    let occurrence: crate::ids::OccurrenceUid =
        made["occurrence"].as_str().unwrap().parse().unwrap();
    let placed = doc.assembly().occurrence(occurrence).unwrap();
    assert_eq!(placed.parent, ComponentUid(1));
    assert!(near(placed.transform.translation, [10.0, 0.0, 0.0]));
    // Where the component was asked for, nothing changes.
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "Frame"}),
    );
    let made = command(&mut doc, json!({"cmd": "create_component", "name": "Bolt"}));
    assert!(made.get("placed_in").is_none(), "{made}");
}

#[test]
fn a_new_component_is_refused_when_no_owner_above_takes_components() {
    let mut doc = Document::new_design(MockKernel::default());
    set(&mut doc, "Root", json!({"components": false}));
    let extrude = new_component_extrude(&mut doc);
    let depth = doc.undo_depth();
    let before = doc.to_json();
    let error = rejected(&mut doc, json!({"cmd": "create_component"}));
    assert!(error.contains("Part1 does not allow components"), "{error}");
    let error = rejected(&mut doc, extrude);
    assert!(error.contains("Part1 does not allow components"), "{error}");
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.to_json(), before);
}

#[test]
fn a_first_sketch_may_attach_to_another_component_by_default() {
    // mitcad#293: B has no sketch or modelling feature yet; the general
    // defaults.
    let (mut doc, body, face, _, _) = siblings();
    let create = json!({"cmd": "sketch.create", "plane": {"face": face, "body": body},
                        "occurrence": "O1"});
    // Sketch-support picking offers A's faces for B's first sketch.
    let supports = query(&doc, json!({"query": "sketch_supports"}));
    assert!(supports["denied_paths"].get("O1").is_none(), "{supports}");
    let first = uid(&command(&mut doc, create.clone()));
    assert_eq!(doc.status(first), Some(&FeatureStatus::Ok));
    // The violation listing counts it as allowed.
    for setting in ["attach_to_others", "first_sketch_attaches"] {
        assert!(breaking(&doc, "B", setting).is_empty(), "{setting}");
    }
    for setting in [
        "attachments_from_others",
        "first_sketch_attachments_from_others",
    ] {
        assert!(breaking(&doc, "A", setting).is_empty(), "{setting}");
    }
    // B has a sketch now: A's faces are left out, and a second attached
    // sketch is refused as before.
    let supports = query(&doc, json!({"query": "sketch_supports"}));
    let reason = supports["denied_paths"]["O1"].as_str().unwrap_or_default();
    assert!(
        reason.contains("B's policy does not allow its sketches to attach"),
        "{supports}"
    );
    let error = rejected(&mut doc, create);
    assert!(
        error.contains("B's policy does not allow its sketches to attach"),
        "{error}"
    );
    // Turning a first-sketch setting off lists the first sketch under it.
    for (component, setting) in [
        ("B", "first_sketch_attaches"),
        ("A", "first_sketch_attachments_from_others"),
    ] {
        let report = query(
            &doc,
            json!({"query": "policy", "component": component, "policy": {setting: false}}),
        );
        let row = report["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["setting"] == setting)
            .unwrap()
            .clone();
        assert_eq!(row["allowed"], false);
        assert_eq!(row["items"][0]["uid"], json!(first), "{report}");
    }

    // A new design's parts: Part2's first sketch may lie on Part1's faces.
    let mut doc = Document::new_design(MockKernel::default());
    block(&mut doc);
    command(
        &mut doc,
        json!({"cmd": "activate_component", "component": "Root"}),
    );
    command(
        &mut doc,
        json!({"cmd": "create_component", "name": "Part2", "policy": "part"}),
    );
    let supports = query(&doc, json!({"query": "sketch_supports"}));
    assert!(supports["denied_paths"].get("O1").is_none(), "{supports}");
    assert_eq!(supports["denied"], json!([]), "{supports}");
}

#[test]
fn either_first_sketch_setting_off_refuses_the_first_sketch() {
    let (mut doc, body, face, _, _) = siblings();
    let create = json!({"cmd": "sketch.create", "plane": {"face": face, "body": body},
                        "occurrence": "O1"});
    // The sketch owner's side.
    set(&mut doc, "B", json!({"first_sketch_attaches": false}));
    let supports = query(&doc, json!({"query": "sketch_supports"}));
    let reason = supports["denied_paths"]["O1"].as_str().unwrap_or_default();
    assert!(
        reason.contains("B's policy does not allow its first sketch to attach"),
        "{supports}"
    );
    let error = rejected(&mut doc, create.clone());
    assert!(
        error.contains("B's policy does not allow its first sketch to attach"),
        "{error}"
    );
    // The face owner's side.
    set(&mut doc, "B", json!({"first_sketch_attaches": true}));
    set(
        &mut doc,
        "A",
        json!({"first_sketch_attachments_from_others": false}),
    );
    let supports = query(&doc, json!({"query": "sketch_supports"}));
    let reason = supports["denied_paths"]["O1"].as_str().unwrap_or_default();
    assert!(
        reason.contains("A's policy does not allow first sketches of other components"),
        "{supports}"
    );
    let error = rejected(&mut doc, create.clone());
    assert!(
        error.contains("A's policy does not allow first sketches of other components"),
        "{error}"
    );
    // Either setting of a side allows that side: A's general one.
    set(&mut doc, "A", json!({"attachments_from_others": true}));
    let sketch = uid(&command(&mut doc, create));
    assert_eq!(doc.status(sketch), Some(&FeatureStatus::Ok));
}

#[test]
fn an_older_stored_policy_reads_the_first_sketch_settings_as_on() {
    // Policies stored before mitcad#293 have seven settings.
    let older = |sketches: bool, components: bool| {
        json!({"sketches": sketches, "features": sketches, "components": components,
               "attach_to_others": false, "attachments_from_others": false,
               "reference_others": true, "references_from_others": true})
    };
    let assembly: Policy = serde_json::from_value(older(false, true)).unwrap();
    assert!(assembly.first_sketch_attaches && assembly.first_sketch_attachments_from_others);
    assert_eq!(assembly.preset_name(), Some("assembly"));
    let part: Policy = serde_json::from_value(older(true, false)).unwrap();
    assert_eq!(part, Policy::PART);
    assert_eq!(part.preset_name(), Some("part"));

    // A new design saved before them: the root an Assembly, Part1 a part.
    let doc = Document::new_design(MockKernel::default());
    let mut file: Value = serde_json::from_str(&doc.to_json()).unwrap();
    file["root_policy"] = older(false, true);
    file["components"][0]["policy"] = older(true, false);
    let loaded = Document::from_json(&file.to_string(), MockKernel::default()).unwrap();
    let root = loaded.policy(ComponentUid::ROOT);
    assert!(root.first_sketch_attaches && root.first_sketch_attachments_from_others);
    assert_eq!(loaded.policy(ComponentUid(1)), Policy::PART);
    let policies = query(&loaded, json!({"query": "policies"}));
    assert_eq!(policies[0]["policy"]["preset"], "assembly");
    assert_eq!(policies[1]["policy"]["preset"], "part");
    // The presets themselves: both settings on, but the Assembly's sketches
    // (it has none) do not attach.
    const {
        assert!(Policy::GENERAL.first_sketch_attaches && Policy::PART.first_sketch_attaches);
        assert!(!Policy::ASSEMBLY.first_sketch_attaches);
        assert!(Policy::ASSEMBLY.first_sketch_attachments_from_others);
    }
}

#[test]
fn an_edit_previews_in_its_own_component_under_the_policy() {
    // mitcad#309: B (active) has its first sketch attached to A, which
    // allows that only as a first sketch. Editing A's fillet previews and
    // commits in A, not as a new feature of B.
    let (mut doc, body, face, a, b) = siblings();
    doc.activate_component(a).unwrap();
    let fillet_def = |radius: f64| {
        json!({"type": "fillet", "body": body,
               "edges": [format!("E{{{0}:side(c1[c4,c2])|{0}:side(c2[c1,c3])}}", body.feature)],
               "radius": radius})
    };
    let fillet = uid(&command(
        &mut doc,
        json!({"cmd": "add_feature", "def": fillet_def(1.0)}),
    ));
    doc.activate_component(b).unwrap();
    let create = json!({"cmd": "sketch.create", "plane": {"face": face, "body": body},
                        "occurrence": "O1"});
    let first = uid(&command(&mut doc, create));
    assert_eq!(doc.status(first), Some(&FeatureStatus::Ok));
    doc.set_policy(
        a,
        Policy {
            attachments_from_others: false,
            first_sketch_attachments_from_others: true,
            ..Policy::PART
        },
    )
    .unwrap();
    doc.set_policy(b, Policy::PART).unwrap();
    assert_eq!(doc.active_component(), b);

    // Previewed as an added feature it would be a new one of B (what the
    // app did for an edit), with a uid of its own.
    let added = json!({"cmd": "add_feature", "def": fillet_def(2.0)});
    if let Ok(report) = doc.preview(&added.to_string()) {
        let report: Value = serde_json::from_str(&report).unwrap();
        assert_ne!(report["uid"], json!(fillet), "{report}");
    }
    // As an edit it is computed in A, under its own uid.
    let edited = json!({"cmd": "edit_feature", "uid": fillet, "def": fillet_def(2.0)});
    let report: Value = serde_json::from_str(&doc.preview(&edited.to_string()).unwrap()).unwrap();
    assert_eq!(report["status"], "ok", "{report}");
    assert_eq!(report["uid"], json!(fillet), "{report}");
    assert_eq!(doc.feature(fillet).unwrap().component, a);
    // ... and commits.
    let done = command(&mut doc, edited);
    assert_eq!(done["error"], Value::Null, "{done}");
    assert_eq!(doc.status(fillet), Some(&FeatureStatus::Ok));

    // The editor rolls the marker back to just before the feature; the
    // edit preview computes the feature there all the same.
    let position = doc.features().position(|f| f.uid == fillet).unwrap();
    doc.set_marker(position).unwrap();
    let edited = json!({"cmd": "edit_feature", "uid": fillet, "def": fillet_def(3.0)});
    let report: Value = serde_json::from_str(&doc.preview(&edited.to_string()).unwrap()).unwrap();
    assert_eq!(report["status"], "ok", "{report}");
}
