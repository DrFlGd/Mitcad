// SPDX-License-Identifier: MIT
//! Body operations: combines, splits, copies and removals, and how the bodies they use are found.

use super::*;

/// The body fingerprint of an external dump for a replayed body (its volume in
/// cm³, as the test kernel measures it).
fn body_fingerprint(doc: &Document<TestKernel>, uid: &str) -> Value {
    let shape = doc.body_shape(uid.parse().unwrap()).unwrap();
    let volume = doc.kernel().mass_properties(shape).unwrap().volume / 1000.0;
    json!({"kind": "body", "objectType": "BRepBody", "name": doc.body_name(uid.parse().unwrap()),
           "volume": volume})
}

#[test]
fn add_in_body_operations_map_to_mitcad_features() {
    // Two blocks, then a combine, a circular pattern of the first extrude
    // and a mirror of the remaining body, as external dumps give them.
    let first = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(first.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let bodies: Vec<String> = reference
        .bodies()
        .iter()
        .map(|b| b.uid.to_string())
        .collect();
    assert_eq!(bodies.len(), 2, "two extrudes of the same region set");
    let target = body_fingerprint(&reference, &bodies[0]);
    let tool = body_fingerprint(&reference, &bodies[1]);
    let mut items = first;
    items.push(
        json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
        "detail": {"targetBody": target, "toolBodies": [tool], "operation": "CutFeatureOperation",
                   "isKeepToolBodies": true, "isNewComponent": false}}),
    );
    items.push(json!({"index": 4, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
        "detail": {"inputEntities": [{"kind": "feature", "objectType": "ExtrudeFeature",
                                      "name": "Extrude1", "timeline_index": 1}],
                   "patternEntityType": "FeaturesPatternType",
                   "axis": {"kind": "construction_axis", "name": "Z", "origin": "Z"},
                   "quantity": param("d6", "4", 4.0, ""), "totalAngle": param("d7", "360 deg", std::f64::consts::TAU, "deg"),
                   "isSymmetric": false, "patternComputeOption": "IdenticalPatternCompute",
                   "suppressedElementsIds": [2]}}));
    let mut dump = timeline_dump(items);
    for (name, expression, unit) in [("d6", "4", ""), ("d7", "360 deg", "deg")] {
        dump.parameters
            .as_mut()
            .unwrap()
            .model
            .as_mut()
            .unwrap()
            .push(
                serde_json::from_value(
                    json!({"name": name, "expression": expression, "unit": unit}),
                )
                .unwrap(),
            );
    }
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(&mut doc, &dump, &mut NoGeometry, &Options::default());
    let def = |uid: &str| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    assert_eq!(
        report.items[3].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let combine = def(&report.items[3].features[0]);
    assert_eq!(combine["target"], bodies[0].as_str());
    assert_eq!(combine["tools"], json!([bodies[1]]));
    assert_eq!(combine["operation"], "cut");
    assert_eq!(combine["keep_tools"], true);
    let pattern = &report.items[4];
    assert_eq!(pattern.outcome, Outcome::Parametric, "{}", report.text());
    let pattern = def(&pattern.features[0]);
    assert_eq!(
        pattern["objects"],
        json!({"type": "features", "features": ["F2"]})
    );
    assert_eq!(pattern["axis"], json!("z"));
    assert_eq!(pattern["quantity"], "d6");
    assert_eq!(pattern["angle"], "d7");
    assert_eq!(pattern["compute"], "identical");
    assert_eq!(pattern["suppressed_elements"], json!([2]));
}

