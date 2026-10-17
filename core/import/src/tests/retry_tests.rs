// SPDX-License-Identifier: MIT
//! Retries on the file's exact state (mitcad#283) on synthetic dumps with
//! the mock kernel: the replay's bodies that match the state stay unless
//! the user chose to replace them, the application's question is asked
//! once, and the report names the bodies kept and replaced. The other base
//! features that bring the bodies to a state keep them alike (mitcad#285).

use std::sync::Mutex;

use mitcad_model::FilletSize;

use super::*;
use crate::{AskUser, RetryBodies, RetryChoice};

/// A fillet item on the edge between faces `a` and `b` (their middle,
/// in cm as the decoder writes it).
fn fillet_on(
    index: i64,
    name: &str,
    a: &mitcad_model::FaceName,
    b: &mitcad_model::FaceName,
) -> Value {
    let p = edge_point(a, b);
    json!({"index": index, "name": name, "objectType": "FilletFeature",
    "detail": {"edgeSets": [{"radius": param(&format!("d{}", 5 + index), "2 mm", 0.2, "mm"),
        "edges": [{"kind": "edge", "length": 0.1, "mid_point": p.map(|x| x / 10.0)}]}]}})
}

/// A block (Sketch1, Extrude1) and Fillet1, whose edge is named by the
/// stored state's faces: the replay's bodies are the state before it, but
/// the fillet does not build on them (the replay's edges are split
/// otherwise, as the geometry kernel's rounding can fail on one and build
/// on the other); on the stored state's bodies it gives its own state.
/// With `approximate`, Fillet0 on an edge of the block before it, which the
/// kernel makes 3e-4 larger than the file's (as a rounding the file stores
/// as splines): the bodies before Fillet1 carry that. Imported with
/// `options` on a kernel that lists new faces with `lists_new_faces`.
fn fillet_on_stored_edges(
    approximate: bool,
    lists_new_faces: bool,
    options: &Options,
) -> (DesignReport, Document<TestKernel>) {
    let mut items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let kernel = TestKernel {
        edge_points: true,
        skew_fillets: approximate,
        lists_new_faces,
        ..TestKernel::default()
    };
    let exact = || TestKernel {
        edge_points: true,
        ..TestKernel::default()
    };
    let replayed = |items: &[Value]| {
        let mut reference = Document::new(exact());
        import_design(
            &mut reference,
            &timeline_dump(items.to_vec()),
            &mut NoGeometry,
            &Options::default(),
        );
        shapes(&reference)
    };
    let mut states = vec![Vec::new(), replayed(&items)];
    if approximate {
        let (a, b) = states[1][0].edges[0].clone();
        items.push(fillet_on(2, "Fillet0", &a, &b));
        states.push(replayed(&items));
    }
    // The stored state's bodies as a base feature has them, and the fillet
    // of one of their edges.
    let before = states.last().unwrap()[0].clone();
    let data = kernel.brep_data(&before).unwrap();
    let (stored, _) = kernel.import_brep(FeatureUid(9), &data, 0).unwrap();
    let (a, b) = (stored.faces[0].clone(), stored.faces[1].clone());
    let edge = EdgeName::new(a.clone(), b.clone());
    let set = FilletSet {
        edges: std::slice::from_ref(&edge),
        faces: &[],
        size: FilletSize::Constant { radius: 2.0 },
        tangent_chain: true,
        curvature: false,
        weight: 0.0,
    };
    let filleted = exact()
        .fillet(FeatureUid(10), &stored, &[set], true)
        .unwrap();
    let index = items.len() as i64;
    items.push(fillet_on(index, "Fillet1", &a, &b));
    states.push(vec![filleted]);
    let mut geometry = TestGeometry {
        states,
        items: (1..=index).map(|i| (i, i as usize)).collect(),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(kernel);
    let report = import_design(&mut doc, &timeline_dump(items), &mut geometry, options);
    (report, doc)
}

fn with_choice(choice: RetryChoice) -> Options {
    Options {
        retry_bodies: Some(Arc::new(choice)),
        ..Options::default()
    }
}

/// The application's side of the question: answers `answer` to each
/// question once asked, and keeps the questions.
struct Answering {
    answer: &'static str,
    asked: Arc<Mutex<Vec<String>>>,
}

impl AskUser for Answering {
    fn ask(&self, question: &str) {
        self.asked.lock().unwrap().push(question.to_owned());
    }

    fn answer(&self) -> Option<String> {
        (!self.asked.lock().unwrap().is_empty()).then(|| self.answer.to_owned())
    }
}

fn asking(answer: &'static str) -> (Options, Arc<Mutex<Vec<String>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let options = with_choice(RetryChoice::ask(Box::new(Answering {
        answer,
        asked: asked.clone(),
    })));
    (options, asked)
}

