// SPDX-License-Identifier: MIT
//! One solve per sketch edit (mitcad#679): the `sketch` query shows the
//! evaluation's own solve while the definition and the parameter values
//! are those it was made from, and solves again as soon as an edit, a
//! parameter change, undo or redo changed them; finishing a drag takes the
//! drag's solve.

use std::sync::Arc;

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::features::FeatureDef;
use mitcad_model::ids::FeatureUid;
use mitcad_model::sketch::solve::Solved;
use mitcad_model::testing::MockKernel;
use mitcad_model_api::{DocumentApi, SketchApi};

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

fn query(doc: &Document<MockKernel>, query: Value) -> Value {
    serde_json::from_str(&doc.query(&query.to_string()).unwrap()).unwrap()
}

const F1: FeatureUid = FeatureUid(1);

/// A circle and a 3 x 3 rectangular pattern of it (the pattern's spacing
/// parameters d1 and d2), as perf_sketch_pattern_*.json at a small size.
fn pattern() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    command(&mut doc, json!({"cmd": "sketch.create"}));
    command(
        &mut doc,
        json!({"cmd": "sketch.circle", "sketch": "F1", "mode": "center",
               "center": [0, 0], "radius": 3}),
    );
    command(
        &mut doc,
        json!({"cmd": "sketch.rectangular_pattern", "sketch": "F1", "entities": ["c1"],
               "count": [3, 3], "spacing": [10, 10]}),
    );
    doc
}

/// The sketch's definition solved now with the parameters' values, as the
/// query solved it every time before.
fn solved_now(doc: &Document<MockKernel>) -> Solved {
    let FeatureDef::Sketch(def) = &doc.feature(F1).unwrap().def else {
        panic!("F1 is a sketch");
    };
    let params = doc.parameters();
    def.solve_with(
        &mut |id| params.value(id).ok_or_else(|| "missing".to_owned()),
        &|id| params.name(id),
    )
    .unwrap()
}

/// The query takes the evaluation's solve, and it is the solve of the
/// definition and values now.
fn assert_reused(doc: &Document<MockKernel>) -> Arc<Solved> {
    let reused = doc
        .evaluated_solve(F1)
        .expect("the evaluation's solve is the current one");
    assert_eq!(*reused, solved_now(doc));
    reused
}

/// The centre of copy `circle` (`c<k>`) as the query shows it.
fn center(doc: &Document<MockKernel>, circle: &str) -> [f64; 2] {
    let sketch = query(doc, json!({"query": "sketch", "uid": "F1"}));
    let entity = sketch["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == circle)
        .unwrap_or_else(|| panic!("no {circle}: {sketch}"));
    let c = &entity["geometry"]["center"];
    [c[0].as_f64().unwrap(), c[1].as_f64().unwrap()]
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
}

#[test]
fn the_query_shows_the_evaluated_solve_until_something_changes() {
    let mut doc = pattern();
    let first = assert_reused(&doc);
    // The last copy, two spacings away in both directions.
    let last = query(&doc, json!({"query": "sketch", "uid": "F1"}))["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "circle")
        .map(|e| e["id"].as_str().unwrap().to_owned())
        .next_back()
        .unwrap();
    assert!(near(center(&doc, &last), [20.0, 20.0]));

    // A drag: the definition changed, the query shows the new solve.
    command(
        &mut doc,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entity": "p2", "to": [1, 2]}),
    );
    let dragged = assert_reused(&doc);
    assert!(!Arc::ptr_eq(&first, &dragged));
    assert_ne!(*first, *dragged);
    assert!(near(center(&doc, "c1"), [1.0, 2.0]));
    assert!(near(center(&doc, &last), [21.0, 22.0]));

    // A parameter: the same definition with another value.
    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "d1", "value": 12}),
    );
    let spaced = assert_reused(&doc);
    assert_ne!(*spaced, *dragged);
    assert!(near(spread(&doc, &last), [24.0, 20.0]));

    // Undo and redo bring the earlier solves back.
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(*assert_reused(&doc), *dragged);
    assert!(near(center(&doc, &last), [21.0, 22.0]));
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(*assert_reused(&doc), *first);
    assert!(near(center(&doc, &last), [20.0, 20.0]));
    command(&mut doc, json!({"cmd": "redo"}));
    assert_eq!(*assert_reused(&doc), *dragged);
    command(&mut doc, json!({"cmd": "redo"}));
    assert_eq!(*assert_reused(&doc), *spaced);
    assert!(near(spread(&doc, &last), [24.0, 20.0]));
}

/// How far copy `circle` lies from the original c1.
fn spread(doc: &Document<MockKernel>, circle: &str) -> [f64; 2] {
    let (a, b) = (center(doc, "c1"), center(doc, circle));
    [b[0] - a[0], b[1] - a[1]]
}

