// SPDX-License-Identifier: MIT
//! Autoselect uses current bounded regions, persistent provenance and the
//! whole sketch's history position, not per-region consumption alone.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::Document;
use crate::kernel::Curve3;
use crate::testing::MockKernel;

fn command(doc: &mut Document<MockKernel>, value: Value) -> Value {
    let result = doc
        .command(&value.to_string())
        .unwrap_or_else(|error| panic!("{value}: {error}"));
    serde_json::from_str(&result).unwrap()
}

fn query(doc: &Document<MockKernel>, value: Value) -> Value {
    serde_json::from_str(&doc.query(&value.to_string()).unwrap()).unwrap()
}

fn groups(doc: &Document<MockKernel>) -> Value {
    query(doc, json!({"query": "autoselect_profiles"}))
}

fn empty() -> Value {
    json!({"eligible": false, "sketch": null, "component": null,
           "projected": [], "drawn": [], "hybrid": [], "all": []})
}

fn assert_partition(doc: &Document<MockKernel>, expected_counts: [usize; 3]) -> Value {
    let value = groups(doc);
    assert_eq!(value["eligible"], true, "{value}");
    let mut union = BTreeSet::new();
    for (group, expected) in ["projected", "drawn", "hybrid"]
        .into_iter()
        .zip(expected_counts)
    {
        let regions = value[group].as_array().unwrap();
        assert_eq!(regions.len(), expected, "{group}: {value}");
        for region in regions {
            assert!(union.insert(region.as_str().unwrap().to_owned()), "{value}");
        }
    }
    let all: BTreeSet<_> = value["all"]
        .as_array()
        .unwrap()
        .iter()
        .map(|region| region.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(union, all);
    assert_eq!(all.len(), value["all"].as_array().unwrap().len());
    let profiles = query(doc, json!({"query": "profiles"}));
    let current: BTreeSet<_> = profiles
        .as_array()
        .unwrap()
        .iter()
        .filter(|profile| profile["sketch"] == value["sketch"])
        .map(|profile| profile["region"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(all, current, "every current region appears exactly once");
    value
}

/// Fixed circles make the distinction between origin and solver freedom
/// explicit. Each tuple is (centre, radius, projected, construction).
fn circles(specs: &[([f64; 2], f64, bool, bool)]) -> Document<MockKernel> {
    let mut entities = Vec::new();
    for (index, (center, radius, projected, construction)) in specs.iter().enumerate() {
        let point = format!("p{}", 2 * index + 1);
        entities.push(json!({"id": point, "type": "point", "at": center, "fixed": true}));
        entities.push(
            json!({"id": format!("c{}", 2 * index + 2), "type": "circle",
                            "center": point, "radius": radius, "fixed": true,
                            "reference": projected, "construction": construction}),
        );
    }
    let mut doc = Document::new(MockKernel::default());
    command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "sketch", "entities": entities}}),
    );
    doc
}

fn projected_rectangle(linked: bool) -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    command(&mut doc, json!({"cmd": "sketch.create"}));
    let rectangle = command(
        &mut doc,
        json!({"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0],
               "width": 60, "height": 40}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": rectangle["region"]}],
               "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}}),
    );
    command(&mut doc, json!({"cmd": "sketch.create"}));
    let corners = [[0., 0., 5.], [60., 0., 5.], [60., 40., 5.], [0., 40., 5.]];
    *doc.kernel().curves.borrow_mut() = (0..4)
        .map(|i| Curve3::Line {
            start: corners[i],
            end: corners[(i + 1) % 4],
        })
        .collect();
    command(
        &mut doc,
        json!({"cmd": "sketch.project", "sketch": "F3", "linked": linked,
               "source": format!("F2:end({})", rectangle["region"].as_str().unwrap())}),
    );
    doc
}

