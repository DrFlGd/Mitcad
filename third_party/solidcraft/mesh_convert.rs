// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 SolidCraft contributors
//
// Adapted from SolidCraft (https://github.com/tangency-labs/solidcraft, commit
// 0b787f8eae4244cd40f07dddccd2e1b27e4cf9f8): the reconstruction of
// constant-section meshes of `crates/kernel/src/mesh_convert.rs`
// (`convert_prismatic_mesh`, `weld`, `extrusion`, `certify_nesting`,
// `fit_loop`). Changes: the mesh is Mitcad's `TriangleMesh` of `[f64; 3]`
// points, with small vector helpers of its own; errors are `String`s; the
// result is the section (a frame, the height and regions of polygons and
// circles), which the caller extrudes with Mitcad's kernel, names and checks
// (`core/model/src/features/mesh_convert.rs`), so the rebuilt volume compared
// with the mesh's is the section's area times the height; the nesting of the
// fitted loops is certified geometrically (raw loops may not meet, a fitted
// circle may not reach across another loop) instead of by planar booleans;
// polygons come out counter-clockwise for outer loops and clockwise for holes,
// each from its lowest corner.
//
// Used under the MIT licence; see `LICENSE-MIT` and `README.md` here.

//! A closed mesh of one profile extruded straight, rebuilt as its section:
//! the extrusion direction is one of the triangles' normals (the 16 that
//! cover most area are tried), the triangles at the lowest and the highest
//! level along it are the caps, and every other triangle must stand upright
//! on the section. The caps' boundaries, projected onto the plane across the
//! direction, must be the same loops, and every side vertex must lie on
//! them; so twisted, tapered and stepped meshes are refused before any
//! fitting. A loop of at least 12 corners on one circle, each chord at most
//! 30 degrees and within the tolerance of the arc, is that circle; other
//! loops lose their corners that lie in a straight line. Nothing is
//! converted unless the section's volume agrees with the mesh's within the
//! tolerance along its boundary.

use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

use crate::exchange::TriangleMesh;
use crate::profile::SketchFrame;

/// Triangles a mesh may have.
pub const MAX_TRIANGLES: usize = 50_000;
/// Boundary vertices a section may have.
const MAX_BOUNDARY: usize = 8192;
/// Boundary loops a section may have: certifying their nesting compares
/// every pair (50 loops, an outline with 49 holes, are 1225 pairs).
pub const MAX_LOOPS: usize = 50;
/// The smallest tolerance, mm.
pub const MIN_TOLERANCE: f64 = 1e-6;
/// The largest tolerance, mm.
pub const MAX_TOLERANCE: f64 = 1.0;

type P3 = [f64; 3];
type P2 = [f64; 2];

/// A boundary loop of a section, in its frame's coordinates (mm).
#[derive(Debug, Clone, PartialEq)]
pub enum SectionLoop {
    /// Corners in order around the loop (three or more), straight edges
    /// between them.
    Polygon(Vec<P2>),
    /// A whole circle.
    Circle { center: P2, radius: f64 },
}

impl SectionLoop {
    /// The area inside the loop.
    pub fn area(&self) -> f64 {
        match self {
            Self::Polygon(points) => signed_area(points).abs(),
            Self::Circle { radius, .. } => PI * radius * radius,
        }
    }

    /// The loop's length.
    pub fn length(&self) -> f64 {
        match self {
            Self::Polygon(points) => {
                let n = points.len();
                (0..n).map(|i| dist2(points[i], points[(i + 1) % n])).sum()
            }
            Self::Circle { radius, .. } => TAU * radius,
        }
    }

    /// Whether the point lies inside the loop.
    pub fn contains(&self, point: P2) -> bool {
        match self {
            Self::Polygon(points) => polygon_contains(points, point),
            Self::Circle { center, radius } => dist2(*center, point) < *radius,
        }
    }

    /// The loop counter-clockwise (or clockwise) from its lowest corner
    /// (the smallest x, then y).
    fn oriented(&self, counter_clockwise: bool) -> Self {
        match self {
            Self::Circle { .. } => self.clone(),
            Self::Polygon(points) => {
                let mut points = points.clone();
                if (signed_area(&points) > 0.0) != counter_clockwise {
                    points.reverse();
                }
                let first = (0..points.len())
                    .min_by(|&i, &j| {
                        points[i][0]
                            .total_cmp(&points[j][0])
                            .then(points[i][1].total_cmp(&points[j][1]))
                    })
                    .unwrap_or(0);
                points.rotate_left(first);
                Self::Polygon(points)
            }
        }
    }
}