#[test]
fn a_rename_keeps_the_evaluated_solve_and_a_rolled_back_sketch_solves() {
    let mut doc = pattern();
    let before = query(&doc, json!({"query": "sketch", "uid": "F1"}));
    // A rename recomputes nothing and changes no definition: the
    // evaluation's solve still holds.
    command(
        &mut doc,
        json!({"cmd": "rename_parameter", "name": "d1", "new_name": "pitch"}),
    );
    assert_reused(&doc);
    let renamed = query(&doc, json!({"query": "sketch", "uid": "F1"}));
    assert_eq!(renamed["entities"], before["entities"]);
    assert_eq!(renamed["dof"], before["dof"]);
    // Rolled back: no evaluation, the query solves the definition.
    command(&mut doc, json!({"cmd": "set_marker", "position": 0}));
    assert!(doc.evaluated_solve(F1).is_none());
    let rolled = query(&doc, json!({"query": "sketch", "uid": "F1"}));
    assert_eq!(rolled["entities"], before["entities"]);
    assert_eq!(rolled["solved"], json!(true));
    command(&mut doc, json!({"cmd": "set_marker", "position": 1}));
    assert_reused(&doc);
}

#[test]
fn finishing_a_drag_keeps_the_drag_solve() {
    let mut doc = pattern();
    let result = command(
        &mut doc,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entity": "c1", "radius": 4}),
    );
    // The stored positions are a solution: solving them again moves
    // nothing, and the status the drag reported is the sketch's.
    let FeatureDef::Sketch(def) = &doc.feature(F1).unwrap().def else {
        panic!("F1 is a sketch");
    };
    let again = solved_now(&doc);
    let mut stored = def.entities.clone();
    again.store(&mut stored);
    for (a, b) in stored.iter().zip(&def.entities) {
        match (&a.kind, &b.kind) {
            (
                mitcad_model::sketch::EntityKind::Point { at: p },
                mitcad_model::sketch::EntityKind::Point { at: q },
            ) => assert!(near(*p, *q), "{} moved: {p:?} {q:?}", a.id.0),
            (
                mitcad_model::sketch::EntityKind::Circle { radius: r, .. },
                mitcad_model::sketch::EntityKind::Circle { radius: s, .. },
            ) => assert!((r - s).abs() < 1e-6, "c{} radius {r} {s}", a.id.0),
            _ => {}
        }
    }
    let sketch = query(&doc, json!({"query": "sketch", "uid": "F1"}));
    let status = &result["status"];
    assert!(status["dof"].is_u64(), "{result}");
    assert_eq!(status["dof"], sketch["dof"]);
    assert_eq!(status["fully_constrained"], sketch["fully_constrained"]);
    assert_eq!(status["redundant"], sketch["redundant"]);
    assert_eq!(status["conflicts"], sketch["conflicts"]);
}

/// The query's text as the sketch is now, not the kept answer.
fn made_now(doc: &Document<MockKernel>) -> String {
    doc.sketch_json(F1).unwrap().to_string()
}

#[test]
fn the_kept_answer_follows_every_change() {
    let mut doc = pattern();
    let text = |doc: &Document<MockKernel>| {
        doc.query(&json!({"query": "sketch", "uid": "F1"}).to_string())
            .unwrap()
    };
    let first = text(&doc);
    assert_eq!(doc.known_sketch_answer(F1).as_deref(), Some(first.as_str()));
    assert_eq!(text(&doc), first);
    assert_eq!(made_now(&doc), first);

    // A rename recomputes nothing, but the answer names the parameter.
    command(
        &mut doc,
        json!({"cmd": "rename_parameter", "name": "d1", "new_name": "pitch"}),
    );
    let renamed = text(&doc);
    assert_ne!(renamed, first);
    assert!(renamed.contains("\"pitch\""), "{renamed}");
    assert_eq!(made_now(&doc), renamed);

    command(
        &mut doc,
        json!({"cmd": "set_parameter", "name": "pitch", "value": 12}),
    );
    let spaced = text(&doc);
    assert_ne!(spaced, renamed);
    assert_eq!(made_now(&doc), spaced);

    command(
        &mut doc,
        json!({"cmd": "sketch.drag", "sketch": "F1", "entity": "p2", "to": [1, 2]}),
    );
    let dragged = text(&doc);
    assert_ne!(dragged, spaced);
    assert_eq!(made_now(&doc), dragged);

    // Rolled back and forward again.
    command(&mut doc, json!({"cmd": "set_marker", "position": 0}));
    assert_eq!(text(&doc), made_now(&doc));
    command(&mut doc, json!({"cmd": "set_marker", "position": 1}));
    assert_eq!(text(&doc), dragged);

    // Undo goes back through each answer.
    command(&mut doc, json!({"cmd": "undo"}));
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(text(&doc), dragged);
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(text(&doc), spaced);
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(text(&doc), renamed);
    command(&mut doc, json!({"cmd": "undo"}));
    assert_eq!(text(&doc), first);
    assert_eq!(made_now(&doc), first);
}