#[test]
fn bodies_the_decoder_named_are_found_by_points_on_their_edges() {
    // A block, then a move of it, the body given as the decoder gives it:
    // names and middle points of some of its edges (cm).
    let first = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    import_design(
        &mut reference,
        &timeline_dump(first.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (uid, shape) = (reference.bodies()[0].uid, shapes(&reference)[0].clone());
    let points: Vec<[f64; 3]> = shape
        .edges
        .iter()
        .take(3)
        .map(|(a, b)| edge_point(a, b).map(|x| x / 10.0))
        .collect();
    let body = |points: Value| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"recipe": "body", "entities": [[{"tag": "304", "kind": 7, "ops": []}]],
                        "edge_points": points}})
    };
    let replay_with = |body: Value, definition: Value| {
        let mut items = first.clone();
        let transform = json!([
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 0.5],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ]);
        let mut detail = json!({"inputEntities": [body], "transform": transform});
        if !definition.is_null() {
            detail["definition"] = definition;
        }
        let item = json!({"index": 2, "name": "Move1", "objectType": "MoveFeature",
                          "detail": detail});
        items.push(item);
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        let def = report.items[2].features.first().map(|uid| {
            serde_json::from_str::<Value>(
                &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                    .unwrap(),
            )
            .unwrap()["def"]
                .clone()
        });
        (report, def)
    };
    let replay = |body: Value| replay_with(body, Value::Null);
    let (report, def) = replay(body(json!(points)));
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = def.unwrap();
    assert_eq!(def["bodies"], json!([uid.to_string()]));
    assert_eq!(def["transform"]["matrix"][0][3], json!(10.0));
    // Without its definition decoded: the matrix stands in.
    assert_eq!(
        report.items[2].input_provenance,
        Some(InputProvenance::Unknown)
    );
    // A free move as decoded: the matrix is its input.
    let (report, def) = replay_with(
        body(json!(points)),
        json!({"_type": "MoveFeatureFreeMoveDefinition"}),
    );
    assert_eq!(def.unwrap()["transform"]["type"], "free");
    assert_eq!(
        report.items[2].input_provenance,
        Some(InputProvenance::Read)
    );
    // A translation along the axes by its distances (cm).
    let (report, def) = replay_with(
        body(json!(points)),
        json!({"_type": "MoveFeatureTranslateXYZDefinition",
               "xDistance": param("d5", "10 mm", 1.0, "mm"),
               "yDistance": param("d6", "5 mm", 0.5, "mm"),
               "zDistance": param("d7", "0 mm", 0.0, "mm")}),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = def.unwrap();
    assert_eq!(def["transform"]["type"], "translate_xyz", "{def}");
    assert_eq!(
        report.items[2].input_provenance,
        Some(InputProvenance::Read)
    );
    // A point on no edge of the replay: not found.
    let (report, _) = replay(body(json!([[123.0, 0.0, 0.0]])));
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
}

/// A removal and a copy of bodies, found by the points of the body's edges.
#[test]
fn a_removal_and_a_copy_of_bodies_are_features() {
    let first = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    import_design(
        &mut reference,
        &timeline_dump(first.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (uid, shape) = (reference.bodies()[0].uid, shapes(&reference)[0].clone());
    let points: Vec<[f64; 3]> = shape
        .edges
        .iter()
        .take(3)
        .map(|(a, b)| edge_point(a, b).map(|x| x / 10.0))
        .collect();
    let body = json!({"kind": "body", "objectType": "BRepBody",
                      "_f3d": {"recipe": "body", "entities": [[{"tag": "304", "kind": 7, "ops": []}]],
                               "edge_points": points}});
    let replay = |object_type: &str, name: &str, detail: Value| {
        let mut items = first.clone();
        items.push(json!({"index": 2, "name": name, "objectType": object_type,
                          "detail": detail,
                          "_f3d": {"base_name": name.trim_end_matches(char::is_numeric)}}));
        let mut doc = Document::new(TestKernel {
            edge_points: true,
            ..TestKernel::default()
        });
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut NoGeometry,
            &Options::default(),
        );
        let def = report.items[2].features.first().map(|uid| {
            serde_json::from_str::<Value>(
                &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                    .unwrap(),
            )
            .unwrap()["def"]
                .clone()
        });
        (report, doc, def)
    };
    let (report, doc, def) = replay(
        "RemoveFeature",
        "RemoveBody-Body1",
        json!({"itemToRemove": body}),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert_eq!(
        def.unwrap(),
        json!({"type": "remove_body", "bodies": [uid.to_string()]})
    );
    assert!(doc.bodies().is_empty());
    let (report, doc, def) = replay(
        "CopyPasteBodies",
        "CopyPasteBodies1",
        json!({"sourceBody": [body]}),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = def.unwrap();
    assert_eq!(def["type"], json!("move"));
    assert_eq!(def["copy"], json!(true));
    assert_eq!(doc.bodies().len(), 2);
    // A cut whose body the file lost keeps the cached result: not a feature.
    let (report, _, _) = replay("CopyPasteBodies", "CutPasteBodies1", json!({}));
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("missing in the file")),
        "{}",
        report.text()
    );
}

/// A combine named only by the items that made its bodies (the decoder's
/// `_f3d.producer`, mitcad#96), whose tool the file consumed: the history
/// keeps the tool as it was, so it is left out from the combine's state
/// on, and the combine comes in with its tool consumed.
#[test]
fn a_combine_finds_its_bodies_by_their_producers_and_consumes_its_tool() {
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let s0 = shapes(&reference);
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let s1 = shapes(&reference);
    let tool = reference
        .body_shape("F3.b0".parse().unwrap())
        .unwrap()
        .clone();
    add(
        &mut reference,
        json!({"type": "combine", "target": "F2.b0", "tools": ["F3.b0"], "operation": "join"}),
    );
    let mut s2 = shapes(&reference);
    s2.push(tool);
    let produced = |producer: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": 0}})
    };
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
               "detail": {"targetBody": produced(1), "toolBodies": [produced(2)],
                          "operation": "JoinFeatureOperation", "isKeepToolBodies": false}}),
    ];
    let mut geometry = TestGeometry {
        states: vec![s0, s1, s2],
        items: HashMap::from([(1, 0), (2, 1), (3, 2)]),
        ids: true,
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    let combine = &report.items[3];
    assert_eq!(combine.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(combine.verified, Some(true));
    let def: Value = serde_json::from_str(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            combine.features[0]
        ))
        .unwrap(),
    )
    .unwrap();
    let tool = &report.items[2].features[0];
    assert_eq!(def["def"]["tools"], json!([format!("{tool}.b0")]));
    assert_ne!(def["def"]["keep_tools"], true);
    // The stored design without the consumed tool: nothing to replace.
    assert_eq!(doc.bodies().len(), 1);
    assert_eq!(report.bodies.len(), 1, "{}", report.text());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

