// SPDX-License-Identifier: MIT
//! Elements as triangle meshes of the geometry the file keeps
//! (mitcad#333, [`mitcad_rvt::brep`]): an element's own bodies and the
//! types it places (a family instance's type at its transform, nested
//! types at theirs), in model millimetres.
//!
//! Each face is triangulated within its loops: a plane's (and a face of a
//! surface not read: its points from the neighbouring faces) in the
//! face's plane; a curved face's in its (u, v), each triangle halved until
//! its sides lie within [`CHORD`] of the surface. The triangles keep the
//! loops' direction; a body whose closed mesh encloses a negative volume
//! is turned over, as is a type placed by a mirroring transform.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use mitcad_ifc::geometry::triangulate;
use mitcad_ifc::math;
use mitcad_rvt::brep::{GRep, P3, Solid, Surface, Uv};
use mitcad_rvt::elements::{FOOT_MM, Model};

use crate::ifc::brep::{mesh_closed, mesh_measures};

/// How far (feet) a curved face's triangles may lie from the surface.
pub const CHORD: f64 = 0.01;

/// How many times a curved face's triangle is halved at most.
const MAX_SPLITS: usize = 10;

/// Types placed in types are followed this deep.
const DEPTH: usize = 6;

/// An element's mesh (mm).
#[derive(Debug, Clone, Default)]
pub struct ElementMesh {
    pub points: Vec<P3>,
    pub triangles: Vec<[u32; 3]>,
    /// Bodies and faces meshed, faces left out (and why, the first few).
    pub bodies: usize,
    pub faces: usize,
    pub faces_left_out: usize,
    pub notes: Vec<String>,
    /// Whether every edge joins two triangles, and the volume (mm³) then.
    pub closed: bool,
    pub volume: Option<f64>,
}

impl ElementMesh {
    /// The box of its points (mm).
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let first = self.points.first()?;
        let (mut lo, mut hi) = (*first, *first);
        for p in &self.points {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        Some((lo, hi))
    }
}

type Trf = [f64; 12];

const IDENTITY: Trf = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

fn apply(t: &Trf, p: P3) -> P3 {
    std::array::from_fn(|r| t[3 * r] * p[0] + t[3 * r + 1] * p[1] + t[3 * r + 2] * p[2] + t[9 + r])
}

/// `outer` after `inner`.
fn compose(outer: &Trf, inner: &Trf) -> Trf {
    let mut m = [0.0; 12];
    for r in 0..3 {
        for c in 0..3 {
            m[3 * r + c] = (0..3).map(|k| outer[3 * r + k] * inner[3 * k + c]).sum();
        }
    }
    let o = apply(outer, [inner[9], inner[10], inner[11]]);
    m[9..].copy_from_slice(&o);
    m
}

fn determinant(t: &Trf) -> f64 {
    t[0] * (t[4] * t[8] - t[5] * t[7]) - t[1] * (t[3] * t[8] - t[5] * t[6])
        + t[2] * (t[3] * t[7] - t[4] * t[6])
}

/// An element's mesh, None without geometry of its own; the types and
/// elements in `skip` are left out where it places them (a railing's
/// balusters and rails, elements of their own).
pub fn element_mesh(
    model: &Model,
    id: i64,
    cache: &mut Cache,
    skip: &HashSet<i64>,
) -> Option<Result<ElementMesh, String>> {
    let g = model.geometry.get(&id)?;
    let mut out = ElementMesh::default();
    let mut seen = skip.clone();
    add_grep(model, g, &IDENTITY, 0, &mut seen, cache, &mut out);
    if out.triangles.is_empty() {
        let why = out
            .notes
            .first()
            .cloned()
            .unwrap_or_else(|| "its geometry has no faces".to_string());
        return Some(Err(why));
    }
    out.closed = mesh_closed(&out.points, &out.triangles);
    if out.closed {
        let (mut v, _) = mesh_measures(&out.points, &out.triangles);
        if v < 0.0 {
            for t in &mut out.triangles {
                t.swap(1, 2);
            }
            v = -v;
        }
        out.volume = Some(v);
    }
    Some(Ok(out))
}

