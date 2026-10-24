// SPDX-License-Identifier: MIT
//! Print Analysis (mitcad#620): where a body will print badly and what it
//! will take, worked out in Rust on the body's triangle mesh
//! ([`crate::Kernel::triangle_mesh`], the mesh the 3MF export gives a
//! slicer). Nothing here changes a body.
//!
//! - **Overhangs** ([`overhangs`]): each triangle's normal against the
//!   print direction. A triangle whose outward normal is within
//!   `90° − angle` of straight down is an overhang (`angle` is measured
//!   from the vertical, 45° by default), unless it lies on the bed. Curved
//!   faces are flagged where they are steep, not as a whole.
//! - **Thin walls** ([`thickness`]): from sample points on each triangle
//!   (its centroid, more on large triangles) a ray along the surface's
//!   inward normal (interpolated from the normals at the triangle's
//!   corners, [`corner_normals`]) to the first triangle of the same mesh
//!   where it leaves the material, through a bounding volume hierarchy of
//!   the triangles. On planar walls the value is exact (next to a tangent
//!   blend within the blend's facet angle); on curved ones it is within the
//!   mesh's deviation.
//! - **Estimate** ([`estimate`]): shell, infill and filament from the
//!   body's volume and area.
//!
//! The functions take a [`TriangleMesh`] and the settings, so they are
//! tested without a kernel (`print_analysis_tests.rs`); the queries that
//! use them are in `api/print.rs`.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use mitcad_vec3::{Vec3, add, cross, dot, norm, scale, sub, unit};
use serde_json::Value;

use crate::exchange::TriangleMesh;
use crate::monitor::Cancelled;
use crate::transform::Transform;

/// Overhang angle from the vertical, degrees.
pub const DEFAULT_ANGLE: f64 = 45.0;
/// The thinnest wall a printer makes, mm (two line widths).
pub const DEFAULT_MINIMUM: f64 = 0.8;
/// Estimate settings, mm and fractions.
pub const DEFAULT_LINE_WIDTH: f64 = 0.4;
pub const DEFAULT_WALLS: u32 = 2;
pub const DEFAULT_LAYER_HEIGHT: f64 = 0.2;
pub const DEFAULT_TOP_BOTTOM_LAYERS: u32 = 4;
pub const DEFAULT_INFILL: f64 = 0.15;
pub const DEFAULT_FILAMENT_DIAMETER: f64 = 1.75;
/// The material of bodies that have none of their own.
pub const DEFAULT_MATERIAL: &str = "pla";

/// Rays shorter than this (mm) hit nothing: the triangle a ray starts on
/// and its coplanar neighbours.
const MIN_HIT: f64 = 1e-6;
/// A wall at the minimum is not thin: the baseline tolerance (mm).
const THIN_TOLERANCE: f64 = 1e-6;
/// A large triangle gets a sample about this often (mm) along its longest
/// edge, at most [`MAX_SPLITS`] per edge.
const SAMPLE_SPACING: f64 = 2.0;
const MAX_SPLITS: usize = 8;
/// Rays between two looks at the cancel request.
const CANCEL_EVERY: usize = 256;
/// Triangles in a leaf of the hierarchy.
const LEAF_SIZE: usize = 4;
/// Triangles whose normals are within 40° of each other belong to one
/// surface when the wall's ray direction is worked out: a curved surface's
/// facets differ by the mesh's angle (15° at the 3MF export's default), the
/// faces of a box or a 45° chamfer by more.
const SMOOTH_COS: f64 = 0.766_044_443_118_978; // cos 40°

/// The mesh placed in the design: its vertices moved by `transform`, and a
/// mirroring placement's triangles turned so that they stay
/// counter-clockwise seen from outside.
pub fn placed(mesh: &TriangleMesh, transform: &Transform) -> TriangleMesh {
    let mirrored = transform.determinant() < 0.0;
    TriangleMesh {
        vertices: mesh
            .vertices
            .iter()
            .map(|v| transform.apply_point(*v))
            .collect(),
        triangles: mesh
            .triangles
            .iter()
            .map(|&[a, b, c]| if mirrored { [a, c, b] } else { [a, b, c] })
            .collect(),
    }
}

fn corners(mesh: &TriangleMesh, triangle: usize) -> [Vec3; 3] {
    mesh.triangles[triangle].map(|i| mesh.vertices[i as usize])
}