/// A region of a section: its outer loop and its holes.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionRegion {
    pub outer: SectionLoop,
    pub holes: Vec<SectionLoop>,
}

impl SectionRegion {
    pub fn area(&self) -> f64 {
        self.outer.area() - self.holes.iter().map(SectionLoop::area).sum::<f64>()
    }
}

/// A mesh's section: the regions in the frame's plane, extruded along the
/// frame's normal from the plane by `height`.
#[derive(Debug, Clone, PartialEq)]
pub struct Prism {
    pub frame: SketchFrame,
    pub height: f64,
    pub regions: Vec<SectionRegion>,
}

impl Prism {
    pub fn volume(&self) -> f64 {
        self.regions.iter().map(SectionRegion::area).sum::<f64>() * self.height
    }
}

/// A mesh over the triangle budget: how big it is, the limit, and that only
/// constant-section parts convert, so a smaller freeform mesh would still
/// be refused.
fn over_budget(triangles: usize) -> String {
    format!(
        "the mesh has {triangles} triangles, more than the {MAX_TRIANGLES} conversion can \
         take; only closed constant-section parts (one profile extruded straight) can be \
         converted, so a freeform mesh is refused at any size"
    )
}

/// The section of a closed, consistently oriented mesh of one profile
/// extruded straight: planes and, where certified within `tolerance` mm,
/// circles. The mesh is not changed. A mesh of any other shape is refused
/// instead of being returned as facets.
pub fn convert_prismatic_mesh(mesh: &TriangleMesh, tolerance: f64) -> Result<Prism, String> {
    if !tolerance.is_finite() || !(MIN_TOLERANCE..=MAX_TOLERANCE).contains(&tolerance) {
        return Err(format!(
            "the tolerance must be between 0.000001 and 1 mm, got {tolerance}"
        ));
    }
    let (positions, triangles) = weld(mesh, tolerance)?;
    if triangles.is_empty() {
        return Err("the mesh has no triangles".to_owned());
    }
    let mut edges: BTreeMap<(u32, u32), (usize, i32)> = BTreeMap::new();
    let mut axes: BTreeMap<(i64, i64, i64), (P3, f64)> = BTreeMap::new();
    for &[a, b, c] in &triangles {
        let (Some(&pa), Some(&pb), Some(&pc)) = (
            positions.get(a as usize),
            positions.get(b as usize),
            positions.get(c as usize),
        ) else {
            return Err("a triangle refers to a missing vertex".to_owned());
        };
        let cross = cross3(sub3(pb, pa), sub3(pc, pa));
        let area = len3(cross);
        if !area.is_finite() || area <= 0.0 {
            return Err("the mesh has a degenerate triangle".to_owned());
        }
        for (a, b) in [(a, b), (b, c), (c, a)] {
            let entry = edges.entry((a.min(b), a.max(b))).or_default();
            entry.0 += 1;
            entry.1 += if a < b { 1 } else { -1 };
        }
        // Opposite normals are one direction.
        let mut n = scale3(cross, 1.0 / area);
        if n[0] < -1e-8
            || (n[0].abs() <= 1e-8 && (n[1] < -1e-8 || (n[1].abs() <= 1e-8 && n[2] < 0.0)))
        {
            n = scale3(n, -1.0);
        }
        let key = (
            (n[0] * 1e7).round() as i64,
            (n[1] * 1e7).round() as i64,
            (n[2] * 1e7).round() as i64,
        );
        axes.entry(key).or_insert((n, 0.0)).1 += area;
    }
    if edges
        .values()
        .any(|&(count, balance)| count != 2 || balance != 0)
    {
        return Err(
            "the mesh must be closed and consistently oriented, with two triangles at every edge"
                .to_owned(),
        );
    }
    let mut axes: Vec<(P3, f64)> = axes.into_values().collect();
    axes.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (axis, _) in axes.into_iter().take(16) {
        if let Some(prism) = extrusion(&positions, &triangles, axis, tolerance)? {
            return Ok(prism);
        }
    }
    Err(
        "the mesh is not a supported constant-section prism (one profile extruded straight); \
         no geometry was converted"
            .to_owned(),
    )
}

