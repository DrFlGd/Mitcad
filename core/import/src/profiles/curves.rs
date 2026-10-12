// SPDX-License-Identifier: MIT
//! The file's sketch curves as [`super`] needs them: points along a curve,
//! where two curves meet, the pieces a curve is cut into and the
//! even-odd test of a point against pieces.
//!
//! Lines, arcs and circles are exact. Ellipses and elliptical arcs are
//! exact for the ray test and the parameter of a point, and meet other
//! curves numerically; splines (NURBS) are numeric throughout: curves are
//! flattened to polylines to find where they meet, and every meeting point
//! is then refined on the curves themselves (Newton's method in both
//! parameters), so that the pieces are cut where the curves really meet
//! and numbered along the curve's parameter as the file numbers them.

use std::f64::consts::TAU;

use mitcad_f3d::design::ir::Geometry;

/// Distances (cm) within which curves meet.
pub(super) const TOL: f64 = 2e-5;

/// A direction for the even-odd ray, unlikely to pass a vertex.
const RAY_ANGLE: f64 = 0.813_771_9;

pub(super) type P = [f64; 2];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// A NURBS curve of the sketch: its domain is `[knots[degree],
/// knots[poles - degree - 1 + degree]]`, the span the curve is defined on.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Spline {
    degree: usize,
    knots: Vec<f64>,
    poles: Vec<P>,
    /// Empty for a non-rational curve.
    weights: Vec<f64>,
    lo: f64,
    hi: f64,
    /// The curve ends where it starts (and is not a point).
    closed: bool,
}

impl Spline {
    fn of(g: &Geometry) -> Option<Spline> {
        let n = g.nurbs.as_ref()?;
        let degree = n["degree"].as_u64()? as usize;
        let knots: Vec<f64> = n["knots"]
            .as_array()?
            .iter()
            .map(|v| v.as_f64())
            .collect::<Option<_>>()?;
        let poles: Vec<P> = n["controlPoints"]
            .as_array()?
            .iter()
            .map(|p| {
                let v = p.as_array()?;
                let (x, y) = (v.first()?.as_f64()?, v.get(1)?.as_f64()?);
                // A spline out of the sketch's plane (a 3D sketch) is no
                // curve of the profiles.
                if v.get(2)
                    .and_then(|z| z.as_f64())
                    .is_some_and(|z| z.abs() > 1e-6)
                {
                    return None;
                }
                Some([x, y])
            })
            .collect::<Option<_>>()?;
        let weights: Vec<f64> = if n["isRational"] == true {
            n["weights"]
                .as_array()?
                .iter()
                .map(|v| v.as_f64())
                .collect::<Option<_>>()?
        } else {
            Vec::new()
        };
        if degree == 0
            || poles.len() <= degree
            || knots.len() != poles.len() + degree + 1
            || (!weights.is_empty() && weights.len() != poles.len())
            || weights.iter().any(|w| *w <= 0.0)
            || knots.windows(2).any(|w| w[1] < w[0])
        {
            return None;
        }
        let (lo, hi) = (knots[degree], knots[poles.len()]);
        if hi <= lo {
            return None;
        }
        let mut s = Spline {
            degree,
            knots,
            poles,
            weights,
            lo,
            hi,
            closed: false,
        };
        s.closed =
            dist(s.point(0.0), s.point(1.0)) < TOL && dist(s.point(0.0), s.point(0.5)) > 10.0 * TOL;
        Some(s)
    }

