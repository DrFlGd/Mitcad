// SPDX-License-Identifier: MIT
//! B-splines in a plane (mitcad#853): the curves a projection gives as
//! B-splines (`ViewCurve::Spline`) evaluated, cut to a part of their range
//! and turned round exactly, by knot insertion (Boehm's algorithm, in
//! homogeneous coordinates for rational ones), so that a cut sheet writes
//! the curve it was given: its degree, knots, poles and weights. A
//! non-rational B-spline of degree 3 or less is also split into its Bézier
//! pieces, raised to cubics, for SVG paths.
//!
//! Knot vectors are full and clamped (the first and the last knot
//! `degree + 1` times), as OCCT gives them.

use super::{P2, ViewSpline};

/// A B-spline curve in a plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Spline2 {
    pub degree: usize,
    /// `poles.len() + degree + 1` knots, clamped.
    pub knots: Vec<f64>,
    pub poles: Vec<P2>,
    /// One per pole; empty when it is not rational.
    pub weights: Vec<f64>,
}

/// A point with its weight: (w x, w y, w).
type Homogeneous = [f64; 3];

fn lerp3(a: Homogeneous, b: Homogeneous, t: f64) -> Homogeneous {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Spline2 {
    /// A projection's B-spline, or None when its data is no clamped
    /// B-spline of degree 1 or more.
    pub fn of(view: &ViewSpline) -> Option<Self> {
        let spline = Self {
            degree: view.degree as usize,
            knots: view.knots.clone(),
            poles: view.poles.clone(),
            weights: view.weights.clone(),
        };
        spline.valid().then_some(spline)
    }

    fn valid(&self) -> bool {
        let p = self.degree;
        let n = self.poles.len();
        if p == 0 || n < p + 1 || self.knots.len() != n + p + 1 {
            return false;
        }
        if !self.weights.is_empty()
            && (self.weights.len() != n
                || self.weights.iter().any(|w| !(w.is_finite() && *w > 0.0)))
        {
            return false;
        }
        let finite = self.knots.iter().all(|k| k.is_finite())
            && self
                .poles
                .iter()
                .all(|q| q[0].is_finite() && q[1].is_finite());
        let increasing = self.knots.windows(2).all(|w| w[1] >= w[0]);
        let (a, b) = (self.knots[p], self.knots[n]);
        let clamped =
            self.knots[..=p].iter().all(|k| *k == a) && self.knots[n..].iter().all(|k| *k == b);
        finite && increasing && clamped && b > a
    }

    /// Its parameter range.
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.poles.len()])
    }

    pub fn is_rational(&self) -> bool {
        let first = self.weights.first().copied().unwrap_or(1.0);
        self.weights
            .iter()
            .any(|w| (w - first).abs() > 1e-12 * first.abs().max(1.0))
    }

    fn weight(&self, i: usize) -> f64 {
        self.weights.get(i).copied().unwrap_or(1.0)
    }

    fn homogeneous(&self, i: usize) -> Homogeneous {
        let w = self.weight(i);
        [self.poles[i][0] * w, self.poles[i][1] * w, w]
    }

    /// How close two knots are to count as one.
    fn knot_tolerance(&self) -> f64 {
        let (a, b) = self.domain();
        1e-12 * (b - a).abs().max(a.abs()).max(b.abs()).max(1.0)
    }

    /// The span of a parameter: k with knots[k] <= u < knots[k + 1], the
    /// last span with any length at the end of the range.
    fn span(&self, u: f64) -> usize {
        let p = self.degree;
        let n = self.poles.len();
        let mut k = p;
        for i in p..n {
            if self.knots[i] <= u && self.knots[i] < self.knots[i + 1] {
                k = i;
            }
        }
        k
    }

    /// The point at a parameter (de Boor's algorithm).
    pub fn point(&self, u: f64) -> P2 {
        let (a, b) = self.domain();
        let u = u.clamp(a, b);
        let p = self.degree;
        let k = self.span(u);
        let mut d: Vec<Homogeneous> = (0..=p).map(|j| self.homogeneous(j + k - p)).collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let lo = self.knots[j + k - p];
                let hi = self.knots[j + 1 + k - r];
                let alpha = if hi > lo { (u - lo) / (hi - lo) } else { 0.0 };
                d[j] = lerp3(d[j - 1], d[j], alpha);
            }
        }
        let h = d[p];
        [h[0] / h[2], h[1] / h[2]]
    }

    /// With the knot `u` inserted once (Boehm), the curve unchanged.
    fn inserted(&self, u: f64) -> Self {
        let p = self.degree;
        let n = self.poles.len();
        let k = self.span(u);
        let mut poles: Vec<Homogeneous> = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let q = if i + p <= k {
                self.homogeneous(i)
            } else if i > k {
                self.homogeneous(i - 1)
            } else {
                let lo = self.knots[i];
                let hi = self.knots[i + p];
                let alpha = if hi > lo { (u - lo) / (hi - lo) } else { 0.0 };
                lerp3(self.homogeneous(i - 1), self.homogeneous(i), alpha)
            };
            poles.push(q);
        }
        let mut knots = self.knots.clone();
        knots.insert(k + 1, u);
        let rational = !self.weights.is_empty();
        Self {
            degree: p,
            knots,
            poles: poles.iter().map(|h| [h[0] / h[2], h[1] / h[2]]).collect(),
            weights: if rational {
                poles.iter().map(|h| h[2]).collect()
            } else {
                Vec::new()
            },
        }
    }

    /// How many knots lie at `u`.
    fn multiplicity(&self, u: f64) -> usize {
        let eps = self.knot_tolerance();
        self.knots.iter().filter(|k| (**k - u).abs() <= eps).count()
    }

    /// `u` moved onto a knot it nearly is.
    fn snapped(&self, u: f64) -> f64 {
        let eps = 1e3 * self.knot_tolerance();
        self.knots
            .iter()
            .copied()
            .find(|k| (k - u).abs() <= eps)
            .unwrap_or(u)
    }

    /// With the knot `u` inserted until it is there `degree` times.
    fn with_knot(&self, u: f64) -> Self {
        let mut out = self.clone();
        let times = self.degree.saturating_sub(self.multiplicity(u));
        for _ in 0..times {
            out = out.inserted(u);
        }
        out
    }

    /// The curve between two parameters (`a < b`), the same curve there.
    pub fn segment(&self, a: f64, b: f64) -> Self {
        let (start, end) = self.domain();
        let a = self.snapped(a.clamp(start, end));
        let b = self.snapped(b.clamp(start, end));
        let eps = self.knot_tolerance();
        if a <= start + eps && b >= end - eps {
            return self.clone();
        }
        let mut s = self.clone();
        if a > start + eps {
            s = s.with_knot(a);
        }
        if b < end - eps {
            s = s.with_knot(b);
        }
        let p = s.degree;
        // The last knot at a and the first at b.
        let last_a = s
            .knots
            .iter()
            .rposition(|k| (k - a).abs() <= eps)
            .unwrap_or(p);
        let first_b = s
            .knots
            .iter()
            .position(|k| (k - b).abs() <= eps)
            .unwrap_or(s.poles.len());
        let m = last_a + 1 - p;
        let l = first_b;
        let mut knots = vec![a; p + 1];
        knots.extend_from_slice(&s.knots[m + p..l]);
        knots.extend(std::iter::repeat_n(b, p + 1));
        let poles = s.poles[m - 1..l].to_vec();
        let weights = if s.weights.is_empty() {
            Vec::new()
        } else {
            s.weights[m - 1..l].to_vec()
        };
        Self {
            degree: p,
            knots,
            poles,
            weights,
        }
    }

    /// The same curve the other way round.
    pub fn reversed(&self) -> Self {
        let (a, b) = self.domain();
        Self {
            degree: self.degree,
            knots: self.knots.iter().rev().map(|k| a + b - k).collect(),
            poles: self.poles.iter().rev().copied().collect(),
            weights: self.weights.iter().rev().copied().collect(),
        }
    }

    /// Moved by a map of the plane that keeps straight lines straight (a
    /// turn and a shift): its poles moved.
    pub fn mapped(&self, f: impl Fn(P2) -> P2) -> Self {
        Self {
            poles: self.poles.iter().map(|p| f(*p)).collect(),
            ..self.clone()
        }
    }

    /// Points from `a` to `b` (either way) with their parameters, within
    /// `deflection` of the curve: the knots' points and enough between.
    pub fn samples(&self, a: f64, b: f64, deflection: f64) -> Vec<(f64, P2)> {
        let (lo, hi) = (a.min(b), a.max(b));
        let mut cuts: Vec<f64> = vec![lo];
        for k in &self.knots {
            if *k > lo && *k < hi && cuts.last().is_some_and(|c| k > c) {
                cuts.push(*k);
            }
        }
        cuts.push(hi);
        let steps = 4 * self.degree.max(1);
        let mut out: Vec<(f64, P2)> = vec![(lo, self.point(lo))];
        for w in cuts.windows(2) {
            for i in 1..=steps {
                let u = w[0] + (w[1] - w[0]) * i as f64 / steps as f64;
                let from = *out.last().expect("a sample");
                self.refine(from, (u, self.point(u)), deflection, 12, &mut out);
            }
        }
        if a > b {
            out.reverse();
        }
        out
    }

    /// Points between two samples while the curve leaves their chord by
    /// more than `deflection`; the second pushed last.
    fn refine(
        &self,
        from: (f64, P2),
        to: (f64, P2),
        deflection: f64,
        depth: u32,
        out: &mut Vec<(f64, P2)>,
    ) {
        let um = (from.0 + to.0) / 2.0;
        let m = self.point(um);
        let chord = [(from.1[0] + to.1[0]) / 2.0, (from.1[1] + to.1[1]) / 2.0];
        if depth > 0 && (m[0] - chord[0]).hypot(m[1] - chord[1]) > deflection {
            self.refine(from, (um, m), deflection, depth - 1, out);
            self.refine((um, m), to, deflection, depth - 1, out);
        } else {
            out.push(to);
        }
    }

    /// A non-rational curve of degree 3 or less as its Bézier pieces, each
    /// `degree + 1` points; None for another.
    pub fn beziers(&self) -> Option<Vec<Vec<P2>>> {
        let p = self.degree;
        if self.is_rational() || p > 3 {
            return None;
        }
        let (a, b) = self.domain();
        let mut s = self.clone();
        let inner: Vec<f64> = {
            let mut v: Vec<f64> = Vec::new();
            for k in &self.knots {
                if *k > a && *k < b && v.last().is_none_or(|l| k > l) {
                    v.push(*k);
                }
            }
            v
        };
        for u in inner {
            s = s.with_knot(u);
        }
        Some(
            (0..(s.poles.len() - 1) / p)
                .map(|i| s.poles[i * p..=i * p + p].to_vec())
                .collect(),
        )
    }
}

/// A Bézier piece of degree 1, 2 or 3 as a cubic's four points.
pub fn cubic(bezier: &[P2]) -> [P2; 4] {
    let at = |a: P2, b: P2, t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    match bezier {
        [p0, p1] => [*p0, at(*p0, *p1, 1.0 / 3.0), at(*p0, *p1, 2.0 / 3.0), *p1],
        [p0, p1, p2] => [*p0, at(*p0, *p1, 2.0 / 3.0), at(*p2, *p1, 2.0 / 3.0), *p2],
        [p0, p1, p2, p3] => [*p0, *p1, *p2, *p3],
        _ => {
            let first = bezier.first().copied().unwrap_or([0.0, 0.0]);
            let last = bezier.last().copied().unwrap_or(first);
            [first, first, last, last]
        }
    }
}