/// A body's triangles (feet, its own coordinates), facing out (mitcad#345:
/// a family type's void geometry, cut from its host); an error when a face
/// is left out or the triangles do not close.
pub fn solid_mesh(s: &Solid) -> Result<(Vec<P3>, Vec<[u32; 3]>), String> {
    let g = GRep {
        solids: vec![s.clone()],
        ..GRep::default()
    };
    let tess = tessellate(&g);
    if tess.left_out > 0 {
        return Err(tess
            .notes
            .first()
            .cloned()
            .unwrap_or_else(|| "a face is left out".to_string()));
    }
    let (points, triangles) = tess.bodies.into_iter().next().ok_or("it has no faces")?;
    if !mesh_closed(&points, &triangles) {
        return Err("its faces do not close".into());
    }
    Ok((points, triangles))
}

/// A geometry's own bodies triangulated (feet, its own coordinates), each
/// body's triangles facing out where it is closed.
#[derive(Debug, Default)]
pub struct Tess {
    pub bodies: Vec<(Vec<P3>, Vec<[u32; 3]>)>,
    pub faces: usize,
    pub left_out: usize,
    pub notes: Vec<String>,
}

/// Triangulated geometries by their address in the model (a type placed
/// many times is triangulated once).
pub type Cache = HashMap<usize, Rc<Tess>>;

fn tessellate(g: &GRep) -> Tess {
    let mut out = Tess::default();
    for s in &g.solids {
        let mut points: Vec<P3> = Vec::new();
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        for f in 0..s.faces.len() {
            match face_mesh(s, f, &mut points, &mut triangles) {
                Ok(()) => out.faces += 1,
                Err(why) => {
                    out.left_out += 1;
                    if out.notes.len() < 5 {
                        out.notes.push(format!(
                            "a face ({}) is left out: {why}",
                            s.faces[f].surface.class()
                        ));
                    }
                }
            }
        }
        if triangles.is_empty() {
            continue;
        }
        // A closed body turned inside out: turned over.
        if mesh_closed(&points, &triangles) && mesh_measures(&points, &triangles).0 < 0.0 {
            for t in &mut triangles {
                t.swap(1, 2);
            }
        }
        out.bodies.push((points, triangles));
    }
    out
}

fn add_grep(
    model: &Model,
    g: &GRep,
    t: &Trf,
    depth: usize,
    seen: &mut HashSet<i64>,
    cache: &mut Cache,
    out: &mut ElementMesh,
) {
    let mirrored = determinant(t) < 0.0;
    let tess = cache
        .entry(g as *const GRep as usize)
        .or_insert_with(|| Rc::new(tessellate(g)))
        .clone();
    out.faces += tess.faces;
    out.faces_left_out += tess.left_out;
    for n in &tess.notes {
        if out.notes.len() < 5 {
            out.notes.push(n.clone());
        }
    }
    for (points, triangles) in &tess.bodies {
        let first = out.points.len() as u32;
        out.points
            .extend(points.iter().map(|p| apply(t, *p).map(|v| v * FOOT_MM)));
        out.triangles.extend(triangles.iter().map(|tr| {
            let mut tr = tr.map(|i| i + first);
            if mirrored {
                tr.swap(1, 2);
            }
            tr
        }));
        out.bodies += 1;
    }
    if depth >= DEPTH {
        if !g.instances.is_empty() {
            out.notes
                .push("types placed deeper than six levels are left out".into());
        }
        return;
    }
    for p in &g.instances {
        let placed = compose(t, &p.transform);
        if let Some(e) = &p.embedded {
            add_grep(model, e, &placed, depth + 1, seen, cache, out);
        }
        if let Some(sym) = p.symbol {
            match model.geometry.get(&sym) {
                // (A type placing itself would not end.)
                Some(sg) if seen.insert(sym) => {
                    add_grep(model, sg, &placed, depth + 1, seen, cache, out);
                    seen.remove(&sym);
                }
                Some(_) => {}
                None if p.embedded.is_none() && out.notes.len() < 5 => {
                    out.notes
                        .push(format!("the type {sym} it places keeps no geometry"));
                }
                None => {}
            }
        }
    }
}

fn dist(a: P3, b: P3) -> f64 {
    math::length(math::sub(a, b))
}

