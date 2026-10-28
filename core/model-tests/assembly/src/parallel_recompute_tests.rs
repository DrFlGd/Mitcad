// SPDX-License-Identifier: MIT
//! Features of independent components evaluated in parallel (mitcad#842)
//! with the mock kernel: two components' features run at the same time on
//! the recompute's workers, a feature that reads another component
//! (linked geometry), a driven dimension's reference or a moved occurrence
//! waits for what it reads, a stop keeps nothing the workers returned, and
//! every recompute gives the document, the cache and the progress of one
//! thread.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use mitcad_model::features::SketchPlane;
use mitcad_model::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use mitcad_model::test_support::{allow_attachments, def, num};
use mitcad_model::testing::{MockKernel, WorkerLog};
use mitcad_model::transform::Transform;
use mitcad_model::{Document, ModelError, RecomputeMonitor};
use mitcad_model_api::DocumentApi;

/// The name of the recompute's worker threads.
const WORKER: &str = "mitcad-recompute-worker";

/// How long an extrusion waits for the others it should meet: they meet
/// at once when they run at the same time.
const PATIENCE: Duration = Duration::from_secs(5);

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

fn shift(x: f64, y: f64, z: f64) -> Transform {
    Transform::translation([x, y, z])
}

/// A 10 x 10 block in the active component as tall as the parameter
/// `height`: its extrude and body.
fn block(doc: &mut Document<MockKernel>, height: &str) -> (FeatureUid, BodyUid) {
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(10.0), &num(10.0))
        .unwrap()
        .region;
    let extrude = doc
        .add_feature(
            &def(json!({"type": "extrude",
                        "profiles": [{"sketch": sketch, "region": region}],
                        "extent": {"type": "distance", "distance": height},
                        "operation": "new_body"})),
            None,
        )
        .unwrap()
        .uid;
    (extrude, BodyUid::new(extrude, 0))
}

struct Blocks {
    a: ComponentUid,
    a_extrude: FeatureUid,
    a_body: BodyUid,
    b_occurrence: OccurrenceUid,
    b_extrude: FeatureUid,
    b_body: BodyUid,
}

/// Root with A:1 (O1) at (100, 0, 10) and B:1 (O2) at (0, 50, -3), each
/// holding a block as tall as `h` (5 mm); B is active and every owner may
/// link to every other. The document's recomputes use `workers` workers
/// from the start.
fn two_blocks(workers: usize) -> (Document<MockKernel>, Blocks) {
    let mut doc = Document::new(MockKernel::default());
    doc.set_recompute_workers(workers);
    command(
        &mut doc,
        json!({"cmd": "add_parameter", "name": "h", "value": "5 mm"}),
    );
    let (a, _) = doc
        .create_component(Some("A"), shift(100.0, 0.0, 10.0), true)
        .unwrap();
    let (a_extrude, a_body) = block(&mut doc, "h");
    doc.activate_component(ComponentUid::ROOT).unwrap();
    let (_, b_occurrence) = doc
        .create_component(Some("B"), shift(0.0, 50.0, -3.0), true)
        .unwrap();
    let (b_extrude, b_body) = block(&mut doc, "h");
    allow_attachments(&mut doc);
    let blocks = Blocks {
        a,
        a_extrude,
        a_body,
        b_occurrence,
        b_extrude,
        b_body,
    };
    (doc, blocks)
}

/// Logs what the document's kernel and its forks do; the extrusions of
/// `together` wait for one another.
fn watch(
    doc: &Document<MockKernel>,
    together: impl IntoIterator<Item = FeatureUid>,
) -> Arc<WorkerLog> {
    let log = Arc::new(WorkerLog::together(together, PATIENCE));
    *doc.kernel().workers.borrow_mut() = Some(log.clone());
    log
}

fn monitor(doc: &mut Document<MockKernel>) -> Arc<RecomputeMonitor> {
    let monitor = Arc::new(RecomputeMonitor::new());
    doc.set_monitor(Some(monitor.clone()));
    monitor
}

/// The extrusions logged as they began: their features and threads.
fn extrusions(log: &WorkerLog) -> Vec<(FeatureUid, String)> {
    log.events()
        .into_iter()
        .filter(|e| e.op == "extrusion begins")
        .map(|e| (e.feature.unwrap(), e.thread))
        .collect()
}

/// Where in the log the extrusion of `feature` ended (the last time).
fn ended(log: &WorkerLog, feature: FeatureUid) -> usize {
    log.events()
        .iter()
        .rposition(|e| e.op == "extrusion ends" && e.feature == Some(feature))
        .unwrap_or_else(|| panic!("{feature} was not extruded"))
}

