// SPDX-License-Identifier: MIT
//! Named profile segments, the loops the kernel sweeps into gears and
//! pulleys (mitcad#472), and points along a profile segment (mitcad#777:
//! below the model, next to the [`Kernel`] trait, for the mock kernel too).

use std::f64::consts::TAU;

#[cfg(doc)]
use crate::kernel::Kernel;
use crate::profile::SegmentGeometry;

/// A named curve of a profile loop.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedSegment {
    /// The face role it sweeps: `tip(3)`.
    pub name: String,
    pub geometry: SegmentGeometry,
}

/// A point of a non-periodic B-spline segment at parameter `t` (de Boor).
pub fn bspline_point(segment: &SegmentGeometry, t: f64) -> Option<[f64; 2]> {
    let SegmentGeometry::BSpline {
        degree,
        poles,
        weights,
        knots,
        multiplicities,
        ..
    } = segment
    else {
        return None;
    };
    let p = *degree as usize;
    let flat: Vec<f64> = knots
        .iter()
        .zip(multiplicities)
        .flat_map(|(k, m)| std::iter::repeat_n(*k, *m as usize))
        .collect();
    let n = poles.len();
    let t = t.clamp(flat[p], flat[n]);
    let mut span = p;
    while span + 1 < n && t >= flat[span + 1] {
        span += 1;
    }
    let weight = |i: usize| weights.get(i).copied().unwrap_or(1.0);
    let mut d: Vec<[f64; 3]> = (0..=p)
        .map(|j| {
            let i = span - p + j;
            let w = weight(i);
            [poles[i][0] * w, poles[i][1] * w, w]
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = span - p + j;
            let denominator = flat[i + p + 1 - r] - flat[i];
            let alpha = if denominator == 0.0 {
                0.0
            } else {
                (t - flat[i]) / denominator
            };
            let before = d[j - 1];
            for (c, value) in d[j].iter_mut().enumerate() {
                *value = (1.0 - alpha) * before[c] + alpha * *value;
            }
        }
    }
    Some([d[p][0] / d[p][2], d[p][1] / d[p][2]])
}

/// The parameter range of a B-spline segment.
pub fn bspline_range(segment: &SegmentGeometry) -> Option<(f64, f64)> {
    match segment {
        SegmentGeometry::BSpline { knots, .. } => Some((knots[0], *knots.last()?)),
        _ => None,
    }
}

/// Points along a segment, its ends included.
pub fn sample(segment: &SegmentGeometry, count: usize) -> Vec<[f64; 2]> {
    let count = count.max(1);
    match segment {
        SegmentGeometry::Line { start, end } => (0..=count)
            .map(|i| {
                let t = i as f64 / count as f64;
                [
                    start[0] + (end[0] - start[0]) * t,
                    start[1] + (end[1] - start[1]) * t,
                ]
            })
            .collect(),
        SegmentGeometry::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => (0..=count)
            .map(|i| {
                let a = start_angle + (end_angle - start_angle) * i as f64 / count as f64;
                [center[0] + radius * a.cos(), center[1] + radius * a.sin()]
            })
            .collect(),
        SegmentGeometry::Circle {
            center,
            radius,
            start,
        } => (0..=count)
            .map(|i| {
                let a = start + TAU * i as f64 / count as f64;
                [center[0] + radius * a.cos(), center[1] + radius * a.sin()]
            })
            .collect(),
        SegmentGeometry::BSpline { .. } => {
            let (a, b) = bspline_range(segment).unwrap_or((0.0, 1.0));
            (0..=count)
                .filter_map(|i| bspline_point(segment, a + (b - a) * i as f64 / count as f64))
                .collect()
        }
        _ => Vec::new(),
    }
}
