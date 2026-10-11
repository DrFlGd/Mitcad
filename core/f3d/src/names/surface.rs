// SPDX-License-Identifier: MIT
//! A point inside a face of any surface type (mitcad#161): feature inputs
//! that are spline faces (a thread's flank as the guide surface of a
//! sweep) are found in the replay by a point inside them, as planar and
//! cylindrical faces are.
//!
//! - Free-form surfaces (B-splines, the ruled surfaces of thread flanks):
//!   the face's edges are taken into the surface's parameters (each edge
//!   point at the parameters of the nearest point of a grid over the
//!   surface, refined by Newton steps), and the point is the one inside
//!   that outline farthest from it ([`super::point_inside`] in the
//!   parameter plane), on the surface. On a ruled surface the point lies
//!   on one of the rulings the surface was built from, where the surface
//!   is exact.
//! - Cones, spheres and tori: as for cylinders, half way between the
//!   lowest and highest edge points (along the axis, in latitude, round
//!   the tube) on the far side of the widest gap between the edges' angles
//!   round the axis.

use crate::brep::{self, Curve, P3, Surface};

/// A point inside a face and the face's normal there (with the face's
/// orientation), in the body's coordinates (its transform not applied).
#[derive(Clone, Debug, PartialEq)]
pub struct Inside {
    pub point: P3,
    pub normal: P3,
    /// More points inside a free-form face, spread over it (on lines
    /// across its outline in the surface's parameters), by which the
    /// import finds every piece of it where the replay splits the face
    /// (a thread's flank, one face over many turns in the file, one per
    /// turn in the replay). Empty for other faces.
    pub spread: Vec<P3>,
}

/// A point inside the face of `surface` bounded by `edges` (points along
/// each edge, in order); `reversed`: the face's normal runs against the
/// surface's.
pub fn point_inside_face(
    surface: &Surface,
    reversed: bool,
    edges: &[&[P3]],
) -> Result<Inside, String> {
    if edges.iter().all(|e| e.is_empty()) {
        return Err("the face has no edges".to_owned());
    }
    let inside = match surface {
        Surface::BSpline(_) | Surface::Ruled { .. } => free_form(surface, edges)?,
        Surface::Cone {
            origin,
            axis,
            ref_dir,
            radius,
            half_angle,
        } => revolved(*origin, *axis, *ref_dir, edges, |angle, h| {
            // `h`: along the axis.
            let radial = round(*axis, *ref_dir, angle);
            let (s, c) = half_angle.sin_cos();
            let point = brep::add(
                brep::add(*origin, brep::scale(*axis, h)),
                brep::scale(radial, radius + h * half_angle.tan()),
            );
            (
                point,
                brep::sub(brep::scale(radial, c), brep::scale(*axis, s)),
            )
        }),
        Surface::Sphere {
            center,
            axis,
            ref_dir,
            radius,
        } => {
            // Latitudes instead of heights.
            let latitude = |p: P3| {
                let d = brep::sub(p, *center);
                (brep::dot(d, *axis) / brep::norm(d).max(1e-300))
                    .clamp(-1.0, 1.0)
                    .asin()
            };
            let lats: Vec<f64> = edges
                .iter()
                .flat_map(|e| e.iter())
                .map(|&p| latitude(p))
                .collect();
            let (low, high) = span(&lats);
            let angle = far_angle(*center, *axis, *ref_dir, edges);
            let lat = 0.5 * (low + high);
            let radial = round(*axis, *ref_dir, angle);
            let normal = brep::add(
                brep::scale(radial, lat.cos()),
                brep::scale(*axis, lat.sin()),
            );
            Inside {
                point: brep::add(*center, brep::scale(normal, *radius)),
                normal,
                spread: Vec::new(),
            }
        }
        Surface::Torus {
            center,
            axis,
            ref_dir,
            major,
            minor,
        } => {
            let angle = far_angle(*center, *axis, *ref_dir, edges);
            let radial = round(*axis, *ref_dir, angle);
            // Round the tube, in the half-plane of `angle`: the widest gap
            // between the edges' tube angles, as round the axis.
            let tube = |p: P3| {
                let d = brep::sub(p, *center);
                let h = brep::dot(d, *axis);
                let rho = brep::norm(brep::sub(d, brep::scale(*axis, h)));
                h.atan2(rho - major)
            };
            let angles: Vec<f64> = edges
                .iter()
                .flat_map(|e| e.iter())
                .map(|&p| tube(p))
                .collect();
            let t = gap_angle(angles);
            let normal = brep::add(brep::scale(radial, t.cos()), brep::scale(*axis, t.sin()));
            Inside {
                point: brep::add(
                    brep::add(*center, brep::scale(radial, *major)),
                    brep::scale(normal, *minor),
                ),
                normal,
                spread: Vec::new(),
            }
        }
        other => {
            return Err(format!(
                "no point inside a face of a {} decoded",
                other.kind()
            ));
        }
    };
    let mut inside = inside;
    if reversed {
        inside.normal = brep::scale(inside.normal, -1.0);
    }
    Ok(inside)
}