#[test]
fn projected_rectangle_and_three_drawn_circles_select_only_the_circle_interiors() {
    for linked in [false, true] {
        let mut doc = projected_rectangle(linked);
        assert_partition(&doc, [1, 0, 0]);
        let mut interiors = Vec::new();
        for x in [10, 30, 50] {
            let added = command(
                &mut doc,
                json!({"cmd": "sketch.add_circle", "sketch": "F3",
                       "center": [x, 10], "diameter": 5}),
            );
            interiors.push(format!("r{{{}}}", added["curves"][0].as_str().unwrap()));
        }
        // Construction geometry and an isolated open curve add no regions.
        command(
            &mut doc,
            json!({"cmd": "sketch.circle", "sketch": "F3", "mode": "center",
                   "center": [30, 30], "radius": 3, "construction": true}),
        );
        command(
            &mut doc,
            json!({"cmd": "sketch.add_line", "sketch": "F3", "start": [70, 0], "end": [80, 0]}),
        );
        let value = assert_partition(&doc, [0, 3, 1]);
        assert_eq!(value["sketch"], "F3");
        assert_eq!(value["component"], "C0");
        let actual: BTreeSet<_> = value["drawn"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| key.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(actual, interiors.into_iter().collect());
        let output = doc.sketch_output("F3".parse().unwrap()).unwrap();
        let surrounding = output
            .regions
            .iter()
            .find(|region| region.key.to_string() == value["hybrid"][0].as_str().unwrap())
            .unwrap();
        assert_eq!(
            surrounding.loops.len(),
            4,
            "outer rectangle and three holes"
        );
    }
}

#[test]
fn projected_origin_survives_unlink_save_and_undo_while_fixed_drawings_stay_drawn() {
    let mut doc = projected_rectangle(true);
    let before = assert_partition(&doc, [1, 0, 0]);
    // Breaking a link removes the source dependency, not the entities'
    // stored reference flags (the same form as Project with linked=false).
    let mut def = query(&doc, json!({"query": "feature", "uid": "F3"}))["def"].clone();
    def["projections"] = json!([]);
    command(
        &mut doc,
        json!({"cmd": "edit_feature", "uid": "F3", "def": def}),
    );
    assert_eq!(groups(&doc), before);
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(groups(&doc), before);
    command(&mut doc, json!({"cmd": "redo"}));
    let mut loaded = Document::from_json(&doc.to_json(), MockKernel::default()).unwrap();
    loaded.recompute();
    assert_eq!(groups(&loaded), before);

    let circle = command(
        &mut loaded,
        json!({"cmd": "sketch.circle", "sketch": "F3", "mode": "center",
               "center": [80, 20], "radius": 4}),
    );
    command(
        &mut loaded,
        json!({"cmd": "sketch.set_fixed", "sketch": "F3", "entities": circle["entities"]}),
    );
    assert_partition(&loaded, [1, 1, 0]);
}

#[test]
fn linked_projection_records_also_preserve_origin_for_older_definitions() {
    let mut doc = projected_rectangle(true);
    let before = assert_partition(&doc, [1, 0, 0]);
    let mut def = query(&doc, json!({"query": "feature", "uid": "F3"}))["def"].clone();
    for entity in def["entities"].as_array_mut().unwrap() {
        entity.as_object_mut().unwrap().remove("reference");
    }
    command(
        &mut doc,
        json!({"cmd": "edit_feature", "uid": "F3", "def": def}),
    );
    assert_eq!(groups(&doc), before);
}

#[test]
fn nested_regions_classify_their_own_outer_boundary_and_immediate_holes() {
    let doc = circles(&[
        ([0., 0.], 10., true, false),
        ([0., 0.], 6., false, false),
        ([0., 0.], 2., true, false),
    ]);
    let value = assert_partition(&doc, [1, 0, 2]);
    assert_eq!(value["projected"], json!(["r{c6}"]));
    assert_eq!(value["hybrid"], json!(["r{c2}", "r{c4}"]));
    let output = doc.sketch_output("F1".parse().unwrap()).unwrap();
    assert_eq!(
        output
            .regions
            .iter()
            .map(|r| r.loops.len())
            .collect::<Vec<_>>(),
        [2, 2, 1]
    );
}

#[test]
fn intersecting_projected_and_drawn_curves_make_three_distinct_hybrid_regions() {
    let doc = circles(&[([0., 0.], 5., true, false), ([6., 0.], 5., false, false)]);
    assert_partition(&doc, [0, 0, 3]);
    for region in &doc.sketch_output("F1".parse().unwrap()).unwrap().regions {
        assert!(region.key.segments().all(|segment| segment.ends.is_some()));
    }
}

#[test]
fn only_the_final_evaluated_sketch_is_offered_and_never_an_older_fallback() {
    let mut doc = Document::new(MockKernel::default());
    assert_eq!(groups(&doc), empty());
    doc = circles(&[([0., 0.], 5., false, false)]);
    let eligible = assert_partition(&doc, [0, 1, 0]);
    command(&mut doc, json!({"cmd": "sketch.create"}));
    assert_eq!(
        groups(&doc),
        empty(),
        "do not search past an empty final sketch"
    );
    command(
        &mut doc,
        json!({"cmd": "sketch.circle", "sketch": "F2", "mode": "center",
               "center": [0, 0], "radius": 5, "construction": true}),
    );
    command(
        &mut doc,
        json!({"cmd": "sketch.add_line", "sketch": "F2", "start": [10, 0], "end": [20, 0]}),
    );
    assert_eq!(groups(&doc), empty());
    command(&mut doc, json!({"cmd": "delete_feature", "uid": "F2"}));
    assert_eq!(groups(&doc), eligible);
    command(&mut doc, json!({"cmd": "suppress_feature", "uid": "F1"}));
    assert_eq!(groups(&doc), empty());
    command(&mut doc, json!({"cmd": "undo"}));
    command(&mut doc, json!({"cmd": "set_marker", "position": 0}));
    assert_eq!(groups(&doc), empty());
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(groups(&doc), eligible);
    let plane = command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "construction_plane",
               "definition": {"type": "offset", "plane": "xy", "distance": 10}}}),
    );
    assert_eq!(groups(&doc), empty());
    command(
        &mut doc,
        json!({"cmd": "reorder_feature", "uid": plane["uid"], "index": 0}),
    );
    assert_eq!(groups(&doc), eligible);
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(groups(&doc), empty());
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(groups(&doc), eligible);
}

