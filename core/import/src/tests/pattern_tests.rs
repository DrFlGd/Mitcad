// SPDX-License-Identifier: MIT
//! Rectangular, circular and path patterns of features and bodies.

use super::*;

#[test]
fn pattern_provenance_marks_only_omitted_dressup_inputs() {
    let mut doc = Document::new(MockKernel::default());
    let dump = timeline_dump(Vec::new());
    let options = Options::default();
    let mut geometry = NoGeometry;
    let mut oracle = Oracle::new(&mut geometry);
    oracle.enabled = false;
    // Candidate translation needs only the imported feature identities;
    // no geometry is evaluated or selected in this test.
    let mut importer = Importer {
        body_scope: body_scope::Scope::capture(&doc).enter(&doc),
        oracle,
        doc: &mut doc,
        dump: &dump,
        options: &options,
        params: ParamMap::default(),
        sketches: HashMap::new(),
        features: HashMap::from([
            (1, "F2".parse().unwrap()),
            (2, "F3".parse().unwrap()),
            (3, "F4".parse().unwrap()),
        ]),
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
        dressups: HashMap::from([
            (2, vec!["F3".parse().unwrap()]),
            (3, vec!["F4".parse().unwrap()]),
        ]),
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
    let feature = |index: i64, kind: &str| json!({"kind": "feature", "objectType": kind, "timeline_index": index});
    let plain = feature(1, "ExtrudeFeature");
    let fillet = feature(2, "FilletFeature");
    let chamfer = feature(3, "ChamferFeature");
    for kind in [
        "CircularPatternFeature",
        "RectangularPatternFeature",
        "MirrorFeature",
    ] {
        for (inputs, expected) in [
            (
                vec![plain.clone()],
                vec![(vec!["F2"], InputProvenance::Unknown)],
            ),
            (
                vec![plain.clone(), fillet.clone(), chamfer.clone()],
                vec![
                    (vec!["F2", "F3", "F4"], InputProvenance::Unknown),
                    (vec!["F2"], InputProvenance::Inferred),
                ],
            ),
            (
                vec![fillet.clone(), chamfer.clone()],
                vec![(vec!["F3", "F4"], InputProvenance::Unknown)],
            ),
        ] {
            let item = serde_json::from_value(json!({"index": 4, "objectType": kind,
            "detail": {
                "inputEntities": inputs,
                "axis": {"kind": "construction_axis", "origin": "Z"},
                "quantity": {"kind": "parameter", "value": 2.0, "unit": ""},
                "totalAngle": {"kind": "parameter", "value": 180.0, "unit": "deg"},
                "directionOne": [1.0, 0.0, 0.0],
                "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
                "distanceOne": {"kind": "parameter", "value": 1.0, "unit": "cm"},
                "mirrorPlane": {"kind": "construction_plane", "origin": "YZ"},
                "isCombine": false
            }}))
            .unwrap();
            let candidates = importer.translate_op(kind, &item).unwrap();
            let multiplier = if kind == "MirrorFeature" { 2 } else { 1 };
            assert_eq!(candidates.len(), expected.len() * multiplier, "{kind}");
            // Without history, the complete selection remains first. The
            // mirror's opposite-combine candidates keep their prior flags.
            for (candidate, (features, provenance)) in candidates.iter().zip(&expected) {
                assert_eq!(candidate.input_provenance, *provenance, "{kind}");
                assert_eq!(
                    candidate.defs[0]["objects"],
                    json!({"type": "features", "features": features}),
                    "{kind}"
                );
                assert!(!candidate.guess, "{kind}");
            }
            for candidate in &candidates[expected.len()..] {
                assert!(candidate.guess);
                assert_eq!(candidate.input_provenance, InputProvenance::Inferred);
            }
        }
    }
    // The settings the file stores (mitcad#186) make the complete selection
    // read, and tried before the history's suggestions; without them (or
    // with an omitted dressup) it stays unknown.
    for (kind, settings, key, expected) in [
        (
            "CircularPatternFeature",
            json!({"isSymmetric": true, "patternComputeOption": "OptimizedPatternCompute",
                   "suppressedElementsIds": [2]}),
            "compute",
            "optimized",
        ),
        (
            "RectangularPatternFeature",
            json!({"isSymmetricInDirectionOne": true,
                   "patternDistanceType": "ExtentPatternDistanceType",
                   "patternComputeOption": "IdenticalPatternCompute",
                   "suppressedElementsIds": []}),
            "distance_type",
            "extent",
        ),
        (
            "MirrorFeature",
            json!({"patternComputeOption": "IdenticalPatternCompute"}),
            "compute",
            "identical",
        ),
    ] {
        for with in [true, false] {
            let mut detail = json!({
                "inputEntities": [plain.clone()],
                "axis": {"kind": "construction_axis", "origin": "Z"},
                "quantity": {"kind": "parameter", "value": 2.0, "unit": ""},
                "totalAngle": {"kind": "parameter", "value": 180.0, "unit": "deg"},
                "directionOne": [1.0, 0.0, 0.0],
                "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
                "distanceOne": {"kind": "parameter", "value": 1.0, "unit": "cm"},
                "mirrorPlane": {"kind": "construction_plane", "origin": "YZ"},
                "isCombine": false
            });
            if with {
                for (k, v) in settings.as_object().unwrap() {
                    detail[k] = v.clone();
                }
            }
            let item =
                serde_json::from_value(json!({"index": 4, "objectType": kind, "detail": detail}))
                    .unwrap();
            let candidates = importer.translate_op(kind, &item).unwrap();
            let c = &candidates[0];
            assert_eq!(c.first, with, "{kind} {with}");
            assert_eq!(
                c.input_provenance,
                if with {
                    InputProvenance::Read
                } else {
                    InputProvenance::Unknown
                },
                "{kind} {with}"
            );
            if with {
                assert_eq!(c.defs[0][key], expected, "{kind}");
            }
        }
    }
    // A direction named by an axis that is not an origin axis is tried by
    // its stored vector as well, after the axis (the replay's axis need not
    // point where the file's does); an origin axis points where its vector
    // does, so only one definition is made.
    for (axis, count) in [
        (json!({"kind": "construction_axis", "timeline_index": 2}), 2),
        (json!({"kind": "construction_axis", "origin": "X"}), 1),
    ] {
        let item = serde_json::from_value(
            json!({"index": 4, "objectType": "RectangularPatternFeature",
            "detail": {"inputEntities": [plain.clone()],
                       "directionOneEntity": axis, "directionOne": [0.0, -1.0, 0.0],
                       "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
                       "distanceOne": {"kind": "parameter", "value": -1.0, "unit": "cm"}}}),
        )
        .unwrap();
        let candidates = importer
            .translate_op("RectangularPatternFeature", &item)
            .unwrap();
        assert_eq!(candidates.len(), count, "{axis}");
        if count == 2 {
            assert_eq!(candidates[0].defs[0]["direction1"]["axis"], "F3");
            assert_eq!(
                candidates[1].defs[0]["direction1"]["axis"],
                json!({"origin": [0.0, 0.0, 0.0], "direction": [0.0, -1.0, 0.0]})
            );
            assert_eq!(
                candidates[1].defs[0]["direction1"]["distance"],
                candidates[0].defs[0]["direction1"]["distance"]
            );
        }
    }
}

#[test]
fn a_rectangular_pattern_uses_named_bodies_and_decoded_directions_without_history() {
    for sketch_axes in [false, true] {
        let mut detail = json!({
            "inputEntities": [{"kind": "body", "objectType": "BRepBody",
                               "_f3d": {"producer": 1, "body_index": 0}}],
            "quantityOne": {"kind": "parameter", "value": 2.0, "unit": ""},
            "quantityTwo": {"kind": "parameter", "value": 3.0, "unit": ""},
            "distanceOne": {"kind": "parameter", "value": -5.0},
            "distanceTwo": {"kind": "parameter", "value": 4.0},
            "directionOne": [2.0, 0.0, 0.0],
            "directionTwo": [0.0, 7.0, 0.0]
        });
        if sketch_axes {
            detail.as_object_mut().unwrap().remove("directionOne");
            detail.as_object_mut().unwrap().remove("directionTwo");
            for (key, curve) in [("directionOneEntity", "c0"), ("directionTwoEntity", "c1")] {
                detail[key] = json!({"kind": "sketch_entity", "objectType": "SketchLine",
                    "sketch_timeline_index": 0, "id": curve});
            }
        }
        let dump = timeline_dump(vec![
            rectangle_sketch(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
            json!({"index": 2, "name": "Pattern", "objectType": "RectangularPatternFeature",
                   "detail": detail}),
        ]);
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        let item = &report.items[2];
        assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
        assert_eq!(doc.bodies().len(), 6);
        let value: Value = serde_json::from_str(
            &doc.query(&format!(
                r#"{{"query": "feature", "uid": "{}"}}"#,
                item.features[0]
            ))
            .unwrap(),
        )
        .unwrap();
        let def = &value["def"];
        assert_eq!(
            def["objects"],
            json!({"type": "bodies", "bodies": ["F2.b0"]})
        );
        let parameters = doc.parameters();
        let distance = |direction: &str| {
            parameters.value(
                parameters
                    .find(def[direction]["distance"].as_str().unwrap())
                    .unwrap(),
            )
        };
        assert_eq!(distance("direction1"), Some(-50.0));
        assert_eq!(distance("direction2"), Some(40.0));
        if sketch_axes {
            assert_eq!(def["direction1"]["axis"]["sketch"], "F1");
            assert_eq!(def["direction2"]["axis"]["sketch"], "F1");
            assert_ne!(
                def["direction1"]["axis"]["curve"],
                def["direction2"]["axis"]["curve"]
            );
        } else {
            assert_eq!(
                def["direction1"]["axis"]["direction"],
                json!([1.0, 0.0, 0.0])
            );
            assert_eq!(
                def["direction2"]["axis"]["direction"],
                json!([0.0, 1.0, 0.0])
            );
        }
    }
}

#[test]
fn a_circular_pattern_uses_its_decoded_sketch_axis_without_history() {
    let dump = timeline_dump(vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 2, "name": "Pattern", "objectType": "CircularPatternFeature",
        "detail": {
            "inputEntities": [{"kind": "body", "objectType": "BRepBody",
                               "_f3d": {"producer": 1, "body_index": 0}}],
            "axis": {"kind": "sketch_entity", "objectType": "SketchLine",
                     "sketch_timeline_index": 0, "id": "c0"},
            "quantity": {"kind": "parameter", "value": 3.0, "unit": ""},
            "totalAngle": {"kind": "parameter", "value": std::f64::consts::PI, "unit": "rad"}
        }}),
    ]);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let item = &report.items[2];
    assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(doc.bodies().len(), 3);
    let value: Value = serde_json::from_str(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            item.features[0]
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(value["def"]["axis"]["sketch"], "F1");
    assert_eq!(
        value["def"]["objects"],
        json!({"type": "bodies", "bodies": ["F2.b0"]})
    );
}