/// Whether a base feature brought the bodies to the state before Fillet1.
fn brought_before_fillet(report: &DesignReport) -> bool {
    report
        .warnings
        .iter()
        .any(|w| w.contains("to the file's state before Fillet1"))
}

#[test]
fn a_retry_keeps_the_bodies_that_are_the_states_by_default() {
    // The block is the state's exactly: nothing differs, so there is no
    // retry, and the fillet takes the file's bodies as an item that fails.
    // The block stays the extrusion's (parametric).
    let (report, doc) = fillet_on_stored_edges(false, false, &Options::default());
    let text = report.text();
    assert_eq!(report.items[1].outcome, Outcome::Parametric, "{text}");
    assert_eq!(report.items[2].outcome, Outcome::Fallback, "{text}");
    assert!(!brought_before_fillet(&report), "{text}");
    let retry = report.items[2].exact_retry.as_ref().expect(&text);
    assert_eq!(retry.state, 1);
    assert_eq!(retry.bodies, RetryBodies::Keep);
    assert_eq!(retry.kept.len(), 1, "{text}");
    assert!(retry.kept[0].contains("F2.b0"), "{text}");
    assert!(retry.replaced.is_empty() && !retry.matched, "{text}");
    assert!(text.contains("retry on state 1 (keep): kept "), "{text}");
    assert_eq!(report.retry_question, None);
    // The only base feature is the fillet's fallback, after it.
    let bases: Vec<FeatureUid> = doc
        .features()
        .filter(|f| matches!(f.def, FeatureDef::Base(_)))
        .map(|f| f.uid)
        .collect();
    assert_eq!(bases.len(), 1, "{text}");
    assert_eq!(
        report.items[2].features,
        vec![bases[0].to_string()],
        "{text}"
    );
    // So does the command line's keep.
    let (kept, _) = fillet_on_stored_edges(
        false,
        false,
        &with_choice(RetryChoice::new(RetryBodies::Keep)),
    );
    assert_eq!(kept.items[2].outcome, Outcome::Fallback);
    assert_eq!(kept.items[2].exact_retry, report.items[2].exact_retry);
}

#[test]
fn a_retry_replaces_the_matching_bodies_when_the_user_chose_so() {
    // As the import did before: every body comes from the stored state,
    // and the fillet builds there.
    let (report, _) = fillet_on_stored_edges(
        false,
        false,
        &with_choice(RetryChoice::new(RetryBodies::Replace)),
    );
    let text = report.text();
    assert_eq!(report.items[2].outcome, Outcome::Parametric, "{text}");
    assert_eq!(report.items[2].verified, Some(true), "{text}");
    assert!(brought_before_fillet(&report), "{text}");
    let retry = report.items[2].exact_retry.as_ref().expect(&text);
    assert_eq!(retry.bodies, RetryBodies::Replace);
    assert!(retry.kept.is_empty(), "{text}");
    assert_eq!(retry.replaced.len(), 1, "{text}");
    assert!(retry.replaced[0].contains("F2.b0"), "{text}");
    assert!(retry.matched, "{text}");
}

#[test]
fn a_retry_keeps_a_body_within_the_tolerance_and_replaces_one_that_differs() {
    // The replay's block carries an approximation (3e-4 larger): its faces
    // lie on the stored block's, so it matches within the geometric
    // tolerance and stays; there is no retry.
    let (report, _) = fillet_on_stored_edges(true, true, &Options::default());
    let text = report.text();
    assert_eq!(report.items[2].name, "Fillet0");
    assert_eq!(report.items[2].outcome, Outcome::Parametric, "{text}");
    assert_eq!(report.items[3].outcome, Outcome::Fallback, "{text}");
    let retry = report.items[3].exact_retry.as_ref().expect(&text);
    assert_eq!(retry.state, 2);
    assert_eq!((retry.kept.len(), retry.replaced.len()), (1, 0), "{text}");
    assert!(!brought_before_fillet(&report), "{text}");
    // Where its faces cannot be told to lie on the stored block's, it
    // differs: it alone is brought to the state, and the fillet builds.
    let (report, _) = fillet_on_stored_edges(true, false, &Options::default());
    let text = report.text();
    assert_eq!(report.items[3].outcome, Outcome::Parametric, "{text}");
    let retry = report.items[3].exact_retry.as_ref().expect(&text);
    assert_eq!(retry.bodies, RetryBodies::Keep);
    assert_eq!((retry.kept.len(), retry.replaced.len()), (0, 1), "{text}");
    assert!(retry.matched && brought_before_fillet(&report), "{text}");
}