/// A loop's model points (feet): each edge's points on this face's
/// surface, or on the other face's where this one's is not read, chained
/// in the loop's direction.
fn loop_points(s: &Solid, face: usize, l: usize) -> Result<Vec<P3>, String> {
    let surface = &s.faces[face].surface;
    let mut runs: Vec<Vec<P3>> = Vec::new();
    for (e, side) in &s.faces[face].loops[l] {
        let edge = &s.edges[*e];
        let own: Option<Vec<P3>> = edge.points[*side]
            .iter()
            .map(|uv| surface.at(*uv))
            .collect();
        let run = match own {
            Some(r) => r,
            None => {
                let other = 1 - side;
                let f = edge.faces[other].ok_or("an open edge on a surface not read")?;
                edge.points[other]
                    .iter()
                    .map(|uv| s.faces[f].surface.at(*uv))
                    .collect::<Option<Vec<P3>>>()
                    .ok_or("an edge between two surfaces not read")?
            }
        };
        runs.push(run);
    }
    let mut out: Vec<P3> = Vec::new();
    let n = runs.len();
    for k in 0..n {
        let run = &runs[k];
        let (Some(first), Some(last)) = (run.first().copied(), run.last().copied()) else {
            continue;
        };
        let forward = match out.last() {
            Some(end) => dist(first, *end) <= dist(last, *end),
            None if n > 1 => {
                let next = &runs[(k + 1) % n];
                let near = |p: P3| {
                    let a = next.first().map_or(f64::INFINITY, |q| dist(p, *q));
                    let b = next.last().map_or(f64::INFINITY, |q| dist(p, *q));
                    a.min(b)
                };
                near(last) <= near(first)
            }
            None => true,
        };
        let mut pts = run.clone();
        if !forward {
            pts.reverse();
        }
        for p in pts {
            if out.last().is_none_or(|q| dist(*q, p) > 1e-9) {
                out.push(p);
            }
        }
    }
    if out.len() > 1 && dist(out[0], out[out.len() - 1]) <= 1e-9 {
        out.pop();
    }
    Ok(out)
}

