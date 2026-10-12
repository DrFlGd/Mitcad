// SPDX-License-Identifier: MIT
//! Components of other documents filled from those documents' own imports
//! (mitcad#196) on synthetic dumps with the mock kernel: a two-document
//! assembly whose part has a component of its own, placed by a captured
//! position of the assembly.

use super::*;
use crate::references::{Referenced, link_references};

/// The part: the block of [`block_dump`] with an occurrence of a component
/// of its own (`Pin`, 3 cm up), which carries a GUID.
fn part_dump() -> Dump {
    let mut v = serde_json::to_value(block_dump()).unwrap();
    v["document"] = json!({"name": "Part", "root_component": "Part"});
    v["components"] = json!([
        {"name": "Part", "is_root": true, "_f3d": {"object_id": 1}},
        {"name": "Pin", "_f3d": {"object_id": 5}}
    ]);
    v["occurrences"] = json!([{
        "component": "Pin",
        "transform": [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 3.0],
                      [0.0, 0.0, 0.0, 1.0]],
        "children": [],
        "_f3d": {"object_id": 80, "component_object": 5, "guids": ["c0ffee00-0000-4000-8000-0000000000b1"]}
    }]);
    dump(v)
}

const PART_URN: &str = "urn:mitcad.test:fs.file:vf.part?version=1";

/// The assembly: two occurrences of the part's root component (1 cm along
/// x and 5 cm along y), and a captured position of the pin inside the
/// first at (1, 2, 0) cm.
fn assembly_dump() -> Dump {
    let occurrence = |id: u64, x: f64, y: f64, guid: &str| {
        json!({
            "transform": [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, 0.0],
                          [0.0, 0.0, 0.0, 1.0]],
            "isReferencedComponent": true,
            "children": [],
            "_f3d": {"object_id": id, "component_object": 1, "external_key": "00000000000000aa",
                     "external_document": PART_URN, "guids": [guid]}
        })
    };
    dump(json!({
        "schema": "mitcad-f3d-dump", "schema_version": 2,
        "source": {"mode": "f3d_stream", "file": "assembly.f3d"},
        "document": {"name": "Assembly", "root_component": "Assembly"},
        "components": [{"name": "Assembly", "is_root": true, "_f3d": {"object_id": 1}}],
        "occurrences": [
            occurrence(10, 1.0, 0.0, "c0ffee00-0000-4000-8000-0000000000a1"),
            occurrence(11, 0.0, 5.0, "c0ffee00-0000-4000-8000-0000000000a2")
        ],
        "timeline": {"items": [{
            "index": 0, "name": "Position1", "objectType": "Snapshot",
            "detail": {"positions": [{
                "occurrence": {"kind": "occurrence", "_f3d": {
                    "path": [10, null],
                    "path_guids": ["c0ffee00-0000-4000-8000-0000000000a1",
                                   "C0FFEE00-0000-4000-8000-0000000000B1"],
                    "context_component": 1}},
                "transform": [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 2.0], [0.0, 0.0, 1.0, 0.0],
                              [0.0, 0.0, 0.0, 1.0]]
            }]},
            "_f3d": {"component": 1}
        }]},
        "_f3d": {"external_documents": [{"key": "00000000000000aa", "urn": PART_URN, "version": 1}]}
    }))
}

#[test]
fn components_of_other_documents_are_filled_from_their_imports() {
    let mut part = Document::new(MockKernel::default());
    let part_report = import(&mut part, &part_dump());
    assert_eq!(part_report.items[1].outcome, Outcome::Parametric);
    assert_eq!(part_report.components.occurrence_guids.len(), 1);

    let mut doc = Document::new(MockKernel::default());
    let mut report = import(&mut doc, &assembly_dump());
    // The replay places the occurrences of an empty component and leaves
    // the position inside it out.
    let inserted = report.components.inserted.clone();
    assert_eq!(inserted.len(), 1, "{:?}", report.components);
    assert_eq!(inserted[0].document, PART_URN);
    assert_eq!(inserted[0].occurrences, 2);
    assert_eq!(report.components.external, 2);
    assert_eq!(report.components.positions_inside.len(), 1);
    let component = inserted[0].component;
    assert!(doc.component_bodies(component).is_empty());

    let sources = HashMap::from([(
        PART_URN.to_owned(),
        Referenced {
            document: &part,
            occurrence_guids: &part_report.components.occurrence_guids,
            name: "Part".to_owned(),
        },
    )]);
    let outcome = link_references(&mut doc, &mut report, &sources);
    assert!(outcome.failed.is_empty(), "{outcome:?}");
    assert_eq!(outcome.linked.len(), 1);
    assert_eq!(outcome.linked[0].name, "Part");
    assert_eq!(doc.assembly().name(component), "Part");
    // The part's block, with its history: the copy's features are the
    // component's.
    doc.recompute();
    assert_eq!(doc.component_bodies(component).len(), 1);
    assert!(doc.features().filter(|f| f.component == component).count() >= 2);
    // The pin inside the part, where the captured position puts it in the
    // first occurrence: (10, 20, 0) mm in the assembly, (0, 20, 0) mm in
    // the part.
    assert_eq!(outcome.positions, 1, "{outcome:?}");
    let pin = report.components.occurrence_guids["c0ffee00-0000-4000-8000-0000000000b1"];
    let placed = doc.assembly().occurrence(pin).unwrap();
    assert_eq!(placed.parent, component);
    doc.recompute();
    let t = doc.placement(pin).unwrap().translation;
    assert!(
        (t[0]).abs() < 1e-9 && (t[1] - 20.0).abs() < 1e-9 && t[2].abs() < 1e-9,
        "{t:?}"
    );
    // The captured position came in as a feature of the part's component,
    // and its report entry says so.
    let item = &report.items[0];
    assert_eq!(item.outcome, Outcome::Parametric, "{item:?}");
    assert_eq!(item.note, None);
    let feature: FeatureUid = item.features[0].parse().unwrap();
    assert_eq!(
        doc.features().find(|f| f.uid == feature).unwrap().component,
        component
    );
    assert_eq!(doc.import_note(feature), None);
    // A second link has nothing left to fill.
    let again = link_references(&mut doc, &mut report, &sources);
    assert_eq!(again.failed.len(), 1);
}

#[test]
fn components_of_documents_not_found_stay_empty() {
    let mut doc = Document::new(MockKernel::default());
    let mut report = import(&mut doc, &assembly_dump());
    let outcome = link_references(&mut doc, &mut report, &HashMap::new());
    assert!(outcome.linked.is_empty() && outcome.failed.is_empty());
    assert_eq!(
        outcome
            .positions_left_out
            .get("a position inside a component of a document not found"),
        Some(&1)
    );
    let component = report.components.inserted[0].component;
    assert!(doc.component_bodies(component).is_empty());
}