/// Twice the triangle's area times its unit normal (outward when the
/// triangle is counter-clockwise seen from outside).
fn area_normal(p: &[Vec3; 3]) -> Vec3 {
    cross(sub(p[1], p[0]), sub(p[2], p[0]))
}

/// A triangle's corners as the 9 numbers the queries answer with.
pub fn flat(mesh: &TriangleMesh, triangle: usize) -> [f64; 9] {
    let [a, b, c] = corners(mesh, triangle);
    [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]]
}

/// Where the bed is, along the print direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bed {
    /// At the body's lowest point (no bed face).
    Lowest,
    /// The plane through this point normal to the print direction (the bed
    /// face's plane).
    Through(Vec3),
}

/// A body's overhangs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overhangs {
    /// Of all triangles, mm².
    pub area: f64,
    /// Of the overhanging triangles, mm².
    pub overhang_area: f64,
    /// The overhanging triangles, indices into the mesh's.
    pub triangles: Vec<usize>,
}

impl Overhangs {
    /// The overhanging part of the area, 0 to 1.
    pub fn share(&self) -> f64 {
        if self.area > 0.0 {
            self.overhang_area / self.area
        } else {
            0.0
        }
    }
}

/// The overhangs of `mesh` printed along `down` (a unit vector, the
/// direction towards the bed) at `angle` degrees from the vertical. The
/// triangles that face down with every corner within `tolerance` of the
/// bed lie on it and are no overhangs.
pub fn overhangs(
    mesh: &TriangleMesh,
    down: Vec3,
    angle: f64,
    bed: Bed,
    tolerance: f64,
) -> Overhangs {
    let mut result = Overhangs::default();
    let bed_level = match bed {
        Bed::Through(point) => dot(point, down),
        Bed::Lowest => mesh
            .vertices
            .iter()
            .map(|v| dot(*v, down))
            .fold(f64::NEG_INFINITY, f64::max),
    };
    // Within 90° − angle of straight down: cos(90° − angle) = sin(angle).
    // A face exactly at the angle prints.
    let limit = angle.to_radians().sin() + 1e-9;
    for t in 0..mesh.triangles.len() {
        let p = corners(mesh, t);
        let n = area_normal(&p);
        let double = norm(n);
        if double.is_nan() || double <= 0.0 {
            continue;
        }
        let area = double / 2.0;
        result.area += area;
        let facing = dot(n, down) / double;
        if facing <= limit {
            continue;
        }
        let on_bed = p
            .iter()
            .all(|v| (dot(*v, down) - bed_level).abs() <= tolerance);
        if on_bed {
            continue;
        }
        result.overhang_area += area;
        result.triangles.push(t);
    }
    result
}

/// A body's walls.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Thickness {
    /// The thinnest wall found, mm; None when no ray met the other side
    /// (an open mesh, or no triangles).
    pub thinnest: Option<f64>,
    /// Where it is: the sample point on the surface.
    pub at: Option<Vec3>,
    /// The point on the other side of that wall.
    pub to: Option<Vec3>,
    /// The triangles of walls thinner than the minimum.
    pub thin: Vec<usize>,
    /// The thin triangles in connected groups: those sharing a corner, and
    /// the two sides of one wall.
    pub regions: usize,
}

/// A ray's first exit from the material.
#[derive(Debug, Clone, Copy)]
struct Hit {
    distance: f64,
    from: Vec3,
    to: Vec3,
    triangle: usize,
}

