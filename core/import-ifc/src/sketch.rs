// SPDX-License-Identifier: MIT
//! Sketches of IFC profiles and paths: the curves of a profile (or a
//! sweep's directrix) as sketch entities in a frame of their own, joined
//! where they meet, and the keys of the profile regions their outer loops
//! bound (the model's region keys, `core/model-base/src/topo.rs`), so that a
//! feature added in the same batch can name them before the sketch is
//! evaluated.

use std::f64::consts::TAU;

use mitcad_ifc::geometry::{Path, Profile, Seg, full_knots, spline_domain};
use mitcad_ifc::math::{self, Matrix};
use mitcad_model::{EntityUid, RegionKey, SegmentKey};
use serde_json::{Value, json};

/// A sketch's definition and what features refer to in it.
#[derive(Debug, Clone)]
pub struct SketchBuild {
    pub def: Value,
    /// The keys of the regions of the profile's areas (their outer loops).
    pub regions: Vec<String>,
    /// The keys of each area's holes.
    pub holes: Vec<Vec<String>>,
    /// The curves of a path, in order.
    pub curves: Vec<String>,
    /// What the sketch leaves out or changes.
    pub notes: Vec<String>,
}

/// One loop's curves: their ids and whether each runs the loop's way.
type LoopCurves = Vec<(u32, bool)>;

struct Builder {
    entities: Vec<Value>,
    /// Points shared by curves that meet: position and id.
    points: Vec<([f64; 2], u32)>,
    next: u32,
    tolerance: f64,
}

impl Builder {
    fn new(tolerance: f64) -> Self {
        Self {
            entities: Vec::new(),
            points: Vec::new(),
            next: 1,
            tolerance,
        }
    }

    fn id(&mut self) -> u32 {
        self.next += 1;
        self.next - 1
    }

    /// A point curves share: the one within the tolerance, or a new one.
    fn shared(&mut self, at: [f64; 2]) -> u32 {
        if let Some((_, id)) = self
            .points
            .iter()
            .find(|(p, _)| (p[0] - at[0]).hypot(p[1] - at[1]) <= self.tolerance)
        {
            return *id;
        }
        let id = self.point(at);
        self.points.push((at, id));
        id
    }

    /// A point of its own (a centre, a control point).
    fn point(&mut self, at: [f64; 2]) -> u32 {
        let id = self.id();
        self.entities
            .push(json!({"id": format!("p{id}"), "type": "point", "at": at}));
        id
    }