/// A split whose history holds two operations (the first piece cut off,
/// then the rest split), only the second named by the item, comes in as two
/// base features (mitcad#181): the bodies the item made are the new bodies
/// of both, in the file's order, the bodies the file keeps keeping their
/// ids (`StoredBody::lineage`). A later input naming the split's first new
/// body (`body_index` 0) is the piece the first operation cut off, as it is
/// after the second.
#[test]
fn a_split_in_two_operations_made_the_new_bodies_of_both() {
    let whole = MockShape::imported("whole", 12);
    let kept = MockShape::imported("kept piece", 6);
    let rest = MockShape::imported("rest", 9);
    let first = MockShape::imported("first new piece", 7);
    let second = MockShape::imported("second new piece", 8);
    let items = vec![
        json!({"index": 0, "name": "Base Feature1", "objectType": "BaseFeature"}),
        json!({"index": 1, "name": "Split1", "objectType": "SplitBodyFeature", "detail": {}}),
    ];
    let mut geometry = TestGeometry {
        states: vec![
            vec![whole.clone()],
            vec![rest.clone(), kept.clone()],
            vec![first.clone(), kept.clone(), second.clone()],
        ],
        items: HashMap::from([(0, 0), (1, 2)]),
        starts: HashMap::from([(1, 0)]),
        ids: true,
        lineages: vec![vec![1], vec![2, 1], vec![2, 1, 3]],
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel::default());
    let dump = timeline_dump(items);
    let options = Options::default();
    let mut importer = Importer {
        body_scope: body_scope::Scope::capture(&doc).enter(&doc),
        oracle: Oracle::new(&mut geometry),
        doc: &mut doc,
        dump: &dump,
        options: &options,
        params: ParamMap::default(),
        sketches: HashMap::new(),
        features: HashMap::new(),
        report: DesignReport::default(),
        pending: Vec::new(),
        defined: Default::default(),
        at_final: false,
        started: std::time::Instant::now(),
        components: components::Components::default(),
        component: ComponentUid::ROOT,
        sketch_copies: HashMap::new(),
        stopped_at: Cell::new(None),
        trying: false,
        alternatives: HashMap::new(),
        guesses: None,
        defer_guesses: false,
        kernel_failed: false,
        start_bases: HashMap::new(),
        deadline: None,
        captures: captures::Captures::default(),
        memory_released: 0,
        frame_entities: Default::default(),
        workers: Default::default(),
        learn: None,
        dressups: HashMap::new(),
        notes: Default::default(),
        suppressed: Default::default(),
        made_ahead: HashMap::new(),
        exact_pairs: None,
        waited: Default::default(),
        deviations: Default::default(),
        deviated: Default::default(),
        deviated_log: Default::default(),
        deviation_points: Default::default(),
        sources: None,
        linker: None,
        focus_paths: HashMap::new(),
        link_paths: Default::default(),
    };
    importer.run();
    assert_eq!(
        importer.report.items[1].outcome,
        Outcome::Fallback,
        "{}",
        importer.report.text()
    );
    let (made, own) = importer.made_by(1);
    assert!(!own);
    let histories: Vec<String> = made
        .iter()
        .map(|b| importer.doc.body_shape(*b).unwrap().history.clone())
        .collect();
    assert_eq!(histories.len(), 2, "{histories:?}");
    assert!(histories[0].contains("first new piece"), "{histories:?}");
    assert!(histories[1].contains("second new piece"), "{histories:?}");
}

