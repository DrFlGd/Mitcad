// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 SolidCraft contributors
//
// Adapted from SolidCraft (https://github.com/tangency-labs/solidcraft, commit
// 0b787f8eae4244cd40f07dddccd2e1b27e4cf9f8): the cam geometry of
// `crates/doc/src/joints_cam.rs` (`first_contact_along`, `touches_turning`,
// `overlaps_turning`, `first_contact_turning`, `next_contact_turning`,
// `dist_to_segment`, `inside`, `area`, `round` and the sampling loop of
// `table`). Changes: points are `[f64; 2]` with a small vector type of their
// own; errors are `String`s; the setup (the profile, the follower's line or
// pivot and arm, the signs) is the caller's (Mitcad's frames and joints), given
// as [`CamSetup`]; `lift_at` is left out (the joint solver interpolates the
// table, `mitcad_solver::Relation::Table`).
//
// Used under the MIT licence; see `LICENSE-MIT` and `README.md` here.

//! A cam's follower over one turn of the cam (mitcad#466): a translating
//! follower (on a slider) stands where it first touches the profile coming
//! along its line; an oscillating one (on a revolute joint) turns from its
//! arm pointing away from the cam's axis until it first touches, and is
//! followed from sample to sample. Everything is in the cam's fixed frame:
//! the profile turns about its z axis (here the follower turns back instead).

use std::f64::consts::TAU;

/// Samples of a cam's table over one turn of the cam (one per degree).
pub const CAM_SAMPLES: usize = 360;
/// Points a cam's profile is sampled into at most.
pub const MAX_PROFILE_POINTS: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq)]
struct V {
    x: f64,
    y: f64,
}

impl V {
    const ZERO: V = V { x: 0.0, y: 0.0 };