/// The unit direction at `angle` round `axis` from `ref_dir`.
fn round(axis: P3, ref_dir: P3, angle: f64) -> P3 {
    let y = brep::cross(axis, ref_dir);
    brep::add(
        brep::scale(ref_dir, angle.cos()),
        brep::scale(y, angle.sin()),
    )
}

fn span(values: &[f64]) -> (f64, f64) {
    values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        })
}

/// The angle in the middle of a face round an axis: on the far side of the
/// widest gap between the edge points' angles where that gap is wide (a
/// part of a turn), else in the gap (a whole turn: any angle).
fn gap_angle(mut angles: Vec<f64>) -> f64 {
    if angles.is_empty() {
        return 0.0;
    }
    angles.sort_by(f64::total_cmp);
    let tau = std::f64::consts::TAU;
    let last = angles[angles.len() - 1];
    let mut gap = angles[0] + tau - last;
    let mut at = last + gap / 2.0;
    for w in angles.windows(2) {
        if w[1] - w[0] > gap {
            gap = w[1] - w[0];
            at = (w[0] + w[1]) / 2.0;
        }
    }
    if gap > std::f64::consts::FRAC_PI_6 {
        at += std::f64::consts::PI;
    }
    at
}

/// [`gap_angle`] of the edge points round an axis through `origin`.
fn far_angle(origin: P3, axis: P3, ref_dir: P3, edges: &[&[P3]]) -> f64 {
    let y = brep::cross(axis, ref_dir);
    let angles = edges
        .iter()
        .flat_map(|e| e.iter())
        .map(|&p| {
            let d = brep::sub(p, origin);
            brep::dot(d, y).atan2(brep::dot(d, ref_dir))
        })
        .collect();
    gap_angle(angles)
}

/// A face of a surface of revolution about `axis` through `origin`: half
/// way along the axis between the edge points, at [`far_angle`]; `at`
/// gives the point and the surface's normal at an angle and a height.
fn revolved(
    origin: P3,
    axis: P3,
    ref_dir: P3,
    edges: &[&[P3]],
    at: impl Fn(f64, f64) -> (P3, P3),
) -> Inside {
    let heights: Vec<f64> = edges
        .iter()
        .flat_map(|e| e.iter())
        .map(|&p| brep::dot(brep::sub(p, origin), axis))
        .collect();
    let (low, high) = span(&heights);
    let (point, normal) = at(far_angle(origin, axis, ref_dir, edges), 0.5 * (low + high));
    Inside {
        point,
        normal,
        spread: Vec::new(),
    }
}

/// A free-form surface's parameters: its domain, the grid of parameters
/// edge points are first matched to, and its points.
struct Patch<'a> {
    surface: &'a Surface,
    u: (f64, f64),
    v: (f64, f64),
    /// Grid parameters in u and v.
    us: Vec<f64>,
    vs: Vec<f64>,
    periodic: [bool; 2],
}