#[test]
fn the_application_asks_once_and_the_answer_holds() {
    // Replace: the fillet builds on the stored bodies.
    let (options, asked) = asking("replace");
    let (report, _) = fillet_on_stored_edges(false, false, &options);
    let text = report.text();
    assert_eq!(report.items[2].outcome, Outcome::Parametric, "{text}");
    let questions = asked.lock().unwrap().clone();
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert!(
        questions[0].starts_with("Fillet1 does not give the file's result")
            && questions[0].contains("replace an imported body that already matches them")
            && !questions[0].contains('\n'),
        "{questions:?}"
    );
    let answered = report.retry_question.clone().expect(&text);
    assert_eq!(
        (answered.item.as_str(), answered.answer),
        ("Fillet1", RetryBodies::Replace)
    );
    assert!(text.contains("asked at Fillet1"), "{text}");
    // The answer holds for the rest of the import (a later try of it, the
    // documents it refers to): not asked again.
    let (again, _) = fillet_on_stored_edges(false, false, &options);
    assert_eq!(again.items[2].outcome, Outcome::Parametric);
    assert_eq!(asked.lock().unwrap().len(), 1);
    // Keep: the bodies stay, the fillet takes the file's bodies.
    let (options, asked) = asking("keep");
    let (report, _) = fillet_on_stored_edges(false, false, &options);
    let text = report.text();
    assert_eq!(report.items[2].outcome, Outcome::Fallback, "{text}");
    assert!(!brought_before_fillet(&report), "{text}");
    assert_eq!(asked.lock().unwrap().len(), 1);
    assert_eq!(
        report.retry_question.map(|q| q.answer),
        Some(RetryBodies::Keep)
    );
    // Nothing to ask where a retry replaces no body that matches: the body
    // that differs is replaced without a question.
    let (options, asked) = asking("keep");
    let (report, _) = fillet_on_stored_edges(true, false, &options);
    assert_eq!(report.items[3].outcome, Outcome::Parametric);
    assert!(asked.lock().unwrap().is_empty());
    assert_eq!(report.retry_question, None);
}

/// Extrude1 of the smaller rectangle on a kernel whose extrusions come out
/// 3e-4 larger than the file's, with faces it tells to lie on the file's
/// within the geometric tolerance: imported correctly. Then `item` (index
/// 2, its state 2 `after`; `unchanged`: the history says it changed no
/// body), or, without one, `after` as the stored design.
fn after_an_approximate_extrusion(
    item: Option<Value>,
    after: Vec<MockShape>,
    unchanged: bool,
) -> (DesignReport, Document<TestKernel>) {
    let mut items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut known = HashMap::from([(1, 1)]);
    if let Some(item) = item {
        items.push(item);
        known.insert(2, 2);
    }
    let mut geometry = TestGeometry {
        states: vec![
            Vec::new(),
            shapes(&reference(vec![extrusion_of(SMALL)])),
            after,
        ],
        items: known,
        unchanged: if unchanged { vec![2] } else { Vec::new() },
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel {
        skew: true,
        lists_new_faces: true,
        ..TestKernel::default()
    });
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    );
    (report, doc)
}

/// The bodies each base feature replaces.
fn bases_replace(doc: &Document<TestKernel>) -> Vec<Vec<BodyUid>> {
    doc.features()
        .filter_map(|f| match &f.def {
            FeatureDef::Base(b) => Some(b.replaces.clone()),
            _ => None,
        })
        .collect()
}

/// The extrusion's body as the replay made it (3e-4 larger than the
/// file's): kept, not the file's.
fn extrusion_kept(report: &DesignReport) -> bool {
    report.bodies.iter().any(|b| {
        b.mitcad.as_deref().is_some_and(|m| m.starts_with("F2.b0"))
            && b.volume_difference.is_some_and(|d| d != 0.0)
    })
}

