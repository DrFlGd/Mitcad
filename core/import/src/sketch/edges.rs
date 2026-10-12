// SPDX-License-Identifier: MIT
//! Sketch curves onto the edges of the face the sketch lies on.
//!
//! The file's geometry is exact to its own tolerance, 1e-5 mm: a curve
//! the file links to the model (a projection of a face's boundary, which
//! comes in as a fixed reference entity) or draws on a face's edge is
//! stored up to that far off the edge, and the replay's edge may lie
//! elsewhere within it (a circle of 170 mm diameter whose stored radius is
//! 85.0000012 mm, which the replay solves to 85 mm, while a sketch drawn on
//! the extruded face's edge without a dimension keeps 85.0000012 mm). A
//! profile on such curves then lies 1e-6 mm off the body's walls: a join
//! leaves the extruded walls a separate solid on the body's face or their
//! faces apart, an edge splits in two. Here each line, circle and arc of
//! the sketch that lies on the curve of an edge of the face within the
//! tolerance takes that curve: its points move onto it (a point two such
//! curves share to where their curves meet) and a circle takes its radius.
//! A curve whose points cannot all move consistently within the tolerance
//! stays as stored. The solve then starts from those positions. Linked
//! curves that follow other edges are put on them after this
//! (`snap.rs`, which keeps the points placed here).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

/// How far a curve may lie off the face's edge it is on, mm (the file's own
/// tolerance).
pub const EDGE_TOLERANCE: f64 = 1e-5;

