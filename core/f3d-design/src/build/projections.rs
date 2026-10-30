// SPDX-License-Identifier: MIT
//! What a sketch's linked curves follow, where it is a curve of an earlier
//! sketch (a projection of another sketch's geometry).
//!
//! A sketch feature ([`SKETCH_FEATURE`]) lists one projection record
//! ([`SKETCH_PROJECTION`]) for each entity projected into its sketch. The
//! record refers to an input slot ([`INPUT_SLOT`]) whose entity input
//! ([`ENTITY_REF`]) names the source as sweeps' paths do: a sketch curve by
//! its ids ([`SKETCH_CURVE_ID`]), the curve as it was stored next to it.
//! The record does not name the curve it made, so each source is matched to
//! the sketch's linked curve that its projection onto the sketch's plane
//! is: the same kind, the same points. On a design with 233 linked curves
//! in 12 sketches (projections of two earlier sketches' profiles) all 250
//! records resolve and each lies on one linked curve; 228 are in the order
//! of the curves, so the order is not used.
//!
//! The linked curve gets the source as its `referencedEntity` (a
//! `sketch_entity` reference, as the API gives it). Records whose input
//! names a face, an edge or a point are left alone.

use super::super::classes::*;
use super::super::ir::*;
use super::super::sketch::SketchInfo;
use super::Builder;

/// How far (cm) a projected source point may lie from the linked curve's
/// to be that curve: both are the file's numbers, so only rounding.
const SAME: f64 = 1e-6;

/// A curve's kind and characteristic points in model space (cm): a line's
/// ends, a circle's centre, an arc's centre and ends; a circle's or an
/// arc's radius.
struct Shape {
    kind: String,
    points: Vec<[f64; 3]>,
    radius: Option<f64>,
    /// Points whose order does not matter (a line's or an arc's ends).
    unordered: bool,
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|x| x * s)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A sketch's frame in model space: origin, x and y axis.
struct Frame {
    origin: [f64; 3],
    x: [f64; 3],
    y: [f64; 3],
}

impl Frame {
    fn of(s: &SketchInfo) -> Option<Frame> {
        let f = s.detail.model_frame.as_ref()?;
        Some(Frame {
            origin: f.origin?,
            x: f.x_axis?,
            y: f.y_axis?,
        })
    }

    fn to_model(&self, p: [f64; 3]) -> [f64; 3] {
        add(self.origin, add(scale(self.x, p[0]), scale(self.y, p[1])))
    }

    /// A model point projected along the frame's normal onto its plane.
    fn onto(&self, p: [f64; 3]) -> [f64; 3] {
        let d = sub(p, self.origin);
        self.to_model([dot(d, self.x), dot(d, self.y), 0.0])
    }
}

/// A curve of a sketch with `frame` as a [`Shape`] in model space.
fn shape(frame: &Frame, c: &SketchCurve) -> Option<Shape> {
    let kind = c.curve_type.clone()?;
    let g = c.geometry.as_ref()?;
    let at = |p: [f64; 3]| frame.to_model(p);
    match kind.as_str() {
        "SketchLine" => Some(Shape {
            points: vec![at(g.start_point?), at(g.end_point?)],
            radius: None,
            kind,
            unordered: true,
        }),
        "SketchCircle" => Some(Shape {
            points: vec![at(g.center?)],
            radius: Some(g.radius?),
            kind,
            unordered: false,
        }),
        "SketchArc" => {
            let (c, r) = (g.center?, g.radius?);
            let (u, n) = (g.reference_vector?, g.normal?);
            let v = cross(n, u);
            let end = |t: f64| add(c, add(scale(u, r * t.cos()), scale(v, r * t.sin())));
            Some(Shape {
                points: vec![at(c), at(end(g.start_angle?)), at(end(g.end_angle?))],
                radius: Some(r),
                kind,
                unordered: true,
            })
        }
        _ => None,
    }
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    let d = sub(a, b);
    dot(d, d).sqrt() <= SAME
}

/// Whether `source`, projected onto `frame`'s plane, is `curve`.
fn projects_onto(frame: &Frame, source: &Shape, curve: &Shape) -> bool {
    if source.kind != curve.kind || source.points.len() != curve.points.len() {
        return false;
    }
    match (source.radius, curve.radius) {
        (Some(a), Some(b)) if (a - b).abs() > SAME => return false,
        (Some(_), None) | (None, Some(_)) => return false,
        _ => {}
    }
    let s: Vec<[f64; 3]> = source.points.iter().map(|&p| frame.onto(p)).collect();
    let same = |a: &[[f64; 3]], b: &[[f64; 3]]| a.iter().zip(b).all(|(p, q)| near(*p, *q));
    if same(&s, &curve.points) {
        return true;
    }
    // A line's or an arc's ends either way round (the centre stays first).
    if source.unordered {
        let keep = if source.kind == "SketchArc" { 1 } else { 0 };
        let mut r = s.clone();
        r[keep..].reverse();
        return same(&r, &curve.points);
    }
    false
}

