// SPDX-License-Identifier: MIT
//! Faceted and advanced breps as the kernel's boundary bodies
//! ([`mitcad_model::boundary`]): the faces, their loops and the edges they
//! share, with each edge's curve and its parameters at its vertices.
//!
//! - Planar faces (faceted breps, surface models of poly loops): vertices
//!   shared by position, an edge per pair of vertices (a line), the outer
//!   loop of each face counter-clockwise about its normal (from the outer
//!   loop's own winding) and its holes the other way.
//! - Advanced faces: vertices and edges shared by the file's instances
//!   (IfcVertexPoint, IfcEdgeCurve); an edge whose curve is a polyline or a
//!   composite curve is an edge per piece. Circles and ellipses take the
//!   angles of the vertices, B-splines the parameters of the vertices'
//!   nearest points. A seam the file repeats as another instance (a closed
//!   edge along another at its vertex) is one edge, used once each way; a
//!   B-spline face closed both ways and bounded by its seams alone, or by
//!   the sides of its domain, is the whole surface.

use std::collections::HashMap;
use std::f64::consts::TAU;

use mitcad_ifc::geometry::{AdvancedFace, Path, Seg, Surface, de_boor, full_knots, spline_domain};
use mitcad_ifc::math::{self, Matrix};
use mitcad_model::boundary::{
    BoundaryBody, BoundaryCurve, BoundaryEdge, BoundaryFace, BoundarySurface,
};

/// Positions within this (mm) are one vertex.
const MERGE: f64 = 1e-6;

fn key(p: [f64; 3]) -> [i64; 3] {
    p.map(|c| (c / MERGE).round() as i64)
}

/// Planar faces (each a list of loops of points, the outer first) as a
/// boundary of one shell.
pub fn faceted(faces: &[Vec<Vec<[f64; 3]>>]) -> Result<BoundaryBody, String> {
    let mut body = BoundaryBody::default();
    let mut vertices: HashMap<[i64; 3], usize> = HashMap::new();
    // Each face's loops of vertex indices, the outer first.
    let mut loops_of: Vec<Vec<Vec<usize>>> = Vec::new();
    for face in faces {
        let mut loops = Vec::new();
        for points in face {
            let mut ids: Vec<usize> = points
                .iter()
                .map(|p| {
                    *vertices.entry(key(*p)).or_insert_with(|| {
                        body.vertices.push(*p);
                        body.vertices.len() - 1
                    })
                })
                .collect();
            ids.dedup();
            while ids.len() > 1 && ids.first() == ids.last() {
                ids.pop();
            }
            if ids.len() >= 3 {
                loops.push(ids);
            } else if loops.is_empty() {
                // A face without an outer loop of three points has no area.
                break;
            }
        }
        if !loops.is_empty() {
            loops_of.push(loops);
        }
    }
    split_t_junctions(&body.vertices, &mut loops_of);
    orient_faces(&body.vertices, &mut loops_of);
    let mut edges: HashMap<(usize, usize), usize> = HashMap::new();
    let mut shell = Vec::new();
    for loops in &loops_of {
        let outer: Vec<[f64; 3]> = loops[0].iter().map(|i| body.vertices[*i]).collect();
        let Some(normal) = newell(&outer) else {
            continue;
        };
        let mut face_loops = Vec::new();
        for (k, ids) in loops.iter().enumerate() {
            let mut ids = ids.clone();
            // Holes run against the outer loop.
            if k > 0 {
                let winding = newell(&ids.iter().map(|i| body.vertices[*i]).collect::<Vec<_>>())
                    .map_or(-1.0, |n| math::dot(n, normal));
                if winding > 0.0 {
                    ids.reverse();
                }
            }
            let mut coedges = Vec::with_capacity(ids.len());
            for i in 0..ids.len() {
                let (a, b) = (ids[i], ids[(i + 1) % ids.len()]);
                let (lo, hi) = (a.min(b), a.max(b));
                let edge = *edges.entry((lo, hi)).or_insert_with(|| {
                    let (p, q) = (body.vertices[lo], body.vertices[hi]);
                    let length = math::length(math::sub(q, p));
                    body.curves.push(BoundaryCurve::Line {
                        origin: p,
                        dir: math::normalize(math::sub(q, p)).unwrap_or([1.0, 0.0, 0.0]),
                    });
                    body.edges.push(BoundaryEdge {
                        curve: body.curves.len() - 1,
                        range: [0.0, length],
                        vertices: [lo, hi],
                    });
                    body.edges.len() - 1
                });
                coedges.push((edge, a == lo));
            }
            face_loops.push(coedges);
        }
        let origin = outer[0];
        let u = math::column(
            &math::frame(origin, Some(normal), Some(math::sub(outer[1], origin))),
            0,
        );
        body.surfaces.push(BoundarySurface::Plane {
            origin,
            normal,
            u_dir: u,
        });
        body.faces.push(BoundaryFace {
            surface: body.surfaces.len() - 1,
            reversed: false,
            loops: face_loops,
        });
        shell.push(body.faces.len() - 1);
    }
    if shell.is_empty() {
        return Err("no face has an area".to_owned());
    }
    body.shells.push(shell);
    Ok(body)
}