/// The walls of `mesh` (closed, counter-clockwise seen from outside), and
/// those thinner than `minimum` (mm). Stops with [`Cancelled`] once
/// `cancelled` says so; the samples run on all cores.
pub fn thickness(
    mesh: &TriangleMesh,
    minimum: f64,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Thickness, Cancelled> {
    let count = mesh.triangles.len();
    let bvh = Bvh::new(mesh);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(count / 512 + 1)
        .max(1);
    let chunk = count.div_ceil(threads).max(1);
    let mut hits: Vec<Option<Hit>> = vec![None; count];
    let stopped = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        for (part, slots) in hits.chunks_mut(chunk).enumerate() {
            let (bvh, stopped) = (&bvh, &stopped);
            scope.spawn(move || {
                let first = part * chunk;
                for (i, slot) in slots.iter_mut().enumerate() {
                    if i % CANCEL_EVERY == 0
                        && (stopped.load(std::sync::atomic::Ordering::Relaxed) || cancelled())
                    {
                        stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                    *slot = bvh.wall(first + i);
                }
            });
        }
    });
    if stopped.into_inner() {
        return Err(Cancelled);
    }
    let mut result = Thickness::default();
    let mut thinnest: Option<Hit> = None;
    let mut thin = vec![false; count];
    for (t, hit) in hits.iter().enumerate() {
        let Some(hit) = hit else { continue };
        if thinnest.is_none_or(|best| hit.distance < best.distance) {
            thinnest = Some(*hit);
        }
        if hit.distance < minimum - THIN_TOLERANCE {
            thin[t] = true;
            result.thin.push(t);
        }
    }
    if let Some(best) = thinnest {
        result.thinnest = Some(best.distance);
        result.at = Some(best.from);
        result.to = Some(best.to);
    }
    // Regions: thin triangles that share a corner, and a thin triangle with
    // the thin one its ray met (the wall's other side).
    let mut groups = UnionFind::new(count);
    let mut by_vertex: Vec<Option<usize>> = vec![None; mesh.vertices.len()];
    for &t in &result.thin {
        for v in mesh.triangles[t] {
            match by_vertex[v as usize] {
                Some(other) => groups.join(t, other),
                None => by_vertex[v as usize] = Some(t),
            }
        }
        if let Some(hit) = hits[t]
            && thin[hit.triangle]
        {
            groups.join(t, hit.triangle);
        }
    }
    let mut roots: Vec<usize> = result.thin.iter().map(|&t| groups.find(t)).collect();
    roots.sort_unstable();
    roots.dedup();
    result.regions = roots.len();
    Ok(result)
}

struct UnionFind(Vec<usize>);

impl UnionFind {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }

    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

/// The sample points of a triangle: the centroids of a k × k subdivision,
/// k growing with the longest edge (the centroid alone on small ones), each
/// with its barycentric weights of the triangle's corners.
fn samples(p: &[Vec3; 3]) -> Vec<(Vec3, [f64; 3])> {
    let longest = [
        norm(sub(p[1], p[0])),
        norm(sub(p[2], p[1])),
        norm(sub(p[0], p[2])),
    ]
    .into_iter()
    .fold(0.0, f64::max);
    let k = ((longest / SAMPLE_SPACING).ceil() as usize).clamp(1, MAX_SPLITS);
    let (e1, e2) = (sub(p[1], p[0]), sub(p[2], p[0]));
    let at = |a: f64, b: f64| {
        let (u, v) = (a / k as f64, b / k as f64);
        (
            add(p[0], add(scale(e1, u), scale(e2, v))),
            [1.0 - u - v, u, v],
        )
    };
    let mut out = Vec::with_capacity(k * k);
    for i in 0..k {
        for j in 0..k - i {
            let (i, j) = (i as f64, j as f64);
            out.push(at(i + 1.0 / 3.0, j + 1.0 / 3.0));
            if i + j + 1.0 < k as f64 {
                out.push(at(i + 2.0 / 3.0, j + 2.0 / 3.0));
            }
        }
    }
    out
}

/// The surface's unit normal at each corner of each triangle, from the
/// triangles that meet at the corner's vertex: their area normals added up,
/// of those facing within [`SMOOTH_COS`] of the triangle (so the normal of
/// a curved surface, the triangle's own on a planar face that meets others
/// at an edge). A ray along a triangle's own normal misses the surface's
/// normal on a curved face by the triangle's tilt, which a skewed triangle
/// of a fine mesh makes large (a sphere's wall then measures a chord
/// beside its centre, 1.3 % short at a 0.01 mm deviation); the interpolated
/// corner normals follow the surface. The triangle's own normal where the
/// mesh shares no vertices.
fn corner_normals(mesh: &TriangleMesh, triangles: &[([Vec3; 3], Vec3)]) -> Vec<[Vec3; 3]> {
    let mut incident: Vec<Vec<usize>> = vec![Vec::new(); mesh.vertices.len()];
    for (t, corners) in mesh.triangles.iter().enumerate() {
        for v in corners {
            incident[*v as usize].push(t);
        }
    }
    let units: Vec<Option<Vec3>> = triangles.iter().map(|(_, n)| unit(*n)).collect();
    mesh.triangles
        .iter()
        .enumerate()
        .map(|(t, corners)| {
            let Some(own) = units[t] else {
                return [[0.0; 3]; 3];
            };
            (*corners).map(|v| {
                let mut sum = [0.0; 3];
                for &s in &incident[v as usize] {
                    if units[s].is_some_and(|n| dot(n, own) >= SMOOTH_COS) {
                        sum = add(sum, triangles[s].1);
                    }
                }
                unit(sum).unwrap_or(own)
            })
        })
        .collect()
}

