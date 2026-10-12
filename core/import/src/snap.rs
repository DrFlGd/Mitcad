// SPDX-License-Identifier: MIT
//! Linked curves on the model's own geometry.
//!
//! A curve the file links to geometry outside the sketch (a projected edge
//! or face) comes in with the numbers the file stored: single-precision
//! values of the length unit, about 1e-6 mm off the geometry they follow
//! (more on a large body). The edge itself is exact in the replay, so a
//! feature that meets it (a tool face tangent to the projected circle, a
//! join along the wall the circle follows) found the two apart by that
//! little: the boolean produced an invalid shape, or the join no longer
//! touched.
//!
//! [`snap_linked`] puts each linked line, circle, arc and ellipse that an
//! edge of the replay's bodies projects onto, within [`SNAP`], on that
//! projection. What is not found stays as it was.

use std::collections::HashMap;
use std::f64::consts::TAU;

use mitcad_model::Curve3;
use mitcad_model::SketchFrame;
use mitcad_model::datum::CurveGeometry;
use mitcad_model::sketch::geometry::Curve2;
use mitcad_model::sketch::project::{Projected, project};
use serde_json::{Value, json};

use crate::sketch::SketchParts;

/// How far (mm) a linked curve's stored geometry may lie from an edge's
/// projection to be that edge's: far above the stored numbers' rounding,
/// far below any distance a design means.
pub(crate) const SNAP: f64 = 2e-3;

type P = [f64; 2];

fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot3(a, a).sqrt();
    (n > 1e-12).then(|| [a[0] / n, a[1] / n, a[2] / n])
}

/// An edge's curve as a model curve for projection (lines, circles and
/// arcs; the rest is no projection this module matches).
fn model_curve(g: &CurveGeometry) -> Option<Curve3> {
    match g {
        CurveGeometry::Line { start, end } => Some(Curve3::Line {
            start: *start,
            end: *end,
        }),
        CurveGeometry::Circle {
            center,
            normal,
            radius,
            start,
            end,
        } => {
            let normal = unit3(*normal)?;
            let x_axis = unit3(sub3(*start, *center))?;
            let y_axis = cross3(normal, x_axis);
            let closed = {
                let d = sub3(*end, *start);
                dot3(d, d).sqrt() <= 1e-9 * radius.max(1.0)
            };
            let to = sub3(*end, *center);
            let angle = dot3(to, y_axis).atan2(dot3(to, x_axis)).rem_euclid(TAU);
            Some(Curve3::Conic {
                center: *center,
                normal,
                x_axis,
                major: *radius,
                minor: *radius,
                start: 0.0,
                end: if closed { TAU } else { angle },
                closed,
            })
        }
        CurveGeometry::Other { .. } => None,
    }
}

/// The curves the edges project to in the sketch's frame (mm), only those
/// that come within `margin` of the box (`low`, `high`).
fn projections(
    edges: &[CurveGeometry],
    frame: &SketchFrame,
    low: P,
    high: P,
    margin: f64,
) -> Vec<Curve2> {
    let mut out = Vec::new();
    for edge in edges {
        let Some(curve) = model_curve(edge) else {
            continue;
        };
        for p in project(std::slice::from_ref(&curve), frame) {
            let Projected::Curve(c) = p else { continue };
            let (lo, hi) = c.bounds();
            if hi[0] < low[0] - margin
                || lo[0] > high[0] + margin
                || hi[1] < low[1] - margin
                || lo[1] > high[1] + margin
            {
                continue;
            }
            out.push(c);
        }
    }
    out
}

/// What a snapped curve changes: points to put elsewhere (by entity id)
/// and the new radii (circles) or minor radii (ellipses).
#[derive(Default)]
struct Change {
    points: Vec<(String, P)>,
    radius: Option<f64>,
}

/// The entities of a sketch by id, with the positions of its points.
struct View<'a> {
    entities: &'a [Value],
    index: HashMap<&'a str, usize>,
}

impl<'a> View<'a> {
    fn new(entities: &'a [Value]) -> Self {
        let index = entities
            .iter()
            .enumerate()
            .filter_map(|(i, e)| Some((e["id"].as_str()?, i)))
            .collect();
        View { entities, index }
    }

    fn point(&self, id: &Value) -> Option<P> {
        let e = &self.entities[*self.index.get(id.as_str()?)?];
        let at = e["at"].as_array()?;
        Some([at.first()?.as_f64()?, at.get(1)?.as_f64()?])
    }
}