/// Undirected edges of the loops and how often they are used.
fn edge_uses(loops_of: &[Vec<Vec<usize>>]) -> HashMap<(usize, usize), usize> {
    let mut uses = HashMap::new();
    for l in loops_of.iter().flatten() {
        for i in 0..l.len() {
            let (a, b) = (l[i], l[(i + 1) % l.len()]);
            *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    uses
}

/// Splits edges used once where vertices of other free edges lie inside
/// them (a face's edge that its neighbours divide: the faces then share
/// the pieces).
fn split_t_junctions(points: &[[f64; 3]], loops_of: &mut [Vec<Vec<usize>>]) {
    let uses = edge_uses(loops_of);
    let free: Vec<(usize, usize)> = uses
        .iter()
        .filter(|(_, n)| **n == 1)
        .map(|(e, _)| *e)
        .collect();
    if free.is_empty() || free.len() > 20_000 {
        return;
    }
    let mut candidates: Vec<usize> = free.iter().flat_map(|(a, b)| [*a, *b]).collect();
    candidates.sort_unstable();
    candidates.dedup();
    // The vertices inside each free edge, by their parameter along it.
    let mut inside: HashMap<(usize, usize), Vec<(f64, usize)>> = HashMap::new();
    for &(a, b) in &free {
        let (p, q) = (points[a], points[b]);
        let d = math::sub(q, p);
        let len2 = math::dot(d, d);
        if len2 <= 0.0 {
            continue;
        }
        let tolerance = (1e-6 * len2.sqrt()).max(MERGE * 10.0);
        for &v in &candidates {
            if v == a || v == b {
                continue;
            }
            let t = math::dot(math::sub(points[v], p), d) / len2;
            if t <= 1e-9 || t >= 1.0 - 1e-9 {
                continue;
            }
            let off = math::sub(math::sub(points[v], p), math::scale(d, t));
            if math::length(off) <= tolerance {
                inside.entry((a, b)).or_default().push((t, v));
            }
        }
    }
    if inside.is_empty() {
        return;
    }
    for list in inside.values_mut() {
        list.sort_by(|x, y| x.0.total_cmp(&y.0));
    }
    for l in loops_of.iter_mut().flatten() {
        let mut out = Vec::with_capacity(l.len());
        for i in 0..l.len() {
            let (a, b) = (l[i], l[(i + 1) % l.len()]);
            out.push(a);
            if let Some(list) = inside.get(&(a.min(b), a.max(b))) {
                if a < b {
                    out.extend(list.iter().map(|(_, v)| *v));
                } else {
                    out.extend(list.iter().rev().map(|(_, v)| *v));
                }
            }
        }
        *l = out;
    }
}

/// Turns faces so that each edge two faces share runs one way in one and
/// the other way in the other (spreading from a face through its
/// neighbours), then each connected set of faces so that it encloses a
/// positive volume (when it encloses any).
fn orient_faces(points: &[[f64; 3]], loops_of: &mut [Vec<Vec<usize>>]) {
    let n = loops_of.len();
    // Directed edge: the faces that run it.
    let mut runs: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (f, loops) in loops_of.iter().enumerate() {
        for l in loops {
            for i in 0..l.len() {
                runs.entry((l[i], l[(i + 1) % l.len()]))
                    .or_default()
                    .push(f);
            }
        }
    }
    // The neighbours of a face, and whether each runs the shared edge the
    // same way.
    let neighbours = |f: usize| -> Vec<(usize, bool)> {
        let mut out = Vec::new();
        for l in &loops_of[f] {
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                for &g in runs.get(&(a, b)).into_iter().flatten() {
                    if g != f {
                        out.push((g, true));
                    }
                }
                for &g in runs.get(&(b, a)).into_iter().flatten() {
                    if g != f {
                        out.push((g, false));
                    }
                }
            }
        }
        out
    };
    // Whether each face is to be turned.
    let mut flip: Vec<Option<bool>> = vec![None; n];
    let mut components: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if flip[start].is_some() {
            continue;
        }
        flip[start] = Some(false);
        let mut component = vec![start];
        let mut queue = vec![start];
        while let Some(f) = queue.pop() {
            let turned = flip[f].expect("set");
            for (g, same) in neighbours(f) {
                if flip[g].is_none() {
                    // A neighbour running the edge the same way is turned
                    // unless this face is.
                    flip[g] = Some(turned ^ same);
                    component.push(g);
                    queue.push(g);
                }
            }
        }
        components.push(component);
    }
    for component in &components {
        // The component's enclosed volume after the turns.
        let mut volume = 0.0;
        for &f in component {
            let turned = flip[f].expect("set");
            for l in &loops_of[f] {
                let p0 = points[l[0]];
                for i in 1..l.len().saturating_sub(1) {
                    let (p1, p2) = (points[l[i]], points[l[i + 1]]);
                    let v = math::dot(p0, math::cross(p1, p2)) / 6.0;
                    volume += if turned { -v } else { v };
                }
            }
        }
        if volume < 0.0 {
            for &f in component {
                flip[f] = flip[f].map(|t| !t);
            }
        }
    }
    for (f, loops) in loops_of.iter_mut().enumerate() {
        if flip[f] == Some(true) {
            for l in loops.iter_mut() {
                l.reverse();
            }
        }
    }
}