#[test]
fn a_fallback_keeps_the_bodies_within_the_tolerance() {
    // Fillet1 cannot be replayed: its state, the block and a second body,
    // comes in by its fallback, which adds the second body and keeps the
    // block (before mitcad#285 it replaced the block too).
    let both = shapes(&reference(vec![extrusion_of(SMALL), extrusion_of(LARGE)]));
    let (report, doc) = after_an_approximate_extrusion(Some(fillet_item(2)), both.clone(), false);
    let text = report.text();
    assert_eq!(report.items[1].outcome, Outcome::Parametric, "{text}");
    assert_eq!(report.items[2].outcome, Outcome::Fallback, "{text}");
    assert_eq!(bases_replace(&doc), vec![Vec::<BodyUid>::new()], "{text}");
    assert_eq!(report.bodies.len(), 2, "{text}");
    assert!(extrusion_kept(&report), "{text}");
    // So do the stored bodies at the end.
    let (report, doc) = after_an_approximate_extrusion(None, both, false);
    let text = report.text();
    assert_eq!(bases_replace(&doc), vec![Vec::<BodyUid>::new()], "{text}");
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("replaces the bodies that differ")),
        "{text}"
    );
    assert!(extrusion_kept(&report), "{text}");
}

#[test]
fn an_item_that_failed_in_the_file_keeps_the_bodies_within_the_tolerance() {
    // Extrude2 changed no body in the file: skipped, and the block, within
    // the tolerance of the state, stays as the replay made it, without a
    // base feature.
    let mut join = extrude(2, 0, "JoinFeatureOperation", "d2");
    join["name"] = json!("Extrude2");
    let block = shapes(&reference(vec![extrusion_of(SMALL)]));
    let (report, doc) = after_an_approximate_extrusion(Some(join), block, true);
    let text = report.text();
    assert_eq!(report.items[2].outcome, Outcome::Skipped, "{text}");
    assert!(
        report.items[2]
            .note
            .as_deref()
            .is_some_and(|n| n.contains("failed in the file")),
        "{text}"
    );
    assert!(bases_replace(&doc).is_empty(), "{text}");
    assert!(extrusion_kept(&report), "{text}");
}

/// A block and Fillet1 on one of its edges, which takes 1 mm³ from it, its
/// stored state measuring `noise` mm³ less (as a stored rounding whose edges
/// lie off its faces measures, mitcad#235), imported on a kernel that lists
/// new faces and measures changes by their faces with `slack` mm³, where
/// given.
fn noisy_rounding(noise: f64, slack: Option<f64>) -> DesignReport {
    let mut items = vec![
        rectangle_sketch(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let exact = || TestKernel {
        edge_points: true,
        fillet_takes: Some(1.0),
        ..TestKernel::default()
    };
    let mut reference = Document::new(exact());
    import_design(
        &mut reference,
        &timeline_dump(items.clone()),
        &mut NoGeometry,
        &Options::default(),
    );
    let block = shapes(&reference);
    let (a, b) = block[0].edges[0].clone();
    let edge = EdgeName::new(a.clone(), b.clone());
    let set = FilletSet {
        edges: std::slice::from_ref(&edge),
        faces: &[],
        size: FilletSize::Constant { radius: 2.0 },
        tangent_chain: true,
        curvature: false,
        weight: 0.0,
    };
    let stored = TestKernel {
        noisy_fillets: true,
        ..exact()
    }
    .fillet(FeatureUid(3), &block[0], &[set], true)
    .unwrap();
    items.push(fillet_on(2, "Fillet1", &a, &b));
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), block, vec![stored]],
        items: HashMap::from([(1, 1), (2, 2)]),
        ..TestGeometry::default()
    };
    let mut doc = Document::new(TestKernel {
        lists_new_faces: true,
        fillet_noise: -noise,
        change_slack: slack,
        ..exact()
    });
    import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options::default(),
    )
}

#[test]
fn a_roundings_change_beyond_the_volumes_but_within_its_faces_slack_is_its_faces_to_judge() {
    // The stored state removes 30 % more than the rounding: by the bodies'
    // volumes that is another result, but the changed faces' edge
    // tolerances leave twice that open, so its faces decide; they lie on
    // the file's.
    let report = noisy_rounding(0.3, Some(0.6));
    let text = report.text();
    assert_eq!(report.items[2].outcome, Outcome::Parametric, "{text}");
    assert_eq!(report.items[2].verified, Some(true), "{text}");
    // Without the faces' measure, or with a slack that leaves the
    // difference beyond DIFFERENT, the volumes decide.
    for slack in [None, Some(0.1)] {
        let report = noisy_rounding(0.3, slack);
        let text = report.text();
        assert_ne!(report.items[2].outcome, Outcome::Parametric, "{text}");
        let note = report.items[2].note.clone().unwrap_or_default();
        assert!(note.contains("off the file's"), "{text}");
    }
}
