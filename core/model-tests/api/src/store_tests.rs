// SPDX-License-Identifier: MIT
//! The result store (P7d), with the mock kernel: results written after a
//! recompute are found by a new document of the same design, only what
//! changed is evaluated, and damaged files, files of another build, quick
//! evaluations and results of renamed parameters are left out.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde_json::json;

use mitcad_model::features::SketchPlane;
use mitcad_model::ids::FeatureUid;
use mitcad_model::store::testing::{mesh_files, set_time, store_files};
use mitcad_model::store::{self, ResultStore};
use mitcad_model::test_support::{Block, block, corner, def, extrude, num};
use mitcad_model::testing::MockKernel;
use mitcad_model::{Document, RecomputeMonitor};
use mitcad_model_api::DocumentApi;

const BUILD: &str = "test build";

/// A store folder of its own, removed afterwards.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "mitcad-store-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }

    fn store(&self) -> ResultStore {
        ResultStore::new(&self.0, BUILD).with_label("part.mitcad")
    }

    fn files(&self) -> Vec<(PathBuf, u64)> {
        store_files(&self.0)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Block (F1, F2 with d3), a boss joined to it (F3, F4 with d6) and a
/// fillet of a corner (F5, d7): three results the store takes.
struct Part {
    b: Block,
    boss: FeatureUid,
    fillet: FeatureUid,
}

fn part() -> Part {
    let mut b = block();
    let sketch = b.doc.add_sketch(SketchPlane::Xy).unwrap().uid;
    let region = b
        .doc
        .add_rectangle(sketch, [10.0, 10.0], &num(20.0), &num(10.0))
        .unwrap()
        .region;
    let join = extrude(sketch, &[&region], 30.0, "join", &[b.body]);
    let boss = b.doc.add_feature(&join, None).unwrap().uid;
    let edge = corner(b.extrude, 1, 0);
    let fillet = def(json!({"type": "fillet", "body": b.body, "edges": [edge], "radius": 2.0}));
    let fillet = b.doc.add_feature(&fillet, None).unwrap().uid;
    Part { b, boss, fillet }
}

/// The project read again with a store, as when the design is opened.
fn open(json: &str, store: Option<ResultStore>) -> Document<MockKernel> {
    let mut doc = Document::from_json(json, MockKernel::default()).unwrap();
    doc.set_result_store(store);
    doc.recompute();
    doc
}

fn histories(doc: &Document<MockKernel>) -> Vec<String> {
    doc.bodies()
        .iter()
        .map(|b| format!("{} {}", b.uid, b.shape.history))
        .collect()
}

#[test]
fn a_new_document_takes_the_stored_results() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    let report = p.b.doc.persist_results(Duration::ZERO);
    // The extrusions and the fillet, not the sketches; the bodies and the
    // extrusions' tools.
    assert_eq!((report.results, report.failed), (3, 0));
    assert!(report.shapes >= 5, "{report:?}");
    assert_eq!(folder.files().len(), 3);
    assert_eq!(
        report.bytes,
        folder.files().iter().map(|(_, size)| size).sum::<u64>()
    );
    // Nothing new to write.
    assert_eq!(p.b.doc.persist_results(Duration::ZERO).results, 0);

    let json = p.b.doc.to_json();
    let opened = open(&json, Some(folder.store()));
    let stats = opened.stats();
    assert_eq!(stats.restored, vec![p.b.extrude, p.boss, p.fillet]);
    assert_eq!(stats.evaluated, vec![p.b.sketch, FeatureUid(3)]);
    assert_eq!(stats.restore_times.len(), 3);
    assert_eq!(opened.kernel().count("extrude"), 0);
    assert_eq!(opened.kernel().count("fillet"), 0);
    assert_eq!(opened.kernel().count("shape_from_bytes"), report.shapes);
    assert_eq!(histories(&opened), histories(&p.b.doc));
    assert_eq!(opened.versions(), p.b.doc.versions());
    // What was read is in the memory cache now, and not written again.
    let mut opened = opened;
    opened.set_parameter("d7", 3.0).unwrap();
    opened.undo();
    assert!(opened.stats().evaluated.is_empty());
    assert!(opened.stats().restored.is_empty());
    assert_eq!(opened.persist_results(Duration::ZERO).results, 0);

    // The command's answer says so.
    let mut command = Document::from_json(&json, MockKernel::default()).unwrap();
    command.set_result_store(Some(folder.store()));
    let answer: serde_json::Value =
        serde_json::from_str(&command.command(r#"{"cmd": "recompute"}"#).unwrap()).unwrap();
    assert_eq!(answer["recomputed"], 2);
    assert_eq!(answer["restored"], 3);
}

#[test]
fn a_change_evaluates_what_follows_it() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    p.b.doc.persist_results(Duration::ZERO);
    // The boss higher: the block comes from the store, the boss and the
    // fillet after it are evaluated.
    p.b.doc.set_parameter("d6", 35.0).unwrap();
    let mut changed = open(&p.b.doc.to_json(), Some(folder.store()));
    assert_eq!(changed.stats().restored, vec![p.b.extrude]);
    assert_eq!(
        changed.stats().evaluated,
        vec![p.b.sketch, FeatureUid(3), p.boss, p.fillet]
    );
    assert_eq!(histories(&changed), histories(&p.b.doc));
    // Stored too, the file of each holds both variants.
    assert_eq!(changed.persist_results(Duration::ZERO).results, 2);
    assert_eq!(folder.files().len(), 3);
    p.b.doc.undo();
    let original = open(&p.b.doc.to_json(), Some(folder.store()));
    assert_eq!(original.stats().restored.len(), 3);
    let again = open(&changed.to_json(), Some(folder.store()));
    assert_eq!(again.stats().restored.len(), 3);
    assert_eq!(histories(&again), histories(&changed));
}

#[test]
fn damaged_files_and_other_builds_are_passed_over() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    p.b.doc.persist_results(Duration::ZERO);
    let json = p.b.doc.to_json();
    let before = store::stats();

    // Another build's results are not used; storing again replaces them.
    let other = ResultStore::new(&folder.0, "another build");
    let mut doc = open(&json, Some(other.clone()));
    assert!(doc.stats().restored.is_empty());
    assert_eq!(doc.stats().evaluated.len(), 5);
    assert_eq!(doc.persist_results(Duration::ZERO).results, 3);
    assert_eq!(folder.files().len(), 3);
    assert_eq!(open(&json, Some(other)).stats().restored.len(), 3);
    assert!(
        open(&json, Some(folder.store()))
            .stats()
            .restored
            .is_empty()
    );
    assert!(store::stats().other_build > before.other_build);

    // Back to this build's, then each file damaged a different way: each
    // is removed when read, and the feature evaluated.
    let mut doc = open(&json, Some(folder.store()));
    doc.persist_results(Duration::ZERO);
    let files = folder.files();
    assert_eq!(files.len(), 3);
    let damage: [&dyn Fn(&Path); 3] = [
        &|path| std::fs::write(path, b"not a store file").unwrap(),
        &|path| {
            let mut bytes = std::fs::read(path).unwrap();
            let last = bytes.len() - 1;
            bytes[last] ^= 0xff;
            std::fs::write(path, bytes).unwrap();
        },
        &|path| {
            let bytes = std::fs::read(path).unwrap();
            std::fs::write(path, &bytes[..bytes.len() / 2]).unwrap();
        },
    ];
    for ((path, _), damage) in files.iter().zip(damage) {
        damage(path);
    }
    let damaged = store::stats().damaged;
    let doc = open(&json, Some(folder.store()));
    assert!(doc.stats().restored.is_empty());
    assert_eq!(doc.stats().evaluated.len(), 5);
    assert_eq!(histories(&doc), histories(&p.b.doc));
    assert!(folder.files().is_empty());
    assert!(store::stats().damaged >= damaged + 3);
}