    /// The point at `t` of [0, 1] along the domain.
    fn point(&self, t: f64) -> P {
        let p = self.degree;
        let n = self.poles.len();
        let u = self.lo + t.clamp(0.0, 1.0) * (self.hi - self.lo);
        let mut i = p;
        while i + 1 < n && self.knots[i + 1] <= u {
            i += 1;
        }
        // The basis functions of the span (Cox-de Boor).
        let mut basis = vec![0.0; p + 1];
        let mut left = vec![0.0; p + 1];
        let mut right = vec![0.0; p + 1];
        basis[0] = 1.0;
        for j in 1..=p {
            left[j] = u - self.knots[i + 1 - j];
            right[j] = self.knots[i + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                let den = right[r + 1] + left[j - r];
                let temp = if den.abs() < 1e-300 {
                    0.0
                } else {
                    basis[r] / den
                };
                basis[r] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            basis[j] = saved;
        }
        let (mut x, mut y, mut w) = (0.0, 0.0, 0.0);
        for (j, b) in basis.iter().enumerate() {
            let k = i - p + j;
            let c = b * self.weights.get(k).copied().unwrap_or(1.0);
            x += c * self.poles[k][0];
            y += c * self.poles[k][1];
            w += c;
        }
        [x / w, y / w]
    }

    /// Flattening points: enough for the pieces' crossings to be found.
    fn samples(&self) -> usize {
        let spans = self.knots.windows(2).filter(|w| w[1] > w[0]).count().max(1);
        (spans * 48).clamp(128, 4096)
    }
}

/// A curve of the file's sketch, parameterised over [0, 1].
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Curve {
    Line {
        s: P,
        e: P,
    },
    /// An arc or circle: the angle from `reference` towards `perp`, from
    /// `a0` to `a1`.
    Round {
        c: P,
        r: f64,
        reference: P,
        perp: P,
        a0: f64,
        a1: f64,
        circle: bool,
    },
    /// An ellipse or elliptical arc: `c + a cos(t) u + b sin(t) v` for the
    /// angle `t` from `a0` to `a1` (a whole ellipse from its major axis).
    Ellipse {
        c: P,
        u: P,
        v: P,
        a: f64,
        b: f64,
        a0: f64,
        a1: f64,
        closed: bool,
    },
    Spline(Spline),
}

impl Curve {
    /// A curve of the file's geometry (sketch space, cm); None for a kind
    /// the profiles do not follow.
    pub(super) fn of(g: &Geometry) -> Option<Curve> {
        let two = |v: Option<[f64; 3]>| v.map(|v| [v[0], v[1]]);
        match g.geometry_type.as_deref()? {
            "Line3D" => Some(Curve::Line {
                s: two(g.start_point)?,
                e: two(g.end_point)?,
            }),
            t @ ("Arc3D" | "Circle3D") => {
                let c = two(g.center)?;
                let r = g.radius.filter(|r| *r > 1e-12)?;
                let sign = if g.normal.map_or(1.0, |n| n[2]) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                let circle = t == "Circle3D";
                let (reference, a0, a1) = if circle {
                    ([1.0, 0.0], 0.0, TAU)
                } else {
                    (two(g.reference_vector)?, g.start_angle?, g.end_angle?)
                };
                Some(Curve::Round {
                    c,
                    r,
                    reference,
                    perp: [-reference[1] * sign, reference[0] * sign],
                    a0,
                    a1,
                    circle,
                })
            }
            t @ ("Ellipse3D" | "EllipticalArc3D") => {
                let c = two(g.center)?;
                let a = g.major_radius.filter(|r| *r > 1e-12)?;
                let b = g.minor_radius.filter(|r| *r > 1e-12)?;
                let major = two(g.major_axis)?;
                let len = major[0].hypot(major[1]);
                if len < 1e-12 {
                    return None;
                }
                let u = [major[0] / len, major[1] / len];
                let sign = if g.normal.map_or(1.0, |n| n[2]) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                let v = [-u[1] * sign, u[0] * sign];
                let closed = t == "Ellipse3D";
                let (a0, a1) = if closed {
                    (0.0, TAU)
                } else {
                    let (a0, mut a1) = (g.start_angle?, g.end_angle?);
                    while a1 <= a0 {
                        a1 += TAU;
                    }
                    (a0, a1.min(a0 + TAU))
                };
                Some(Curve::Ellipse {
                    c,
                    u,
                    v,
                    a,
                    b,
                    a0,
                    a1,
                    closed: closed || (a1 - a0 - TAU).abs() < 1e-9,
                })
            }
            "NurbsCurve3D" => Spline::of(g).map(Curve::Spline),
            _ => None,
        }
    }

    pub(super) fn point(&self, t: f64) -> P {
        match self {
            Curve::Line { s, e } => [s[0] + t * (e[0] - s[0]), s[1] + t * (e[1] - s[1])],
            Curve::Round {
                c,
                r,
                reference,
                perp,
                a0,
                a1,
                ..
            } => {
                let a = a0 + t * (a1 - a0);
                let (cos, sin) = (a.cos(), a.sin());
                [
                    c[0] + r * (cos * reference[0] + sin * perp[0]),
                    c[1] + r * (cos * reference[1] + sin * perp[1]),
                ]
            }
            Curve::Ellipse {
                c,
                u,
                v,
                a,
                b,
                a0,
                a1,
                ..
            } => {
                let t = a0 + t * (a1 - a0);
                let (x, y) = (a * t.cos(), b * t.sin());
                [c[0] + x * u[0] + y * v[0], c[1] + x * u[1] + y * v[1]]
            }
            Curve::Spline(s) => s.point(t),
        }
    }