/// The normal of a polygon by Newell's method.
fn newell(points: &[[f64; 3]]) -> Option<[f64; 3]> {
    let mut n = [0.0; 3];
    for k in 0..points.len() {
        let (a, b) = (points[k], points[(k + 1) % points.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    math::normalize(n)
}

fn surface(s: &Surface) -> Result<BoundarySurface, String> {
    let frame_parts = |f: &Matrix| (math::origin(f), math::column(f, 2), math::column(f, 0));
    Ok(match s {
        Surface::Plane { frame } => {
            let (origin, normal, u_dir) = frame_parts(frame);
            BoundarySurface::Plane {
                origin,
                normal,
                u_dir,
            }
        }
        Surface::Cylinder { frame, radius } => {
            let (origin, axis, ref_dir) = frame_parts(frame);
            BoundarySurface::Cone {
                origin,
                axis,
                ref_dir,
                radius: *radius,
                half_angle: 0.0,
            }
        }
        Surface::Cone {
            frame,
            radius,
            semi_angle,
        } => {
            let (origin, axis, ref_dir) = frame_parts(frame);
            BoundarySurface::Cone {
                origin,
                axis,
                ref_dir,
                radius: *radius,
                half_angle: *semi_angle,
            }
        }
        Surface::Sphere { frame, radius } => {
            let (center, axis, ref_dir) = frame_parts(frame);
            BoundarySurface::Sphere {
                center,
                axis,
                ref_dir,
                radius: *radius,
            }
        }
        Surface::Torus {
            frame,
            major,
            minor,
        } => {
            let (center, axis, ref_dir) = frame_parts(frame);
            BoundarySurface::Torus {
                center,
                axis,
                ref_dir,
                major: *major,
                minor: *minor,
            }
        }
        Surface::BSpline {
            u_degree,
            v_degree,
            poles,
            weights,
            u_knots,
            u_multiplicities,
            v_knots,
            v_multiplicities,
        } => BoundarySurface::BSpline {
            u_degree: *u_degree,
            v_degree: *v_degree,
            u_knots: u_knots.clone(),
            u_multiplicities: u_multiplicities.clone(),
            v_knots: v_knots.clone(),
            v_multiplicities: v_multiplicities.clone(),
            poles: poles.clone(),
            weights: weights.clone(),
        },
        Surface::Extrusion { curve, direction } => {
            let d = math::length(*direction);
            BoundarySurface::Extrusion {
                curve: whole_curve(curve)?,
                dir: math::normalize(*direction)
                    .filter(|_| d > 0.0)
                    .ok_or("an extrusion of no length")?,
            }
        }
        Surface::Revolution {
            curve,
            origin,
            axis,
        } => BoundarySurface::Revolution {
            curve: whole_curve(curve)?,
            origin: *origin,
            axis: math::normalize(*axis).ok_or("a zero axis")?,
        },
    })
}

/// A path as one curve (a swept surface's): one piece, or a polyline as a
/// B-spline of degree 1.
fn whole_curve(path: &Path) -> Result<BoundaryCurve, String> {
    match path.segments.as_slice() {
        [one] => Ok(seg_curve(one).0),
        many if many.iter().all(|s| matches!(s, Seg::Line { .. })) => {
            let mut poles = vec![many[0].start()];
            poles.extend(many.iter().map(Seg::end));
            let n = poles.len();
            let mut knots = vec![0.0];
            let mut multiplicities = vec![2];
            let mut t = 0.0;
            for k in 1..n {
                t += math::length(math::sub(poles[k], poles[k - 1])).max(1e-9);
                knots.push(t);
                multiplicities.push(if k == n - 1 { 2 } else { 1 });
            }
            Ok(BoundaryCurve::BSpline {
                degree: 1,
                knots,
                multiplicities,
                poles,
                weights: Vec::new(),
            })
        }
        _ => Err("a swept curve of several curves that are not all lines".to_owned()),
    }
}

/// A piece's curve, and its parameters at the piece's start and end (the
/// end's greater for conics and B-splines run their own way).
fn seg_curve(seg: &Seg) -> (BoundaryCurve, [f64; 2]) {
    match seg {
        Seg::Line { start, end } => (
            BoundaryCurve::Line {
                origin: *start,
                dir: math::normalize(math::sub(*end, *start)).unwrap_or([1.0, 0.0, 0.0]),
            },
            [0.0, math::length(math::sub(*end, *start))],
        ),
        Seg::Arc {
            frame,
            radius,
            start,
            sweep,
        } => conic(frame, *radius, *radius, *start, *sweep),
        Seg::Ellipse {
            frame,
            a,
            b,
            start,
            sweep,
        } => conic(frame, *a, *b, *start, *sweep),
        Seg::Spline {
            degree,
            poles,
            weights,
            knots,
            multiplicities,
        } => {
            let full = full_knots(knots, multiplicities);
            let (lo, hi) = spline_domain(*degree, poles.len(), &full);
            (
                BoundaryCurve::BSpline {
                    degree: *degree,
                    knots: knots.clone(),
                    multiplicities: multiplicities.clone(),
                    poles: poles.clone(),
                    weights: weights.clone(),
                },
                [lo, hi],
            )
        }
    }
}

/// A conic run counter-clockwise about its normal: a clockwise one gets
/// the opposite normal (its angles then run the other way).
fn conic(frame: &Matrix, a: f64, b: f64, start: f64, sweep: f64) -> (BoundaryCurve, [f64; 2]) {
    let center = math::origin(frame);
    let x = math::column(frame, 0);
    let mut normal = math::column(frame, 2);
    let (mut t0, mut t1) = (start, start + sweep);
    if sweep < 0.0 {
        normal = math::scale(normal, -1.0);
        // Angles about the flipped normal from the same x axis are the
        // negatives.
        t0 = -t0;
        t1 = -t1;
    }
    // Into [0, 2π) at the start.
    let shift = t0.rem_euclid(TAU) - t0;
    (
        BoundaryCurve::Ellipse {
            center,
            normal,
            major_dir: x,
            major: a,
            minor: b,
        },
        [t0 + shift, t1 + shift],
    )
}

/// The parameter of the point of a B-spline nearest to `p`.
fn spline_parameter(
    degree: u32,
    poles: &[[f64; 3]],
    weights: &[f64],
    full: &[f64],
    lo: f64,
    hi: f64,
    p: [f64; 3],
) -> f64 {
    let at = |t: f64| de_boor(degree as usize, poles, weights, full, t);
    let d = |t: f64| math::length(math::sub(at(t), p));
    let n = 200;
    let mut best = (lo, d(lo));
    for i in 1..=n {
        let t = lo + (hi - lo) * i as f64 / n as f64;
        let dt = d(t);
        if dt < best.1 {
            best = (t, dt);
        }
    }
    // Golden section around the best sample.
    let step = (hi - lo) / n as f64;
    let (mut a, mut b) = ((best.0 - step).max(lo), (best.0 + step).min(hi));
    let g = 0.618_033_988_75;
    for _ in 0..60 {
        let c = b - g * (b - a);
        let e = a + g * (b - a);
        if d(c) < d(e) {
            b = e;
        } else {
            a = c;
        }
    }
    (a + b) / 2.0
}

/// Points along a curved edge, to find it again.
const SAMPLES: usize = 32;

/// An edge of the body and points along it from its first vertex.
type Sampled = (usize, Vec<[f64; 3]>);

fn near(p: [f64; 3], q: [f64; 3]) -> bool {
    math::length(math::sub(p, q)) <= 1e-6 * (1.0 + math::length(q))
}

/// Whether `points` lie along `along` (points of another edge from its
/// first vertex), within `fraction` of its length: `Some(true)` when they
/// run the same way.
fn same_curve(points: &[[f64; 3]], along: &[[f64; 3]], fraction: f64) -> Option<bool> {
    let length: f64 = along
        .windows(2)
        .map(|w| math::length(math::sub(w[1], w[0])))
        .sum();
    let tolerance = fraction * length;
    if tolerance <= 0.0 {
        return None;
    }
    // The nearest piece of `along` to a point, and the distance.
    let nearest = |p: [f64; 3]| -> (usize, f64) {
        let mut best = (0, f64::INFINITY);
        for (i, w) in along.windows(2).enumerate() {
            let d = math::sub(w[1], w[0]);
            let dd = math::dot(d, d);
            let t = if dd > 0.0 {
                (math::dot(math::sub(p, w[0]), d) / dd).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance = math::length(math::sub(p, math::add(w[0], math::scale(d, t))));
            if distance < best.1 {
                best = (i, distance);
            }
        }
        best
    };
    if points.iter().any(|p| nearest(*p).1 > tolerance) {
        return None;
    }
    // A quarter along `points`: in the first half of `along` when the same
    // way.
    let (i, _) = nearest(points[points.len() / 4]);
    Some(i < along.len() / 2)
}

/// Whether a B-spline face is its whole surface, closed both ways (a
/// torus): every edge of its bounds runs along a side of the surface's
/// domain (the iso-curve at a first or last knot; clamped knots only) and
/// every side that is not a point has an edge along it. Files bound such a
/// face by its seams and sides, which a closed surface shares with itself.
fn whole_surface(face: &AdvancedFace) -> bool {
    let Surface::BSpline {
        u_degree,
        v_degree,
        poles,
        weights,
        u_knots,
        u_multiplicities,
        v_knots,
        v_multiplicities,
    } = &face.surface
    else {
        return false;
    };
    let clamped = |degree: u32, m: &[u32]| {
        m.first().is_some_and(|f| *f == degree + 1) && m.last().is_some_and(|l| *l == degree + 1)
    };
    let (n, m) = (poles.len(), poles.first().map_or(0, Vec::len));
    if n < 2
        || m < 2
        || !clamped(*u_degree, u_multiplicities)
        || !clamped(*v_degree, v_multiplicities)
    {
        return false;
    }
    let rational = weights.len() == n;
    let uk = full_knots(u_knots, u_multiplicities);
    let vk = full_knots(v_knots, v_multiplicities);
    let side = |degree: u32, ps: Vec<[f64; 3]>, ws: Vec<f64>, knots: &[f64]| -> Vec<[f64; 3]> {
        let (lo, hi) = spline_domain(degree, ps.len(), knots);
        (0..=64)
            .map(|i| {
                de_boor(
                    degree as usize,
                    &ps,
                    &ws,
                    knots,
                    lo + (hi - lo) * f64::from(i) / 64.0,
                )
            })
            .collect()
    };
    let row = |i: usize| {
        (
            poles[i].clone(),
            if rational {
                weights[i].clone()
            } else {
                Vec::new()
            },
        )
    };
    let column = |j: usize| {
        (
            poles.iter().map(|r| r[j]).collect::<Vec<_>>(),
            if rational {
                weights.iter().map(|r| r[j]).collect()
            } else {
                Vec::new()
            },
        )
    };
    let sides: Vec<Vec<[f64; 3]>> = [row(0), row(n - 1)]
        .into_iter()
        .map(|(p, w)| side(*v_degree, p, w, &vk))
        .chain(
            [column(0), column(m - 1)]
                .into_iter()
                .map(|(p, w)| side(*u_degree, p, w, &uk)),
        )
        .collect();
    let length = |s: &[[f64; 3]]| -> f64 {
        s.windows(2)
            .map(|w| math::length(math::sub(w[1], w[0])))
            .sum()
    };
    // Closed both ways (each pair of opposite sides one curve, or a point):
    // the whole surface then bounds a solid. A face on an open surface keeps
    // its loops (its body is a sheet).
    let closed = |a: &[[f64; 3]], b: &[[f64; 3]]| {
        (length(a) <= MERGE && length(b) <= MERGE) || same_curve(a, b, 1e-3).is_some()
    };
    if !closed(&sides[0], &sides[1]) || !closed(&sides[2], &sides[3]) {
        return false;
    }
    let mut covered = [false; 4];
    for bound in &face.bounds {
        for e in &bound.edges {
            for seg in &e.curve.segments {
                let points = seg.sample(16);
                let mut along = false;
                for (k, s) in sides.iter().enumerate() {
                    if length(s) > MERGE && same_curve(&points, s, 1e-3).is_some() {
                        covered[k] = true;
                        along = true;
                    }
                }
                if !along {
                    return false;
                }
            }
        }
    }
    !face.bounds.is_empty()
        && sides
            .iter()
            .zip(covered)
            .all(|(s, c)| c || length(s) <= MERGE)
}

/// Advanced faces as a boundary of one shell.
pub fn advanced(faces: &[AdvancedFace]) -> Result<BoundaryBody, String> {
    let mut body = BoundaryBody::default();
    let mut by_id: HashMap<(u64, usize), usize> = HashMap::new();
    let mut by_position: HashMap<[i64; 3], usize> = HashMap::new();
    let mut edges: HashMap<(u64, usize), usize> = HashMap::new();
    let mut line_edges: HashMap<(usize, usize), usize> = HashMap::new();
    // Closed curved edges by their vertex, with points along them: files
    // repeat a seam of a closed surface as another instance.
    let mut curved_edges: HashMap<usize, Vec<Sampled>> = HashMap::new();
    let mut shell = Vec::new();
    let mut vertex = |body: &mut BoundaryBody, id: u64, index: usize, p: [f64; 3]| -> usize {
        if id != 0
            && let Some(v) = by_id.get(&(id, index))
        {
            return *v;
        }
        let v = *by_position.entry(key(p)).or_insert_with(|| {
            body.vertices.push(p);
            body.vertices.len() - 1
        });
        if id != 0 {
            by_id.insert((id, index), v);
        }
        v
    };
    for face in faces {
        let s = surface(&face.surface)?;
        body.surfaces.push(s);
        let surface_index = body.surfaces.len() - 1;
        // A face that is its whole B-spline surface: no loops (the kernel
        // bounds it), when no other face shares its edges.
        if faces.len() == 1 && whole_surface(face) {
            body.faces.push(BoundaryFace {
                surface: surface_index,
                reversed: !face.same_sense,
                loops: Vec::new(),
            });
            shell.push(body.faces.len() - 1);
            continue;
        }
        let mut loops = Vec::new();
        // The outer bound first.
        let mut bounds: Vec<_> = face.bounds.iter().collect();
        bounds.sort_by_key(|b| !b.outer);
        for bound in bounds {
            let mut coedges = Vec::new();
            for e in &bound.edges {
                // The edge in its curve's own direction: from `first` to
                // `last`.
                let (first, last) = if e.forward {
                    (e.start, e.end)
                } else {
                    (e.end, e.start)
                };
                let (id_first, id_last) = if e.forward {
                    (e.vertices[0], e.vertices[1])
                } else {
                    (e.vertices[1], e.vertices[0])
                };
                let mut pieces = e.curve.segments.clone();
                // A path run from its last vertex to its first is turned.
                if let (Some(s0), Some(s1)) = (e.curve.start(), e.curve.end()) {
                    let d_same =
                        math::length(math::sub(s0, first)) + math::length(math::sub(s1, last));
                    let d_turned =
                        math::length(math::sub(s0, last)) + math::length(math::sub(s1, first));
                    if pieces.len() > 1 && d_turned < d_same {
                        pieces = e.curve.reversed().segments;
                    }
                }
                let count = pieces.len();
                let mut own: Vec<(usize, bool)> = Vec::with_capacity(count);
                for (k, seg) in pieces.iter().enumerate() {
                    let a = if k == 0 { first } else { seg.start() };
                    let b = if k + 1 == count { last } else { seg.end() };
                    let va = vertex(
                        &mut body,
                        if k == 0 { id_first } else { e.id },
                        if k == 0 { 0 } else { k + 1_000_000 },
                        a,
                    );
                    let vb = vertex(
                        &mut body,
                        if k + 1 == count { id_last } else { e.id },
                        if k + 1 == count { 0 } else { k + 1 + 1_000_000 },
                        b,
                    );
                    let shared_key = (e.id, k);
                    if e.id != 0
                        && let Some(edge) = edges.get(&shared_key)
                    {
                        let forward = body.edges[*edge].vertices[0] == va;
                        own.push((*edge, forward));
                        continue;
                    }
                    if e.id == 0 && matches!(seg, Seg::Line { .. }) {
                        let (lo, hi) = (va.min(vb), va.max(vb));
                        if let Some(edge) = line_edges.get(&(lo, hi)) {
                            own.push((*edge, va == lo));
                            continue;
                        }
                    }
                    // A closed curve that runs from its vertex round to it,
                    // along another closed edge at that vertex, is that edge
                    // (a seam; edges between two vertices are left apart:
                    // holes may touch along one).
                    let samples = (va == vb
                        && !matches!(seg, Seg::Line { .. })
                        && near(seg.start(), a)
                        && near(seg.end(), b))
                    .then(|| seg.sample(SAMPLES));
                    if let Some(points) = &samples
                        && let Some(found) = curved_edges.get(&va).and_then(|list| {
                            list.iter().find_map(|(edge, along)| {
                                same_curve(points, along, 1e-2).map(|f| (*edge, f))
                            })
                        })
                    {
                        own.push(found);
                        continue;
                    }
                    let (curve, range) = match seg {
                        Seg::Line { .. } => (
                            BoundaryCurve::Line {
                                origin: a,
                                dir: math::normalize(math::sub(b, a)).unwrap_or([1.0, 0.0, 0.0]),
                            },
                            [0.0, math::length(math::sub(b, a))],
                        ),
                        Seg::Arc { .. } | Seg::Ellipse { .. } => {
                            let (curve, own_range) = seg_curve(seg);
                            let BoundaryCurve::Ellipse {
                                center,
                                normal,
                                major_dir,
                                major,
                                minor,
                            } = &curve
                            else {
                                unreachable!("a conic")
                            };
                            let y = math::cross(*normal, *major_dir);
                            let angle = |p: [f64; 3]| {
                                let d = math::sub(p, *center);
                                (math::dot(d, y) / minor).atan2(math::dot(d, *major_dir) / major)
                            };
                            let t0 = angle(a);
                            let mut t1 = angle(b);
                            while t1 <= t0 + 1e-12 {
                                t1 += TAU;
                            }
                            // A whole conic between one vertex.
                            if va == vb && (own_range[1] - own_range[0]) > TAU - 1e-6 {
                                t1 = t0 + TAU;
                            } else if t1 - t0 > TAU {
                                t1 -= TAU;
                            }
                            (curve, [t0, t1])
                        }
                        Seg::Spline {
                            degree,
                            poles,
                            weights,
                            knots,
                            multiplicities,
                        } => {
                            let (curve, _) = seg_curve(seg);
                            let full = full_knots(knots, multiplicities);
                            let (lo, hi) = spline_domain(*degree, poles.len(), &full);
                            let ends_at = |p: [f64; 3], q: [f64; 3]| {
                                math::length(math::sub(p, q)) <= 1e-6 * (1.0 + math::length(q))
                            };
                            let t0 = if ends_at(seg.start(), a) {
                                lo
                            } else {
                                spline_parameter(*degree, poles, weights, &full, lo, hi, a)
                            };
                            let t1 = if ends_at(seg.end(), b) {
                                hi
                            } else {
                                spline_parameter(*degree, poles, weights, &full, lo, hi, b)
                            };
                            let (t0, t1) = if va == vb && (t1 - t0).abs() < 1e-12 {
                                (lo, hi)
                            } else {
                                (t0, t1)
                            };
                            if t1 < t0 {
                                return Err("an edge runs against its B-spline".to_owned());
                            }
                            (curve, [t0, t1])
                        }
                    };
                    body.curves.push(curve);
                    body.edges.push(BoundaryEdge {
                        curve: body.curves.len() - 1,
                        range,
                        vertices: [va, vb],
                    });
                    let edge = body.edges.len() - 1;
                    if e.id != 0 {
                        edges.insert(shared_key, edge);
                    } else if matches!(seg, Seg::Line { .. }) {
                        line_edges.insert((va.min(vb), va.max(vb)), edge);
                    }
                    if let Some(points) = samples {
                        curved_edges.entry(va).or_default().push((edge, points));
                    }
                    own.push((edge, true));
                }
                // In the bound's order.
                if e.forward {
                    coedges.extend(own);
                } else {
                    coedges.extend(own.into_iter().rev().map(|(edge, f)| (edge, !f)));
                }
            }
            if !coedges.is_empty() {
                loops.push(coedges);
            }
        }
        // An edge twice in a face is a seam of its closed surface: once each
        // way (files have it twice the same way).
        let mut seen: HashMap<usize, Vec<bool>> = HashMap::new();
        for (edge, forward) in loops.iter_mut().flatten() {
            let uses = seen.entry(*edge).or_default();
            if uses.len() == 1 && uses[0] == *forward {
                *forward = !*forward;
            }
            uses.push(*forward);
        }
        if loops.is_empty() {
            // A face without bounds: its surface's whole domain is not
            // built here.
            continue;
        }
        // A B-spline face bounded only by seams (closed edges, each once
        // each way: a torus) is its whole surface too, whatever the file's
        // curves of the seams (they need not lie on the surface).
        if matches!(face.surface, Surface::BSpline { .. })
            && seen
                .values()
                .all(|uses| uses.len() == 2 && uses[0] != uses[1])
            && seen
                .keys()
                .all(|e| body.edges[*e].vertices[0] == body.edges[*e].vertices[1])
        {
            loops.clear();
        }
        body.faces.push(BoundaryFace {
            surface: surface_index,
            reversed: !face.same_sense,
            loops,
        });
        shell.push(body.faces.len() - 1);
    }
    if shell.is_empty() {
        return Err("no face has a bound".to_owned());
    }
    body.shells.push(shell);
    Ok(body)
}

/// Whether every edge of the triangles joins exactly two of them, once
/// each way (points at one position are one: face sets often repeat
/// their points per face).
pub fn mesh_closed(points: &[[f64; 3]], triangles: &[[u32; 3]]) -> bool {
    let mut first: HashMap<[i64; 3], u32> = HashMap::new();
    let welded: Vec<u32> = points
        .iter()
        .enumerate()
        .map(|(i, p)| *first.entry(key(*p)).or_insert(i as u32))
        .collect();
    let triangles: Vec<[u32; 3]> = triangles
        .iter()
        .map(|t| t.map(|i| welded.get(i as usize).copied().unwrap_or(i)))
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[2] != t[0])
        .collect();
    let triangles = triangles.as_slice();
    let mut uses: HashMap<(u32, u32), i32> = HashMap::new();
    for t in triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_insert(0) += if a < b { 1 } else { -1 };
        }
    }
    let mut count: HashMap<(u32, u32), u32> = HashMap::new();
    for t in triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *count.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    !triangles.is_empty()
        && count.values().all(|n| *n == 2)
        && uses.values().all(|balance| *balance == 0)
}

/// The volume and area of triangles (counter-clockwise from outside).
pub fn mesh_measures(points: &[[f64; 3]], triangles: &[[u32; 3]]) -> (f64, f64) {
    let mut volume = 0.0;
    let mut area = 0.0;
    for t in triangles {
        let (a, b, c) = (
            points[t[0] as usize],
            points[t[1] as usize],
            points[t[2] as usize],
        );
        volume += math::dot(a, math::cross(b, c)) / 6.0;
        area += math::length(math::cross(math::sub(b, a), math::sub(c, a))) / 2.0;
    }
    (volume, area)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_ifc::geometry::{Bound, Edge};

    #[test]
    fn a_cube_of_poly_loops() {
        let p = |x: f64, y: f64, z: f64| [x, y, z];
        let c = [
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 0.0, 1.0),
            p(1.0, 0.0, 1.0),
            p(1.0, 1.0, 1.0),
            p(0.0, 1.0, 1.0),
        ];
        let f = |i: [usize; 4]| vec![i.iter().map(|k| c[*k]).collect::<Vec<_>>()];
        let faces = vec![
            f([0, 3, 2, 1]),
            f([4, 5, 6, 7]),
            f([0, 1, 5, 4]),
            f([3, 7, 6, 2]),
            f([0, 4, 7, 3]),
            f([1, 2, 6, 5]),
        ];
        let body = faceted(&faces).expect("a body");
        assert_eq!(body.vertices.len(), 8);
        assert_eq!(body.edges.len(), 12);
        assert_eq!(body.faces.len(), 6);
        body.check().expect("consistent");
        // Every edge is used once each way.
        let mut uses: HashMap<usize, (usize, usize)> = HashMap::new();
        for face in &body.faces {
            for (e, forward) in face.loops.iter().flatten() {
                let u = uses.entry(*e).or_default();
                if *forward {
                    u.0 += 1;
                } else {
                    u.1 += 1;
                }
            }
        }
        assert!(uses.values().all(|u| *u == (1, 1)), "{uses:?}");
    }

    /// Every edge used once each way.
    fn closed(body: &BoundaryBody) -> bool {
        let mut uses: HashMap<usize, (usize, usize)> = HashMap::new();
        for face in &body.faces {
            for (e, forward) in face.loops.iter().flatten() {
                let u = uses.entry(*e).or_default();
                if *forward {
                    u.0 += 1;
                } else {
                    u.1 += 1;
                }
            }
        }
        uses.values().all(|u| *u == (1, 1))
    }

    #[test]
    fn turned_faces_and_t_junctions_are_mended() {
        let c = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
            [2.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
            // The middle of the bottom's long edges.
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
        ];
        let f = |i: &[usize]| vec![i.iter().map(|k| c[*k]).collect::<Vec<_>>()];
        let faces = vec![
            // The bottom in two halves, the sides whole: T-junctions.
            f(&[0, 3, 9, 8]),
            f(&[8, 9, 2, 1]),
            // The top turned the wrong way.
            f(&[7, 6, 5, 4]),
            f(&[0, 1, 5, 4]),
            f(&[3, 7, 6, 2]),
            f(&[0, 4, 7, 3]),
            f(&[1, 2, 6, 5]),
        ];
        let body = faceted(&faces).expect("a body");
        assert!(closed(&body), "{body:?}");
    }

    #[test]
    fn clockwise_arcs_turn_their_normal() {
        let (curve, range) = conic(&math::IDENTITY, 2.0, 2.0, 0.5, -1.0);
        let BoundaryCurve::Ellipse { normal, .. } = curve else {
            panic!("an ellipse");
        };
        assert_eq!(normal, [0.0, 0.0, -1.0]);
        assert!(range[1] > range[0]);
        assert!((range[1] - range[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn tetrahedron_measures() {
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let (v, _) = mesh_measures(&points, &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]);
        assert!((v - 1.0 / 6.0).abs() < 1e-12);
    }

    #[test]
    fn repeated_seams_are_one_edge_each_way() {
        // A torus face bounded by its two seams, each given twice as its own
        // instance and both times the same way.
        let major = Seg::Arc {
            frame: math::IDENTITY,
            radius: 12.0,
            start: 0.0,
            sweep: TAU,
        };
        let minor = Seg::Arc {
            frame: [
                [1.0, 0.0, 0.0, 10.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
            ],
            radius: 2.0,
            start: 0.0,
            sweep: TAU,
        };
        let v = [12.0, 0.0, 0.0];
        let edge = |id: u64, seg: &Seg, forward: bool| Edge {
            id,
            vertices: [1, 1],
            curve: Path {
                segments: vec![seg.clone()],
            },
            start: v,
            end: v,
            forward,
        };
        let face = AdvancedFace {
            surface: Surface::Torus {
                frame: math::IDENTITY,
                major: 10.0,
                minor: 2.0,
            },
            bounds: vec![Bound {
                outer: true,
                edges: vec![
                    edge(10, &major, true),
                    edge(11, &minor, true),
                    edge(12, &major.reversed(), false),
                    edge(13, &minor.reversed(), false),
                ],
            }],
            same_sense: true,
        };
        let body = advanced(&[face]).expect("a body");
        assert_eq!(body.edges.len(), 2, "{body:?}");
        assert_eq!(
            body.faces[0].loops,
            vec![vec![(0, true), (1, true), (0, false), (1, false)]]
        );
    }
}