/// The curve a round or straight linked entity stands for, and what to
/// change for the projection `to` it matches; None where it does not.
fn fit(entity: &Value, view: &View, projected: &[Curve2]) -> Option<Change> {
    let near = |a: P, b: P| dist(a, b) <= SNAP;
    let id = |key: &str| entity[key].as_str().map(str::to_owned);
    match entity["type"].as_str()? {
        "line" => {
            let (a, b) = (view.point(&entity["start"])?, view.point(&entity["end"])?);
            projected.iter().find_map(|c| match c {
                Curve2::Line { a: x, b: y } if near(a, *x) && near(b, *y) => Some(Change {
                    points: vec![(id("start")?, *x), (id("end")?, *y)],
                    radius: None,
                }),
                Curve2::Line { a: x, b: y } if near(a, *y) && near(b, *x) => Some(Change {
                    points: vec![(id("start")?, *y), (id("end")?, *x)],
                    radius: None,
                }),
                _ => None,
            })
        }
        "circle" => {
            let center = view.point(&entity["center"])?;
            let radius = entity["radius"].as_f64()?;
            projected.iter().find_map(|c| match c {
                Curve2::Circle {
                    center: x,
                    radius: r,
                    ..
                } if near(center, *x) && (radius - r).abs() <= SNAP => Some(Change {
                    points: vec![(id("center")?, *x)],
                    radius: Some(*r),
                }),
                _ => None,
            })
        }
        "arc" => {
            let center = view.point(&entity["center"])?;
            let (a, b) = (view.point(&entity["start"])?, view.point(&entity["end"])?);
            projected.iter().find_map(|c| {
                let Curve2::Arc {
                    center: x,
                    radius: r,
                    ..
                } = c
                else {
                    return None;
                };
                let (s, e) = c.ends()?;
                let radius = dist(center, a);
                if !(near(center, *x) && (radius - r).abs() <= SNAP && near(a, s) && near(b, e)) {
                    return None;
                }
                Some(Change {
                    points: vec![(id("center")?, *x), (id("start")?, s), (id("end")?, e)],
                    radius: None,
                })
            })
        }
        kind @ ("ellipse" | "elliptical_arc") => {
            let center = view.point(&entity["center"])?;
            let major = view.point(&entity["major"])?;
            let minor = entity["minor_radius"].as_f64()?;
            let length = dist(center, major);
            let direction = [
                (major[0] - center[0]) / length,
                (major[1] - center[1]) / length,
            ];
            projected.iter().find_map(|c| {
                let (x, big, small, rotation) = match c {
                    Curve2::Ellipse {
                        center,
                        major,
                        minor,
                        rotation,
                    }
                    | Curve2::EllipticalArc {
                        center,
                        major,
                        minor,
                        rotation,
                        ..
                    } => (*center, *major, *minor, *rotation),
                    _ => return None,
                };
                // The major axis points either way along the line.
                let axis = [rotation.cos(), rotation.sin()];
                let along = direction[0] * axis[0] + direction[1] * axis[1];
                if !(near(center, x)
                    && (length - big).abs() <= SNAP
                    && (minor - small).abs() <= SNAP
                    && (along.abs() - 1.0).abs() <= 1e-6)
                {
                    return None;
                }
                let sign = along.signum();
                let tip = [x[0] + sign * axis[0] * big, x[1] + sign * axis[1] * big];
                let mut points = vec![(id("center")?, x), (id("major")?, tip)];
                if kind == "elliptical_arc" {
                    let (s, e) = c.ends()?;
                    let (a, b) = (view.point(&entity["start"])?, view.point(&entity["end"])?);
                    if !(near(a, s) && near(b, e)) {
                        return None;
                    }
                    points.push((id("start")?, s));
                    points.push((id("end")?, e));
                }
                Some(Change {
                    points,
                    radius: Some(small),
                })
            })
        }
        _ => None,
    }
}

/// For the trace: how far the nearest projected curve of the entity's kind
/// is (centre and radius, or ends).
fn nearest(entity: &Value, view: &View, projected: &[Curve2]) -> String {
    let mut best: Option<(f64, String)> = None;
    let mut offer = |d: f64, text: String| {
        if best.as_ref().is_none_or(|(b, _)| d < *b) {
            best = Some((d, text));
        }
    };
    match entity["type"].as_str() {
        Some("circle" | "arc") => {
            let (Some(center), Some(radius)) = (
                view.point(&entity["center"]),
                entity["radius"].as_f64().or_else(|| {
                    Some(dist(
                        view.point(&entity["center"])?,
                        view.point(&entity["start"])?,
                    ))
                }),
            ) else {
                return String::new();
            };
            for c in projected {
                let (x, r) = match c {
                    Curve2::Circle { center, radius, .. } | Curve2::Arc { center, radius, .. } => {
                        (*center, *radius)
                    }
                    _ => continue,
                };
                let d = dist(center, x).max((radius - r).abs());
                offer(
                    d,
                    format!(
                        "centre off by {:.2e}, radius {:.4} against {:.4}",
                        dist(center, x),
                        radius,
                        r
                    ),
                );
            }
        }
        Some("line") => {
            let (Some(a), Some(b)) = (view.point(&entity["start"]), view.point(&entity["end"]))
            else {
                return String::new();
            };
            for c in projected {
                if let Curve2::Line { a: x, b: y } = c {
                    let d = (dist(a, *x) + dist(b, *y)).min(dist(a, *y) + dist(b, *x));
                    offer(d, format!("ends off by {d:.2e}"));
                }
            }
        }
        _ => {}
    }
    best.map(|(_, t)| t).unwrap_or_default()
}