impl Builder<'_> {
    /// The sketch curve object a projection record's entity input names.
    fn projection_source(&self, record: u64) -> Option<u64> {
        let seg = self.seg;
        let slot = seg
            .ref_ids(seg.data_of(record))
            .into_iter()
            .find(|&r| seg.guid_of(r) == Some(INPUT_SLOT))?;
        let input = seg
            .ref_ids(seg.data_of(slot))
            .into_iter()
            .find(|&r| seg.guid_of(r) == Some(ENTITY_REF))?;
        self.input_entity_object(input)
            .filter(|&e| !is_point_class(seg.guid_of(e)))
    }

    /// Gives the linked curves of `sketches` that are projections of
    /// earlier sketches' curves those curves as their `referencedEntity`.
    pub(super) fn projection_sources(&self, sketches: &mut [SketchInfo]) {
        let seg = self.seg;
        let mut found: Vec<(usize, usize, Reference)> = Vec::new();
        for (si, s) in sketches.iter().enumerate() {
            let linked: Vec<usize> = s
                .detail
                .curves
                .iter()
                .flatten()
                .enumerate()
                .filter(|(_, c)| c.is_linked == Some(true))
                .map(|(i, _)| i)
                .collect();
            if linked.is_empty() {
                continue;
            }
            let (Some(&feature), Some(frame)) = (self.feat_of_sk.get(&s.id), Frame::of(s)) else {
                continue;
            };
            let curves = s.detail.curves.as_deref().unwrap_or(&[]);
            let mut records = seg.ref_ids(seg.data_of(feature));
            records.retain(|&r| seg.guid_of(r) == Some(SKETCH_PROJECTION));
            let mut seen = std::collections::HashSet::new();
            records.retain(|r| seen.insert(*r));
            let mut taken = vec![false; curves.len()];
            for record in records {
                let Some(e) = self.projection_source(record) else {
                    continue;
                };
                let Some((sid, lid)) = self.local_ids.get(&e) else {
                    continue;
                };
                let Some(source) = sketches.iter().find(|o| o.id == *sid).and_then(|o| {
                    let c = o
                        .detail
                        .curves
                        .iter()
                        .flatten()
                        .find(|c| c.id.as_ref() == Some(lid))?;
                    shape(&Frame::of(o)?, c)
                }) else {
                    continue;
                };
                let hit = linked.iter().copied().find(|&i| {
                    !taken[i]
                        && shape(&frame, &curves[i])
                            .is_some_and(|c| projects_onto(&frame, &source, &c))
                });
                let (Some(i), Some(r)) = (hit, self.sketch_entity(e)) else {
                    continue;
                };
                taken[i] = true;
                found.push((si, i, r));
            }
        }
        for (si, i, r) in found {
            if let Some(c) = sketches[si]
                .detail
                .curves
                .as_mut()
                .and_then(|c| c.get_mut(i))
            {
                c.referenced_entity = Some(r);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: [f64; 3], b: [f64; 3]) -> Shape {
        Shape {
            kind: "SketchLine".into(),
            points: vec![a, b],
            radius: None,
            unordered: true,
        }
    }

    /// A plane at x = 1.7 cm looking along x, as the ribs' sketches of the
    /// design in the module documentation.
    fn rib_plane() -> Frame {
        Frame {
            origin: [1.7, 0.0, 0.0],
            x: [0.0, 0.0, -1.0],
            y: [0.0, 1.0, 0.0],
        }
    }

    #[test]
    fn a_line_of_a_parallel_sketch_projects_onto_its_copy() {
        let f = rib_plane();
        let source = line([-1.7, 0.24, 6.0], [-1.7, 2.94, 6.0]);
        assert!(projects_onto(
            &f,
            &source,
            &line([1.7, 0.24, 6.0], [1.7, 2.94, 6.0])
        ));
        // Either way round.
        assert!(projects_onto(
            &f,
            &source,
            &line([1.7, 2.94, 6.0], [1.7, 0.24, 6.0])
        ));
        // Not onto the wall's other side.
        assert!(!projects_onto(
            &f,
            &source,
            &line([1.7, 0.24, -6.0], [1.7, 2.94, -6.0])
        ));
    }

    #[test]
    fn circles_and_arcs_need_their_radius() {
        let f = rib_plane();
        let circle = |c: [f64; 3], r: f64| Shape {
            kind: "SketchCircle".into(),
            points: vec![c],
            radius: Some(r),
            unordered: false,
        };
        assert!(projects_onto(
            &f,
            &circle([-3.0, 1.0, 2.0], 0.5),
            &circle([1.7, 1.0, 2.0], 0.5)
        ));
        assert!(!projects_onto(
            &f,
            &circle([-3.0, 1.0, 2.0], 0.5),
            &circle([1.7, 1.0, 2.0], 0.6)
        ));
        let arc = |ends: [[f64; 3]; 2]| Shape {
            kind: "SketchArc".into(),
            points: vec![[1.7, 0.0, 0.0], ends[0], ends[1]],
            radius: Some(1.0),
            unordered: true,
        };
        let a = arc([[1.7, 1.0, 0.0], [1.7, 0.0, 1.0]]);
        // The ends either way round, the centre first.
        assert!(projects_onto(
            &f,
            &a,
            &arc([[1.7, 0.0, 1.0], [1.7, 1.0, 0.0]])
        ));
        assert!(!projects_onto(
            &f,
            &line([1.7, 1.0, 0.0], [1.7, 0.0, 1.0]),
            &a
        ));
    }
}