/// A path pattern of a body along a sketch line (mitcad#468), as the stream
/// decoder gives it (`settings` false: without the start point, flip,
/// orientation and symmetry) or as a dump does.
fn path_pattern_item(path: Value, settings: bool) -> Value {
    let mut detail = json!({
        "inputEntities": [{"kind": "body", "objectType": "BRepBody",
                           "_f3d": {"producer": 1, "body_index": 0}}],
        "patternEntityType": "BodiesPatternType",
        "path": path,
        "quantity": {"kind": "parameter", "value": 3.0, "unit": ""},
        "distance": {"kind": "parameter", "value": 1.5, "unit": "mm"},
        "patternDistanceType": "SpacingPatternDistanceType",
        "patternComputeOption": "AdjustPatternCompute",
        "suppressedElementsIds": []
    });
    if settings {
        for (k, v) in [
            ("startPoint", json!(0.0)),
            ("isFlipDirection", json!(true)),
            ("isOrientationAlongPath", json!(false)),
            ("isSymmetric", json!(false)),
        ] {
            detail[k] = v;
        }
    }
    json!({"index": 2, "name": "PathPattern", "objectType": "PathPatternFeature",
           "detail": detail})
}

#[test]
fn a_path_pattern_follows_its_sketch_line() {
    let line = json!([{"_type": "PathEntity", "entity": {"kind": "sketch_entity",
        "objectType": "SketchLine", "sketch_timeline_index": 0, "id": "c0"}}]);
    for settings in [false, true] {
        let dump = timeline_dump(vec![
            rectangle_sketch(0),
            extrude(1, 0, "NewBodyFeatureOperation", "d2"),
            path_pattern_item(line.clone(), settings),
        ]);
        let mut doc = Document::new(MockKernel::default());
        let report = import(&mut doc, &dump);
        let item = &report.items[2];
        assert_eq!(item.outcome, Outcome::Parametric, "{}", report.text());
        assert_eq!(doc.bodies().len(), 3);
        let value: Value = serde_json::from_str(
            &doc.query(&format!(
                r#"{{"query": "feature", "uid": "{}"}}"#,
                item.features[0]
            ))
            .unwrap(),
        )
        .unwrap();
        let def = &value["def"];
        assert_eq!(def["type"], "path_pattern");
        // One curve of the sketch (its id renumbered, points first).
        assert_eq!(def["path"]["sketch"], "F1");
        assert!(def["path"]["curve"].is_string(), "{def}");
        assert_eq!(
            def["objects"],
            json!({"type": "bodies", "bodies": ["F2.b0"]})
        );
        assert_eq!(def["distance_type"], "spacing");
        // Given, the settings are read; else the defaults, inferred.
        assert_eq!(def["flip"].as_bool().unwrap_or(false), settings);
        let provenance = if settings {
            InputProvenance::Read
        } else {
            InputProvenance::Inferred
        };
        assert_eq!(item.input_provenance, Some(provenance), "{}", report.text());
    }
}