/// How near points are one point: a ten-millionth of the mesh's size, at
/// most a hundredth of the tolerance.
fn epsilon_of(size: f64, tolerance: f64) -> f64 {
    (size.max(1.0) * 1e-7).min(tolerance * 0.01).max(1e-9)
}

/// The diagonal of the points' box (0 without points).
fn diagonal(points: &[P3]) -> f64 {
    let Some(&first) = points.first() else {
        return 0.0;
    };
    let (mut min, mut max) = (first, first);
    for p in points {
        min = std::array::from_fn(|k| min[k].min(p[k]));
        max = std::array::from_fn(|k| max[k].max(p[k]));
    }
    len3(sub3(max, min))
}

/// The mesh with its vertices that lie within the conversion's epsilon of
/// each other made one.
fn weld(mesh: &TriangleMesh, tolerance: f64) -> Result<(Vec<P3>, Vec<[u32; 3]>), String> {
    if mesh.triangles.len() > MAX_TRIANGLES {
        return Err(over_budget(mesh.triangles.len()));
    }
    if mesh.vertices.len() > MAX_TRIANGLES * 3 {
        return Err(format!(
            "the mesh has {} vertices, more than the {} conversion can take",
            mesh.vertices.len(),
            MAX_TRIANGLES * 3
        ));
    }
    let epsilon = epsilon_of(diagonal(&mesh.vertices), tolerance);
    if !epsilon.is_finite()
        || mesh
            .vertices
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || (v / epsilon).abs() > 1e15)
    {
        return Err("the mesh's coordinates exceed what the conversion can take".to_owned());
    }
    let key = |p: P3| {
        (
            (p[0] / epsilon).floor() as i64,
            (p[1] / epsilon).floor() as i64,
            (p[2] / epsilon).floor() as i64,
        )
    };
    let mut grid: BTreeMap<(i64, i64, i64), Vec<u32>> = BTreeMap::new();
    let mut positions: Vec<P3> = Vec::new();
    let mut remap: Vec<u32> = Vec::with_capacity(mesh.vertices.len());
    let mut comparisons = 0usize;
    for &p in &mesh.vertices {
        let k = key(p);
        let mut found = None;
        'cells: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &i in grid
                        .get(&(k.0 + dx, k.1 + dy, k.2 + dz))
                        .into_iter()
                        .flatten()
                    {
                        comparisons += 1;
                        if comparisons > 8_000_000 {
                            return Err(
                                "welding the mesh's vertices exceeds its work budget".to_owned()
                            );
                        }
                        if positions
                            .get(i as usize)
                            .is_some_and(|q| len3(sub3(*q, p)) <= epsilon)
                        {
                            found = Some(i);
                            break 'cells;
                        }
                    }
                }
            }
        }
        let i = match found {
            Some(i) => i,
            None => {
                let i = u32::try_from(positions.len())
                    .map_err(|_| "the mesh has too many vertices".to_owned())?;
                positions.push(p);
                grid.entry(k).or_default().push(i);
                i
            }
        };
        remap.push(i);
    }
    let triangles = mesh
        .triangles
        .iter()
        .map(|t| {
            let (Some(&a), Some(&b), Some(&c)) = (
                remap.get(t[0] as usize),
                remap.get(t[1] as usize),
                remap.get(t[2] as usize),
            ) else {
                return Err("a triangle refers to a missing vertex".to_owned());
            };
            Ok([a, b, c])
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((positions, triangles))
}

/// A frame whose normal is `axis` (a unit vector), its origin `lo` along
/// it from the model's origin.
fn frame_across(axis: P3, lo: f64) -> SketchFrame {
    let k = (0..3)
        .min_by(|&i, &j| axis[i].abs().total_cmp(&axis[j].abs()))
        .unwrap_or(0);
    let mut helper = [0.0; 3];
    helper[k] = 1.0;
    let x = sub3(helper, scale3(axis, dot3(helper, axis)));
    let x = scale3(x, 1.0 / len3(x));
    SketchFrame {
        origin: scale3(axis, lo),
        x_axis: x,
        y_axis: cross3(axis, x),
    }
}

fn grid_point(k: (i64, i64), epsilon: f64) -> P2 {
    [k.0 as f64 * epsilon, k.1 as f64 * epsilon]
}

/// The mesh as an extrusion along `axis`; None when it is not one.
fn extrusion(
    positions: &[P3],
    triangles: &[[u32; 3]],
    axis: P3,
    tolerance: f64,
) -> Result<Option<Prism>, String> {
    let (lo, hi) = positions
        .iter()
        .map(|p| dot3(*p, axis))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| {
            (lo.min(h), hi.max(h))
        });
    let height = hi - lo;
    let size = diagonal(positions);
    let epsilon = epsilon_of(size, tolerance);
    if !height.is_finite() || height <= epsilon {
        return Ok(None);
    }
    let frame = frame_across(axis, lo);
    let local = |p: P3| {
        let d = sub3(p, frame.origin);
        [dot3(d, frame.x_axis), dot3(d, frame.y_axis)]
    };
    let mut caps: [BTreeMap<(u32, u32), usize>; 2] = [BTreeMap::new(), BTreeMap::new()];
    for &[a, b, c] in triangles {
        let (Some(&pa), Some(&pb), Some(&pc)) = (
            positions.get(a as usize),
            positions.get(b as usize),
            positions.get(c as usize),
        ) else {
            return Err("a triangle refers to a missing vertex".to_owned());
        };
        let hs = [dot3(pa, axis), dot3(pb, axis), dot3(pc, axis)];
        let cap = if hs.iter().all(|h| (h - lo).abs() <= epsilon) {
            0
        } else if hs.iter().all(|h| (h - hi).abs() <= epsilon) {
            1
        } else {
            // A side triangle stands upright: its projection is a line. STL
            // keeps coordinates as f32, so the projected triangle's distance
            // from a line is tested, not an angle that the rounding of a
            // narrow triangle would make large.
            let [qa, qb, qc] = [local(pa), local(pb), local(pc)];
            let length = dist2(qa, qb).max(dist2(qb, qc)).max(dist2(qc, qa));
            if cross2(sub2(qb, qa), sub2(qc, qa)).abs() > epsilon * 3.0 * length {
                return Ok(None);
            }
            continue;
        };
        for (u, v) in [(a, b), (b, c), (c, a)] {
            *caps[cap].entry((u.min(v), u.max(v))).or_default() += 1;
        }
    }
    // The vertices projected onto the plane, those within epsilon one point.
    let raw_key = |p: P2| {
        (
            (p[0] / epsilon).floor() as i64,
            (p[1] / epsilon).floor() as i64,
        )
    };
    let mut points: BTreeMap<(i64, i64), P2> = BTreeMap::new();
    let mut projected: Vec<(i64, i64)> = Vec::with_capacity(positions.len());
    for p in positions {
        let p = local(*p);
        let k = raw_key(p);
        let mut canonical = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                let candidate = (k.0 + dx, k.1 + dy);
                if points
                    .get(&candidate)
                    .is_some_and(|q| dist2(*q, p) <= epsilon)
                {
                    canonical = Some(candidate);
                }
            }
        }
        let canonical = canonical.unwrap_or(k);
        points.entry(canonical).or_insert(p);
        projected.push(canonical);
    }
    type Adjacency = BTreeMap<(i64, i64), Vec<(i64, i64)>>;
    let boundary = |cap: &BTreeMap<(u32, u32), usize>| -> Option<Adjacency> {
        let mut adjacency: BTreeMap<(i64, i64), Vec<(i64, i64)>> = BTreeMap::new();
        for (&(a, b), &count) in cap {
            if count == 2 {
                continue;
            }
            if count != 1 {
                return None;
            }
            let (a, b) = (*projected.get(a as usize)?, *projected.get(b as usize)?);
            if a == b {
                return None;
            }
            adjacency.entry(a).or_default().push(b);
            adjacency.entry(b).or_default().push(a);
        }
        for neighbours in adjacency.values_mut() {
            neighbours.sort_unstable();
        }
        Some(adjacency)
    };
    let (Some(mut adjacency), Some(other)) = (boundary(&caps[0]), boundary(&caps[1])) else {
        return Ok(None);
    };
    if adjacency != other
        || adjacency.is_empty()
        || adjacency.len() > MAX_BOUNDARY
        || adjacency.values().any(|n| n.len() != 2)
    {
        return Ok(None);
    }
    // Every side vertex lies on an unchanged straight extrusion of the
    // caps' boundary: this refuses twisted, tapered and stepped meshes
    // before any circle is fitted.
    let mut segments: Vec<(P2, P2)> = Vec::new();
    for (&a, neighbours) in &adjacency {
        for &b in neighbours {
            if a < b {
                segments.push((grid_point(a, epsilon), grid_point(b, epsilon)));
            }
        }
    }
    if positions.len().saturating_mul(segments.len()) > 32_000_000 {
        return Err("certifying the section's boundary exceeds its work budget".to_owned());
    }
    for p in positions {
        let h = dot3(*p, axis);
        if (h - lo).abs() <= epsilon || (h - hi).abs() <= epsilon {
            continue;
        }
        let q = local(*p);
        if !segments
            .iter()
            .any(|&(a, b)| segment_distance(q, a, b) <= epsilon * 3.0)
        {
            return Ok(None);
        }
    }
    // The boundary's loops.
    let mut raw: Vec<Vec<P2>> = Vec::new();
    let mut fitted: Vec<SectionLoop> = Vec::new();
    while let Some((&start, _)) = adjacency.first_key_value() {
        let mut current = start;
        let mut previous = None;
        let mut chain: Vec<P2> = Vec::new();
        for _ in 0..=MAX_BOUNDARY {
            let Some(neighbours) = adjacency.remove(&current) else {
                return Ok(None);
            };
            chain.push(
                points
                    .get(&current)
                    .copied()
                    .unwrap_or_else(|| grid_point(current, epsilon)),
            );
            let Some(next) = neighbours.into_iter().find(|n| Some(*n) != previous) else {
                return Ok(None);
            };
            if next == start {
                break;
            }
            previous = Some(current);
            current = next;
        }
        if chain.len() < 3 || current == start {
            return Ok(None);
        }
        fitted.push(fit_loop(&chain, tolerance, epsilon));
        raw.push(chain);
    }
    certify_nesting(&raw, &fitted, epsilon)?;
    // Nested by the raw loops, so that fitting cannot change which loop is
    // a hole: inside an even number of loops an outer loop, else a hole of
    // the smallest region around it.
    let owners = |i: usize| {
        raw.iter()
            .enumerate()
            .filter(|(j, other)| *j != i && polygon_contains(other, raw[i][0]))
            .count()
    };
    let mut regions: Vec<SectionRegion> = Vec::new();
    for (i, fit) in fitted.iter().enumerate() {
        if owners(i) % 2 == 0 {
            regions.push(SectionRegion {
                outer: fit.oriented(true),
                holes: Vec::new(),
            });
        }
    }
    for (i, fit) in fitted.iter().enumerate() {
        if owners(i) % 2 == 1 {
            let probe = raw[i][0];
            let owner = regions
                .iter()
                .enumerate()
                .filter(|(_, r)| r.outer.contains(probe))
                .min_by(|(_, a), (_, b)| a.outer.area().total_cmp(&b.outer.area()))
                .map(|(k, _)| k);
            let Some(region) = owner.and_then(|k| regions.get_mut(k)) else {
                return Ok(None);
            };
            region.holes.push(fit.oriented(false));
        }
    }
    let perimeter: f64 = fitted.iter().map(SectionLoop::length).sum();
    let original = mesh_volume(positions, triangles);
    if !original.is_finite() || original <= 0.0 {
        return Err(
            "the mesh encloses no volume (its triangles may face inwards); no geometry was \
             converted"
                .to_owned(),
        );
    }
    let prism = Prism {
        frame,
        height,
        regions,
    };
    if (prism.volume() - original).abs() > perimeter * height * tolerance + epsilon * size * size {
        return Err(
            "the rebuilt volume does not agree with the mesh's; no geometry was converted"
                .to_owned(),
        );
    }
    Ok(Some(prism))
}