/// The curve of a model edge in the sketch's coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Carrier {
    /// Through `point` along the unit vector `direction`.
    Line {
        point: [f64; 2],
        direction: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

impl Carrier {
    /// The point of the carrier nearest `p`.
    fn nearest(&self, p: [f64; 2]) -> [f64; 2] {
        match *self {
            Self::Line { point, direction } => {
                add(point, scale(direction, dot(sub(p, point), direction)))
            }
            Self::Circle { center, radius } => {
                let d = sub(p, center);
                let n = dot(d, d).sqrt();
                if n < 1e-300 {
                    return p;
                }
                add(center, scale(d, radius / n))
            }
        }
    }

    fn distance(&self, p: [f64; 2]) -> f64 {
        dist(self.nearest(p), p)
    }

    /// The same curve (within `tol`).
    fn same(&self, other: &Self, tol: f64) -> bool {
        match (*self, *other) {
            (
                Self::Line {
                    point: _,
                    direction,
                },
                Self::Line {
                    point: q,
                    direction: e,
                },
            ) => {
                (direction[0] * e[1] - direction[1] * e[0]).abs() < 1e-12 && self.distance(q) <= tol
            }
            (
                Self::Circle { center, radius },
                Self::Circle {
                    center: c,
                    radius: r,
                },
            ) => dist(center, c) <= tol && (radius - r).abs() <= tol,
            _ => false,
        }
    }

    /// Where two carriers meet, nearest `near`.
    fn meet(&self, other: &Self, near: [f64; 2]) -> Option<[f64; 2]> {
        let mut points = Vec::new();
        match (*self, *other) {
            (
                Self::Line { point, direction },
                Self::Line {
                    point: q,
                    direction: e,
                },
            ) => {
                let det = direction[0] * e[1] - direction[1] * e[0];
                if det.abs() < 1e-12 {
                    return None;
                }
                let w = sub(q, point);
                let t = (w[0] * e[1] - w[1] * e[0]) / det;
                points.push(add(point, scale(direction, t)));
            }
            (Self::Line { point, direction }, Self::Circle { center, radius })
            | (Self::Circle { center, radius }, Self::Line { point, direction }) => {
                let foot = add(point, scale(direction, dot(sub(center, point), direction)));
                let h = dist(foot, center);
                let s2 = radius * radius - h * h;
                let s = if s2 < 0.0 { 0.0 } else { s2.sqrt() };
                points.push(add(foot, scale(direction, s)));
                points.push(add(foot, scale(direction, -s)));
            }
            (
                Self::Circle {
                    center: c1,
                    radius: r1,
                },
                Self::Circle {
                    center: c2,
                    radius: r2,
                },
            ) => {
                let d = dist(c1, c2);
                if d < 1e-12 {
                    return None;
                }
                let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
                let h2 = r1 * r1 - a * a;
                let h = if h2 < 0.0 { 0.0 } else { h2.sqrt() };
                let u = scale(sub(c2, c1), 1.0 / d);
                let base = add(c1, scale(u, a));
                let n = [-u[1], u[0]];
                points.push(add(base, scale(n, h)));
                points.push(add(base, scale(n, -h)));
            }
        }
        points
            .into_iter()
            .min_by(|a, b| dist(*a, near).total_cmp(&dist(*b, near)))
    }
}

/// What a snapped curve asks of a point: to be its carrier's centre, or to
/// lie on it.
#[derive(Debug, Clone, Copy)]
enum Ask {
    Center(usize),
    On(usize),
}

/// Moves the curves among `entities` (the sketch's definition entities)
/// onto the `carriers` they lie on within `tol`, and the points `positions`
/// keeps with them. Returns the ids of the curves that took a carrier.
pub fn snap_to_edges(
    entities: &mut [Value],
    positions: &mut [(u32, [f64; 2])],
    carriers: &[Carrier],
    tol: f64,
) -> Vec<String> {
    if carriers.is_empty() {
        return Vec::new();
    }
    let at: BTreeMap<String, [f64; 2]> = entities
        .iter()
        .filter(|e| e["type"] == "point")
        .filter_map(|e| {
            let p = e["at"].as_array()?;
            Some((
                e["id"].as_str()?.to_owned(),
                [p.first()?.as_f64()?, p.get(1)?.as_f64()?],
            ))
        })
        .collect();
    let point = |id: &Value| id.as_str().and_then(|i| at.get(i).copied());
    // Each curve's carrier and what it asks of its points.
    let mut asks: BTreeMap<String, Vec<Ask>> = BTreeMap::new();
    let mut curves: Vec<(String, usize, Vec<String>)> = Vec::new();
    for e in entities.iter() {
        let Some(id) = e["id"].as_str() else { continue };
        let pick = |fits: &dyn Fn(&Carrier) -> Option<f64>| {
            carriers
                .iter()
                .enumerate()
                .filter_map(|(k, c)| fits(c).filter(|d| *d <= tol).map(|d| (d, k)))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, k)| k)
        };
        let (carrier, wants) = match e["type"].as_str() {
            Some("line") => {
                let (Some(a), Some(b)) = (point(&e["start"]), point(&e["end"])) else {
                    continue;
                };
                let k = pick(&|c| match c {
                    Carrier::Line { .. } => Some(c.distance(a).max(c.distance(b))),
                    Carrier::Circle { .. } => None,
                });
                (k, vec![(&e["start"], false), (&e["end"], false)])
            }
            Some("circle") => {
                let (Some(center), Some(r)) = (point(&e["center"]), e["radius"].as_f64()) else {
                    continue;
                };
                let k = pick(&|c| match *c {
                    Carrier::Circle {
                        center: cc,
                        radius: rc,
                    } => Some(dist(center, cc).max((r - rc).abs())),
                    Carrier::Line { .. } => None,
                });
                (k, vec![(&e["center"], true)])
            }
            Some("arc") => {
                let (Some(center), Some(s), Some(t)) =
                    (point(&e["center"]), point(&e["start"]), point(&e["end"]))
                else {
                    continue;
                };
                let k = pick(&|c| match *c {
                    Carrier::Circle { center: cc, .. } => {
                        Some(dist(center, cc).max(c.distance(s)).max(c.distance(t)))
                    }
                    Carrier::Line { .. } => None,
                });
                (
                    k,
                    vec![
                        (&e["center"], true),
                        (&e["start"], false),
                        (&e["end"], false),
                    ],
                )
            }
            _ => continue,
        };
        let Some(k) = carrier else { continue };
        let mut points = Vec::new();
        for (p, center) in wants {
            let Some(p) = p.as_str() else { continue };
            asks.entry(p.to_owned()).or_default().push(if center {
                Ask::Center(k)
            } else {
                Ask::On(k)
            });
            points.push(p.to_owned());
        }
        curves.push((id.to_owned(), k, points));
    }
    // Each point where its curves' carriers put it; None where they do not
    // agree within the tolerance.
    let mut moved: BTreeMap<String, Option<[f64; 2]>> = BTreeMap::new();
    for (p, list) in &asks {
        let Some(old) = at.get(p).copied() else {
            moved.insert(p.clone(), None);
            continue;
        };
        let centers: Vec<usize> = list
            .iter()
            .filter_map(|a| match a {
                Ask::Center(k) => Some(*k),
                Ask::On(_) => None,
            })
            .collect();
        let mut on: Vec<usize> = Vec::new();
        for a in list {
            if let Ask::On(k) = a
                && !on.iter().any(|j| carriers[*j].same(&carriers[*k], 1e-12))
            {
                on.push(*k);
            }
        }
        let target = match (centers.first(), on.as_slice()) {
            (Some(&k), []) => match carriers[k] {
                Carrier::Circle { center, .. } => Some(center),
                Carrier::Line { .. } => None,
            },
            (Some(_), _) => None,
            (None, [k]) => Some(carriers[*k].nearest(old)),
            (None, [a, b, ..]) => carriers[*a].meet(&carriers[*b], old),
            (None, []) => None,
        };
        let target = target.filter(|t| {
            dist(*t, old) <= tol
                && centers.iter().all(|k| match carriers[*k] {
                    Carrier::Circle { center, .. } => dist(center, *t) <= 1e-9,
                    Carrier::Line { .. } => false,
                })
                && on.iter().all(|k| carriers[*k].distance(*t) <= 1e-9)
        });
        moved.insert(p.clone(), target);
    }
    // The curves all of whose points move; their points move with them.
    let mut snapped = Vec::new();
    let mut moving: BTreeSet<String> = BTreeSet::new();
    let mut radius: BTreeMap<String, f64> = BTreeMap::new();
    for (id, k, points) in &curves {
        if points
            .iter()
            .all(|p| moved.get(p).is_some_and(Option::is_some))
        {
            snapped.push(id.clone());
            moving.extend(points.iter().cloned());
            if let Carrier::Circle { radius: r, .. } = carriers[*k] {
                radius.insert(id.clone(), r);
            }
        }
    }
    let mut changed: BTreeMap<u32, [f64; 2]> = BTreeMap::new();
    for e in entities.iter_mut() {
        let Some(id) = e["id"].as_str().map(str::to_owned) else {
            continue;
        };
        if e["type"] == "point" && moving.contains(&id) {
            if let Some(Some(p)) = moved.get(&id) {
                e["at"] = json!(p);
                if let Some(n) = id.strip_prefix('p').and_then(|n| n.parse().ok()) {
                    changed.insert(n, *p);
                }
            }
        } else if e["type"] == "circle"
            && let Some(r) = radius.get(&id)
        {
            e["radius"] = json!(r);
        }
    }
    for (n, p) in positions.iter_mut() {
        if let Some(q) = changed.get(n) {
            *p = *q;
        }
    }
    snapped
}