/// A bounding volume hierarchy of a mesh's triangles: nodes in depth-first
/// order, a leaf holding a run of `order`.
struct Bvh {
    /// Each triangle's corners and its area normal.
    triangles: Vec<([Vec3; 3], Vec3)>,
    /// The surface's unit normal at each triangle's corners
    /// ([`corner_normals`]).
    normals: Vec<[Vec3; 3]>,
    nodes: Vec<Node>,
    order: Vec<usize>,
}

struct Node {
    min: Vec3,
    max: Vec3,
    /// A leaf: the first of its triangles in `order`; else the index of the
    /// second child (the first follows the node).
    index: usize,
    /// A leaf's number of triangles, 0 for inner nodes.
    count: usize,
}

impl Bvh {
    fn new(mesh: &TriangleMesh) -> Self {
        let triangles: Vec<([Vec3; 3], Vec3)> = (0..mesh.triangles.len())
            .map(|t| {
                let p = corners(mesh, t);
                (p, area_normal(&p))
            })
            .collect();
        let normals = corner_normals(mesh, &triangles);
        let mut bvh = Self {
            triangles,
            normals,
            nodes: Vec::new(),
            order: (0..mesh.triangles.len()).collect(),
        };
        if !bvh.order.is_empty() {
            let count = bvh.order.len();
            bvh.build(0, count);
        }
        bvh
    }

    fn centroid(&self, t: usize) -> Vec3 {
        let p = &self.triangles[t].0;
        scale(add(add(p[0], p[1]), p[2]), 1.0 / 3.0)
    }

    /// Builds the node of `order[start..end]`; returns its index.
    fn build(&mut self, start: usize, end: usize) -> usize {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        let mut cmin = [f64::INFINITY; 3];
        let mut cmax = [f64::NEG_INFINITY; 3];
        for &t in &self.order[start..end] {
            for p in &self.triangles[t].0 {
                min = std::array::from_fn(|a| min[a].min(p[a]));
                max = std::array::from_fn(|a| max[a].max(p[a]));
            }
            let c = self.centroid(t);
            cmin = std::array::from_fn(|a| cmin[a].min(c[a]));
            cmax = std::array::from_fn(|a| cmax[a].max(c[a]));
        }
        let node = self.nodes.len();
        self.nodes.push(Node {
            min,
            max,
            index: start,
            count: end - start,
        });
        if end - start <= LEAF_SIZE {
            return node;
        }
        let axis = (0..3)
            .max_by(|&a, &b| (cmax[a] - cmin[a]).total_cmp(&(cmax[b] - cmin[b])))
            .unwrap_or(0);
        let middle = start + (end - start) / 2;
        let triangles = &self.triangles;
        let key = |t: &usize| {
            let p = &triangles[*t].0;
            p[0][axis] + p[1][axis] + p[2][axis]
        };
        self.order[start..end]
            .select_nth_unstable_by(middle - start, |a, b| key(a).total_cmp(&key(b)));
        self.nodes[node].count = 0;
        self.build(start, middle);
        let second = self.build(middle, end);
        self.nodes[node].index = second;
        node
    }

    /// The thinnest wall under triangle `t`: of its samples, the shortest
    /// ray along the surface's inward normal there (the corners' normals
    /// interpolated) to where it leaves the material.
    fn wall(&self, t: usize) -> Option<Hit> {
        let (p, n) = &self.triangles[t];
        let own = unit(*n)?;
        let corners = &self.normals[t];
        let mut best: Option<Hit> = None;
        for (from, w) in samples(p) {
            let blended = add(
                add(scale(corners[0], w[0]), scale(corners[1], w[1])),
                scale(corners[2], w[2]),
            );
            let outward = unit(blended)
                .filter(|s| dot(*s, own) >= SMOOTH_COS)
                .unwrap_or(own);
            let inward = scale(outward, -1.0);
            if let Some((distance, triangle)) = self.first_exit(from, inward, t)
                && best.is_none_or(|b| distance < b.distance)
            {
                best = Some(Hit {
                    distance,
                    from,
                    to: add(from, scale(inward, distance)),
                    triangle,
                });
            }
        }
        best
    }