/// Refuses loops that overlap, meet, or that fitting would make cross or
/// merge: a circle that reaches across a nearby wall.
pub(super) fn certify_nesting(
    raw: &[Vec<P2>],
    fitted: &[SectionLoop],
    epsilon: f64,
) -> Result<(), String> {
    if raw.len() > MAX_LOOPS {
        return Err(format!(
            "the mesh is too complex to convert ({} boundary loops; at most {MAX_LOOPS} are \
             supported)",
            raw.len()
        ));
    }
    let crossing = || "the conversion would cross or merge boundary loops".to_owned();
    let mut work = 0usize;
    for (i, a) in raw.iter().enumerate() {
        for (j, b) in raw.iter().enumerate().skip(i + 1) {
            work = work.saturating_add(a.len().saturating_mul(b.len()));
            if work > 16_000_000 {
                return Err("certifying the boundary loops exceeds its work budget".to_owned());
            }
            let (Some(&a0), Some(&b0)) = (a.first(), b.first()) else {
                return Err("an empty boundary loop".to_owned());
            };
            let (ab, ba) = (polygon_contains(a, b0), polygon_contains(b, a0));
            if ab && ba {
                return Err("the mesh's boundary loops overlap".to_owned());
            }
            if polygons_meet(a, b, epsilon) {
                return Err(crossing());
            }
            let (Some(fa), Some(fb)) = (fitted.get(i), fitted.get(j)) else {
                return Err("a fitted loop is missing".to_owned());
            };
            let kept = if ab {
                loop_inside(fb, fa)
            } else if ba {
                loop_inside(fa, fb)
            } else {
                loops_apart(fa, fb)
            };
            if !kept {
                return Err(crossing());
            }
        }
    }
    Ok(())
}