/// What must be the same with and without workers: the design, the
/// results' versions, statuses and warnings, the bodies with their face
/// names, what the recompute evaluated and restored, and the memory cache.
#[derive(Debug, PartialEq)]
struct Outcome {
    json: String,
    versions: String,
    results: Vec<String>,
    bodies: Vec<String>,
    evaluated: Vec<FeatureUid>,
    restored: Vec<FeatureUid>,
    cache: String,
    usage: String,
    cache_bytes: u64,
}

fn outcome(doc: &Document<MockKernel>) -> Outcome {
    let stats = doc.stats();
    Outcome {
        json: doc.to_json(),
        versions: format!("{:?}", doc.versions()),
        results: doc
            .last_result()
            .results
            .iter()
            .map(|r| format!("{} {:?} {:?}", r.uid, r.status, r.warnings()))
            .collect(),
        bodies: doc
            .bodies()
            .iter()
            .map(|b| format!("{} {:?}", b.name, b.shape))
            .collect(),
        evaluated: stats.evaluated,
        restored: stats.restored,
        cache: format!("{:?}", doc.cache().stats),
        usage: format!("{:?}", doc.cache().usage()),
        cache_bytes: doc.memory_cache_bytes(),
    }
}

#[test]
fn two_components_are_evaluated_at_the_same_time_with_the_result_of_one_thread() {
    let (mut one, _) = two_blocks(1);
    let (mut many, blocks) = two_blocks(4);
    assert_eq!(many.recompute_workers(), 4);
    let (a, b) = (blocks.a_extrude, blocks.b_extrude);
    let alone = watch(&one, []);
    let log = watch(&many, [a, b]);
    let (one_monitor, many_monitor) = (monitor(&mut one), monitor(&mut many));
    one.set_parameter("h", 8.0).unwrap();
    many.set_parameter("h", 8.0).unwrap();

    // Each extrusion began while the other ran, both on workers.
    assert_eq!(log.met(), BTreeSet::from([a, b]));
    let begun = extrusions(&log);
    assert_eq!(begun.len(), 2, "{begun:?}");
    assert!(
        begun.iter().all(|(_, thread)| thread == WORKER),
        "{begun:?}"
    );
    // One worker: everything on the recompute's own thread.
    let begun = extrusions(&alone);
    assert_eq!(begun.len(), 2, "{begun:?}");
    assert!(alone.events().iter().all(|e| e.thread != WORKER));

    // The same document, cache and progress as one thread.
    assert_eq!(many.stats().evaluated, vec![a, b]);
    assert_eq!(outcome(&many), outcome(&one));
    assert_eq!(many_monitor.progress(), one_monitor.progress());
    assert!(
        many.body_shape(blocks.b_body)
            .unwrap()
            .history
            .contains(":0..8)")
    );

    // Undone, every result comes from the cache: no worker evaluates.
    one.undo().unwrap();
    many.undo().unwrap();
    assert_eq!(extrusions(&log).len(), 2);
    assert_eq!(many.stats().evaluated, Vec::<FeatureUid>::new());
    assert_eq!(outcome(&many), outcome(&one));
    assert_eq!(many_monitor.progress(), one_monitor.progress());
}

#[test]
fn under_a_memory_budget_the_cache_drops_what_it_drops_with_one_thread() {
    let (mut one, _) = two_blocks(1);
    let (mut many, blocks) = two_blocks(4);
    // Every result that goes in makes the cache drop others it can.
    one.set_memory_budget(Some(1));
    many.set_memory_budget(Some(1));
    let (a, b) = (blocks.a_extrude, blocks.b_extrude);
    let log = watch(&many, [a, b]);
    one.set_parameter("h", 8.0).unwrap();
    many.set_parameter("h", 8.0).unwrap();
    // The workers ran; the recompute stopped them before A's block went
    // into the cache (B's is taken if B's worker sent it before, else
    // evaluated by the recompute).
    assert_eq!(log.met(), BTreeSet::from([a, b]));
    assert_eq!(many.stats().evaluated, vec![a, b]);
    assert_eq!(outcome(&many), outcome(&one));
    one.undo().unwrap();
    many.undo().unwrap();
    assert_eq!(outcome(&many), outcome(&one));
}

