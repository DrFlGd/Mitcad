// SPDX-License-Identifier: MIT
//! Threads and tapped holes: the faces the decoder found, handedness, diameters and groove middles.

use super::*;

/// Threads (mitcad#35) on the faces the decoder found in the history: the
/// size as Mitcad lists it, a partial thread from the end the file's axis
/// points to (here against the mock cylinder's), a class the face's side
/// does not take left out, pipe threads by Mitcad's names.
#[test]
fn threads_come_in_on_the_faces_the_decoder_found() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut base = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(base.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let point = face_point(&side).map(|x| x / 10.0);
    let iso = |class: &str| {
        json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
               "threadDesignation": "M10x1.5", "threadClass": class, "isInternal": false})
    };
    let pipe = json!({"_type": "ThreadInfo", "threadType": "BSP Pipe Threads",
                      "threadDesignation": "G 1/4-19", "threadClass": "A"});
    base.push(thread_item(2, iso("6g"), false, -1.0, point));
    base.push(thread_item(3, iso("6H"), true, 1.0, point));
    base.push(thread_item(4, pipe, true, 1.0, point));
    // The forming screw threads, with and without the `ST` of the size.
    let forming = |designation: &str| {
        json!({"_type": "ThreadInfo", "threadType": "Metric Forming Screw Threads",
               "threadDesignation": designation, "threadClass": "6g", "isInternal": false})
    };
    base.push(thread_item(5, forming("ST3.5x1.3"), true, 1.0, point));
    base.push(thread_item(6, forming("2.9x1.4"), true, 1.0, point));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(base),
        &mut NoGeometry,
        &Options::default(),
    );
    let outcomes: Vec<_> = report.items.iter().map(|i| i.outcome).collect();
    assert_eq!(outcomes[2..], [Outcome::Parametric; 5], "{}", report.text());
    let def = |uid: u64| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "F{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    let partial = def(3);
    assert_eq!(partial["faces"][0]["face"], side.to_string());
    assert_eq!(
        partial["thread"],
        json!({"designation": "M10x1.5", "class": "6g"})
    );
    // The length a parameter of the thread's own, the offset the file's d5.
    assert!(partial["length"].is_string(), "{partial}");
    assert_eq!(partial["offset"], "d5");
    // The file's axis runs down, the mock cylinder's up: the high end in
    // the file is Mitcad's low end.
    assert_eq!(partial["location"], "low_end");
    let full = def(4);
    assert_eq!(full["thread"], json!({"designation": "M10x1.5"}));
    assert!(full.get("length").is_none());
    assert!(
        report.items[3]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("class")),
        "{}",
        report.text()
    );
    assert_eq!(
        def(5)["thread"],
        json!({"standard": "whitworth", "designation": "G 1/4", "class": "A"})
    );
    assert_eq!(
        def(6)["thread"],
        json!({"standard": "forming_screw", "designation": "ST3.5x1.3", "class": "6g"})
    );
    assert_eq!(
        def(7)["thread"],
        json!({"standard": "forming_screw", "designation": "2.9x1.4", "class": "6g"})
    );
}