    /// The parameter of a point on the curve (of the nearest point when it
    /// is not exactly on it).
    pub(super) fn param(&self, p: P) -> f64 {
        match self {
            Curve::Line { s, e } => {
                let d = sub(*e, *s);
                dot(sub(p, *s), d) / dot(d, d)
            }
            Curve::Round {
                c,
                r,
                reference,
                perp,
                a0,
                a1,
                circle,
            } => {
                let v = sub(p, *c);
                let mut a = (dot(v, *perp).atan2(dot(v, *reference)) - a0).rem_euclid(TAU);
                if !circle && a > TAU - TOL / r {
                    a -= TAU;
                }
                a / (a1 - a0)
            }
            Curve::Ellipse {
                c,
                u,
                v,
                a,
                b,
                a0,
                a1,
                closed,
            } => {
                let w = sub(p, *c);
                let mut t = ((dot(w, *v) / b).atan2(dot(w, *u) / a) - a0).rem_euclid(TAU);
                if !closed && t > TAU - TOL / a.min(*b) {
                    t -= TAU;
                }
                t / (a1 - a0)
            }
            Curve::Spline(_) => self.nearest(p),
        }
    }

    /// The parameter of the point of a numeric curve nearest to `p`.
    fn nearest(&self, p: P) -> f64 {
        let n = self.sample_count();
        let mut best = (f64::INFINITY, 0usize);
        for i in 0..=n {
            let d = dist(self.point(i as f64 / n as f64), p);
            if d < best.0 {
                best = (d, i);
            }
        }
        let mut lo = (best.1.saturating_sub(1)) as f64 / n as f64;
        let mut hi = ((best.1 + 1).min(n)) as f64 / n as f64;
        // Golden-section search for the nearest point within the two
        // neighbouring samples.
        let phi = 0.618_033_988_749_894_9;
        let (mut x1, mut x2) = (hi - phi * (hi - lo), lo + phi * (hi - lo));
        let (mut f1, mut f2) = (dist(self.point(x1), p), dist(self.point(x2), p));
        for _ in 0..60 {
            if f1 < f2 {
                hi = x2;
                x2 = x1;
                f2 = f1;
                x1 = hi - phi * (hi - lo);
                f1 = dist(self.point(x1), p);
            } else {
                lo = x1;
                x1 = x2;
                f1 = f2;
                x2 = lo + phi * (hi - lo);
                f2 = dist(self.point(x2), p);
            }
        }
        let t = 0.5 * (lo + hi);
        // A closed curve's end is its start.
        if self.is_closed() && t > 1.0 - 1e-12 {
            0.0
        } else {
            t
        }
    }

    /// How many segments a numeric curve is flattened to.
    fn sample_count(&self) -> usize {
        match self {
            Curve::Line { .. } => 1,
            Curve::Round { a0, a1, .. } => {
                (((a1 - a0).abs() / TAU * 360.0).ceil() as usize).clamp(8, 360)
            }
            Curve::Ellipse { a0, a1, .. } => {
                (((a1 - a0).abs() / TAU * 360.0).ceil() as usize).clamp(8, 360)
            }
            Curve::Spline(s) => s.samples(),
        }
    }

    fn samples(&self) -> Vec<(f64, P)> {
        let n = self.sample_count();
        (0..=n)
            .map(|i| {
                let t = i as f64 / n as f64;
                (t, self.point(t))
            })
            .collect()
    }

    pub(super) fn length(&self) -> f64 {
        match self {
            Curve::Line { s, e } => dist(*s, *e).max(1e-9),
            Curve::Round { r, a0, a1, .. } => (r * (a1 - a0).abs()).max(1e-9),
            _ => self
                .samples()
                .windows(2)
                .map(|w| dist(w[0].1, w[1].1))
                .sum::<f64>()
                .max(1e-9),
        }
    }