#[test]
fn a_path_pattern_along_edges_is_not_translated() {
    let edge = json!([{"_type": "PathEntity", "entity": {"kind": "edge",
        "objectType": "BRepEdge"}}]);
    let dump = timeline_dump(vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        path_pattern_item(edge, true),
    ]);
    let mut doc = Document::new(MockKernel::default());
    let report = import(&mut doc, &dump);
    let item = &report.items[2];
    assert_ne!(item.outcome, Outcome::Parametric, "{}", report.text());
}

#[test]
fn route_b_patterns_and_holes_fall_back() {
    // The stream decoder gives a pattern's angle only: no inputs, no axis.
    let items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
        json!({"index": 2, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
               "detail": {"totalAngle": param("d7", "360 deg", std::f64::consts::TAU, "deg")}}),
    ];
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Skipped,
        "{}",
        report.text()
    );
    assert!(
        report.items[2]
            .note
            .as_deref()
            .unwrap()
            .contains("no inputs"),
        "{}",
        report.text()
    );
}

#[test]
fn a_pattern_gets_the_extrusion_profiles_it_copies() {
    for learning in [false, true] {
        check_pattern_reselects_decoded_profiles(learning);
    }
}

fn check_pattern_reselects_decoded_profiles(learning: bool) {
    // Both rectangles extruded give the same body as far as the history
    // tells (the test kernel measures every body alike), but only the
    // file's region gives the pattern's copies: the extrusion takes the
    // first, and the pattern after it changes it to the file's.
    let mut items = decoded_small_rectangle();
    // The decoded loops select the large rectangle first. Without a
    // learning dump, deferring every other set used to discard the small
    // rectangle needed by the later pattern (mitcad#181).
    for (k, curve) in items[1]["detail"]["_f3d_profile_loops"][0]["loops"][0]["curves"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        curve["id"] = json!(format!("c{k}"));
        curve["primary"] = json!(k);
    }
    items.push(
        json!({"index": 2, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
                      "detail": {"quantity": param("d6", "2", 2.0, "")}}),
    );
    let file = reference(vec![
        extrusion_of(SMALL),
        json!({"type": "circular_pattern", "objects": {"type": "features", "features": ["F2"]},
               "axis": "z", "quantity": 2, "angle": std::f64::consts::TAU, "symmetric": false,
               "compute": "adjust"}),
    ]);
    let extruded = file.bodies()[..1].iter().map(|b| b.shape.clone()).collect();
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), extruded, shapes(&file)],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel {
        alike: true,
        ..TestKernel::default()
    });
    let dir = std::env::temp_dir().join(format!("mitcad-learn-revised-{}", std::process::id()));
    let options = Options {
        learn: learning.then(|| crate::learn::LearnOptions {
            dir: dir.clone(),
            path: None,
        }),
        ..Options::default()
    };
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, &options);
    let settled = learning.then(|| {
        let settled = std::fs::read_dir(dir.join("settled"))
            .unwrap()
            .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
            .collect::<String>();
        let _ = std::fs::remove_dir_all(&dir);
        settled
    });
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Parametric,
            Outcome::Parametric,
            Outcome::Parametric
        ],
        "{}",
        report.text()
    );
    assert_eq!(report.items[2].verified, Some(true));
    assert_eq!(
        report.items[1].input_provenance,
        Some(InputProvenance::Inferred)
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F2"}"#).unwrap()).unwrap();
    assert_eq!(def["def"]["profiles"][0]["region"], SMALL);
    // (The large rectangle was the first guess.)
    assert!(
        report.items[1]
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("chosen for C-Pattern1"),
        "{}",
        report.text()
    );
    // The learning answer follows the final extrusion, not the initially
    // accepted region that the pattern made it replace.
    let Some(settled) = settled else { return };
    let lines: Vec<Value> = settled
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let line = lines.iter().find(|line| line["item"] == 1).unwrap();
    assert_eq!(line["answer"]["input_provenance"], "inferred");
    assert_eq!(line["status"], "settled");
    let profile = &line["answer"]["defs"][0]["profiles"][0];
    assert_eq!(profile["sketch"], "T0");
    let region = profile["region"].as_u64().unwrap() as usize;
    let mut outer: Vec<&str> = line["context"]["sketches"]["T0"]["regions"][region]["outer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|curve| curve.as_str().unwrap())
        .collect();
    outer.sort_unstable();
    assert_eq!(outer, ["c4", "c5", "c6", "c7"]);
}

#[test]
fn a_rectangular_pattern_without_directions_finds_a_hexagonal_lattice() {
    // The oldest item version stores no directions (mitcad#74): the file's
    // pattern repeats the extrusion along x and at 60° to it, on both sides.
    let lattice = json!({"type": "rectangular_pattern",
        "objects": {"type": "features", "features": ["F2"]},
        "direction1": {"axis": "x", "quantity": 2, "distance": 30, "symmetric": true},
        "direction2": {"axis": {"origin": [0.0, 0.0, 0.0],
                                "direction": [0.5, 3f64.sqrt() / 2.0, 0.0]},
                       "quantity": 2, "distance": 30, "symmetric": true},
        "distance_type": "spacing", "original_bodies": true});
    let file = reference(vec![extrusion_of(SMALL), lattice]);
    let extruded = file.bodies()[..1].iter().map(|b| b.shape.clone()).collect();
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), extruded, shapes(&file)],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    let distance = param("d9", "30 mm", 3.0, "mm");
    let items = vec![
        two_rectangles(0),
        json!({"index": 1, "name": "Extrude1", "objectType": "ExtrudeFeature",
               "detail": {"operation": "NewBodyFeatureOperation",
                          "profile": [{"kind": "profile", "sketch": "Sketch1",
                                       "sketch_timeline_index": 0}],
                          "extentType": "OneSideFeatureExtentType",
                          "extentOne": {"_type": "DistanceExtentDefinition",
                                        "distance": param("d2", "10 mm", 1.0, "mm")}}}),
        json!({"index": 2, "name": "R-Pattern1", "objectType": "RectangularPatternFeature",
               "detail": {"inputEntities": [{"kind": "feature", "objectType": "ExtrudeFeature",
                                             "name": "Extrude1", "timeline_index": 1}],
                          "patternEntityType": "FeaturesPatternType",
                          "quantityOne": param("d7", "2", 2.0, ""),
                          "quantityTwo": param("d8", "2", 2.0, ""),
                          "distanceOne": distance, "distanceTwo": distance}}),
    ];
    let mut doc = Document::new(TestKernel::default());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str(&doc.query(r#"{"query": "feature", "uid": "F3"}"#).unwrap()).unwrap();
    let def = &def["def"];
    assert_eq!(def["direction1"]["symmetric"], json!(true), "{def}");
    let across = def["direction2"]["axis"]["direction"][1]
        .as_f64()
        .unwrap_or_default();
    assert!((across - 3f64.sqrt() / 2.0).abs() < 1e-9, "{def}");
    assert_eq!(def["original_bodies"], json!(true), "{def}");
}

#[test]
fn patterns_of_features_with_a_chamfer_repeat_it_on_the_copies() {
    // The block with a chamfer on its first edge, and a circular pattern
    // of the extrusion and the chamfer (mitcad#105).
    let block_items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel::default());
    import_design(
        &mut reference,
        &timeline_dump(block_items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (a, b) = shapes(&reference)[0].edges[0].clone();
    let p = edge_point(&a, &b);
    let edge = json!({"kind": "edge", "mid_point": [p[0] / 10.0, p[1] / 10.0, p[2] / 10.0],
                      "length": 0.1});
    let mut items = block_items;
    items.push(
        json!({"index": 2, "name": "Chamfer1", "objectType": "ChamferFeature",
        "detail": {"edgeSets": [{"edges": [edge], "distance": param("d5", "2 mm", 0.2, "mm")}]}}),
    );
    items.push(
        json!({"index": 3, "name": "C-Pattern1", "objectType": "CircularPatternFeature",
        "detail": {"inputEntities": [
                       {"kind": "feature", "objectType": "ExtrudeFeature", "name": "Extrude1",
                        "timeline_index": 1},
                       {"kind": "feature", "objectType": "ChamferFeature", "name": "Chamfer1",
                        "timeline_index": 2}],
                   "patternEntityType": "FeaturesPatternType",
                   "axis": {"kind": "construction_axis", "name": "Z", "origin": "Z"},
                   "quantity": param("d6", "2", 2.0, ""),
                   "totalAngle": param("d7", "360 deg", std::f64::consts::TAU, "deg"),
                   "isSymmetric": false}}),
    );
    let mut dump = timeline_dump(items);
    for (name, expression, unit) in [("d6", "2", ""), ("d7", "360 deg", "deg")] {
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
    let mut doc = Document::new(TestKernel {
        edge_points: true,
        ..TestKernel::default()
    });
    let report = import_design(&mut doc, &dump, &mut NoGeometry, &Options::default());
    let pattern = &report.items[3];
    assert_eq!(pattern.outcome, Outcome::Parametric, "{}", report.text());
    assert_eq!(
        pattern.note.as_deref(),
        Some("its fillets and chamfers repeated on the copies")
    );
    let chamfer = &report.items[2].features[0];
    let def: Value = serde_json::from_str::<Value>(
        &doc.query(&format!(
            r#"{{"query": "feature", "uid": "{}"}}"#,
            pattern.features[0]
        ))
        .unwrap(),
    )
    .unwrap()["def"]
        .clone();
    // The chamfer among the objects (the first definition tried).
    assert_eq!(
        def["objects"],
        json!({"type": "features", "features": ["F2", chamfer]})
    );
    // The copy of the block got the chamfer's copy, named after the pattern.
    let prefix = format!("{}:chamfer(", pattern.features[0]);
    assert!(
        shapes(&doc)
            .iter()
            .any(|s| s.faces.iter().any(|f| f.to_string().starts_with(&prefix))),
        "{:?}",
        shapes(&doc)
    );
}

#[test]
fn patterns_with_read_inputs_or_a_missing_feature_are_not_searched() {
    let read = |provenance, guess| {
        let mut c = Candidate::new(json!({"type": "rectangular_pattern"}));
        c.input_provenance = provenance;
        c.guess = guess;
        c
    };
    let not_searched = ops::pattern_not_searched;
    // The decoded definitions have read inputs: no history suggestions.
    for kind in ["CircularPatternFeature", "RectangularPatternFeature"] {
        let decoded = Ok(vec![
            read(InputProvenance::Read, false),
            read(InputProvenance::Unknown, false),
        ]);
        assert!(not_searched(kind, &decoded), "{kind}");
        // Inputs not read, or only guessed ones: the suggestions come too.
        let unknown = Ok(vec![read(InputProvenance::Unknown, false)]);
        assert!(!not_searched(kind, &unknown), "{kind}");
        let guessed = Ok(vec![read(InputProvenance::Read, true)]);
        assert!(!not_searched(kind, &guessed), "{kind}");
        // The copied feature did not come in: the copies of another one
        // are not the file's.
        let missing = Err(ops::PATTERNED_NOT_IMPORTED.to_owned());
        assert!(not_searched(kind, &missing), "{kind}");
        let other = Err("direction one was not decoded".to_owned());
        assert!(!not_searched(kind, &other), "{kind}");
    }
    // Mirrors and other items keep their suggestions.
    let decoded = Ok(vec![read(InputProvenance::Read, false)]);
    assert!(!not_searched("MirrorFeature", &decoded));
}
