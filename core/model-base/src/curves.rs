// SPDX-License-Identifier: MIT
//! Points and ends of the kernel's 3D curves and of sweep paths (F3), plain
//! geometry without the model (mitcad#777: below the model, next to the
//! [`Kernel`] trait, for the mock kernel too).

use mitcad_vec3::{add, distance, scale};

use crate::kernel::Curve3;
#[cfg(doc)]
use crate::kernel::Kernel;
use crate::sweeps::PathCurve;

const TAU: f64 = std::f64::consts::TAU;

/// A point of a conic at parameter `t`.
pub fn conic_point(
    center: [f64; 3],
    normal: [f64; 3],
    x_axis: [f64; 3],
    (major, minor): (f64, f64),
    t: f64,
) -> [f64; 3] {
    let y_axis = crate::profile::cross(normal, x_axis);
    add(
        center,
        add(
            scale(x_axis, major * t.cos()),
            scale(y_axis, minor * t.sin()),
        ),
    )
}

/// A point of a helix at parameter `t` (see [`Curve3::Helix`]).
pub fn helix_point(
    center: [f64; 3],
    axis: [f64; 3],
    x_axis: [f64; 3],
    (radius, pitch): (f64, f64),
    t: f64,
) -> [f64; 3] {
    add(
        conic_point(center, axis, x_axis, (radius, radius), t),
        scale(axis, pitch * t / TAU),
    )
}

/// A point of a B-spline with its full knot vector (de Boor, rational in
/// homogeneous coordinates).
pub fn bspline_point(
    degree: usize,
    poles: &[[f64; 3]],
    weights: &[f64],
    knots: &[f64],
    t: f64,
) -> [f64; 3] {
    let n = poles.len();
    let p = degree;
    let mut k = p;
    while k + 1 < n && knots[k + 1] <= t {
        k += 1;
    }
    let mut d: Vec<[f64; 4]> = (0..=p)
        .map(|j| {
            let i = k + j - p;
            let w = weights.get(i).copied().unwrap_or(1.0);
            let q = poles[i];
            [q[0] * w, q[1] * w, q[2] * w, w]
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = k + j - p;
            let span = knots[i + p + 1 - r] - knots[i];
            let alpha = if span == 0.0 {
                0.0
            } else {
                (t - knots[i]) / span
            };
            d[j] = std::array::from_fn(|c| (1.0 - alpha) * d[j - 1][c] + alpha * d[j][c]);
        }
    }
    let h = d[p];
    [h[0] / h[3], h[1] / h[3], h[2] / h[3]]
}

/// The start and end of an open curve; None for a closed one.
pub fn curve_ends(curve: &Curve3) -> Option<([f64; 3], [f64; 3])> {
    match curve {
        Curve3::Point(p) => Some((*p, *p)),
        Curve3::Line { start, end } => Some((*start, *end)),
        Curve3::Conic { closed: true, .. } => None,
        Curve3::Conic {
            center,
            normal,
            x_axis,
            major,
            minor,
            start,
            end,
            ..
        } => {
            let at = |t| conic_point(*center, *normal, *x_axis, (*major, *minor), t);
            Some((at(*start), at(*end)))
        }
        Curve3::BSpline {
            degree,
            poles,
            weights,
            knots,
        } => {
            let p = *degree as usize;
            if p == 0 || poles.len() <= p || knots.len() != poles.len() + p + 1 {
                return poles.first().zip(poles.last()).map(|(a, b)| (*a, *b));
            }
            let (t0, t1) = (knots[p], knots[poles.len()]);
            let a = bspline_point(p, poles, weights, knots, t0);
            let b = bspline_point(p, poles, weights, knots, t1);
            let size = poles.iter().flatten().fold(1.0_f64, |m, v| m.max(v.abs()));
            (distance(a, b) > 1e-9 * size).then_some((a, b))
        }
        Curve3::Helix {
            center,
            axis,
            x_axis,
            radius,
            pitch,
            start,
            end,
        } => {
            let at = |t| helix_point(*center, *axis, *x_axis, (*radius, *pitch), t);
            Some((at(*start), at(*end)))
        }
    }
}

/// The curve run the other way.
pub fn reversed(curve: Curve3) -> Curve3 {
    match curve {
        Curve3::Point(p) => Curve3::Point(p),
        Curve3::Line { start, end } => Curve3::Line {
            start: end,
            end: start,
        },
        // With the normal reversed, the point at t is the old point at -t.
        Curve3::Conic {
            center,
            normal,
            x_axis,
            major,
            minor,
            start,
            end,
            closed,
        } => Curve3::Conic {
            center,
            normal: normal.map(|v| -v),
            x_axis,
            major,
            minor,
            start: -end,
            end: -start,
            closed,
        },
        Curve3::BSpline {
            degree,
            mut poles,
            mut weights,
            knots,
        } => {
            poles.reverse();
            weights.reverse();
            let (first, last) = (knots[0], knots[knots.len() - 1]);
            let knots = knots.iter().rev().map(|k| first + last - k).collect();
            Curve3::BSpline {
                degree,
                poles,
                weights,
                knots,
            }
        }
        // As a conic's: with the axis reversed the point at t is the old
        // point at -t (the rise turns with the axis).
        Curve3::Helix {
            center,
            axis,
            x_axis,
            radius,
            pitch,
            start,
            end,
        } => Curve3::Helix {
            center,
            axis: axis.map(|v| -v),
            x_axis,
            radius,
            pitch,
            start: -end,
            end: -start,
        },
    }
}

/// True when the path's last curve ends where its first starts (or it is
/// one closed curve).
pub fn is_closed(path: &[PathCurve]) -> bool {
    let (Some(first), Some(last)) = (path.first(), path.last()) else {
        return false;
    };
    match (curve_ends(&first.curve), curve_ends(&last.curve)) {
        (None, _) => true,
        (Some((start, _)), Some((_, end))) => distance(start, end) <= tolerance(path),
        _ => false,
    }
}

/// The path run the other way.
pub fn reversed_path(path: Vec<PathCurve>) -> Vec<PathCurve> {
    path.into_iter()
        .rev()
        .map(|p| PathCurve {
            name: p.name,
            curve: reversed(p.curve),
        })
        .collect()
}

/// Ends closer than this join.
pub fn tolerance(path: &[PathCurve]) -> f64 {
    let mut size: f64 = 1.0;
    for piece in path {
        if let Some((a, b)) = curve_ends(&piece.curve) {
            size = a.iter().chain(&b).fold(size, |m, v| m.max(v.abs()));
        }
    }
    1e-6 * size
}