/// Whether the fitted `inner` lies inside the fitted `outer`. Two polygons
/// are their raw loops less corners in a line, which do not meet.
fn loop_inside(inner: &SectionLoop, outer: &SectionLoop) -> bool {
    match (inner, outer) {
        (SectionLoop::Polygon(_), SectionLoop::Polygon(_)) => true,
        (
            SectionLoop::Circle {
                center: c1,
                radius: r1,
            },
            SectionLoop::Circle {
                center: c2,
                radius: r2,
            },
        ) => dist2(*c1, *c2) + r1 <= *r2,
        (SectionLoop::Polygon(points), SectionLoop::Circle { center, radius }) => {
            points.iter().all(|p| dist2(*p, *center) <= *radius)
        }
        (SectionLoop::Circle { center, radius }, SectionLoop::Polygon(points)) => {
            polygon_contains(points, *center) && edges_clear_of(points, *center, *radius)
        }
    }
}

/// Whether the fitted loops lie outside each other.
fn loops_apart(a: &SectionLoop, b: &SectionLoop) -> bool {
    match (a, b) {
        (SectionLoop::Polygon(_), SectionLoop::Polygon(_)) => true,
        (
            SectionLoop::Circle {
                center: c1,
                radius: r1,
            },
            SectionLoop::Circle {
                center: c2,
                radius: r2,
            },
        ) => dist2(*c1, *c2) >= r1 + r2,
        (SectionLoop::Polygon(points), SectionLoop::Circle { center, radius })
        | (SectionLoop::Circle { center, radius }, SectionLoop::Polygon(points)) => {
            !polygon_contains(points, *center) && edges_clear_of(points, *center, *radius)
        }
    }
}