/// Puts the linked curves of `parts` that an edge projects onto on that
/// projection; how many it moved. `edges` are the replay's edges in the
/// space of the sketch's `frame` (mm).
#[cfg(test)]
pub(crate) fn snap_linked(
    parts: &mut SketchParts,
    frame: &SketchFrame,
    edges: &[CurveGeometry],
) -> usize {
    snap_linked_keeping(parts, frame, edges, &HashMap::new())
}

/// [`snap_linked`], with points already placed (`kept`, by entity id: the
/// ends of curves put on the edges of the sketch's face, where two of them
/// meet): a curve that would move one of them elsewhere stays as it was.
pub(crate) fn snap_linked_keeping(
    parts: &mut SketchParts,
    frame: &SketchFrame,
    edges: &[CurveGeometry],
    kept: &HashMap<String, P>,
) -> usize {
    let linked: Vec<usize> = parts
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e["reference"] == json!(true) && e["type"].as_str().is_some_and(|t| t != "point")
        })
        .map(|(i, _)| i)
        .collect();
    if linked.is_empty() || edges.is_empty() {
        return 0;
    }
    // The box of the linked curves' points, to leave out far edges.
    let view = View::new(&parts.entities);
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for e in &parts.entities {
        if e["type"] == "point"
            && let Some(p) = view.point(&e["id"])
        {
            for k in 0..2 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    if !(low[0].is_finite() && high[0].is_finite()) {
        return 0;
    }
    let extent = dist(low, high);
    let projected = projections(edges, frame, low, high, extent + 1.0);
    if crate::tracing() {
        eprintln!(
            "import:   snap: {} linked curves, {} edges, {} projected near",
            linked.len(),
            edges.len(),
            projected.len()
        );
    }
    if projected.is_empty() {
        return 0;
    }
    let mut changes: Vec<(usize, Change)> = Vec::new();
    for &i in &linked {
        if let Some(change) = fit(&parts.entities[i], &view, &projected) {
            changes.push((i, change));
        } else if crate::tracing() {
            eprintln!(
                "import:   snap: {} {} found no edge: {}",
                parts.entities[i]["type"],
                parts.entities[i]["id"],
                nearest(&parts.entities[i], &view, &projected)
            );
        }
    }
    // A point two curves share goes where the first put it.
    let mut placed: HashMap<String, P> = kept.clone();
    let mut moved = 0;
    let mut radii: Vec<(usize, f64)> = Vec::new();
    for (i, change) in changes {
        if change
            .points
            .iter()
            .any(|(id, at)| placed.get(id).is_some_and(|p| dist(*p, *at) > 1e-9))
        {
            continue;
        }
        for (id, at) in change.points {
            placed.insert(id, at);
        }
        if let Some(r) = change.radius {
            radii.push((i, r));
        }
        moved += 1;
    }
    for e in &mut parts.entities {
        let Some(id) = e["id"].as_str() else { continue };
        if let Some(at) = placed.get(id) {
            e["at"] = json!(at);
        }
    }
    for (i, r) in radii {
        let key = if parts.entities[i]["type"] == "circle" {
            "radius"
        } else {
            "minor_radius"
        };
        parts.entities[i][key] = json!(r);
    }
    // The positions the solved sketch is checked against.
    for (n, at) in &mut parts.positions {
        if let Some(p) = placed.get(&format!("p{n}")) {
            *at = *p;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle_edge(center: [f64; 3], radius: f64) -> CurveGeometry {
        CurveGeometry::Circle {
            center,
            normal: [0.0, 0.0, 1.0],
            radius,
            start: [center[0] + radius, center[1], center[2]],
            end: [center[0] + radius, center[1], center[2]],
        }
    }

    fn parts(entities: Vec<Value>) -> SketchParts {
        let positions = entities
            .iter()
            .filter(|e| e["type"] == "point")
            .filter_map(|e| {
                let n: u32 = e["id"].as_str()?.trim_start_matches('p').parse().ok()?;
                let at = e["at"].as_array()?;
                Some((n, [at[0].as_f64()?, at[1].as_f64()?]))
            })
            .collect();
        SketchParts {
            entities,
            positions,
            ..SketchParts::default()
        }
    }

    #[test]
    fn a_linked_circle_goes_to_its_edge() {
        // The stored radius and centre are off by about 1.2e-6 mm.
        let mut p = parts(vec![
            json!({"id": "p1", "type": "point", "at": [0.0, 65.000_001_2], "fixed": true, "reference": true}),
            json!({"id": "c2", "type": "circle", "center": "p1", "radius": 7.000_001_2,
                   "fixed": true, "reference": true}),
        ]);
        // The sketch looks down at the plane z = 20 from the XY plane's side, y flipped.
        let frame = SketchFrame {
            origin: [0.0, 0.0, 20.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
        };
        let edges = vec![
            circle_edge([0.0, 65.0, 20.0], 7.0),
            circle_edge([0.0, 0.0, 0.0], 3.0),
        ];
        assert_eq!(snap_linked(&mut p, &frame, &edges), 1);
        assert_eq!(p.entities[1]["radius"], json!(7.0));
        assert_eq!(p.entities[0]["at"], json!([0.0, 65.0]));
        assert_eq!(p.positions, vec![(1, [0.0, 65.0])]);
    }

    /// A point already placed (by the face's edges) keeps its place: the
    /// linked circle that would move it stays as it was.
    #[test]
    fn a_kept_point_stays() {
        let mut p = parts(vec![
            json!({"id": "p1", "type": "point", "at": [0.0, 65.000_001_2], "reference": true}),
            json!({"id": "c2", "type": "circle", "center": "p1", "radius": 7.000_001_2,
                   "reference": true}),
        ]);
        let edges = vec![circle_edge([0.0, 65.0, 0.0], 7.0)];
        let kept = HashMap::from([("p1".to_owned(), [0.0, 65.000_001_2])]);
        assert_eq!(
            snap_linked_keeping(&mut p, &SketchFrame::XY, &edges, &kept),
            0
        );
        assert_eq!(p.entities[1]["radius"], json!(7.000_001_2));
        assert_eq!(p.entities[0]["at"], json!([0.0, 65.000_001_2]));
    }

    #[test]
    fn a_circle_apart_from_every_edge_stays() {
        let mut p = parts(vec![
            json!({"id": "p1", "type": "point", "at": [0.0, 0.0], "reference": true}),
            json!({"id": "c2", "type": "circle", "center": "p1", "radius": 7.01, "reference": true}),
        ]);
        let edges = vec![circle_edge([0.0, 0.0, 0.0], 7.0)];
        assert_eq!(snap_linked(&mut p, &SketchFrame::XY, &edges), 0);
        assert_eq!(p.entities[1]["radius"], json!(7.01));
    }

    #[test]
    fn a_linked_line_and_the_arc_on_its_end_share_the_point() {
        let mut p = parts(vec![
            json!({"id": "p1", "type": "point", "at": [0.000_001, 0.0], "reference": true}),
            json!({"id": "p2", "type": "point", "at": [10.0, 0.000_001], "reference": true}),
            json!({"id": "p3", "type": "point", "at": [10.0, 5.000_001], "reference": true}),
            json!({"id": "p4", "type": "point", "at": [15.0, 5.0], "reference": true}),
            json!({"id": "c5", "type": "line", "start": "p1", "end": "p2", "reference": true}),
            json!({"id": "c6", "type": "arc", "center": "p3", "start": "p2", "end": "p4", "reference": true}),
        ]);
        let edges = vec![
            CurveGeometry::Line {
                start: [0.0, 0.0, 3.0],
                end: [10.0, 0.0, 3.0],
            },
            CurveGeometry::Circle {
                center: [10.0, 5.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                radius: 5.0,
                start: [10.0, 0.0, 0.0],
                end: [15.0, 5.0, 0.0],
            },
        ];
        assert_eq!(snap_linked(&mut p, &SketchFrame::XY, &edges), 2);
        assert_eq!(p.entities[0]["at"], json!([0.0, 0.0]));
        assert_eq!(p.entities[1]["at"], json!([10.0, 0.0]));
        assert_eq!(p.entities[2]["at"], json!([10.0, 5.0]));
        assert_eq!(p.entities[3]["at"], json!([15.0, 5.0]));
    }
}