/// The bodies a cut with decoded participants is tried on (mitcad#181):
/// where the history's state changed only the bodies the file names, not
/// all bodies as well (on all bodies the cut gives those the same, and the
/// others must stay); where it changed others too, all bodies are tried.
#[test]
fn named_participants_alone_when_the_history_changed_only_them() {
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let s0 = shapes(&reference);
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let s1 = shapes(&reference);
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let named = mitcad_f3d::design::ir::Reference::from_map(
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": 1, "body_index": 0}})
        .as_object()
        .unwrap()
        .clone(),
    );
    let cut_first = MockShape::imported("the first block cut", 9);
    let cut_second = MockShape::imported("the second block cut", 9);
    for (after, alone) in [
        (vec![cut_first.clone(), s1[1].clone()], true),
        (vec![cut_first, cut_second], false),
    ] {
        let mut geometry = TestGeometry {
            // (The stored design is the bodies before the cut, so that the
            // end of the replay leaves them.)
            states: vec![s0.clone(), s1.clone(), after, s1.clone()],
            items: HashMap::from([(1, 0), (2, 1), (3, 2)]),
            ids: true,
            ..TestGeometry::default()
        };
        let mut doc = Document::new(TestKernel::default());
        let dump = timeline_dump(items.clone());
        let options = Options::default();
        let mut importer = Importer {
            body_scope: body_scope::Scope::capture(&doc).enter(&doc),
            oracle: Oracle::new(&mut geometry),
            doc: &mut doc,
            dump: &dump,
            options: &options,
            params: ParamMap::default(),
            sketches: HashMap::new(),
            features: HashMap::new(),
            report: DesignReport::default(),
            pending: Vec::new(),
            defined: Default::default(),
            at_final: false,
            started: std::time::Instant::now(),
            components: components::Components::default(),
            component: ComponentUid::ROOT,
            sketch_copies: HashMap::new(),
            stopped_at: Cell::new(None),
            trying: false,
            alternatives: HashMap::new(),
            guesses: None,
            defer_guesses: false,
            kernel_failed: false,
            start_bases: HashMap::new(),
            deadline: None,
            captures: captures::Captures::default(),
            memory_released: 0,
            frame_entities: Default::default(),
            workers: Default::default(),
            learn: None,
            dressups: HashMap::new(),
            notes: Default::default(),
            suppressed: Default::default(),
            made_ahead: HashMap::new(),
            exact_pairs: None,
            waited: Default::default(),
            deviations: Default::default(),
            deviated: Default::default(),
            deviated_log: Default::default(),
            deviation_points: Default::default(),
            sources: None,
            linker: None,
            focus_paths: HashMap::new(),
            link_paths: Default::default(),
        };
        importer.run();
        let first = importer.report.items[1].features[0].clone();
        let sketch = importer.sketches[&0].uid;
        let options = importer.participant_options(3, &Some(vec![named.clone()]), "cut", sketch);
        let decoded = vec![format!("{first}.b0")];
        if alone {
            assert_eq!(options, vec![decoded], "{}", importer.report.text());
        } else {
            assert_eq!(
                options,
                vec![decoded, Vec::new()],
                "{}",
                importer.report.text()
            );
        }
    }
}

