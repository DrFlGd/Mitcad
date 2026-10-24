// SPDX-License-Identifier: MIT
//! Definitions evaluated ahead in parallel (mitcad#95) and work beside the replay (mitcad#103).

use super::*;

// An item's definitions evaluated ahead, in parallel (mitcad#95).

/// One profile of the two rectangles extruded as a new body (as
/// [`slow_profile_import`]) against the file's state `right` on `kernel`,
/// with `threads`: the report, each feature's name and definition, the
/// seconds the import took, and the kernel (whose forks count the slow
/// extrusions stopped in its `stopped`).
fn ahead_import(
    threads: usize,
    known: bool,
    right: (&str, f64, bool),
    kernel: impl Fn() -> TestKernel,
    item_seconds: f64,
) -> (
    DesignReport,
    Vec<Value>,
    f64,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let items = vec![
        two_rectangles(0),
        extrude(1, 0, "NewBodyFeatureOperation", "d2"),
    ];
    let mut reference = Document::new(TestKernel {
        slow: None,
        ..kernel()
    });
    import_design(
        &mut reference,
        &timeline_dump(items[..1].to_vec()),
        &mut NoGeometry,
        &Options::default(),
    );
    let (region, distance, flip) = right;
    add(
        &mut reference,
        json!({"type": "extrude", "profiles": [{"sketch": "F1", "region": region}],
               "extent": {"type": "distance", "distance": distance}, "flip": flip,
               "operation": "new_body"}),
    );
    let mut geometry = TestGeometry {
        states: vec![Vec::new(), shapes(&reference)],
        items: if known {
            HashMap::from([(1, 1)])
        } else {
            HashMap::new()
        },
        ..TestGeometry::default()
    };
    let mut doc = Document::new(kernel());
    let started = std::time::Instant::now();
    let report = import_design(
        &mut doc,
        &timeline_dump(items),
        &mut geometry,
        &Options {
            item_seconds,
            threads,
            ..Options::default()
        },
    );
    let elapsed = started.elapsed().as_secs_f64();
    let features = doc
        .features()
        .map(|f| f.uid)
        .collect::<Vec<_>>()
        .into_iter()
        .map(|uid| {
            let text = doc
                .query(&format!(r#"{{"query": "feature", "uid": "{uid}"}}"#))
                .unwrap();
            serde_json::from_str::<Value>(&text).unwrap()
        })
        .collect();
    (report, features, elapsed, Arc::clone(&doc.kernel().stopped))
}

#[test]
fn definitions_evaluated_ahead_give_the_same_import() {
    // The history picks the profile and direction; with workers the
    // definitions after the one evaluated are evaluated ahead, and the one
    // taken (here a worker's, as it is not ranked first) comes in with the
    // same features, names and report as without them.
    let kernel = || TestKernel {
        directed: true,
        ..TestKernel::default()
    };
    for known in [true, false] {
        for right in [(LARGE, 10.0, false), (LARGE, 10.0, true)] {
            let (one, one_features, _, _) = ahead_import(1, known, right, kernel, 60.0);
            let (eight, eight_features, _, _) = ahead_import(8, known, right, kernel, 60.0);
            assert_eq!(
                one.items[1].outcome,
                Outcome::Parametric,
                "{right:?} {known}: {}",
                one.text()
            );
            assert_eq!(one.text(), eight.text());
            assert_eq!(
                serde_json::to_string(&one.items).unwrap(),
                serde_json::to_string(&eight.items).unwrap()
            );
            assert_eq!(one_features, eight_features);
        }
    }
}

#[test]
fn the_first_definition_that_gives_the_state_is_taken() {
    // Every extrusion measures the same here, so each definition gives the
    // state. The first one ranked takes a while; the workers have the
    // others done before it, but it is the one taken, as without workers.
    let kernel = || TestKernel {
        alike: true,
        directed: true,
        slow: Some(SlowExtrusions {
            regions: &[SMALL],
            along: true,
            against: false,
            seconds: 0.5,
            deaf: false,
        }),
        ..TestKernel::default()
    };
    for known in [true, false] {
        let (one, one_features, _, _) = ahead_import(1, known, (SMALL, 10.0, false), kernel, 60.0);
        let (eight, eight_features, _, _) =
            ahead_import(8, known, (SMALL, 10.0, false), kernel, 60.0);
        assert_eq!(one.items[1].verified, Some(true), "{}", one.text());
        assert_eq!(one.text(), eight.text());
        assert_eq!(one_features, eight_features);
        let def = &eight_features.last().unwrap()["def"];
        assert_eq!(def["profiles"][0]["region"], SMALL, "{}", eight.text());
        assert_ne!(def["flip"], true, "{}", eight.text());
    }
}

#[test]
fn definitions_after_the_one_taken_are_cut_short() {
    // The definitions of the small rectangle give no state, the third (the
    // large one along the sketch's normal) gives it, and the fourth
    // (against the normal) would take 30 s. Its worker is stopped as soon
    // as the third is taken, and the import does not wait for it.
    let began = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stopped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let kernel = || TestKernel {
        directed: true,
        slow: Some(SlowExtrusions {
            regions: &[LARGE],
            along: false,
            against: true,
            seconds: 30.0,
            deaf: false,
        }),
        began: Arc::clone(&began),
        stopped: Arc::clone(&stopped),
        ..TestKernel::default()
    };
    let (report, features, elapsed, _) = ahead_import(8, false, (LARGE, 10.0, false), kernel, 60.0);
    assert_eq!(report.items[1].verified, Some(true), "{}", report.text());
    let def = &features.last().unwrap()["def"];
    assert_eq!(def["profiles"][0]["region"], LARGE, "{}", report.text());
    assert_ne!(def["flip"], true, "{}", report.text());
    assert!(elapsed < 10.0, "{elapsed} s");
    // Its worker stops within a few milliseconds of the stop (unless the
    // stop came before its extrusion began).
    let load =
        |n: &Arc<std::sync::atomic::AtomicUsize>| n.load(std::sync::atomic::Ordering::SeqCst);
    let waited = std::time::Instant::now();
    while load(&stopped) < load(&began) && waited.elapsed().as_secs_f64() < 10.0 {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(load(&stopped), load(&began));
}

#[test]
fn a_definition_evaluated_ahead_that_does_not_stop_is_given_up() {
    // The third definition (the large rectangle along the sketch's normal),
    // evaluated ahead, runs 15 s in a kernel call that does not check for a
    // request to stop, as a fillet's contour round a closed chain did for
    // hours (mitcad#434); the fourth (against the normal) gives the state.
    // The importer waits for the third until its share of the item's 6 s
    // (2 s) and another share have passed, takes it as cut short at its
    // share, and the fourth comes in. Before, the import waited the 15 s
    // (or for ever).
    let kernel = || TestKernel {
        directed: true,
        slow: Some(SlowExtrusions {
            regions: &[LARGE],
            along: true,
            against: false,
            seconds: 15.0,
            deaf: true,
        }),
        ..TestKernel::default()
    };
    let (report, features, elapsed, _) = ahead_import(8, false, (LARGE, 10.0, true), kernel, 6.0);
    assert_eq!(report.items[1].verified, Some(true), "{}", report.text());
    let def = &features.last().unwrap()["def"];
    assert_eq!(def["profiles"][0]["region"], LARGE, "{}", report.text());
    assert_eq!(def["flip"], true, "{}", report.text());
    assert!(elapsed < 10.0, "{elapsed} s");
}

// Work beside the replay in parallel (mitcad#103).

#[test]
fn in_order_hands_the_results_over_in_order() {
    for threads in [1, 2, 8] {
        for count in [0, 1, 5, 40] {
            let mut seen = Vec::new();
            crate::in_order(
                count,
                threads,
                |k| {
                    // Later ones finish first.
                    std::thread::sleep(std::time::Duration::from_micros(((count - k) * 50) as u64));
                    k * k
                },
                |k, square| seen.push((k, square)),
            );
            let expected: Vec<(usize, usize)> = (0..count).map(|k| (k, k * k)).collect();
            assert_eq!(seen, expected, "{threads} threads, {count} items");
        }
    }
}

#[test]
fn in_order_passes_a_panic_on() {
    let caught = std::panic::catch_unwind(|| {
        crate::in_order(
            20,
            4,
            |k| {
                assert!(k != 7, "item 7");
                k
            },
            |_, _| {},
        );
    });
    assert!(caught.is_err());
}

#[test]
fn in_order_shares_its_threads_with_work_beside_it() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let share = crate::Share::new(4);
    let a = share.take(1).expect("four left");
    let b = share.take(1).expect("three left");
    let c = share.take(1).expect("two left");
    assert!(share.take(1).is_none(), "the workers keep one");
    drop(c);
    assert_eq!(share.left(), 2);
    // Two threads taken: at most two workers at once, until one is given
    // back half way.
    let (running, most, done) = (
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    );
    let given = std::sync::Mutex::new(Some(a));
    let mut seen = Vec::new();
    crate::in_order_sharing(
        40,
        4,
        Some(&share),
        |k| {
            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
            if done.load(Ordering::SeqCst) < 20 {
                most.fetch_max(now, Ordering::SeqCst);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
            running.fetch_sub(1, Ordering::SeqCst);
            if done.fetch_add(1, Ordering::SeqCst) + 1 == 20 {
                given.lock().unwrap().take();
            }
            k * 2
        },
        |k, twice| seen.push((k, twice)),
    );
    assert!(most.load(Ordering::SeqCst) <= 2, "{most:?}");
    assert_eq!(seen, (0..40).map(|k| (k, k * 2)).collect::<Vec<_>>());
    drop(b);
    assert_eq!(share.left(), 4);
}

#[test]
fn threads_beside_the_import_share_one_budget() {
    let progress = Progress::new();
    progress.set_threads(3);
    let a = progress.helper().expect("one of three");
    let b = progress.helper().expect("two of three");
    assert!(progress.helper().is_none(), "the import's own is the third");
    drop(a);
    let c = progress.helper().expect("given back");
    drop((b, c));
    // Not while memory is short.
    progress.set_memory_share(0.55);
    assert!(progress.helper().is_none());
    progress.set_memory_share(0.1);
    assert!(progress.helper().is_some());
    progress.set_threads(1);
    assert!(progress.helper().is_none(), "one thread: the import's own");
}