/// A thread's handedness read from the file comes in as the
/// thread's, and a thread of which every input is read says so: here a
/// left-handed one; one whose handedness the file does not give, or whose
/// class Mitcad does not list for the face's side, does not.
#[test]
fn a_threads_handedness_comes_in_and_its_inputs_are_read() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut base = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(base.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let point = face_point(&side).map(|x| x / 10.0);
    let iso = |class: &str, right: Option<bool>| {
        let mut info = json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
                              "threadDesignation": "M10x1.5", "threadClass": class,
                              "isInternal": false});
        if let Some(right) = right {
            info["isRightHanded"] = json!(right);
        }
        info
    };
    base.push(thread_item(2, iso("6g", Some(false)), true, 1.0, point));
    base.push(thread_item(3, iso("6g", None), true, 1.0, point));
    base.push(thread_item(4, iso("6H", Some(true)), true, 1.0, point));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(base),
        &mut NoGeometry,
        &Options::default(),
    );
    let text = report.text();
    assert_eq!(
        report.items[2..]
            .iter()
            .map(|i| i.outcome)
            .collect::<Vec<_>>(),
        [Outcome::Parametric; 3],
        "{text}"
    );
    let def = |uid: u64| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "F{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    assert_eq!(def(3)["thread"]["right_handed"], json!(false), "{}", def(3));
    assert_eq!(def(4)["thread"].get("right_handed"), None, "{}", def(4));
    let provenance: Vec<_> = report.items[2..]
        .iter()
        .map(|i| i.input_provenance)
        .collect();
    assert_eq!(
        provenance,
        [
            Some(InputProvenance::Read),
            Some(InputProvenance::Unknown),
            // (6H is no class of an external thread: left out.)
            Some(InputProvenance::Unknown)
        ],
        "{text}"
    );
}

/// A modelled thread (mitcad#57) comes in with the file's diameters: the
/// thread alone first (it sizes its part of the face itself). Mitcad
/// models only the 60-degree threads: a modelled pipe thread falls back.
#[test]
fn modelled_threads_take_the_files_diameters() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut items = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let point = face_point(&side).map(|x| x / 10.0);
    let info = json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
        "threadDesignation": "M10x1.5", "threadClass": "6g", "isInternal": false,
        "majorDiameter": 0.985, "minorDiameter": 0.8141, "pitchDiameter": 0.8928});
    // A modelled pipe thread on the same face, in a design of its own: the
    // modelled metric thread's crest lies inside the face and leaves no
    // piece of it (mitcad#433).
    let pipe = json!({"_type": "ThreadInfo", "threadType": "BSP Pipe Threads",
                      "threadDesignation": "G 1/4-19", "threadClass": "A"});
    let mut modelled_pipe = thread_item(2, pipe, true, 1.0, point);
    modelled_pipe["detail"]["isModeled"] = json!(true);
    let mut pipe_items = items.clone();
    pipe_items.push(modelled_pipe);
    let mut modelled = thread_item(2, info, true, 1.0, point);
    modelled["detail"]["isModeled"] = json!(true);
    items.push(modelled);
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def: Value =
        serde_json::from_str::<Value>(&doc.query(r#"{"query": "feature", "uid": "F3"}"#).unwrap())
            .unwrap()["def"]
            .clone();
    assert_eq!(def["type"], "thread", "{def}");
    assert_eq!(def["modeled"], true, "{def}");
    let spec = doc.kernel().mock.specs.borrow().join("\n");
    assert!(
        spec.contains(&format!(
            "thread {side} pitch 1.5 diameters 9.8500/8.1410/8.9280"
        )),
        "{spec}"
    );
    // Without a history the angle is not measured.
    assert!(def.get("angle").is_none(), "{def}");
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(pipe_items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_ne!(report.items[2].outcome, Outcome::Parametric);
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("Whitworth")),
        "{}",
        report.text()
    );
}

/// A thread sizes its face to the thread's major diameter first, as the
/// file does (mitcad#35); the thread keeps the item's name.
#[test]
fn a_thread_sizes_its_face_first() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut items = vec![
        circle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let side = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "side")
        .cloned()
        .expect("a side face");
    let info = json!({"_type": "ThreadInfo", "threadType": "ISO Metric profile",
        "threadDesignation": "M10x1.5", "threadClass": "6g", "isInternal": false,
        "majorDiameter": 0.985, "minorDiameter": 0.8141});
    items.push(thread_item(
        2,
        info,
        true,
        1.0,
        face_point(&side).map(|x| x / 10.0),
    ));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("sized")),
        "{}",
        report.text()
    );
    let offset = doc.feature(FeatureUid(3)).unwrap();
    assert!(
        matches!(offset.def, FeatureDef::OffsetFace(_)),
        "{offset:?}"
    );
    let thread = doc.feature(FeatureUid(4)).unwrap();
    assert!(matches!(thread.def, FeatureDef::Thread(_)));
    assert_eq!(thread.name, "Thread2");
}