    fn of(p: [f64; 2]) -> V {
        V { x: p[0], y: p[1] }
    }
    fn add(self, o: V) -> V {
        V {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }
    fn sub(self, o: V) -> V {
        V {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }
    fn mul(self, k: f64) -> V {
        V {
            x: self.x * k,
            y: self.y * k,
        }
    }
    fn dot(self, o: V) -> f64 {
        self.x * o.x + self.y * o.y
    }
    fn cross(self, o: V) -> f64 {
        self.x * o.y - self.y * o.x
    }
    fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    fn len2(self) -> f64 {
        self.dot(self)
    }
    fn dist(self, o: V) -> f64 {
        self.sub(o).len()
    }
    fn normalized(self) -> Option<V> {
        let l = self.len();
        (l > 1e-9 && l.is_finite()).then(|| self.mul(1.0 / l))
    }
    /// Rotated +90°.
    fn perp(self) -> V {
        V {
            x: -self.y,
            y: self.x,
        }
    }
    fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }
    fn from_angle(a: f64) -> V {
        V {
            x: a.cos(),
            y: a.sin(),
        }
    }
}

fn rot(v: V, a: f64) -> V {
    let (s, c) = a.sin_cos();
    V {
        x: v.x * c - v.y * s,
        y: v.x * s + v.y * c,
    }
}

/// Where a translating follower coming along `-d` from far along its line
/// through `g` first touches the closed polygon `poly`: the largest `t` of
/// its point `g + t·d` (its roller's centre for `r > 0`; for `flat`, its
/// face square to `d`). `None` when it misses the profile.
fn first_contact_along(poly: &[V], g: V, d: V, r: f64, flat: bool) -> Option<f64> {
    let n = d.perp();
    let mut best: Option<f64> = None;
    let mut take = |t: f64| {
        if t.is_finite() && best.is_none_or(|b| t > b) {
            best = Some(t);
        }
    };
    if flat {
        for p in poly {
            take(p.sub(g).dot(d));
        }
        return best;
    }
    let count = poly.len();
    for i in 0..count {
        let (Some(&p), Some(&q)) = (poly.get(i), poly.get((i + 1) % count)) else {
            continue;
        };
        let (a, b) = (p.sub(g).dot(n), q.sub(g).dot(n));
        // Edges wholly outside the band the follower sweeps.
        if a.min(b) > r || a.max(b) < -r {
            continue;
        }
        if r <= 0.0 {
            if a == b {
                // Along the line: both ends.
                take(p.sub(g).dot(d));
                take(q.sub(g).dot(d));
            } else {
                let k = a / (a - b);
                take(p.sub(g).dot(d) + q.sub(p).dot(d) * k);
            }
            continue;
        }
        // The roller's centre a radius from the edge: on its two parallels
        // (the foot within the edge) or about its start (each vertex starts
        // one edge).
        let len = p.dist(q);
        if let Some(e) = q.sub(p).normalized() {
            let m = e.perp();
            let dm = d.dot(m);
            if dm.abs() > 1e-15 {
                for side in [r, -r] {
                    let t = (side - g.sub(p).dot(m)) / dm;
                    let foot = g.add(d.mul(t)).sub(p).dot(e);
                    if (0.0..=len).contains(&foot) {
                        take(t);
                    }
                }
            }
        }
        if a.abs() <= r {
            take(p.sub(g).dot(d) + (r * r - a * a).max(0.0).sqrt());
        }
    }
    best
}

/// The angles in [0, 2π) where an oscillating follower's tip touches the
/// closed polygon `poly`: its tip (a roller's centre for `r > 0`) at `arm`
/// from the pivot, turned from the angle `start` the way `turn` says (+1
/// counter-clockwise), sorted.
fn touches_turning(poly: &[V], pivot: V, arm: f64, r: f64, start: f64, turn: f64) -> Vec<f64> {
    let tip = |a: f64| pivot.add(V::from_angle(a).mul(arm));
    let count = poly.len();
    let edge = |i: usize| Some((*poly.get(i)?, *poly.get((i + 1) % count)?));
    let mut out = Vec::new();
    let mut take = |a: f64, keep: &dyn Fn(V) -> bool| {
        let turned = (turn * (a - start)).rem_euclid(TAU);
        if turned.is_finite() && turned < TAU && keep(tip(a)) {
            out.push(turned);
        }
    };
    // Angles of the tip's direction u where u·w = h: both solutions.
    let meet = |w: V, h: f64| -> Vec<f64> {
        let len = w.len();
        if len <= 0.0 || !(h / len).is_finite() || (h / len).abs() > 1.0 {
            return Vec::new();
        }
        let (base, spread) = (w.angle(), (h / len).clamp(-1.0, 1.0).acos());
        vec![base + spread, base - spread]
    };
    for (p, q) in (0..count).filter_map(edge) {
        let near = dist_to_segment(pivot, p, q);
        let far = p.sub(pivot).len().max(q.sub(pivot).len());
        if near > arm + r || far < arm - r {
            continue;
        }
        let len = p.dist(q);
        let Some(e) = q.sub(p).normalized() else {
            continue;
        };
        let m = e.perp();
        // On a parallel of the edge a radius away (the edge itself for a
        // knife), the foot within it.
        let sides: &[f64] = if r > 0.0 { &[r, -r] } else { &[0.0] };
        for side in sides {
            for a in meet(m.mul(arm), side - pivot.sub(p).dot(m)) {
                take(a, &|t: V| (0.0..=len).contains(&t.sub(p).dot(e)));
            }
        }
        if r > 0.0 {
            // A radius from the edge's start: |pivot + arm·u − p| = r.
            let w = p.sub(pivot);
            for a in meet(w, (arm * arm + w.len2() - r * r) / (2.0 * arm)) {
                take(a, &|_| true);
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// Whether an oscillating follower's tip, turned `turned` from `start` the
/// way `turn` says, overlaps the profile by more than `slack`.
fn overlaps_turning(
    poly: &[V],
    pivot: V,
    arm: f64,
    r: f64,
    (start, turn): (f64, f64),
    turned: f64,
    slack: f64,
) -> bool {
    let at = pivot.add(V::from_angle(start + turn * turned).mul(arm));
    let n = poly.len();
    let gap = (0..n)
        .filter_map(|i| Some((*poly.get(i)?, *poly.get((i + 1) % n)?)))
        .map(|(p, q)| dist_to_segment(at, p, q))
        .fold(f64::INFINITY, f64::min);
    // Inside counts only clear of the boundary: a knife on an edge touches
    // it.
    gap < r - slack || (gap > slack && inside(poly, at))
}

/// Where an oscillating follower first touches the profile turning from
/// `start` (the angle it turns through, in (0, 2π)). `None` when it misses
/// the profile or starts on it.
fn first_contact_turning(
    poly: &[V],
    pivot: V,
    arm: f64,
    r: f64,
    start: f64,
    turn: f64,
) -> Option<f64> {
    if overlaps_turning(poly, pivot, arm, r, (start, turn), 0.0, -f64::EPSILON) {
        return None;
    }
    touches_turning(poly, pivot, arm, r, start, turn)
        .into_iter()
        .find(|t| *t > 0.0)
}

/// Where an oscillating follower that touched the profile at `from` is
/// after the cam turned on a little: pushed back to where the profile now
/// lets it be when its tip overlaps the profile (by more than `slack`),
/// else on to where it next touches.
fn next_contact_turning(
    poly: &[V],
    pivot: V,
    arm: f64,
    r: f64,
    (start, turn): (f64, f64),
    from: f64,
    slack: f64,
) -> Option<f64> {
    let touches = touches_turning(poly, pivot, arm, r, start, turn);
    let clear = |t: &f64| !overlaps_turning(poly, pivot, arm, r, (start, turn), *t, slack);
    if overlaps_turning(poly, pivot, arm, r, (start, turn), from, slack) {
        touches.into_iter().rev().filter(|t| *t < from).find(clear)
    } else {
        touches
            .into_iter()
            .filter(|t| *t >= from - 1e-12)
            .find(clear)
    }
}

fn dist_to_segment(x: V, p: V, q: V) -> f64 {
    let d = q.sub(p);
    let t = if d.len2() > 0.0 {
        (x.sub(p).dot(d) / d.len2()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    x.dist(p.add(d.mul(t)))
}

/// Even-odd containment in a closed polygon.
fn inside(poly: &[V], x: V) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (Some(a), Some(b)) = (poly.get(i), poly.get((i + 1) % n)) else {
            continue;
        };
        if (a.y > x.y) != (b.y > x.y) && x.x < a.x + (x.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
}

/// A polygon's signed area, from its first point.
pub fn area(poly: &[[f64; 2]]) -> f64 {
    let o = poly.first().map_or(V::ZERO, |p| V::of(*p));
    let n = poly.len();
    (0..n)
        .filter_map(|i| {
            let a = V::of(*poly.get(i)?).sub(o);
            let b = V::of(*poly.get((i + 1) % n)?).sub(o);
            Some(a.cross(b))
        })
        .sum::<f64>()
        / 2.0
}

/// The follower's places at the samples 0 ..= `CAM_SAMPLES` of a turn,
/// `place(u, prev)` giving it with the cam `u` samples on (from `prev` when
/// it is followed from sample to sample). A followed follower must come
/// back where it started after the turn. A step much larger than the steps
/// either side is followed again in finer steps: a smooth rise shrinks with
/// them, a jump (the follower leaving the profile and landing on another
/// part of it) stays one step and is refused. Steps no larger than `floor`
/// are not looked at.
fn round(
    place: &dyn Fn(f64, Option<f64>) -> Result<f64, String>,
    follows: bool,
    floor: f64,
) -> Result<Vec<f64>, String> {
    let mut raw: Vec<f64> = Vec::with_capacity(CAM_SAMPLES + 1);
    for k in 0..=CAM_SAMPLES {
        let prev = if follows { raw.last().copied() } else { None };
        raw.push(place(k as f64, prev)?);
    }
    let at = |k: usize| raw.get(k).copied().unwrap_or(f64::NAN);
    if follows && (at(0) - at(CAM_SAMPLES)).abs() > 1e-6 {
        return Err(
            "after a turn of the cam the follower does not come back where it started (it would jump)"
                .to_owned(),
        );
    }
    let step = |k: usize| (at(k + 1) - at(k)).abs();
    for k in 0..CAM_SAMPLES {
        let (d, around) = (
            step(k),
            step((k + CAM_SAMPLES - 1) % CAM_SAMPLES).max(step((k + 1) % CAM_SAMPLES)),
        );
        if d <= floor || d <= 3.0 * around {
            continue;
        }
        let (mut here, mut most) = (at(k), 0.0f64);
        for j in 1..=32 {
            let next = place(k as f64 + f64::from(j) / 32.0, follows.then_some(here))?;
            most = most.max((next - here).abs());
            here = next;
        }
        if most > 0.5 * d {
            return Err(format!(
                "the follower would jump near {k} degrees of the cam's turn (it leaves the profile there)"
            ));
        }
    }
    Ok(raw)
}

/// Everything a cam's table is worked out from, in the cam's fixed frame.
#[derive(Debug, Clone, PartialEq)]
pub struct CamSetup {
    /// The profile at cam value 0, a closed polygon.
    pub poly: Vec<[f64; 2]>,
    /// +1 when the profile turns by +value about the frame's z, −1
    /// otherwise.
    pub turn: f64,
    /// A translating follower's line (a point and its unit direction), or
    /// an oscillating follower's pivot.
    pub at: [f64; 2],
    pub dir: [f64; 2],
    /// An oscillating follower's arm (pivot to tip) where it is now.
    pub arm: Option<[f64; 2]>,
    /// The side a translating follower rides from (+1: from +dir), or the
    /// way an oscillating one turns toward the cam (+1: counter-clockwise).
    pub side: f64,
    /// The follower's value per mm of travel along `dir`, or per radian of
    /// its arm's turn about z.
    pub gain: f64,
    /// A roller's radius (0 for a knife or a flat face).
    pub radius: f64,
    pub flat: bool,
    /// The profile's size, for tolerances.
    pub size: f64,
}

/// A cam's table: the follower's values at the cam's values 2π·k /
/// `CAM_SAMPLES` (the follower's offset left out): `gain` times the
/// distance along the line from `at`, or times the arm's angle.
pub fn table(s: &CamSetup) -> Result<Vec<f64>, String> {
    let poly: Vec<V> = s.poly.iter().map(|p| V::of(*p)).collect();
    let at = V::of(s.at);
    let dir = V::of(s.dir);
    let start = at.angle();
    let follows = s.arm.is_some();
    let place = |u: f64, prev: Option<f64>| -> Result<f64, String> {
        // The follower turned back instead of the profile turned on.
        let q = -s.turn * TAU * u / CAM_SAMPLES as f64;
        match s.arm {
            None => {
                first_contact_along(&poly, rot(at, q), rot(dir.mul(s.side), q), s.radius, s.flat)
                    .map(|t| t * s.side)
                    .ok_or_else(|| "the follower's line misses the cam at some angles".to_owned())
            }
            Some(arm) => {
                let (pivot, from) = (rot(at, q), (start + q, s.side));
                let arm = V::of(arm).len();
                match prev {
                    None => first_contact_turning(&poly, pivot, arm, s.radius, from.0, from.1),
                    Some(p) => {
                        next_contact_turning(&poly, pivot, arm, s.radius, from, p, 1e-9 * s.size)
                    }
                }
                .ok_or_else(|| {
                    "the follower's arm does not reach the cam at every angle (or starts on it)"
                        .to_owned()
                })
            }
        }
    };
    let floor = if follows { 1e-6 } else { 1e-6 * s.size };
    let raw = round(&place, follows, floor)?;
    let out: Vec<f64> = raw
        .iter()
        .take(CAM_SAMPLES)
        .map(|v| s.gain * if follows { start + s.side * v } else { *v })
        .collect();
    if !out
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1e9 * s.size.max(1.0))
    {
        return Err("the follower's travel is out of range".to_owned());
    }
    Ok(out)
}