/// The loop with the largest area first (the outer one), by Newell's
/// normal's length.
fn outer_first<T: Clone>(loops: &[Vec<T>], to3: impl Fn(&T) -> P3) -> Vec<Vec<T>> {
    let area = |l: &Vec<T>| {
        let mut n = [0.0; 3];
        for k in 0..l.len() {
            let (a, b) = (to3(&l[k]), to3(&l[(k + 1) % l.len()]));
            n[0] += (a[1] - b[1]) * (a[2] + b[2]);
            n[1] += (a[2] - b[2]) * (a[0] + b[0]);
            n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        math::length(n)
    };
    let mut v: Vec<Vec<T>> = loops.to_vec();
    v.sort_by(|a, b| area(b).total_cmp(&area(a)));
    v
}

/// Triangles of a polygon with holes (the first loop the outer one),
/// appended with their points.
fn polygon(loops: &[Vec<P3>], points: &mut Vec<P3>, out: &mut Vec<[u32; 3]>) -> usize {
    let mut local: Vec<P3> = Vec::new();
    let mut rings: Vec<Vec<u32>> = Vec::new();
    for l in loops {
        let first = local.len() as u32;
        local.extend_from_slice(l);
        rings.push((first..first + l.len() as u32).collect());
    }
    let tris = triangulate(&local, &rings[0], &rings[1..]);
    let base = points.len() as u32;
    points.extend_from_slice(&local);
    let n = tris.len();
    out.extend(tris.into_iter().map(|t| t.map(|i| i + base)));
    n
}

fn face_mesh(
    s: &Solid,
    face: usize,
    points: &mut Vec<P3>,
    triangles: &mut Vec<[u32; 3]>,
) -> Result<(), String> {
    let f = &s.faces[face];
    if f.loops.is_empty() {
        return Err("it has no loops".into());
    }
    match &f.surface {
        Surface::Plane { .. } | Surface::Other(_) => {
            let loops: Vec<Vec<P3>> = (0..f.loops.len())
                .map(|l| loop_points(s, face, l))
                .collect::<Result<_, _>>()?;
            let loops: Vec<Vec<P3>> = outer_first(&loops, |p| *p)
                .into_iter()
                .filter(|l| l.len() >= 3)
                .collect();
            if loops.is_empty() {
                return Err("its loops have no area".into());
            }
            if polygon(&loops, points, triangles) == 0 {
                return Err("its loops did not triangulate".into());
            }
            Ok(())
        }
        surface => {
            let loops: Vec<Vec<Uv>> = (0..f.loops.len()).map(|l| s.loop_uv(face, l)).collect();
            let loops: Vec<Vec<Uv>> = outer_first(&loops, |p| [p[0], p[1], 0.0])
                .into_iter()
                .filter(|l| l.len() >= 3)
                .collect();
            if loops.is_empty() {
                return band(s, face, surface, points, triangles);
            }
            let flat: Vec<Vec<P3>> = loops
                .iter()
                .map(|l| l.iter().map(|p| [p[0], p[1], 0.0]).collect())
                .collect();
            let mut uv_points = Vec::new();
            let mut uv_tris = Vec::new();
            if polygon(&flat, &mut uv_points, &mut uv_tris) == 0 {
                return band(s, face, surface, points, triangles);
            }
            for t in uv_tris {
                let tri = t.map(|i| {
                    let p = uv_points[i as usize];
                    [p[0], p[1]]
                });
                curved_triangle(surface, tri, 0, points, triangles)?;
            }
            Ok(())
        }
    }
}

/// A curved face's triangle in (u, v): on the surface, halved across the
/// side whose middle lies furthest off its chord while that is more than
/// [`CHORD`].
fn curved_triangle(
    surface: &Surface,
    t: [Uv; 3],
    depth: usize,
    points: &mut Vec<P3>,
    triangles: &mut Vec<[u32; 3]>,
) -> Result<(), String> {
    let at = |uv: Uv| surface.at(uv).ok_or("a surface not read");
    let p = [at(t[0])?, at(t[1])?, at(t[2])?];
    let mid = |a: Uv, b: Uv| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let mut worst = (0.0, 0);
    for k in 0..3 {
        let q = at(mid(t[k], t[(k + 1) % 3]))?;
        let c = math::scale(math::add(p[k], p[(k + 1) % 3]), 0.5);
        let d = dist(q, c);
        if d > worst.0 {
            worst = (d, k);
        }
    }
    if worst.0 > CHORD && depth < MAX_SPLITS {
        let k = worst.1;
        let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
        let m = mid(a, b);
        curved_triangle(surface, [a, m, c], depth + 1, points, triangles)?;
        curved_triangle(surface, [m, b, c], depth + 1, points, triangles)?;
        return Ok(());
    }
    let base = points.len() as u32;
    points.extend_from_slice(&p);
    triangles.push([base, base + 1, base + 2]);
    Ok(())
}

/// A band round a surface of revolution between two loops that each go
/// once round (in (u, v) each a line across the period): the strip
/// between them, in the loops' directions.
fn band(
    s: &Solid,
    face: usize,
    surface: &Surface,
    points: &mut Vec<P3>,
    triangles: &mut Vec<[u32; 3]>,
) -> Result<(), String> {
    let f = &s.faces[face];
    if f.loops.len() != 2 {
        return Err(format!(
            "its {} loops have no area in its parameters",
            f.loops.len()
        ));
    }
    let a = s.loop_uv(face, 0);
    let b = s.loop_uv(face, 1);
    if a.len() < 2 || b.len() < 2 {
        return Err("its loops have too few points".into());
    }
    // Both runs along increasing u: a as its loop runs, b against it.
    let rising = |l: &Vec<Uv>| l[l.len() - 1][0] >= l[0][0];
    let (mut a, mut b) = (a, b);
    let a_forward = rising(&a);
    if !a_forward {
        a.reverse();
    }
    let b_reversed = rising(&b);
    if b_reversed {
        b.reverse();
    }
    // (b now rising too: in its loop's direction it falls.)
    b.reverse();
    let at = |uv: Uv| surface.at(uv).ok_or("a surface not read".to_string());
    let base = points.len() as u32;
    for p in a.iter().chain(&b) {
        points.push(at(*p)?);
    }
    let (n, m) = (a.len(), b.len());
    let (mut i, mut j) = (0, 0);
    let mut tris = Vec::new();
    while i + 1 < n || j + 1 < m {
        let take_a = j + 1 >= m || (i + 1 < n && a[i + 1][0] <= b[j + 1][0]);
        if take_a {
            tris.push([base + i as u32, base + i as u32 + 1, base + (n + j) as u32]);
            i += 1;
        } else {
            tris.push([
                base + (n + j) as u32 + 1,
                base + (n + j) as u32,
                base + i as u32,
            ]);
            j += 1;
        }
    }
    // The loops' own directions: a's as built when it rose.
    let keep = a_forward;
    triangles.extend(tris.into_iter().map(|mut t| {
        if !keep {
            t.swap(1, 2);
        }
        t
    }));
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use mitcad_rvt::brep::{Edge, Face};

    /// A unit cube as the file keeps one: six planes, twelve edges.
    pub(crate) fn cube() -> Solid {
        // Faces: 0 bottom (z=0), 1 top (z=1), 2 y=0, 3 x=1, 4 y=1, 5 x=0;
        // each plane's (u, v) its model x and y, x and y, x and z, y and
        // z, x and z, y and z.
        let plane = |o: P3, x: P3, y: P3| Surface::Plane { origin: o, x, y };
        let faces = vec![
            plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            plane([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            plane([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            plane([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            plane([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ];
        // The model point on a face's (u, v).
        let uv = |f: usize, p: P3| -> Uv {
            match f {
                0 | 1 => [p[0], p[1]],
                2 | 4 => [p[0], p[2]],
                _ => [p[1], p[2]],
            }
        };
        // Each face's loop as corners, counter-clockwise seen from
        // outside.
        let corners: [[P3; 4]; 6] = [
            [
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
            ],
            [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],
            [
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
            ],
            [
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
        ];
        let mut edges: Vec<(P3, P3, [Option<usize>; 2])> = Vec::new();
        let mut loops: Vec<Vec<(usize, usize)>> = vec![Vec::new(); 6];
        for (f, c) in corners.iter().enumerate() {
            for k in 0..4 {
                let (a, b) = (c[k], c[(k + 1) % 4]);
                if let Some(e) = edges.iter().position(|(p, q, _)| *p == b && *q == a) {
                    edges[e].2[1] = Some(f);
                    loops[f].push((e, 1));
                } else {
                    edges.push((a, b, [Some(f), None]));
                    loops[f].push((edges.len() - 1, 0));
                }
            }
        }
        Solid {
            faces: faces
                .into_iter()
                .zip(loops)
                .map(|(surface, l)| Face {
                    surface,
                    loops: vec![l],
                    material: None,
                })
                .collect(),
            edges: edges
                .into_iter()
                .map(|(a, b, faces)| Edge {
                    faces,
                    points: [
                        vec![uv(faces[0].unwrap(), a), uv(faces[0].unwrap(), b)],
                        vec![uv(faces[1].unwrap(), a), uv(faces[1].unwrap(), b)],
                    ],
                })
                .collect(),
        }
    }

    #[test]
    fn a_cube_placed_by_a_type() {
        let mut model = Model::default();
        model.geometry.insert(
            7,
            GRep {
                solids: vec![cube()],
                ..GRep::default()
            },
        );
        // The instance places the type moved by (10, 0, 0) and mirrored in
        // x.
        model.geometry.insert(
            8,
            GRep {
                instances: vec![mitcad_rvt::brep::Placed {
                    symbol: Some(7),
                    transform: [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0],
                    embedded: None,
                }],
                ..GRep::default()
            },
        );
        let m = element_mesh(&model, 8, &mut Cache::new(), &HashSet::new())
            .unwrap()
            .unwrap();
        assert_eq!((m.bodies, m.faces, m.faces_left_out), (1, 6, 0));
        assert_eq!(m.triangles.len(), 12);
        assert!(m.closed);
        let v = m.volume.unwrap();
        assert!((v - FOOT_MM.powi(3)).abs() < 1e-6 * v, "{v}");
        let (lo, hi) = m.bounds().unwrap();
        assert!((lo[0] - 9.0 * FOOT_MM).abs() < 1e-9 && (hi[0] - 10.0 * FOOT_MM).abs() < 1e-9);
        // Its triangles face out: the volume they enclose is positive.
        assert!(mesh_measures(&m.points, &m.triangles).0 > 0.0);
        // A type without geometry.
        model.geometry.get_mut(&8).unwrap().instances[0].symbol = Some(99);
        assert!(
            element_mesh(&model, 8, &mut Cache::new(), &HashSet::new())
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn a_body_s_closed_mesh() {
        // mitcad#345: a void's body as a cut tool: closed, facing out.
        let (points, triangles) = solid_mesh(&cube()).unwrap();
        assert_eq!(triangles.len(), 12);
        assert!(mesh_measures(&points, &triangles).0 > 0.0);
        let mut open = cube();
        open.faces.pop();
        assert!(solid_mesh(&open).is_err());
    }

    #[test]
    fn a_cylinder_is_split_until_it_fits() {
        // A quarter cylinder face of radius 1, height 1.
        let surface = Surface::Cylinder {
            center: [0.0; 3],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
            radius: 1.0,
        };
        let mut points = Vec::new();
        let mut triangles = Vec::new();
        let q = std::f64::consts::FRAC_PI_2;
        curved_triangle(
            &surface,
            [[0.0, 0.0], [q, 0.0], [q, 1.0]],
            0,
            &mut points,
            &mut triangles,
        )
        .unwrap();
        assert!(triangles.len() > 4);
        for p in &points {
            assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-9);
        }
    }
}
