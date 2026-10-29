// SPDX-License-Identifier: MIT
//! Regions of many loops (mitcad#678): the nesting test of the loops of
//! disjoint parts tries only the faces whose bounds hold the probe.

use std::time::{Duration, Instant};

use super::EntityUid;
use super::geometry::Curve2;
use super::regions::{Region, RegionCurve, regions};

/// `side` × `side` disjoint circles of radius 2 on a 10 mm grid, the
/// curves c1, c2, ... row by row.
fn circle_grid(side: usize) -> Vec<RegionCurve> {
    (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) as f64 * 10.0, (i / side) as f64 * 10.0);
            RegionCurve::new(
                EntityUid(i as u32 + 1),
                Curve2::Circle {
                    center: [x, y],
                    radius: 2.0,
                    start: 0.0,
                },
            )
        })
        .collect()
}

/// A rectangle of four lines (curves `first`..`first + 3`) from `lo` to `hi`.
fn rectangle(first: u32, lo: [f64; 2], hi: [f64; 2]) -> Vec<RegionCurve> {
    let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
    (0..4)
        .map(|k| {
            RegionCurve::new(
                EntityUid(first + k as u32),
                Curve2::Line {
                    a: corners[k],
                    b: corners[(k + 1) % 4],
                },
            )
        })
        .collect()
}

fn keys(regions: &[Region]) -> Vec<String> {
    regions.iter().map(|r| r.profile.key.to_string()).collect()
}

fn sorted_circle_keys(n: usize) -> Vec<String> {
    let mut keys: Vec<String> = (1..=n).map(|i| format!("r{{c{i}}}")).collect();
    keys.sort();
    keys
}

#[test]
fn four_hundred_disjoint_circles_are_four_hundred_regions() {
    let found = regions(&circle_grid(20));
    assert_eq!(found.len(), 400);
    let mut names = keys(&found);
    names.sort();
    assert_eq!(names, sorted_circle_keys(400));
    for region in &found {
        assert_eq!(region.profile.loops.len(), 1, "{}", region.profile.key);
        assert!(region.holes.is_empty(), "{}", region.profile.key);
        assert!((region.area - 4.0 * std::f64::consts::PI).abs() < 1e-6);
    }
}

#[test]
fn circles_inside_a_rectangle_are_its_holes() {
    let mut curves = circle_grid(20);
    curves.extend(rectangle(401, [-5.0, -5.0], [195.0, 195.0]));
    // A circle around one circle of the grid: that circle is its hole,
    // not the rectangle's.
    curves.push(RegionCurve::new(
        EntityUid(405),
        Curve2::Circle {
            center: [50.0, 50.0],
            radius: 4.0,
            start: 0.0,
        },
    ));
    let found = regions(&curves);
    assert_eq!(found.len(), 402);
    let outer = found
        .iter()
        .find(|r| r.profile.loops[0].segments.len() == 4)
        .expect("the rectangle's region");
    assert_eq!(outer.profile.loops.len(), 1 + 399 + 1);
    assert!(
        (outer.area
            - (200.0 * 200.0 - 399.0 * 4.0 * std::f64::consts::PI - 16.0 * std::f64::consts::PI))
            .abs()
            < 1e-6
    );
    let ring = found
        .iter()
        .find(|r| r.profile.key.to_string() == "r{c405}")
        .expect("the ring");
    assert_eq!(ring.profile.loops.len(), 2);
    assert_eq!(ring.profile.loops[1].segments[0].key.to_string(), "c106");
}

/// The time of the regions of `side` × `side` circles, the median of
/// `runs`.
fn median_time(side: usize, runs: usize) -> Duration {
    let curves = circle_grid(side);
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            let found = regions(&curves);
            let time = start.elapsed();
            assert_eq!(found.len(), side * side);
            time
        })
        .collect();
    times.sort();
    times[runs / 2]
}

/// A guard against the nesting test growing with the square of the loops
/// again (mitcad#678): 400 circles take about 5 ms in a release build
/// (800 ms before), the limits leave room for a loaded machine.
#[test]
fn the_regions_of_many_circles_are_fast() {
    let limit = if cfg!(debug_assertions) {
        Duration::from_millis(2000)
    } else {
        Duration::from_millis(100)
    };
    let time = median_time(20, 3);
    assert!(time < limit, "400 circles took {time:?}");
}

/// Prints the times of 25, 100 and 400 circles:
/// `cargo test --release -p mitcad-model regions_timings -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement"]
fn regions_timings() {
    for side in [5, 10, 20] {
        let time = median_time(side, 3);
        println!(
            "regions of {} circles: {:.3} ms",
            side * side,
            time.as_secs_f64() * 1000.0
        );
    }
}

/// `count` disjoint 4 × 4 rectangles in a row, each of four lines, the
/// curves from c`first` on.
fn rectangle_row(first: u32, count: u32) -> Vec<RegionCurve> {
    (0..count)
        .flat_map(|k| {
            let x = 10.0 * f64::from(k);
            rectangle(first + 4 * k, [x, 0.0], [x + 4.0, 4.0])
        })
        .collect()
}

/// The region keys and areas, sorted by key.
fn keys_and_areas(regions: &[Region]) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = regions
        .iter()
        .map(|r| (r.profile.key.to_string(), (r.area * 1e6).round() as i64))
        .collect();
    out.sort();
    out
}

/// A line drawn twice among many rectangles (mitcad#889: the repeats are
/// found among the pieces with the same ends): the lower curve id stays,
/// as when it is the only one.
#[test]
fn a_line_drawn_twice_among_many_rectangles_keeps_the_lower_id() {
    let plain = rectangle_row(10, 60);
    // Rectangle 7's bottom line again with a higher id: it is left out.
    let mut higher = plain.clone();
    let bottom = higher[4 * 7].curve.clone();
    higher.push(RegionCurve::new(EntityUid(1000), bottom));
    assert_eq!(
        keys_and_areas(&regions(&higher)),
        keys_and_areas(&regions(&plain))
    );
    // Rectangle 30's top line again with a lower id (the other way round):
    // it stays and the higher one is left out.
    let mut lower = plain.clone();
    let top = lower[4 * 30 + 2].curve.clone();
    let reversed = match top {
        Curve2::Line { a, b } => Curve2::Line { a: b, b: a },
        other => other,
    };
    lower.push(RegionCurve::new(EntityUid(1), reversed.clone()));
    let mut expected = plain.clone();
    expected[4 * 30 + 2] = RegionCurve::new(EntityUid(1), reversed);
    let found = regions(&lower);
    assert_eq!(found.len(), 60);
    assert_eq!(keys_and_areas(&found), keys_and_areas(&regions(&expected)));
}

/// A line that ends inside many rectangles' region cuts a side in two
/// pieces; the line dangles and is dropped, and the pieces of the side are
/// one again (mitcad#889: each vertex looks only at its own edges).
#[test]
fn a_dangling_line_leaves_the_side_it_cut_whole() {
    let plain = rectangle_row(10, 60);
    for k in [0_u32, 41, 59] {
        let x = 10.0 * f64::from(k);
        let mut cut = plain.clone();
        // From the middle of the bottom side up into the rectangle.
        cut.push(RegionCurve::new(
            EntityUid(1000),
            Curve2::Line {
                a: [x + 2.0, 0.0],
                b: [x + 2.0, 1.5],
            },
        ));
        let found = regions(&cut);
        assert_eq!(found.len(), 60);
        assert_eq!(keys_and_areas(&found), keys_and_areas(&regions(&plain)));
    }
}