/// Whether every edge of the polygon is at least `radius` from `center`.
fn edges_clear_of(points: &[P2], center: P2, radius: f64) -> bool {
    let n = points.len();
    (0..n).all(|i| segment_distance(center, points[i], points[(i + 1) % n]) >= radius)
}

/// Whether two polygons' edges come within epsilon of each other.
fn polygons_meet(a: &[P2], b: &[P2], epsilon: f64) -> bool {
    let (n, m) = (a.len(), b.len());
    (0..n).any(|i| {
        (0..m).any(|j| segments_distance(a[i], a[(i + 1) % n], b[j], b[(j + 1) % m]) < epsilon)
    })
}

/// A loop's corners as a circle when there are enough of them (12) to tell
/// it from a polygon, all on one circle, each chord at most 30 degrees and
/// its deviation from the arc within the tolerance; else the polygon less
/// its corners that lie in a line.
pub(super) fn fit_loop(points: &[P2], tolerance: f64, epsilon: f64) -> SectionLoop {
    let n = points.len();
    if n >= 12 {
        let (a, b, c) = (points[0], points[n / 3], points[2 * n / 3]);
        let (u, v) = (sub2(b, a), sub2(c, a));
        let d = 2.0 * cross2(u, v);
        if d.abs() > epsilon * epsilon {
            let (uu, vv) = (dot2(u, u), dot2(v, v));
            let center = [
                a[0] + (v[1] * uu - u[1] * vv) / d,
                a[1] + (u[0] * vv - v[0] * uu) / d,
            ];
            let radius = dist2(center, a);
            if radius.is_finite()
                && radius > epsilon
                && points
                    .iter()
                    .all(|p| (dist2(center, *p) - radius).abs() <= epsilon * 3.0)
            {
                let mut sweep = 0.0;
                let mut direction = 0.0_f64;
                let valid = (0..n).all(|i| {
                    let (p, q) = (sub2(points[i], center), sub2(points[(i + 1) % n], center));
                    let signed = cross2(p, q).atan2(dot2(p, q));
                    if direction == 0.0 {
                        direction = signed.signum();
                    }
                    if signed * direction <= 0.0 {
                        return false;
                    }
                    sweep += signed;
                    let angle = signed.abs();
                    angle <= PI / 6.0 + 1e-8
                        && radius * (1.0 - (angle * 0.5).cos()) + epsilon * 10.0 <= tolerance
                });
                if valid && (sweep.abs() - TAU).abs() <= 1e-7 {
                    return SectionLoop::Circle { center, radius };
                }
            }
        }
    }
    let mut points = points.to_vec();
    let linear = epsilon / n.max(1) as f64;
    loop {
        let m = points.len();
        if m <= 3 {
            break;
        }
        let Some(index) = (0..m).find(|&i| {
            segment_distance(points[i], points[(i + m - 1) % m], points[(i + 1) % m]) <= linear
        }) else {
            break;
        };
        points.remove(index);
    }
    SectionLoop::Polygon(points)
}