    /// The nearest triangle but `skip` that the ray from `origin` along
    /// `direction` (unit) meets from inside (its outward normal along the
    /// ray): the distance and the triangle.
    fn first_exit(&self, origin: Vec3, direction: Vec3, skip: usize) -> Option<(f64, usize)> {
        if self.nodes.is_empty() {
            return None;
        }
        let inverse = direction.map(|d| 1.0 / d);
        let mut nearest = f64::INFINITY;
        let mut found = None;
        let mut stack = vec![0usize];
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            if !box_hit(node, origin, inverse, nearest) {
                continue;
            }
            if node.count == 0 {
                stack.push(node.index);
                stack.push(index + 1);
                continue;
            }
            for &t in &self.order[node.index..node.index + node.count] {
                if t == skip {
                    continue;
                }
                let (p, n) = &self.triangles[t];
                if dot(*n, direction) <= 0.0 {
                    continue;
                }
                if let Some(d) = ray_triangle(origin, direction, p)
                    && d > MIN_HIT
                    && d < nearest
                {
                    nearest = d;
                    found = Some(t);
                }
            }
        }
        found.map(|t| (nearest, t))
    }
}

/// Whether the ray enters the node's box nearer than `limit`.
fn box_hit(node: &Node, origin: Vec3, inverse: Vec3, limit: f64) -> bool {
    let mut near = 0.0_f64;
    let mut far = limit;
    let slabs = node.min.iter().zip(&node.max).zip(origin).zip(inverse);
    for (((low, high), origin), inverse) in slabs {
        // A ray parallel to the slab: inside it or not at all.
        if !inverse.is_finite() {
            if origin < low - 1e-9 || origin > high + 1e-9 {
                return false;
            }
            continue;
        }
        let t1 = (low - origin) * inverse;
        let t2 = (high - origin) * inverse;
        let (lo, hi) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };
        near = near.max(lo);
        far = far.min(hi);
        if near > far + 1e-9 {
            return false;
        }
    }
    true
}

/// The distance along `direction` to the triangle, edges included (the
/// classic ray and triangle test by barycentric coordinates).
fn ray_triangle(origin: Vec3, direction: Vec3, p: &[Vec3; 3]) -> Option<f64> {
    const EPS: f64 = 1e-12;
    const EDGE: f64 = 1e-9;
    let e1 = sub(p[1], p[0]);
    let e2 = sub(p[2], p[0]);
    let h = cross(direction, e2);
    let det = dot(e1, h);
    if det.abs() < EPS {
        return None;
    }
    let f = 1.0 / det;
    let s = sub(origin, p[0]);
    let u = f * dot(s, h);
    if !(-EDGE..=1.0 + EDGE).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = f * dot(direction, q);
    if v < -EDGE || u + v > 1.0 + EDGE {
        return None;
    }
    Some(f * dot(e2, q))
}

/// The settings of an estimate (mm, fractions, a price per kilogram).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstimateSettings {
    pub line_width: f64,
    pub walls: u32,
    pub layer_height: f64,
    pub top_bottom_layers: u32,
    pub infill: f64,
    pub filament_diameter: f64,
    pub price_per_kg: Option<f64>,
}

impl Default for EstimateSettings {
    fn default() -> Self {
        Self {
            line_width: DEFAULT_LINE_WIDTH,
            walls: DEFAULT_WALLS,
            layer_height: DEFAULT_LAYER_HEIGHT,
            top_bottom_layers: DEFAULT_TOP_BOTTOM_LAYERS,
            infill: DEFAULT_INFILL,
            filament_diameter: DEFAULT_FILAMENT_DIAMETER,
            price_per_kg: None,
        }
    }
}