#[test]
fn a_feature_that_reads_another_component_waits_for_it() {
    // B derives A's block through A:1 (linked geometry, mitcad#100).
    let build = |workers| {
        let (mut doc, blocks) = two_blocks(workers);
        let link = json!({blocks.a_body.to_string(): {"source": "O1"}});
        let derive = doc
            .add_feature(
                &def(json!({"type": "derive", "bodies": [blocks.a_body], "links": link})),
                None,
            )
            .unwrap()
            .uid;
        assert!(
            doc.status(derive).unwrap().is_ok(),
            "{:?}",
            doc.status(derive)
        );
        (doc, blocks, derive)
    };
    let (mut one, _, _) = build(1);
    let (mut many, blocks, derive) = build(4);
    let (a, b) = (blocks.a_extrude, blocks.b_extrude);
    let log = watch(&many, [a, b]);
    one.set_parameter("h", 8.0).unwrap();
    many.set_parameter("h", 8.0).unwrap();

    // The blocks ran at the same time on workers; the derive moved A's
    // block on the recompute's own thread once A's extrusion had ended.
    assert_eq!(log.met(), BTreeSet::from([a, b]));
    let events = log.events();
    let moves: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.op == "transform")
        .map(|(i, _)| i)
        .collect();
    assert!(!moves.is_empty(), "{events:?}");
    for i in moves {
        assert!(i > ended(&log, a), "{events:?}");
        assert_ne!(events[i].thread, WORKER, "{events:?}");
    }
    let copy = many.body_shape(BodyUid::new(derive, 0)).unwrap();
    assert!(copy.history.contains(":0..8)"), "{}", copy.history);
    assert_eq!(many.stats().evaluated, vec![a, b, derive]);
    assert_eq!(outcome(&many), outcome(&one));
}

/// A:1 holds a 60 x 40 sketch whose left side is a driven dimension (a
/// reference parameter, mitcad#231) and a block of it as tall as `h`; B:1
/// a block as tall as the reference. Returns the document, A's sketch and
/// extrude, B's extrude and body, and the name of the sketch's height.
fn driven(workers: usize) -> (Document<MockKernel>, [FeatureUid; 3], BodyUid, String) {
    let mut doc = Document::new(MockKernel::default());
    doc.set_recompute_workers(workers);
    command(
        &mut doc,
        json!({"cmd": "add_parameter", "name": "h", "value": "5 mm"}),
    );
    doc.create_component(Some("A"), shift(100.0, 0.0, 10.0), true)
        .unwrap();
    let sketch = doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let added = doc
        .add_rectangle(sketch, [0.0, 0.0], &num(60.0), &num(40.0))
        .unwrap();
    let height = added.parameters[1].clone();
    let dimension = command(
        &mut doc,
        json!({"cmd": "sketch.add_dimension", "sketch": sketch.to_string(),
               "dimension": {"type": "length", "line": "c4"}, "driven": true}),
    );
    let reference = dimension["parameters"][0].as_str().unwrap().to_owned();
    let a_extrude = doc
        .add_feature(
            &def(json!({"type": "extrude",
                        "profiles": [{"sketch": sketch, "region": added.region}],
                        "extent": {"type": "distance", "distance": "h"},
                        "operation": "new_body"})),
            None,
        )
        .unwrap()
        .uid;
    doc.activate_component(ComponentUid::ROOT).unwrap();
    doc.create_component(Some("B"), shift(0.0, 50.0, -3.0), true)
        .unwrap();
    let (b_extrude, b_body) = block(&mut doc, &reference);
    assert!(doc.body_shape(b_body).unwrap().history.contains(":0..40)"));
    (doc, [sketch, a_extrude, b_extrude], b_body, height)
}

#[test]
fn a_feature_that_reads_a_driven_dimension_waits_for_its_sketch() {
    let (mut one, _, _, height) = driven(1);
    let (mut many, [sketch, a, b], b_body, _) = driven(4);
    let log = watch(&many, [a, b]);
    // A's sketch is 30 tall: its driven reference, and B's block, too.
    one.set_parameter(&height, 30.0).unwrap();
    many.set_parameter(&height, 30.0).unwrap();

    // The sketch is a barrier: B's extrusion waited for it, then ran beside
    // A's on a worker, once, with the reference the sketch measured.
    assert_eq!(log.met(), BTreeSet::from([a, b]));
    let begun = extrusions(&log);
    assert_eq!(
        begun.iter().filter(|(feature, _)| *feature == b).count(),
        1,
        "{begun:?}"
    );
    assert!(
        begun.iter().all(|(_, thread)| thread == WORKER),
        "{begun:?}"
    );
    let shape = many.body_shape(b_body).unwrap();
    assert!(shape.history.contains(":0..30)"), "{}", shape.history);
    assert_eq!(many.stats().evaluated, vec![sketch, a, b]);
    assert_eq!(outcome(&many), outcome(&one));
}