/// The volume a closed mesh encloses (negative when its triangles face
/// inwards).
fn mesh_volume(positions: &[P3], triangles: &[[u32; 3]]) -> f64 {
    triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| positions.get(i as usize).copied().unwrap_or([0.0; 3]));
            dot3(a, cross3(b, c)) / 6.0
        })
        .sum()
}

fn signed_area(points: &[P2]) -> f64 {
    let n = points.len();
    0.5 * (0..n)
        .map(|i| cross2(points[i], points[(i + 1) % n]))
        .sum::<f64>()
}

/// Whether the point lies inside the polygon (even-odd).
fn polygon_contains(points: &[P2], q: P2) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for (i, &a) in points.iter().enumerate() {
        let b = points[j];
        if (a[1] > q[1]) != (b[1] > q[1]) {
            let x = a[0] + (q[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if q[0] < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// The distance from `p` to the segment from `a` to `b`.
fn segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let ab = sub2(b, a);
    let l2 = dot2(ab, ab);
    let t = if l2 > 0.0 {
        (dot2(sub2(p, a), ab) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist2(p, [a[0] + ab[0] * t, a[1] + ab[1] * t])
}

/// The distance between two segments (0 where they cross).
fn segments_distance(a: P2, b: P2, c: P2, d: P2) -> f64 {
    let o1 = cross2(sub2(b, a), sub2(c, a));
    let o2 = cross2(sub2(b, a), sub2(d, a));
    let o3 = cross2(sub2(d, c), sub2(a, c));
    let o4 = cross2(sub2(d, c), sub2(b, c));
    if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
        return 0.0;
    }
    segment_distance(a, c, d)
        .min(segment_distance(b, c, d))
        .min(segment_distance(c, a, b))
        .min(segment_distance(d, a, b))
}

fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scale3(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn len3(a: P3) -> f64 {
    dot3(a, a).sqrt()
}

fn sub2(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot2(a: P2, b: P2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn cross2(a: P2, b: P2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dist2(a: P2, b: P2) -> f64 {
    let d = sub2(a, b);
    dot2(d, d).sqrt()
}