    /// An ellipse or spline, whose meetings with other curves are found
    /// numerically.
    pub(super) fn is_numeric(&self) -> bool {
        matches!(self, Curve::Ellipse { .. } | Curve::Spline(_))
    }

    /// A curve that closes on itself (its pieces are numbered from the
    /// piece that holds its start).
    pub(super) fn is_closed(&self) -> bool {
        match self {
            Curve::Round { circle, .. } => *circle,
            Curve::Ellipse { closed, .. } => *closed,
            Curve::Spline(s) => s.closed,
            Curve::Line { .. } => false,
        }
    }

    fn contains(&self, t: f64) -> bool {
        if self.is_closed() {
            return true;
        }
        let tol = TOL / self.length();
        (-tol..=1.0 + tol).contains(&t)
    }

    fn ends(&self) -> Vec<P> {
        if self.is_closed() {
            return Vec::new();
        }
        match self {
            Curve::Line { s, e } => vec![*s, *e],
            _ => vec![self.point(0.0), self.point(1.0)],
        }
    }

    /// Whether `p` lies on the curve (within [`TOL`]).
    fn passes(&self, p: P) -> bool {
        let t = self.param(p);
        self.contains(t) && dist(self.point(t.clamp(0.0, 1.0)), p) < 2.0 * TOL
    }