/// A tapped hole (mitcad#35): the hole, and a thread on its wall from the
/// hole's start.
#[test]
fn a_tapped_hole_comes_in_with_its_thread_on_the_wall() {
    let kernel = || TestKernel {
        face_listing: true,
        ..TestKernel::default()
    };
    let mut items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(kernel());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let top = reference.bodies()[0]
        .shape
        .faces
        .iter()
        .find(|f| f.role == "end")
        .cloned()
        .expect("a top face");
    items.push(json!({
        "index": 2, "name": "Hole1", "objectType": "HoleFeature",
        "detail": {
            "holeType": "SimpleHoleType", "holeTapType": "TappedHoleTapType",
            "holeDiameter": param("d6", "5 mm", 0.5, "mm"),
            "tipAngle": param("d7", "118 deg", 2.0594885173533086, "deg"),
            "extentDefinition": {"_type": "DistanceExtentDefinition",
                                 "distance": param("d8", "15 mm", 1.5, "mm")},
            "position": [2.0, 1.0, 1.0],
            "holePositionDefinition": {"planarEntity": {
                "kind": "face", "geometry": {"type": "Plane"},
                "point_on_face": face_point(&top).map(|x| x / 10.0)}},
            "tappedHoleInfo": {"_type": "ThreadInfo", "threadType": "ISO Metric profile",
                               "threadDesignation": "M6x1", "threadClass": "6H",
                               "isInternal": true, "minorDiameter": 0.5035},
            "thread": {"_type": "ThreadFeature", "isModeled": false, "isFullLength": false,
                       "threadLength": param("d9", "10 mm", 1.0, "mm"),
                       "threadOffset": param("d10", "0 mm", 0.0, "mm")}
        }
    }));
    let mut doc = Document::new(kernel());
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut NoGeometry,
        &Options::default(),
    );
    assert_eq!(
        report.items[2].outcome,
        Outcome::Parametric,
        "{}",
        report.text()
    );
    let def = |uid: &str| -> Value {
        serde_json::from_str::<Value>(
            &doc.query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap(),
        )
        .unwrap()["def"]
            .clone()
    };
    let hole = def("F3");
    assert_eq!(hole["type"], "hole");
    assert!(hole.get("thread").is_none(), "{hole}");
    let thread = def("F4");
    assert_eq!(thread["faces"][0]["face"], "F3:hole0.wall", "{thread}");
    assert_eq!(
        thread["thread"],
        json!({"designation": "M6x1", "class": "6H"})
    );
    assert_eq!(thread["location"], "low_end");
    // The bore is the hole's diameter (its own state's); the minor
    // diameter is the next guess.
    let params = doc.parameters();
    let bore = hole["diameter"].as_str().and_then(|n| params.find(n));
    assert_eq!(
        bore.and_then(|id| params.get(id)).map(|p| p.value()),
        Some(5.0),
        "{hole}"
    );
}

/// A modelled thread's groove found where the faces cross a segment one
/// pitch long at the pitch radius (mitcad#161): its middle, also where the
/// groove runs across the segment's ends; none for two grooves.
#[test]
fn a_thread_groove_middle_comes_from_the_crossings() {
    use crate::ops::groove_middle;
    // Out at 0.2, in at 0.7: the groove's middle is at 0.45.
    let middle = groove_middle(&[(0.2, false), (0.7, true)]).unwrap();
    assert!((middle - 0.45).abs() < 1e-12, "{middle}");
    // In at 0.3, out at 0.8: the groove runs from 0.8 round to 0.3, its
    // middle at 0.05.
    let middle = groove_middle(&[(0.3, true), (0.8, false)]).unwrap();
    assert!((middle - 0.05).abs() < 1e-12, "{middle}");
    assert_eq!(
        groove_middle(&[(0.1, false), (0.2, true), (0.5, false), (0.6, true)]),
        None
    );
}