#[test]
fn a_shape_the_kernel_cannot_read_is_evaluated() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    p.b.doc.persist_results(Duration::ZERO);
    let mut doc = Document::from_json(&p.b.doc.to_json(), MockKernel::default()).unwrap();
    doc.kernel().fail.borrow_mut().insert("shape_from_bytes");
    doc.set_result_store(Some(folder.store()));
    doc.recompute();
    assert!(doc.stats().restored.is_empty());
    assert_eq!(histories(&doc), histories(&p.b.doc));
    // And one that cannot be written is reported, not stored.
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.kernel().fail.borrow_mut().insert("shape_bytes");
    p.b.doc.set_result_store(Some(folder.store()));
    let report = p.b.doc.persist_results(Duration::ZERO);
    assert_eq!((report.results, report.failed), (0, 3));
    assert!(
        report.errors[0].contains("shape_bytes failed"),
        "{report:?}"
    );
    assert!(folder.files().is_empty());
}

#[test]
fn quick_failed_and_renamed_results_are_not_written() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    // The mock evaluates at once: nothing takes an hour.
    assert_eq!(
        p.b.doc.persist_results(Duration::from_secs(3600)).results,
        0
    );
    // A renamed parameter: the results it was evaluated with hold its old
    // name. The fillet's radius, d7, is renamed; its result stays in the
    // cache (the rename recomputes nothing) but is not written.
    p.b.doc.rename_parameter("d7", "radius").unwrap();
    assert_eq!(p.b.doc.persist_results(Duration::ZERO).results, 2);
    // A failed feature is not written either.
    let folder = Folder::new();
    let mut q = part();
    q.b.doc.kernel().fail.borrow_mut().insert("fillet");
    q.b.doc.set_parameter("d7", 2.5).unwrap();
    assert!(!q.b.doc.status(q.fillet).unwrap().is_ok());
    q.b.doc.set_result_store(Some(folder.store()));
    assert_eq!(q.b.doc.persist_results(Duration::ZERO).results, 2);
}