    /// The curve's bounding box.
    fn bounds(&self, samples: &[(f64, P)]) -> [P; 2] {
        let _ = self;
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for (_, p) in samples {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        [lo, hi]
    }
}

/// Where a line meets a circle (centre `c`, radius `r`), within the line.
fn line_circle(s: P, e: P, c: P, r: f64) -> Vec<P> {
    let d = sub(e, s);
    let a = dot(d, d);
    let len = a.sqrt();
    let t0 = -dot(sub(s, c), d) / a;
    let at = |t: f64| [s[0] + t * d[0], s[1] + t * d[1]];
    let foot = at(t0);
    let gap = dist(foot, c);
    if gap > r + TOL {
        return Vec::new();
    }
    let h2 = r * r - gap * gap;
    let h = if h2 > 0.0 { h2.sqrt() } else { 0.0 };
    let ts = if h < 3.0 * TOL || r - gap < TOL {
        vec![t0]
    } else {
        vec![t0 - h / len, t0 + h / len]
    };
    ts.into_iter()
        .filter(|t| (-TOL / len..=1.0 + TOL / len).contains(t))
        .map(at)
        .collect()
}

/// Where segments `p0p1` and `q0q1` cross: the parameters along each.
fn segments_cross(p0: P, p1: P, q0: P, q1: P) -> Option<(f64, f64)> {
    let (d1, d2) = (sub(p1, p0), sub(q1, q0));
    let den = cross(d1, d2);
    if den.abs() < 1e-14 * (d1[0].hypot(d1[1]) * d2[0].hypot(d2[1])).max(1e-300) {
        return None;
    }
    let w = sub(q0, p0);
    let t = cross(w, d2) / den;
    let u = cross(w, d1) / den;
    let eps = 1e-9;
    ((-eps..=1.0 + eps).contains(&t) && (-eps..=1.0 + eps).contains(&u)).then_some((t, u))
}

/// Refines a crossing of two curves, near the parameters `ta` and `tb`, by
/// Newton's method on the difference of the two points.
fn refine(a: &Curve, b: &Curve, mut ta: f64, mut tb: f64) -> (f64, f64) {
    let h = 1e-6;
    let wrap = |t: f64, c: &Curve| {
        if c.is_closed() {
            t.rem_euclid(1.0)
        } else {
            t.clamp(0.0, 1.0)
        }
    };
    for _ in 0..30 {
        let f = sub(a.point(wrap(ta, a)), b.point(wrap(tb, b)));
        if f[0].hypot(f[1]) < 1e-12 {
            break;
        }
        let da = {
            let (t0, t1) = (
                (ta - h).max(if a.is_closed() { f64::MIN } else { 0.0 }),
                ta + h,
            );
            let (p0, p1) = (a.point(wrap(t0, a)), a.point(wrap(t1, a)));
            [(p1[0] - p0[0]) / (t1 - t0), (p1[1] - p0[1]) / (t1 - t0)]
        };
        let db = {
            let (t0, t1) = (
                (tb - h).max(if b.is_closed() { f64::MIN } else { 0.0 }),
                tb + h,
            );
            let (p0, p1) = (b.point(wrap(t0, b)), b.point(wrap(t1, b)));
            [(p1[0] - p0[0]) / (t1 - t0), (p1[1] - p0[1]) / (t1 - t0)]
        };
        // da * dta - db * dtb = -f
        let den = cross(da, [-db[0], -db[1]]);
        if den.abs() < 1e-18 {
            break;
        }
        let rhs = [-f[0], -f[1]];
        let dta = cross(rhs, [-db[0], -db[1]]) / den;
        let dtb = cross(da, rhs) / den;
        ta += dta;
        tb += dtb;
        if dta.abs() < 1e-14 && dtb.abs() < 1e-14 {
            break;
        }
    }
    (wrap(ta, a), wrap(tb, b))
}

/// Where two curves meet, when one is an ellipse or spline: found on the
/// curves flattened to polylines, refined on the curves, and the ends of
/// either curve that lie on the other.
fn crossings_numeric(a: &Curve, b: &Curve) -> Vec<P> {
    let (sa, sb) = (a.samples(), b.samples());
    let (ba, bb) = (a.bounds(&sa), b.bounds(&sb));
    let mut out = Vec::new();
    let margin = 1e-3 + 10.0 * TOL;
    let disjoint = (0..2).any(|k| ba[1][k] + margin < bb[0][k] || bb[1][k] + margin < ba[0][k]);
    if disjoint {
        return out;
    }
    for wa in sa.windows(2) {
        let (lo, hi) = (
            [
                wa[0].1[0].min(wa[1].1[0]) - margin,
                wa[0].1[1].min(wa[1].1[1]) - margin,
            ],
            [
                wa[0].1[0].max(wa[1].1[0]) + margin,
                wa[0].1[1].max(wa[1].1[1]) + margin,
            ],
        );
        for wb in sb.windows(2) {
            if wb[0].1[0].max(wb[1].1[0]) < lo[0]
                || wb[0].1[0].min(wb[1].1[0]) > hi[0]
                || wb[0].1[1].max(wb[1].1[1]) < lo[1]
                || wb[0].1[1].min(wb[1].1[1]) > hi[1]
            {
                continue;
            }
            if let Some((fa, fb)) = segments_cross(wa[0].1, wa[1].1, wb[0].1, wb[1].1) {
                let ta = wa[0].0 + fa * (wa[1].0 - wa[0].0);
                let tb = wb[0].0 + fb * (wb[1].0 - wb[0].0);
                let (ta, tb) = refine(a, b, ta, tb);
                let (p, q) = (a.point(ta), b.point(tb));
                if dist(p, q) < 10.0 * TOL {
                    out.push(p);
                }
            }
        }
    }
    out.extend(a.ends().into_iter().filter(|p| b.passes(*p)));
    out.extend(b.ends().into_iter().filter(|p| a.passes(*p)));
    if out.len() > 2 {
        out = overlap_ends(a, b, out);
    }
    out
}

/// Of the meeting points of two curves, those that are no more than the
/// ends of a stretch where they lie on each other: the points inside a
/// stretch of overlap (a spline along an arc) cut nothing the ends of the
/// stretch do not.
fn overlap_ends(a: &Curve, b: &Curve, points: Vec<P>) -> Vec<P> {
    let mut at: Vec<(f64, P)> = points.into_iter().map(|p| (a.param(p), p)).collect();
    at.sort_by(|x, y| x.0.total_cmp(&y.0));
    let tol = TOL / a.length();
    at.dedup_by(|y, x| y.0 - x.0 <= tol);
    // Whether the stretch from point i to the next lies on `b`.
    let on: Vec<bool> = at
        .windows(2)
        .map(|w| b.passes(a.point(0.5 * (w[0].0 + w[1].0))))
        .collect();
    at.iter()
        .enumerate()
        .filter(|(i, _)| !(*i > 0 && on[*i - 1] && on.get(*i).copied().unwrap_or(false)))
        .map(|(_, (_, p))| *p)
        .collect()
}

/// Where two curves meet; the ends of an overlap where they lie on each
/// other.
pub(super) fn crossings(a: &Curve, b: &Curve) -> Vec<P> {
    match (a, b) {
        (Curve::Line { s: s1, e: e1 }, Curve::Line { s: s2, e: e2 }) => {
            let (s1, e1, s2, e2) = (*s1, *e1, *s2, *e2);
            let (d1, d2) = (sub(e1, s1), sub(e2, s2));
            let den = cross(d1, d2);
            let (l1, l2) = (d1[0].hypot(d1[1]), d2[0].hypot(d2[1]));
            if den.abs() < 1e-7 * l1 * l2 {
                if cross(d1, sub(s2, s1)).abs() > TOL * l1 {
                    return Vec::new();
                }
                let mut out: Vec<P> = [s2, e2]
                    .into_iter()
                    .filter(|p| a.contains(a.param(*p)))
                    .collect();
                out.extend([s1, e1].into_iter().filter(|p| b.contains(b.param(*p))));
                return out;
            }
            let t = cross(sub(s2, s1), d2) / den;
            let u = cross(sub(s2, s1), d1) / den;
            if (-TOL / l1..=1.0 + TOL / l1).contains(&t)
                && (-TOL / l2..=1.0 + TOL / l2).contains(&u)
            {
                vec![a.point(t)]
            } else {
                Vec::new()
            }
        }
        (Curve::Line { s, e }, Curve::Round { c, r, .. }) => line_circle(*s, *e, *c, *r)
            .into_iter()
            .filter(|p| b.contains(b.param(*p)))
            .collect(),
        (Curve::Round { .. }, Curve::Line { .. }) => crossings(b, a),
        (Curve::Round { c: c1, r: r1, .. }, Curve::Round { c: c2, r: r2, .. }) => {
            let (c1, r1, c2, r2) = (*c1, *r1, *c2, *r2);
            let d = dist(c1, c2);
            if d < 1e-9 {
                if (r1 - r2).abs() >= TOL {
                    return Vec::new();
                }
                let mut out: Vec<P> = a
                    .ends()
                    .into_iter()
                    .filter(|p| b.contains(b.param(*p)))
                    .collect();
                out.extend(b.ends().into_iter().filter(|p| a.contains(a.param(*p))));
                return out;
            }
            if d > r1 + r2 + TOL || d < (r1 - r2).abs() - TOL {
                return Vec::new();
            }
            let x = (d * d + r1 * r1 - r2 * r2) / (2.0 * d);
            let h = (r1 * r1 - x * x).max(0.0).sqrt();
            let ex = [(c2[0] - c1[0]) / d, (c2[1] - c1[1]) / d];
            let ey = [-ex[1], ex[0]];
            let mut pts = vec![[c1[0] + x * ex[0] + h * ey[0], c1[1] + x * ex[1] + h * ey[1]]];
            if h > TOL {
                pts.push([c1[0] + x * ex[0] - h * ey[0], c1[1] + x * ex[1] - h * ey[1]]);
            }
            pts.into_iter()
                .filter(|p| a.contains(a.param(*p)) && b.contains(b.param(*p)))
                .collect()
        }
        _ => crossings_numeric(a, b),
    }
}

/// Sorted values with those within `tol` of the one before left out.
fn cluster(mut ts: Vec<f64>, tol: f64) -> Vec<f64> {
    ts.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for t in ts {
        if out.last().is_some_and(|l| t - l <= tol) {
            continue;
        }
        out.push(t);
    }
    out
}

/// The pieces (parameter ranges) of `c` cut where the `others` cross it,
/// in the file's order.
pub(super) fn pieces(c: &Curve, others: &[&Curve]) -> Vec<(f64, f64)> {
    let tol = TOL / c.length();
    let cuts: Vec<f64> = others
        .iter()
        .flat_map(|o| crossings(c, o))
        .map(|p| c.param(p))
        .collect();
    if c.is_closed() {
        let mut cuts = cluster(cuts.into_iter().map(|t| t.rem_euclid(1.0)).collect(), tol);
        if cuts.len() > 1 && cuts[cuts.len() - 1] - cuts[0] > 1.0 - tol {
            cuts.pop();
        }
        if cuts.is_empty() {
            return vec![(0.0, 1.0)];
        }
        let mut out: Vec<(f64, f64)> = (0..cuts.len())
            .map(|i| (cuts[i], cuts.get(i + 1).copied().unwrap_or(cuts[0] + 1.0)))
            .collect();
        // The piece holding the circle's start first.
        if cuts[0] > tol {
            out.rotate_right(1);
        }
        return out;
    }
    let mut ts = vec![0.0];
    ts.extend(
        cluster(cuts, tol)
            .into_iter()
            .filter(|t| tol < *t && *t < 1.0 - tol),
    );
    ts.push(1.0);
    ts.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Where the ray from `p` crosses a piece of a curve: the distances along
/// the ray.
fn ray_hits(c: &Curve, p: P, t0: f64, t1: f64) -> Vec<f64> {
    let ray = [RAY_ANGLE.cos(), RAY_ANGLE.sin()];
    let mut hits = Vec::new();
    match c {
        Curve::Line { s, e } => {
            let d = sub(*e, *s);
            let den = cross(ray, d);
            if den.abs() < 1e-12 {
                return Vec::new();
            }
            let w = sub(*s, p);
            let along = cross(w, d) / den;
            if along > 1e-9 {
                hits.push((along, cross(w, ray) / den));
            }
        }
        Curve::Round { c: centre, r, .. } => {
            let f = sub(p, *centre);
            let b = dot(f, ray);
            let disc = b * b - (dot(f, f) - r * r);
            if disc < 0.0 {
                return Vec::new();
            }
            let q = disc.sqrt();
            for along in [-b - q, -b + q] {
                if along > 1e-9 {
                    hits.push((
                        along,
                        c.param([p[0] + along * ray[0], p[1] + along * ray[1]]),
                    ));
                }
            }
        }
        Curve::Ellipse {
            c: centre,
            u,
            v,
            a,
            b,
            ..
        } => {
            // In the frame where the ellipse is the unit circle.
            let w = sub(p, *centre);
            let f = [dot(w, *u) / a, dot(w, *v) / b];
            let d = [dot(ray, *u) / a, dot(ray, *v) / b];
            let (qa, qb, qc) = (dot(d, d), dot(f, d), dot(f, f) - 1.0);
            let disc = qb * qb - qa * qc;
            if disc < 0.0 {
                return Vec::new();
            }
            let q = disc.sqrt();
            for along in [(-qb - q) / qa, (-qb + q) / qa] {
                if along > 1e-9 {
                    hits.push((
                        along,
                        c.param([p[0] + along * ray[0], p[1] + along * ray[1]]),
                    ));
                }
            }
        }
        Curve::Spline(_) => {
            let n = c.sample_count();
            let side = |q: P| cross(ray, sub(q, p));
            let mut prev = (0.0, c.point(0.0));
            let mut prev_side = side(prev.1);
            for i in 1..=n {
                let t = i as f64 / n as f64;
                let q = c.point(t);
                let s = side(q);
                if (prev_side < 0.0) != (s < 0.0) && prev_side != s {
                    // The crossing within the segment, on the curve itself.
                    let (mut lo, mut hi) = (prev.0, t);
                    let mut s_lo = prev_side;
                    for _ in 0..50 {
                        let mid = 0.5 * (lo + hi);
                        let sm = side(c.point(mid));
                        if (sm < 0.0) == (s_lo < 0.0) {
                            lo = mid;
                            s_lo = sm;
                        } else {
                            hi = mid;
                        }
                    }
                    let u = 0.5 * (lo + hi);
                    let along = dot(sub(c.point(u), p), ray);
                    if along > 1e-9 {
                        hits.push((along, u));
                    }
                }
                prev = (t, q);
                prev_side = s;
            }
        }
    }
    hits.into_iter()
        .filter(|(_, u)| {
            if c.is_closed() {
                (u - t0).rem_euclid(1.0) <= t1 - t0 + 1e-9
            } else {
                t0 - 1e-9 <= *u && *u <= t1 + 1e-9
            }
        })
        .map(|(along, _)| along)
        .collect()
}

/// Whether `p` lies inside the pieces by the even-odd rule; crossings at
/// one place (coincident copies of a curve) count once.
pub(super) fn inside(pieces: &[(Curve, f64, f64)], p: P) -> bool {
    let mut hits: Vec<f64> = pieces
        .iter()
        .flat_map(|(c, t0, t1)| ray_hits(c, p, *t0, *t1))
        .collect();
    hits.sort_by(f64::total_cmp);
    let mut count = 0;
    let mut last = f64::NEG_INFINITY;
    for h in hits {
        if h - last >= 1e-6 {
            count += 1;
            last = h;
        }
    }
    count % 2 == 1
}