    /// The curve of a piece (in the sketch's coordinates, z = 0): its id
    /// and whether it runs the piece's way; None for a piece of no length.
    fn curve(&mut self, seg: &Seg, notes: &mut Vec<String>) -> Option<(u32, bool, bool)> {
        let xy = |p: [f64; 3]| [p[0], p[1]];
        match seg {
            Seg::Line { start, end } => {
                let (a, b) = (self.shared(xy(*start)), self.shared(xy(*end)));
                if a == b {
                    return None;
                }
                let id = self.id();
                self.entities
                    .push(json!({"id": format!("c{id}"), "type": "line",
                    "start": format!("p{a}"), "end": format!("p{b}")}));
                Some((id, true, false))
            }
            Seg::Arc {
                frame,
                radius,
                sweep,
                ..
            } => {
                let center = xy(math::origin(frame));
                let up = math::column(frame, 2)[2] >= 0.0;
                if sweep.abs() >= TAU - 1e-9 {
                    let c = self.point(center);
                    let id = self.id();
                    let s = seg.start();
                    let start = (s[1] - center[1]).atan2(s[0] - center[0]);
                    self.entities
                        .push(json!({"id": format!("c{id}"), "type": "circle",
                        "center": format!("p{c}"), "radius": radius, "start_angle": start}));
                    return Some((id, true, true));
                }
                let ccw = (*sweep > 0.0) == up;
                let (s, e) = (xy(seg.start()), xy(seg.end()));
                let (a, b) = (self.shared(s), self.shared(e));
                if a == b {
                    return None;
                }
                let c = self.point(center);
                let id = self.id();
                let (from, to) = if ccw { (a, b) } else { (b, a) };
                self.entities.push(json!({"id": format!("c{id}"), "type": "arc",
                    "center": format!("p{c}"), "start": format!("p{from}"), "end": format!("p{to}")}));
                Some((id, ccw, false))
            }
            Seg::Ellipse {
                frame, a, b, sweep, ..
            } => {
                let center = xy(math::origin(frame));
                let up = math::column(frame, 2)[2] >= 0.0;
                let x = math::column(frame, 0);
                let y = math::column(frame, 1);
                // The major axis is the longer one.
                let (major_dir, major, minor) = if a >= b { (x, *a, *b) } else { (y, *b, *a) };
                let major_point = [
                    center[0] + major * major_dir[0],
                    center[1] + major * major_dir[1],
                ];
                let c = self.point(center);
                let m = self.point(major_point);
                if sweep.abs() >= TAU - 1e-9 {
                    let id = self.id();
                    self.entities.push(json!({"id": format!("c{id}"), "type": "ellipse",
                        "center": format!("p{c}"), "major": format!("p{m}"), "minor_radius": minor}));
                    return Some((id, true, true));
                }
                let ccw = (*sweep > 0.0) == up;
                let (s, e) = (xy(seg.start()), xy(seg.end()));
                let (p, q) = (self.shared(s), self.shared(e));
                if p == q {
                    return None;
                }
                let id = self.id();
                let (from, to) = if ccw { (p, q) } else { (q, p) };
                self.entities
                    .push(json!({"id": format!("c{id}"), "type": "elliptical_arc",
                    "center": format!("p{c}"), "major": format!("p{m}"), "minor_radius": minor,
                    "start": format!("p{from}"), "end": format!("p{to}")}));
                Some((id, ccw, false))
            }
            Seg::Spline {
                degree,
                poles,
                weights,
                knots,
                multiplicities,
            } => {
                let full = full_knots(knots, multiplicities);
                let (lo, hi) = spline_domain(*degree, poles.len(), &full);
                let clamped = multiplicities.first().is_some_and(|m| *m > *degree)
                    && multiplicities.last().is_some_and(|m| *m > *degree);
                if !clamped {
                    notes.push(
                        "an unclamped B-spline: its ends may not meet the curves beside it"
                            .to_owned(),
                    );
                }
                let n = poles.len();
                let mut control = Vec::with_capacity(n);
                for (i, p) in poles.iter().enumerate() {
                    let id = if i == 0 || i == n - 1 {
                        self.shared(xy(*p))
                    } else {
                        self.point(xy(*p))
                    };
                    control.push(format!("p{id}"));
                }
                let id = self.id();
                // The model's splines run over [0, 1] when not told;
                // the file's knots, scaled, keep the parameterisation.
                let span = if hi > lo { hi - lo } else { 1.0 };
                let knots: Vec<f64> = full.iter().map(|k| (k - lo) / span).collect();
                let mut e = json!({"id": format!("c{id}"), "type": "spline",
                    "degree": degree, "control": control, "knots": knots});
                if !weights.is_empty() {
                    e["weights"] = json!(weights);
                }
                self.entities.push(e);
                Some((id, true, false))
            }
            Seg::Helix { .. } => {
                notes.push("a helix is no sketch curve: it is left out".to_owned());
                None
            }
        }
    }

    fn path(&mut self, path: &Path, notes: &mut Vec<String>) -> LoopCurves {
        path.segments
            .iter()
            .filter_map(|s| self.curve(s, notes))
            .map(|(id, forward, _)| (id, forward))
            .collect()
    }
}

/// The key of the region a closed loop bounds.
fn loop_key(curves: &LoopCurves) -> Option<String> {
    let c = |id: u32| EntityUid(id);
    let segments: Vec<SegmentKey> = match curves.as_slice() {
        [] => return None,
        [(id, _)] => vec![SegmentKey::closed(c(*id))],
        _ => {
            let n = curves.len();
            (0..n)
                .map(|i| {
                    let (id, forward) = curves[i];
                    let prev = curves[(i + n - 1) % n].0;
                    let next = curves[(i + 1) % n].0;
                    if forward {
                        SegmentKey::between(c(id), c(prev), c(next))
                    } else {
                        SegmentKey::between(c(id), c(next), c(prev))
                    }
                })
                .collect()
        }
    };
    RegionKey::new(segments).map(|k| k.to_string())
}