/// The points of the curves `ids` among `entities` (their centres and
/// ends) with their positions, by entity id.
pub fn points_of(
    entities: &[Value],
    ids: &[String],
) -> std::collections::HashMap<String, [f64; 2]> {
    let at: BTreeMap<&str, [f64; 2]> = entities
        .iter()
        .filter(|e| e["type"] == "point")
        .filter_map(|e| {
            let p = e["at"].as_array()?;
            Some((
                e["id"].as_str()?,
                [p.first()?.as_f64()?, p.get(1)?.as_f64()?],
            ))
        })
        .collect();
    let mut out = std::collections::HashMap::new();
    for e in entities.iter().filter(|e| {
        e["id"]
            .as_str()
            .is_some_and(|id| ids.iter().any(|s| s == id))
    }) {
        for key in ["center", "start", "end"] {
            if let Some(p) = e[key].as_str()
                && let Some(q) = at.get(p)
            {
                out.insert(p.to_owned(), *q);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(id: &str, x: f64, y: f64) -> Value {
        json!({"id": id, "type": "point", "at": [x, y]})
    }

    fn at(entities: &[Value], id: &str) -> [f64; 2] {
        let e = entities.iter().find(|e| e["id"] == id).unwrap();
        [e["at"][0].as_f64().unwrap(), e["at"][1].as_f64().unwrap()]
    }

    /// Two half arcs of a circle of radius 85 stored 1.2e-6 mm too large
    /// (one linked, one drawn) and a linked line from the arcs' common end
    /// take the face's circle and line; a curve off the edges stays where it
    /// is.
    #[test]
    fn arcs_and_lines_take_the_face_edges() {
        let r = 85.0 + 1.2e-6;
        let mut entities = vec![
            point("p1", 0.0, 0.0),
            point("p2", r, 0.0),
            point("p3", -r, 1e-7),
            point("p4", r + 10.0, 2e-6),
            point("p5", 3.0, 4.0),
            json!({"id": "c1", "type": "arc", "center": "p1", "start": "p2", "end": "p3", "reference": true, "fixed": true}),
            json!({"id": "c2", "type": "arc", "center": "p1", "start": "p3", "end": "p2"}),
            json!({"id": "c3", "type": "line", "start": "p2", "end": "p4", "reference": true, "fixed": true}),
            json!({"id": "c4", "type": "circle", "center": "p5", "radius": 1.0}),
        ];
        let mut positions = vec![(2, [r, 0.0]), (5, [3.0, 4.0])];
        let carriers = [
            Carrier::Circle {
                center: [0.0, 0.0],
                radius: 85.0,
            },
            Carrier::Line {
                point: [0.0, 0.0],
                direction: [1.0, 0.0],
            },
        ];
        let snapped = snap_to_edges(&mut entities, &mut positions, &carriers, EDGE_TOLERANCE);
        assert_eq!(snapped, vec!["c1", "c2", "c3"]);
        // The arcs' common end where the circle meets the line.
        let p2 = at(&entities, "p2");
        assert!(dist(p2, [85.0, 0.0]) < 1e-9, "{p2:?}");
        assert!((dist(at(&entities, "p3"), [0.0, 0.0]) - 85.0).abs() < 1e-9);
        assert!(at(&entities, "p4")[1].abs() < 1e-12);
        assert_eq!(positions[0].1, p2);
        // Off the edges: as it was.
        assert_eq!(at(&entities, "p5"), [3.0, 4.0]);
        assert_eq!(positions[1].1, [3.0, 4.0]);
    }

    /// A circle farther off than the tolerance stays as stored; one within
    /// it takes the edge's radius.
    #[test]
    fn curves_off_the_edges_stay() {
        let mut entities = vec![
            point("p1", 0.0, 0.0),
            json!({"id": "c1", "type": "circle", "center": "p1", "radius": 10.001}),
        ];
        let carriers = [Carrier::Circle {
            center: [0.0, 0.0],
            radius: 10.0,
        }];
        assert!(snap_to_edges(&mut entities, &mut [], &carriers, EDGE_TOLERANCE).is_empty());
        assert_eq!(entities[1]["radius"], json!(10.001));

        let mut entities = vec![
            point("p1", 0.0, 0.0),
            json!({"id": "c1", "type": "circle", "center": "p1", "radius": 10.000001, "reference": true}),
        ];
        let snapped = snap_to_edges(&mut entities, &mut [], &carriers, EDGE_TOLERANCE);
        assert_eq!(snapped, vec!["c1"]);
        assert_eq!(entities[1]["radius"], json!(10.0));
    }
}