impl<'a> Patch<'a> {
    fn new(surface: &'a Surface) -> Result<Self, String> {
        let even = |(a, b): (f64, f64), n: usize| -> Vec<f64> {
            (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
        };
        match surface {
            Surface::BSpline(b) => {
                let (u, v) = (b.u_range(), b.v_range());
                // A few grid lines per knot span (thread flanks run along
                // many spans).
                let nu = (4 * b.u_knots.len()).clamp(32, 2000);
                let nv = (4 * b.v_knots.len()).clamp(32, 2000);
                Ok(Patch {
                    surface,
                    u,
                    v,
                    us: even(u, nu),
                    vs: even(v, nv),
                    periodic: [b.u_periodic, b.v_periodic],
                })
            }
            Surface::Ruled { from, to } => {
                let range = from
                    .range()
                    .filter(|_| to.range() == from.range())
                    .ok_or("a ruled surface of unbounded curves")?;
                // The curves' own parameters: the rulings the surface was
                // built from, where it is exact.
                let vs = match from {
                    Curve::Interpolated { params, .. } => params.clone(),
                    _ => even(range, 512),
                };
                Ok(Patch {
                    surface,
                    u: (0.0, 1.0),
                    v: range,
                    us: even((0.0, 1.0), 32),
                    vs,
                    periodic: [false, false],
                })
            }
            other => Err(format!("{} is not a free-form surface", other.kind())),
        }
    }

    fn eval(&self, u: f64, v: f64) -> P3 {
        match self.surface {
            Surface::BSpline(b) => b.eval(u, v),
            Surface::Ruled { from, to } => brep::add(
                brep::scale(from.eval(v), 1.0 - u),
                brep::scale(to.eval(v), u),
            ),
            _ => [f64::NAN; 3],
        }
    }

    /// The partial derivatives at (u, v), by differences inside the
    /// domain.
    fn derivatives(&self, u: f64, v: f64) -> (P3, P3) {
        let hu = (self.u.1 - self.u.0) * 1e-6;
        let hv = (self.v.1 - self.v.0) * 1e-6;
        let (u0, u1) = ((u - hu).max(self.u.0), (u + hu).min(self.u.1));
        let (v0, v1) = ((v - hv).max(self.v.0), (v + hv).min(self.v.1));
        let du = brep::scale(
            brep::sub(self.eval(u1, v), self.eval(u0, v)),
            1.0 / (u1 - u0).max(1e-300),
        );
        let dv = brep::scale(
            brep::sub(self.eval(u, v1), self.eval(u, v0)),
            1.0 / (v1 - v0).max(1e-300),
        );
        (du, dv)
    }

    /// The parameters of the surface point nearest `p`, from the grid
    /// point nearest it.
    fn invert(&self, grid: &[P3], p: P3) -> (f64, f64) {
        let nv = self.vs.len();
        let k = (0..grid.len())
            .min_by(|&a, &b| brep::dist(grid[a], p).total_cmp(&brep::dist(grid[b], p)))
            .unwrap_or(0);
        let (mut u, mut v) = (self.us[k / nv], self.vs[k % nv]);
        // Newton steps on the distance (Gauss-Newton: the normal
        // equations of the tangent plane).
        for _ in 0..8 {
            let s = self.eval(u, v);
            let (du, dv) = self.derivatives(u, v);
            let d = brep::sub(p, s);
            let (a, b, c) = (brep::dot(du, du), brep::dot(du, dv), brep::dot(dv, dv));
            let det = a * c - b * b;
            if det.abs() < 1e-300 {
                break;
            }
            let (ru, rv) = (brep::dot(d, du), brep::dot(d, dv));
            let step_u = (c * ru - b * rv) / det;
            let step_v = (a * rv - b * ru) / det;
            u = (u + step_u).clamp(self.u.0, self.u.1);
            v = (v + step_v).clamp(self.v.0, self.v.1);
            if step_u.abs() <= 1e-12 * (self.u.1 - self.u.0)
                && step_v.abs() <= 1e-12 * (self.v.1 - self.v.0)
            {
                break;
            }
        }
        (u, v)
    }
}

/// [`point_inside_face`] of a free-form surface: the point inside the
/// face's outline in the surface's parameters farthest from it.
fn free_form(surface: &Surface, edges: &[&[P3]]) -> Result<Inside, String> {
    let patch = Patch::new(surface)?;
    let grid: Vec<P3> = patch
        .us
        .iter()
        .flat_map(|&u| patch.vs.iter().map(move |&v| (u, v)))
        .map(|(u, v)| patch.eval(u, v))
        .collect();
    let mut flat: Vec<Vec<[f64; 2]>> = edges
        .iter()
        .map(|e| {
            e.iter()
                .map(|&p| {
                    let (u, v) = patch.invert(&grid, p);
                    [u, v]
                })
                .collect()
        })
        .collect();
    // A periodic direction: its seam where the outline leaves its widest
    // gap.
    for (k, periodic) in patch.periodic.iter().enumerate() {
        if !periodic {
            continue;
        }
        let (a, b) = if k == 0 { patch.u } else { patch.v };
        let period = b - a;
        let mut values: Vec<f64> = flat.iter().flatten().map(|p| p[k]).collect();
        values.sort_by(f64::total_cmp);
        let mut cut = a;
        let mut gap = values.first().map_or(0.0, |f| f - a) + values.last().map_or(0.0, |l| b - l);
        for w in values.windows(2) {
            if w[1] - w[0] > gap {
                gap = w[1] - w[0];
                cut = 0.5 * (w[0] + w[1]);
            }
        }
        for p in flat.iter_mut().flatten() {
            if p[k] < cut {
                p[k] += period;
            }
        }
    }
    // The parameter plane with both directions scaled to the outline's
    // extent, so that the point keeps clear of the edges both ways.
    let all: Vec<[f64; 2]> = flat.iter().flatten().copied().collect();
    let (ul, uh) = span(&all.iter().map(|p| p[0]).collect::<Vec<_>>());
    let (vl, vh) = span(&all.iter().map(|p| p[1]).collect::<Vec<_>>());
    if !(uh > ul && vh > vl) {
        return Err("the face's edges do not enclose any of its surface".to_owned());
    }
    let to_plane = |p: [f64; 2]| [(p[0] - ul) / (uh - ul), (p[1] - vl) / (vh - vl), 0.0];
    let lines: Vec<Vec<P3>> = flat
        .iter()
        .map(|e| e.iter().map(|&p| to_plane(p)).collect())
        .collect();
    let line_refs: Vec<&[P3]> = lines.iter().map(Vec::as_slice).collect();
    let mean = all.iter().fold([0.0; 2], |s, p| [s[0] + p[0], s[1] + p[1]]);
    let n = all.len() as f64;
    let guess = to_plane([mean[0] / n, mean[1] / n]);
    let q = super::point_inside([0.0, 0.0, 1.0], &line_refs, guess);
    let wrap = |x: f64, (a, b): (f64, f64), periodic: bool| {
        if periodic && x > b {
            x - (b - a)
        } else {
            x.clamp(a, b)
        }
    };
    // A point of the parameter plane on the surface: on a ruled surface on
    // the nearest ruling it was built from.
    let on_surface = |x: f64, y: f64| {
        let u = wrap(ul + x * (uh - ul), patch.u, patch.periodic[0]);
        let mut v = wrap(vl + y * (vh - vl), patch.v, patch.periodic[1]);
        if matches!(surface, Surface::Ruled { .. })
            && let Some(&near) = patch
                .vs
                .iter()
                .min_by(|a, b| (*a - v).abs().total_cmp(&(*b - v).abs()))
        {
            v = near;
        }
        (u, v)
    };
    let (u, v) = on_surface(q[0], q[1]);
    let point = patch.eval(u, v);
    let (du, dv) = patch.derivatives(u, v);
    let normal = brep::normalize(brep::cross(du, dv));
    if !point.iter().chain(&normal).all(|x| x.is_finite()) {
        return Err("no point inside the face found".to_owned());
    }
    // More points: the middles of the stretches inside on lines across
    // the outline at evenly spaced heights in each direction.
    let segments: Vec<([f64; 2], [f64; 2])> = lines
        .iter()
        .flat_map(|e| {
            e.windows(2)
                .map(|w| ([w[0][0], w[0][1]], [w[1][0], w[1][1]]))
        })
        .filter(|(a, b)| a != b)
        .collect();
    let mut spread = Vec::new();
    for axis in 0..2 {
        let other = 1 - axis;
        for k in 0..SPREAD_LINES {
            let at = (k as f64 + 0.5) / SPREAD_LINES as f64;
            let mut xs: Vec<f64> = segments
                .iter()
                .filter(|(a, b)| (a[axis] <= at) != (b[axis] <= at))
                .map(|(a, b)| {
                    a[other] + (at - a[axis]) / (b[axis] - a[axis]) * (b[other] - a[other])
                })
                .collect();
            if !xs.len().is_multiple_of(2) {
                continue;
            }
            xs.sort_by(f64::total_cmp);
            for pair in xs.chunks(2) {
                let mid = 0.5 * (pair[0] + pair[1]);
                let (x, y) = if axis == 0 { (at, mid) } else { (mid, at) };
                let (u, v) = on_surface(x, y);
                let p = patch.eval(u, v);
                if p.iter().all(|c| c.is_finite()) {
                    spread.push(p);
                }
            }
        }
    }
    Ok(Inside {
        point,
        normal,
        spread,
    })
}

/// Lines across a free-form face's outline in each direction of its
/// parameters, for [`Inside::spread`].
const SPREAD_LINES: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of a ruled surface between two helices (a thread's flank):
    /// the point lies between the helices, half way along the strip.
    #[test]
    fn a_point_inside_a_ruled_flank() {
        let helix = |r: f64, z: f64, t: f64| [r * t.cos(), r * t.sin(), z + t];
        let params: Vec<f64> = (0..=96).map(|i| -0.2 + 12.97 * i as f64 / 96.0).collect();
        let curve = |r: f64, z: f64| Curve::Interpolated {
            params: params.clone(),
            points: params.iter().map(|&t| helix(r, z, t)).collect(),
        };
        let surface = Surface::Ruled {
            from: curve(10.0, 0.0),
            to: curve(12.0, 1.0),
        };
        // The face: from 0 to 4 turns of the parameter, between u 0.1 and
        // 0.9 (two helices and two rulings).
        let at = |u: f64, t: f64| {
            let (a, b) = (helix(10.0, 0.0, t), helix(12.0, 1.0, t));
            brep::add(brep::scale(a, 1.0 - u), brep::scale(b, u))
        };
        let end = 4.0 * std::f64::consts::PI;
        let along =
            |u: f64| -> Vec<P3> { (0..=64).map(|i| at(u, end * i as f64 / 64.0)).collect() };
        let across = |t: f64| -> Vec<P3> { (0..=8).map(|i| at(0.1 + 0.1 * i as f64, t)).collect() };
        let (low, high, start, stop) = (along(0.1), along(0.9), across(0.0), across(end));
        let edges: Vec<&[P3]> = vec![&low, &stop, &high, &start];
        let inside = point_inside_face(&surface, false, &edges).unwrap();
        let p = inside.point;
        // On the surface (a ruling) between the helices.
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        assert!(r > 10.5 && r < 11.5, "{p:?}");
        let t = p[1].atan2(p[0]).rem_euclid(std::f64::consts::TAU);
        let u = (r - 10.0) / 2.0;
        let turns = (p[2] - u) / std::f64::consts::TAU;
        assert!((turns - 1.0).abs() < 0.1, "{p:?} {turns}");
        assert!(brep::dist(p, at(u, p[2] - u)) < 1e-9, "{p:?} {t}");
        // The normal is across the strip.
        let (du, dv) = (
            brep::sub(at(u + 1e-6, p[2] - u), at(u, p[2] - u)),
            brep::sub(at(u, p[2] - u + 1e-6), at(u, p[2] - u)),
        );
        let n = brep::normalize(brep::cross(du, dv));
        assert!(brep::dot(n, inside.normal) > 0.999, "{:?}", inside.normal);
    }

    /// A cone's face from a quarter turn: the point lies in it, half way
    /// up, with the cone's outward normal.
    #[test]
    fn a_point_inside_a_conical_face() {
        let half: f64 = 0.3;
        let surface = Surface::Cone {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            ref_dir: [1.0, 0.0, 0.0],
            radius: 5.0,
            half_angle: half,
        };
        let at = |a: f64, h: f64| {
            let r = 5.0 + h * half.tan();
            [r * a.cos(), r * a.sin(), h]
        };
        let q = std::f64::consts::FRAC_PI_2;
        let arc = |h: f64| -> Vec<P3> { (0..=16).map(|i| at(q * i as f64 / 16.0, h)).collect() };
        let line = |a: f64| -> Vec<P3> { (0..=4).map(|i| at(a, 2.0 * i as f64)).collect() };
        let (bottom, top, left, right) = (arc(0.0), arc(8.0), line(0.0), line(q));
        let edges: Vec<&[P3]> = vec![&bottom, &right, &top, &left];
        let inside = point_inside_face(&surface, false, &edges).unwrap();
        let expected = at(q / 2.0, 4.0);
        assert!(
            brep::dist(inside.point, expected) < 1e-9,
            "{:?}",
            inside.point
        );
        assert!(
            inside.normal[2] < 0.0 && inside.normal[0] > 0.0,
            "{:?}",
            inside.normal
        );
    }
}