#[test]
fn restored_results_are_not_counted_as_evaluated() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    p.b.doc.persist_results(Duration::ZERO);
    let mut doc = Document::from_json(&p.b.doc.to_json(), MockKernel::default()).unwrap();
    doc.set_result_store(Some(folder.store()));
    let monitor = Arc::new(RecomputeMonitor::new());
    doc.set_monitor(Some(monitor.clone()));
    doc.try_recompute().unwrap();
    assert_eq!(monitor.progress().evaluated, 2);
    assert_eq!(monitor.progress().position, 5);
}

#[test]
fn gc_removes_the_files_used_longest_ago() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    p.b.doc.persist_results(Duration::ZERO);
    let mut files = folder.files();
    files.sort();
    let total: u64 = files.iter().map(|(_, size)| size).sum();
    // Within the budget: nothing goes.
    let report = store::gc(&folder.0, total);
    assert_eq!((report.files, report.bytes, report.removed), (3, total, 0));
    // The oldest go until 80 % of the budget is used.
    let base = SystemTime::now() - Duration::from_secs(3600);
    for (i, (path, _)) in files.iter().enumerate() {
        set_time(path, base + Duration::from_secs(60 * i as u64));
    }
    let budget = total - 1;
    let report = store::gc(&folder.0, budget);
    assert!(report.removed >= 1);
    assert!(report.bytes as f64 <= budget as f64 * 0.8);
    let left: Vec<PathBuf> = folder.files().into_iter().map(|(path, _)| path).collect();
    assert!(!left.contains(&files[0].0));
    assert_eq!(report.files as usize, left.len());
    // A hit makes a file the newest: the newest stays, whichever it was.
    let doc = open(&p.b.doc.to_json(), Some(folder.store()));
    assert_eq!(doc.stats().restored.len(), left.len());
    // A temporary file left by a program that stopped goes too.
    let dir = files[0].0.parent().unwrap().to_owned();
    std::fs::create_dir_all(&dir).unwrap();
    let temporary = dir.join(".x.mrs.1.0.tmp");
    std::fs::write(&temporary, b"partial").unwrap();
    set_time(&temporary, base - Duration::from_secs(3600));
    store::gc(&folder.0, u64::MAX);
    assert!(!temporary.exists());
    // Clear removes everything.
    let report = store::clear(&folder.0);
    assert_eq!(report.removed as usize, left.len());
    assert!(folder.files().is_empty());
}

#[test]
fn an_import_s_results_are_handed_over_however_quick() {
    // The import process hands its evaluation over in a store of its own
    // (mitcad#413): every result the store takes, however quick, without
    // a store of the document's, and none marked written there.
    let handed = Folder::new();
    let own = Folder::new();
    let mut p = part();
    let report = p.b.doc.hand_off_results(&handed.store());
    assert_eq!((report.results, report.failed), (3, 0));
    assert_eq!(handed.files().len(), 3);
    assert!(p.b.doc.result_store().is_none());
    p.b.doc.set_result_store(Some(own.store()));
    assert_eq!(
        p.b.doc.persist_results(Duration::from_secs(3600)).results,
        0,
        "the quick ones are left out of the document's store"
    );
    assert_eq!(p.b.doc.persist_results(Duration::ZERO).results, 3);
    // The design read with it takes them instead of evaluating them.
    let opened = open(&p.b.doc.to_json(), Some(handed.store()));
    assert_eq!(opened.stats().restored, vec![p.b.extrude, p.boss, p.fillet]);
    assert_eq!(opened.kernel().count("extrude"), 0);
    assert_eq!(histories(&opened), histories(&p.b.doc));
    assert_eq!(opened.versions(), p.b.doc.versions());
}