#[test]
fn a_partial_or_suppressed_consumer_disqualifies_the_sketch_even_when_rolled_back() {
    let mut doc = circles(&[([0., 0.], 5., false, false), ([20., 0.], 5., false, false)]);
    let before = assert_partition(&doc, [0, 2, 0]);
    command(
        &mut doc,
        json!({"cmd": "add_feature", "def": {"type": "extrude",
               "profiles": [{"sketch": "F1", "region": before["drawn"][0]}],
               "extent": {"type": "distance", "distance": 5}, "operation": "new_body"}}),
    );
    let profiles = query(&doc, json!({"query": "profiles"}));
    assert_eq!(
        profiles
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["consumed"] == false)
            .count(),
        1
    );
    assert_eq!(groups(&doc), empty());
    command(&mut doc, json!({"cmd": "suppress_feature", "uid": "F2"}));
    assert_eq!(groups(&doc), empty());
    command(&mut doc, json!({"cmd": "set_marker", "position": 1}));
    assert_eq!(groups(&doc), empty());
    command(&mut doc, json!({"cmd": "set_marker", "position": 2}));
    command(&mut doc, json!({"cmd": "delete_feature", "uid": "F2"}));
    assert_eq!(groups(&doc), before);
}

#[test]
fn topology_edits_and_undo_redo_return_only_current_region_keys() {
    let mut doc = circles(&[([0., 0.], 5., false, false)]);
    let before = assert_partition(&doc, [0, 1, 0]);
    command(
        &mut doc,
        json!({"cmd": "sketch.add_line", "sketch": "F1", "start": [-10, 0], "end": [10, 0]}),
    );
    let divided = assert_partition(&doc, [0, 2, 0]);
    assert!(
        !divided["all"]
            .as_array()
            .unwrap()
            .contains(&before["all"][0])
    );
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(groups(&doc), before);
    command(&mut doc, json!({"cmd": "redo"}));
    assert_eq!(groups(&doc), divided);
}

#[test]
fn text_contours_are_drawings_and_their_counters_stay_separate_regions() {
    let mut doc = Document::new(MockKernel::default());
    command(&mut doc, json!({"cmd": "sketch.create"}));
    command(
        &mut doc,
        json!({"cmd": "sketch.add_text", "sketch": "F1", "text": "O",
               "height": 10, "at": [0, 0], "font": "Mock Sans"}),
    );
    assert_partition(&doc, [0, 2, 0]);
}