impl EstimateSettings {
    /// Why the settings make no estimate, if they do not.
    pub fn check(&self) -> Result<(), String> {
        let positive = |value: f64, what: &str| {
            if value > 0.0 && value.is_finite() {
                Ok(())
            } else {
                Err(format!("the {what} must be greater than zero, got {value}"))
            }
        };
        positive(self.line_width, "line width")?;
        positive(self.layer_height, "layer height")?;
        positive(self.filament_diameter, "filament diameter")?;
        if !(0.0..=1.0).contains(&self.infill) {
            return Err(format!(
                "the infill must be a fraction from 0 to 1, got {}",
                self.infill
            ));
        }
        if let Some(price) = self.price_per_kg
            && !(price >= 0.0 && price.is_finite())
        {
            return Err(format!("the price must not be negative, got {price}"));
        }
        Ok(())
    }
}

/// What printing a body takes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Estimate {
    /// The body's, mm³ and mm².
    pub volume: f64,
    pub area: f64,
    /// The walls: area × walls × line width (the top and bottom layers
    /// counted in it), at most the volume, mm³.
    pub shell_volume: f64,
    /// (volume − shell) × infill, mm³.
    pub infill_volume: f64,
    /// shell + infill, mm³.
    pub filament_volume: f64,
    /// Of the filament, g.
    pub mass: f64,
    /// Of the filament, m.
    pub length: f64,
    /// mass × price per kilogram, when a price is given.
    pub cost: Option<f64>,
}

/// The estimate of a body of `volume` (mm³) and `area` (mm²) printed in a
/// material of `density` (g/cm³).
pub fn estimate(volume: f64, area: f64, density: f64, settings: &EstimateSettings) -> Estimate {
    let volume = volume.max(0.0);
    let area = area.max(0.0);
    let shell_volume = (area * f64::from(settings.walls) * settings.line_width).min(volume);
    let infill_volume = (volume - shell_volume) * settings.infill;
    let filament_volume = shell_volume + infill_volume;
    // mm³ → cm³.
    let mass = filament_volume / 1000.0 * density;
    let radius = settings.filament_diameter / 2.0;
    // mm → m.
    let length = filament_volume / (std::f64::consts::PI * radius * radius) / 1000.0;
    Estimate {
        volume,
        area,
        shell_volume,
        infill_volume,
        filament_volume,
        mass,
        length,
        cost: settings.price_per_kg.map(|price| price * mass / 1000.0),
    }
}

/// Bounds of what the print cache keeps: meshes are large.
const CACHED_MESHES: usize = 8;
const CACHED_RESULTS: usize = 32;

/// The meshes and results of print analyses (mitcad#620), by the
/// fingerprint of what they depend on (the body's shape version, its
/// placement, the mesh tolerance and the settings), so that showing an
/// analysis again computes nothing. Separate from modelling results:
/// analysing changes no state. The oldest go first.
#[derive(Default)]
pub(crate) struct PrintCache {
    inner: Mutex<CacheEntries>,
}

#[derive(Default)]
struct CacheEntries {
    meshes: VecDeque<(u128, Arc<TriangleMesh>)>,
    results: VecDeque<(u128, Arc<Value>)>,
}

fn find<T>(entries: &VecDeque<(u128, Arc<T>)>, key: u128) -> Option<Arc<T>> {
    entries
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| Arc::clone(v))
}

fn keep<T>(entries: &mut VecDeque<(u128, Arc<T>)>, key: u128, value: Arc<T>, bound: usize) {
    entries.retain(|(k, _)| *k != key);
    entries.push_back((key, value));
    while entries.len() > bound {
        entries.pop_front();
    }
}

impl PrintCache {
    pub(crate) fn mesh(&self, key: u128) -> Option<Arc<TriangleMesh>> {
        self.inner.lock().ok().and_then(|c| find(&c.meshes, key))
    }

    pub(crate) fn keep_mesh(&self, key: u128, mesh: Arc<TriangleMesh>) {
        if let Ok(mut c) = self.inner.lock() {
            keep(&mut c.meshes, key, mesh, CACHED_MESHES);
        }
    }

    pub(crate) fn result(&self, key: u128) -> Option<Arc<Value>> {
        self.inner.lock().ok().and_then(|c| find(&c.results, key))
    }

    pub(crate) fn keep_result(&self, key: u128, result: Arc<Value>) {
        if let Ok(mut c) = self.inner.lock() {
            keep(&mut c.results, key, result, CACHED_RESULTS);
        }
    }
}