#[test]
fn planar_faces_are_found_once_per_body_shape() {
    let mut doc = Document::new(MockKernel::default());
    import(&mut doc, &block_dump());
    let found = || crate::refs::planar_faces(&doc);
    let plain = found();
    assert!(!plain.is_empty());
    let kept = crate::refs::KeptPlanes::new();
    assert_eq!(found(), plain);
    assert_eq!(found(), plain);
    // Kept by version: the same version is not looked at again.
    let finds = std::cell::Cell::new(0);
    let find = || {
        finds.set(finds.get() + 1);
        Vec::new()
    };
    crate::refs::planes_of(1, find);
    crate::refs::planes_of(1, find);
    crate::refs::planes_of(2, find);
    assert_eq!(finds.get(), 2);
    drop(kept);
    // Not kept outside an import.
    crate::refs::planes_of(1, find);
    assert_eq!(finds.get(), 3);
}

/// A combine whose target is in one component and its tool in another
/// (the file's assembly context, mitcad#104): the tool comes in by its
/// occurrence link, and the combine goes where its target is.
#[test]
fn a_combine_takes_a_tool_of_another_component_by_its_link() {
    let produced = |producer: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": 0}})
    };
    let owned = |mut item: Value, owner: u64| {
        item["_f3d"]["component"] = json!(owner);
        item
    };
    let mut value: Value = serde_json::to_value(timeline_dump(vec![
        owned(rectangle_sketch(0), 3),
        owned(extrude(1, 0, "NewBodyFeatureOperation", "d2"), 3),
        owned(rectangle_sketch(2), 10),
        owned(extrude(3, 2, "NewBodyFeatureOperation", "d2"), 10),
        owned(
            json!({"index": 4, "name": "Combine1", "objectType": "CombineFeature",
                   "detail": {"targetBody": produced(3), "toolBodies": [produced(1)],
                              "operation": "CutFeatureOperation", "isKeepToolBodies": true}}),
            10,
        ),
    ]))
    .unwrap();
    value["document"] = json!({"root_component": "Assembly"});
    value["components"] = json!([
        {"name": "Assembly", "is_root": true, "_f3d": {"object_id": 3}},
        {"name": "Plate", "_f3d": {"object_id": 10}}]);
    value["occurrences"] = json!([
        {"component": "Plate", "_f3d": {"component_object": 10},
         "transform": [[1.0, 0.0, 0.0, 5.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0],
                       [0.0, 0.0, 0.0, 1.0]]}]);
    let dump = Dump::from_json(&value.to_string()).unwrap();
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let combine = &report.items[4];
    assert_eq!(combine.outcome, Outcome::Parametric, "{}", report.text());
    let uid: FeatureUid = combine.features[0].parse().unwrap();
    let plate = doc.assembly().components[0].uid;
    assert_eq!(doc.feature(uid).unwrap().component, plate);
    let def: Value = serde_json::from_str(
        &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
            .unwrap(),
    )
    .unwrap();
    let tool = format!("{}.b0", report.items[1].features[0]);
    assert_eq!(def["def"]["tools"], json!([tool]));
    assert_eq!(
        def["def"]["tool_links"],
        json!({tool.clone(): {"target": "O1"}})
    );
    // The root's block, moved into Plate (5 cm back along X), cut it.
    let target = format!("{}.b0", report.items[3].features[0]);
    let cut = &doc.body_shape(target.parse().unwrap()).unwrap().history;
    assert!(
        cut.starts_with("cut(") && cut.contains(",-50,0,0)"),
        "{cut}"
    );
}