/// The sketch frame's JSON (on the component's xy plane, whose frame is
/// the component's own).
fn frame_json(frame: &Matrix) -> Value {
    json!({"origin": math::origin(frame), "x_axis": math::column(frame, 0),
           "y_axis": math::column(frame, 1)})
}

fn tolerance_of(points: impl Iterator<Item = [f64; 3]>) -> f64 {
    let size = points
        .map(|p| p[0].abs().max(p[1].abs()))
        .fold(1.0, f64::max);
    (size * 1e-9).max(1e-7)
}

/// A sketch of a profile in `frame` (its xy plane; the profile's curves in
/// the frame's coordinates).
pub fn profile_sketch(frame: &Matrix, profile: &Profile) -> Result<SketchBuild, String> {
    let tolerance = tolerance_of(
        profile
            .areas
            .iter()
            .flat_map(|a| a.outer.segments.iter().flat_map(|s| s.sample(2))),
    );
    let mut b = Builder::new(tolerance);
    let mut notes = Vec::new();
    let mut regions = Vec::new();
    let mut holes = Vec::new();
    for area in &profile.areas {
        let outer = b.path(&area.outer, &mut notes);
        let mut keys = Vec::new();
        for h in &area.holes {
            let curves = b.path(h, &mut notes);
            keys.extend(loop_key(&curves));
        }
        holes.push(keys);
        match loop_key(&outer) {
            Some(key) => regions.push(key),
            None => notes.push("an area without curves".to_owned()),
        }
    }
    if regions.is_empty() {
        return Err("the profile has no area".to_owned());
    }
    Ok(SketchBuild {
        def: json!({"type": "sketch", "plane": "xy", "frame": frame_json(frame),
                    "entities": b.entities}),
        regions,
        holes,
        curves: Vec::new(),
        notes,
    })
}

/// A sketch of a path in `frame` (the path in the frame's coordinates,
/// z = 0).
pub fn path_sketch(frame: &Matrix, path: &Path) -> Result<SketchBuild, String> {
    let tolerance = tolerance_of(path.segments.iter().flat_map(|s| s.sample(2)));
    let mut b = Builder::new(tolerance);
    let mut notes = Vec::new();
    let curves = b.path(path, &mut notes);
    if curves.is_empty() {
        return Err("the path has no length".to_owned());
    }
    Ok(SketchBuild {
        def: json!({"type": "sketch", "plane": "xy", "frame": frame_json(frame),
                    "entities": b.entities}),
        regions: Vec::new(),
        holes: Vec::new(),
        curves: curves.iter().map(|(id, _)| format!("c{id}")).collect(),
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_ifc::geometry::{Area, rounded};

    #[test]
    fn region_keys_of_loops() {
        let square = rounded(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], &[0.0; 4]);
        let profile = Profile {
            kind: String::new(),
            name: String::new(),
            areas: vec![Area {
                outer: square,
                holes: Vec::new(),
            }],
        };
        let s = profile_sketch(&math::IDENTITY, &profile).expect("a sketch");
        // Points p1, p2 (c3), p4 (c5), p6 (c7), back to p1 (c8).
        assert_eq!(
            s.regions,
            vec!["r{c3[c8,c5],c5[c3,c7],c7[c5,c8],c8[c7,c3]}"]
        );
        let circle = Profile {
            kind: String::new(),
            name: String::new(),
            areas: vec![Area {
                outer: Path {
                    segments: vec![Seg::Arc {
                        frame: math::IDENTITY,
                        radius: 2.0,
                        start: 0.0,
                        sweep: TAU,
                    }],
                },
                holes: Vec::new(),
            }],
        };
        let s = profile_sketch(&math::IDENTITY, &circle).expect("a sketch");
        assert_eq!(s.regions, vec!["r{c2}"]);
    }
}