#[test]
fn without_a_store_nothing_is_written_or_read() {
    let mut p = part();
    assert_eq!(p.b.doc.persist_results(Duration::ZERO), Default::default());
    let doc = open(&p.b.doc.to_json(), None);
    assert!(doc.stats().restored.is_empty());
    assert_eq!(doc.kernel().count("shape_bytes"), 0);
}

#[test]
fn stored_tapped_holes_keep_the_walls_their_threads_took() {
    // A modelled tapped hole whose radial clearance leaves no piece of its
    // wall: the threads query lists it from the wall the hole measured
    // before the thread (mitcad#436), which a restored result keeps.
    let folder = Folder::new();
    let mut b = block();
    let hole = def(json!({"type": "hole",
        "placement": {"type": "face", "body": "F2.b0",
                      "face": "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})",
                      "points": [[10, 10, 20]]},
        "diameter": 6, "extent": {"type": "distance", "depth": 10},
        "thread": {"designation": "M6x1", "class": "6H", "modeled": true,
                   "radial_clearance": 0.1}}));
    let uid = b.doc.add_feature(&hole, None).unwrap().uid;
    let threads = b.doc.query(r#"{"query": "threads"}"#).unwrap();
    assert!(threads.contains("F3:hole0.wall"), "{threads}");
    b.doc.set_result_store(Some(folder.store()));
    assert_eq!(b.doc.persist_results(Duration::ZERO).failed, 0);
    let opened = open(&b.doc.to_json(), Some(folder.store()));
    assert!(opened.stats().restored.contains(&uid));
    assert_eq!(opened.kernel().count("thread"), 0);
    assert_eq!(opened.query(r#"{"query": "threads"}"#).unwrap(), threads);
}

/// The project read again with a store, by a kernel that keeps the shapes
/// of restored results packed until something reads them (mitcad#858).
fn open_lazy(json: &str, store: Option<ResultStore>) -> Document<MockKernel> {
    let kernel = MockKernel::default();
    kernel.lazy.set(true);
    let mut doc = Document::from_json(json, kernel).unwrap();
    doc.set_result_store(store);
    doc.recompute();
    doc
}

/// Whether the result of `uid` in the last recompute still has its
/// bodies' shapes packed.
fn packed(doc: &Document<MockKernel>, uid: FeatureUid) -> bool {
    let output = doc
        .last_result()
        .result(uid)
        .unwrap()
        .output
        .as_ref()
        .unwrap();
    !output.changes.is_unpacked()
}

#[test]
fn a_state_replaced_before_anything_reads_it_is_not_unpacked() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    let report = p.b.doc.persist_results(Duration::ZERO);
    let json = p.b.doc.to_json();
    let opened = open_lazy(&json, Some(folder.store()));
    assert_eq!(opened.stats().restored, vec![p.b.extrude, p.boss, p.fillet]);
    // The block's state and the boss's are replaced by restored results
    // before anything reads them (the boss's sketch is on the XY plane and
    // projects nothing): never unpacked; the fillet's is the body's.
    assert!(packed(&opened, p.b.extrude));
    assert!(packed(&opened, p.boss));
    assert!(!packed(&opened, p.fillet));
    // Only the tools and the body were read: the bodies are the same.
    assert!(opened.kernel().count("shape_from_bytes") < report.shapes);
    assert_eq!(histories(&opened), histories(&p.b.doc));
    assert_eq!(opened.versions(), p.b.doc.versions());
    // The timeline's history unpacks a state when something reads it.
    let before = |doc: &Document<MockKernel>| {
        doc.body_before(p.fillet, p.b.body)
            .map(|shape| shape.history.clone())
    };
    assert_eq!(before(&opened), before(&p.b.doc));
    assert!(!packed(&opened, p.boss));
    assert!(packed(&opened, p.b.extrude));

    // The command's answer counts what stayed packed.
    let mut command = Document::from_json(&json, MockKernel::default()).unwrap();
    command.kernel().lazy.set(true);
    command.set_result_store(Some(folder.store()));
    let answer: serde_json::Value =
        serde_json::from_str(&command.command(r#"{"cmd": "recompute"}"#).unwrap()).unwrap();
    assert_eq!(answer["restored"], 3);
    assert_eq!(answer["restored_packed"], 2);
    // Without the lazy kernel, nothing stays packed.
    let whole = open(&json, Some(folder.store()));
    assert!(!packed(&whole, p.b.extrude));
    assert_eq!(whole.kernel().count("shape_from_bytes"), report.shapes);
}

#[test]
fn an_evaluation_reads_the_restored_shapes_unpacked() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    p.b.doc.persist_results(Duration::ZERO);
    let mut opened = open_lazy(&p.b.doc.to_json(), Some(folder.store()));
    // Another fillet radius: the block and the boss come from the memory
    // cache still packed, and the fillet, evaluated, reads the boss's body
    // unpacked; the block's state is never read.
    opened.set_parameter("d7", 3.0).unwrap();
    p.b.doc.set_parameter("d7", 3.0).unwrap();
    assert_eq!(opened.stats().evaluated, vec![p.fillet]);
    assert!(!packed(&opened, p.boss));
    assert!(packed(&opened, p.b.extrude));
    assert_eq!(histories(&opened), histories(&p.b.doc));
    // And a change before them evaluates from the start, as without.
    opened.set_parameter("d6", 35.0).unwrap();
    p.b.doc.set_parameter("d6", 35.0).unwrap();
    assert_eq!(histories(&opened), histories(&p.b.doc));
}

#[test]
fn display_meshes_are_kept_for_the_body_s_shape() {
    let folder = Folder::new();
    let mut p = part();
    p.b.doc.set_result_store(Some(folder.store()));
    p.b.doc.persist_results(Duration::ZERO);
    let json = p.b.doc.to_json();
    let store = folder.store();
    let options = "final deviation=0.01";
    let version = p.b.doc.body_version(p.b.body).unwrap();
    let shape = p.b.doc.body_shape(p.b.body).unwrap().clone();
    let written = store
        .write_display_mesh(p.b.doc.kernel(), version, options, &shape)
        .unwrap();
    let meshes = mesh_files(&folder.0);
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].1, written);

    // Opened again: the body's shape has the same version and takes them.
    let opened = open_lazy(&json, Some(folder.store()));
    assert_eq!(opened.body_version(p.b.body), Some(version));
    let body = opened.body_shape(p.b.body).unwrap();
    let hits = store::stats().mesh_hits;
    assert_eq!(
        store.restore_display_mesh(opened.kernel(), version, options, body),
        Some(body.faces.len())
    );
    assert!(store::stats().mesh_hits > hits);
    // Other options or another build find none.
    assert_eq!(
        store.restore_display_mesh(opened.kernel(), version, "other", body),
        None
    );
    let other = ResultStore::new(&folder.0, "another build");
    assert_eq!(
        other.restore_display_mesh(opened.kernel(), version, options, body),
        None
    );
    // A changed body has another version, which has none.
    let mut changed = open(&json, Some(folder.store()));
    changed.set_parameter("d7", 3.0).unwrap();
    let now = changed.body_version(p.b.body).unwrap();
    assert_ne!(now, version);
    let body = changed.body_shape(p.b.body).unwrap();
    assert_eq!(
        store.restore_display_mesh(changed.kernel(), now, options, body),
        None
    );
    // Meshes of another shape under a version write nothing.
    let block = p.b.doc.body_before(p.boss, p.b.body).unwrap().clone();
    store
        .write_display_mesh(p.b.doc.kernel(), now, options, &block)
        .unwrap();
    assert_eq!(
        store.restore_display_mesh(changed.kernel(), now, options, body),
        None
    );

    // The store's size, its clean-up and Clear count them.
    let usage = store::usage(&folder.0, BUILD);
    assert_eq!((usage.meshes, usage.entries.len()), (2, 3));
    assert_eq!(usage.files, 5);
    let report = store::gc(&folder.0, 1);
    assert_eq!(report.removed, 5);
    assert!(folder.files().is_empty());
}