/// A combine whose history state is a cut while its detail says join: the
/// history's guesses find the cut, unless the `.ipt` decoder read the
/// operation from the record (`_ipt_inputs`), whose operation is not
/// guessed (mitcad#60: a guessed cut of a coil's body took gigabytes).
#[test]
fn a_combine_operation_read_from_its_record_is_not_guessed() {
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        ]),
        &mut NoGeometry,
        &Options::default(),
    );
    let s0 = shapes(&reference);
    let small = "r{c13[c16,c14],c14[c13,c15],c15[c14,c16],c16[c15,c13]}";
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": small}],
               "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}),
    );
    let s1 = shapes(&reference);
    add(
        &mut reference,
        json!({"type": "combine", "target": "F2.b0", "tools": ["F3.b0"], "operation": "cut",
               "keep_tools": true}),
    );
    let s2 = shapes(&reference);
    let produced = |producer: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": 0}})
    };
    let outcome = |laid_out: bool| {
        let mut detail = json!({"targetBody": produced(1), "toolBodies": [produced(2)],
                                "operation": "JoinFeatureOperation", "isKeepToolBodies": true});
        if laid_out {
            detail["_ipt_inputs"] = json!(true);
        }
        let items = vec![
            two_rectangles(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
            extrude(2, 0, "NewBodyFeatureOperation", "d2"),
            json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
                   "detail": detail}),
        ];
        let mut geometry = TestGeometry {
            states: vec![s0.clone(), s1.clone(), s2.clone()],
            items: HashMap::from([(1, 0), (2, 1), (3, 2)]),
            ids: true,
            ..TestGeometry::default()
        };
        let mut doc = Document::new(TestKernel::default());
        let report = import_design(
            &mut doc,
            &timeline_dump(items),
            &mut geometry,
            &Options::default(),
        );
        (report.items[3].outcome, report.text())
    };
    let (guessed, text) = outcome(false);
    assert_eq!(guessed, Outcome::Parametric, "{text}");
    let (read, text) = outcome(true);
    assert_ne!(read, Outcome::Parametric, "{text}");
}

/// A join or intersection that consumes its tools gives the same solid
/// whichever of its bodies is the target (mitcad#181): such combines share
/// a key, those keeping their tools or cutting have none.
#[test]
fn combines_of_the_same_bodies_give_the_same_result() {
    let combine = |target: &str, tools: &[&str], operation: &str, keep: bool| {
        Candidate::new(json!({"type": "combine", "target": target, "tools": tools,
                              "operation": operation, "keep_tools": keep}))
    };
    let a = same_result_combine(&combine("F1.b0", &["F2.b0", "F3.b0"], "join", false));
    let b = same_result_combine(&combine("F2.b0", &["F1.b0", "F3.b0"], "join", false));
    assert!(a.is_some());
    assert_eq!(a, b);
    assert_ne!(
        a,
        same_result_combine(&combine("F1.b0", &["F2.b0"], "join", false))
    );
    assert!(same_result_combine(&combine("F1.b0", &["F2.b0"], "intersect", false)).is_some());
    assert_eq!(
        same_result_combine(&combine("F1.b0", &["F2.b0"], "join", true)),
        None
    );
    assert_eq!(
        same_result_combine(&combine("F1.b0", &["F2.b0"], "cut", false)),
        None
    );
}

/// A cut that divides its target in two: the target keeps the record of
/// the item that made it, so the cut's own records are its new pieces
/// only, and a removal naming the cut's first record takes the first new
/// piece, not the target's remaining one (the pieces come out in the
/// replay's order, which is not the file's).
#[test]
fn a_removal_by_a_cuts_record_takes_a_new_piece_not_the_target() {
    let produced = |producer: i64, index: i64| {
        json!({"kind": "body", "objectType": "BRepBody",
               "_f3d": {"producer": producer, "body_index": index}})
    };
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        extrude(2, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 3, "name": "Combine1", "objectType": "CombineFeature",
               "detail": {"targetBody": produced(1, 0), "toolBodies": [produced(2, 0)],
                          "operation": "CutFeatureOperation", "isKeepToolBodies": true}}),
        json!({"index": 4, "name": "RemoveBody-Body3", "objectType": "RemoveFeature",
               "detail": {"itemToRemove": produced(3, 0)}}),
    ];
    let mut doc = Document::new(TestKernel::default());
    doc.kernel().mock.split.set(Some(2));
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    let combine = &report.items[3];
    assert_eq!(combine.outcome, Outcome::Parametric, "{}", report.text());
    let remove = &report.items[4];
    assert_eq!(remove.outcome, Outcome::Parametric, "{}", report.text());
    let def: Value = serde_json::from_str(
        &doc.query(&json!({"query": "feature", "uid": remove.features[0]}).to_string())
            .unwrap(),
    )
    .unwrap();
    // The new piece of the cut (the cut's own body), not the target F2.b0.
    assert_eq!(
        def["def"]["bodies"],
        json!([format!("{}.b0", combine.features[0])]),
        "{}",
        report.text()
    );
}