#[test]
fn a_feature_that_reads_a_moved_occurrence_waits_for_the_move() {
    // The root moves B:1 by h along x; A derives B's block through B:1
    // (O2), where the move put it.
    let build = |workers| {
        let (mut doc, blocks) = two_blocks(workers);
        doc.activate_component(ComponentUid::ROOT).unwrap();
        let moved = doc
            .add_feature(
                &def(
                    json!({"type": "move_occurrence", "occurrences": [blocks.b_occurrence],
                            "transform": {"type": "translate_xyz", "x": "h", "y": 0, "z": 0}}),
                ),
                None,
            )
            .unwrap()
            .uid;
        doc.activate_component(blocks.a).unwrap();
        let link = json!({blocks.b_body.to_string(): {"source": "O2"}});
        let derive = doc
            .add_feature(
                &def(json!({"type": "derive", "bodies": [blocks.b_body], "links": link})),
                None,
            )
            .unwrap()
            .uid;
        assert!(
            doc.status(derive).unwrap().is_ok(),
            "{:?}",
            doc.status(derive)
        );
        (doc, blocks, moved, derive)
    };
    let (mut one, _, _, _) = build(1);
    let (mut many, blocks, moved, derive) = build(4);
    let (a, b) = (blocks.a_extrude, blocks.b_extrude);
    let log = watch(&many, [a, b]);
    one.set_parameter("h", 8.0).unwrap();
    many.set_parameter("h", 8.0).unwrap();

    // The blocks ran at the same time; the move is a barrier, and the
    // derive moved B's block on the recompute's own thread after both.
    assert_eq!(log.met(), BTreeSet::from([a, b]));
    let events = log.events();
    let moves: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.op == "transform")
        .map(|(i, _)| i)
        .collect();
    assert!(!moves.is_empty(), "{events:?}");
    for i in moves {
        assert!(i > ended(&log, a) && i > ended(&log, b), "{events:?}");
        assert_ne!(events[i].thread, WORKER, "{events:?}");
    }
    // B:1 at (8, 50, -3) after the move, seen from A:1 at (100, 0, 10).
    let copy = many.body_shape(BodyUid::new(derive, 0)).unwrap();
    assert!(copy.history.contains(",-92,50,-13,"), "{}", copy.history);
    let evaluated = many.stats().evaluated;
    assert!(
        evaluated.contains(&moved) && evaluated.ends_with(&[derive]),
        "{evaluated:?}"
    );
    assert_eq!(outcome(&many), outcome(&one));
}

#[test]
fn a_stop_during_a_parallel_recompute_keeps_nothing_the_workers_returned() {
    let (mut doc, blocks) = two_blocks(4);
    let (a, b) = (blocks.a_extrude, blocks.b_extrude);
    let log = watch(&doc, [a, b]);
    let monitor = monitor(&mut doc);
    // Stopped once A's block, taken from its worker first, is kept.
    monitor.cancel_after(1);
    let before = doc.to_json();
    assert_eq!(doc.set_parameter("h", 8.0), Err(ModelError::Cancelled));
    assert_eq!(doc.to_json(), before);
    assert_eq!(monitor.progress().evaluated, 1);
    // Both blocks were evaluated, at the same time ...
    assert_eq!(log.met(), BTreeSet::from([a, b]));

    // ... but only A's is cached: the next try evaluates B's alone, and
    // gives what one thread gives.
    doc.set_monitor(None);
    doc.set_recompute_workers(1);
    doc.set_parameter("h", 8.0).unwrap();
    assert_eq!(doc.stats().evaluated, vec![b]);
    let (mut one, _) = two_blocks(1);
    one.set_parameter("h", 8.0).unwrap();
    assert_eq!(doc.to_json(), one.to_json());
    assert_eq!(
        format!("{:?}", doc.versions()),
        format!("{:?}", one.versions())
    );
}

#[test]
fn a_stop_while_the_recompute_waits_for_a_worker_cancels_the_workers() {
    let (mut doc, blocks) = two_blocks(4);
    let (a, b) = (blocks.a_extrude, blocks.b_extrude);
    // The extrusions wait two seconds for one that never comes, as a long
    // operation would run: the stop comes while the recompute waits for
    // A's worker.
    let log = Arc::new(WorkerLog::together(
        [a, b, FeatureUid(999)],
        Duration::from_secs(2),
    ));
    *doc.kernel().workers.borrow_mut() = Some(log.clone());
    let monitor = monitor(&mut doc);
    let stop = monitor.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        stop.cancel();
    });
    let before = doc.to_json();
    assert_eq!(doc.set_parameter("h", 8.0), Err(ModelError::Cancelled));
    stopper.join().unwrap();
    assert_eq!(doc.to_json(), before);
    assert_eq!(monitor.progress().evaluated, 0);
    assert!(log.met().is_empty());

    // Nothing of the workers' was kept: both are evaluated again.
    doc.set_monitor(None);
    *doc.kernel().workers.borrow_mut() = None;
    doc.set_parameter("h", 8.0).unwrap();
    assert_eq!(doc.stats().evaluated, vec![a, b]);
}
