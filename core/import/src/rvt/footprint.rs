// SPDX-License-Identifier: MIT
//! Plan geometry of the building elements (mitcad#275), in model feet: a
//! wall's footprint with its ends where its joins put them, a slab's
//! boundary loops as profile areas, a type's cut loops at an insert. The
//! profiles come out in millimetres as the IFC import's geometry
//! ([`mitcad_ifc::geometry`]), which its feature builder sketches.
//!
//! **Wall joins** (verified on the MIT project's walls against their IFC
//! export): at an end where walls meet end to end, the first wall the file
//! lists there runs on to the far face of the others, and the others stop
//! at its near face; a wall whose end meets another wall's side stops at
//! that side's near face. Faces are the wall's location line offset by half
//! its type's width to either side. A miter join (join kind 1) cuts both
//! walls on the line through the meeting points of their faces.

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use mitcad_ifc::geometry::{Area, Path, Profile, Seg};
use mitcad_ifc::math::{self, Matrix};
use mitcad_rvt::building::{Building, Curve, JoinedEnd, P3, Wall};
use mitcad_rvt::elements::FOOT_MM;

pub type P2 = [f64; 2];

fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn mul(a: P2, s: f64) -> P2 {
    [a[0] * s, a[1] * s]
}

fn dot(a: P2, b: P2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn cross(a: P2, b: P2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn unit(a: P2) -> Option<P2> {
    let l = a[0].hypot(a[1]);
    (l > 1e-12).then(|| [a[0] / l, a[1] / l])
}

fn xy(p: P3) -> P2 {
    [p[0], p[1]]
}

/// A line: a point and a unit direction.
#[derive(Debug, Clone, Copy)]
struct Line {
    p: P2,
    d: P2,
}

impl Line {
    fn offset(&self, by: f64) -> Line {
        Line {
            p: add(self.p, mul([-self.d[1], self.d[0]], by)),
            d: self.d,
        }
    }

    /// Where two lines meet (None when parallel).
    fn meet(&self, other: &Line) -> Option<P2> {
        let den = cross(self.d, other.d);
        if den.abs() < 1e-9 {
            return None;
        }
        let t = cross(sub(other.p, self.p), other.d) / den;
        Some(add(self.p, mul(self.d, t)))
    }
}

/// A wall's half width (feet).
fn half_width(building: &Building, wall: &Wall) -> Option<f64> {
    wall.type_id
        .and_then(|t| building.width(t))
        .map(|w| w / 2.0)
}

/// Where a wall's faces lie: the offsets (feet, to the left of its
/// location line) of its right and left faces: its reference faces' when
/// they span its type's width (a location line off the wall's centre),
/// else half its width to either side.
pub fn sides(building: &Building, wall: &Wall) -> Option<[f64; 2]> {
    let h = half_width(building, wall)?;
    Some(match wall.faces {
        Some([lo, hi]) if ((hi - lo) - 2.0 * h).abs() < 1e-4 => [lo, hi],
        _ => [-h, h],
    })
}

/// The line of a wall's location curve where it passes `near` (a line
/// wall's own line; an arc wall's tangent at its nearer end).
fn wall_line(wall: &Wall, near: P2) -> Option<Line> {
    match &wall.curve {
        Curve::Line { start, end } => Some(Line {
            p: xy(*start),
            d: unit(sub(xy(*end), xy(*start)))?,
        }),
        arc @ (Curve::Arc { .. } | Curve::Ellipse { .. } | Curve::Helix { .. }) => {
            let (s, e) = (xy(arc.start()), xy(arc.end()));
            let f = if dot(sub(near, s), sub(near, s)) <= dot(sub(near, e), sub(near, e)) {
                0.0
            } else {
                1.0
            };
            let a = xy(arc.at((f - 1e-6f64).clamp(0.0, 1.0)));
            let b = xy(arc.at((f + 1e-6f64).clamp(0.0, 1.0)));
            Some(Line {
                p: xy(arc.at(f)),
                d: unit(sub(b, a))?,
            })
        }
    }
}

/// The faces (two lines) of a joined wall near a point.
fn faces(building: &Building, element: i64, near: P2) -> Result<[Line; 2], String> {
    let wall = building
        .walls
        .get(&element)
        .ok_or_else(|| format!("its joined element {element} is no wall read"))?;
    let [lo, hi] = sides(building, wall)
        .ok_or_else(|| format!("the joined wall {element} has no type width"))?;
    let line =
        wall_line(wall, near).ok_or_else(|| format!("the joined wall {element} has no length"))?;
    Ok([line.offset(hi), line.offset(lo)])
}

/// The cut line at one end of a straight wall, and what it is.
fn end_cut(
    building: &Building,
    wall: &Wall,
    end: usize,
    center: &Line,
    s: [f64; 2],
    point: P2,
    out: P2,
) -> Result<(Line, &'static str), String> {
    let square = Line {
        p: point,
        d: [-out[1], out[0]],
    };
    if wall.no_join[end] {
        return Ok((square, "free"));
    }
    // Joins with what is no layered wall (room separation lines, curtain
    // walls), or with a wall above or below this one (a storey's wall on
    // the one under it, joined at the same corner), shape nothing.
    let heights = |w: &Wall| Some((w.base?, w.base? + w.height?));
    let overlaps = |w: &Wall| match (heights(wall), heights(w)) {
        (Some((a0, a1)), Some((b0, b1))) => a0.max(b0) < a1.min(b1) - 1e-3,
        _ => true,
    };
    let list: Vec<JoinedEnd> = wall.joins[end]
        .iter()
        .filter(|j| {
            j.element == wall.id
                || building
                    .walls
                    .get(&j.element)
                    .is_some_and(|w| half_width(building, w).is_some() && overlaps(w))
        })
        .copied()
        .collect();
    let others: Vec<&JoinedEnd> = list.iter().filter(|j| j.element != wall.id).collect();
    if others.is_empty() {
        return Ok((square, "free"));
    }
    // Where a face line crosses the location line, along `out` from the
    // end point.
    let along = |face: &Line| -> Option<(f64, Line)> {
        let at = center.meet(face)?;
        Some((dot(sub(at, point), out), *face))
    };
    // A side join: stop at that wall's near face.
    if let Some(side) = others.iter().find(|j| j.end >= 2) {
        let f = faces(building, side.element, point)?;
        let mut hits: Vec<(f64, Line)> = f.iter().filter_map(along).collect();
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        // A wall running on in line ends at the joint.
        return Ok(hits
            .first()
            .map_or((square, "in line"), |near| (near.1, "side")));
    }
    if wall.join_types[end] == 1 && others.len() == 1 {
        // A miter: the line through the meetings of the faces.
        let f = faces(building, others[0].element, point)?;
        let (left, right) = (center.offset(s[1]), center.offset(s[0]));
        let mut best: Option<(f64, Line)> = None;
        for (a, b) in [(0, 1), (1, 0)] {
            let (Some(p), Some(q)) = (left.meet(&f[a]), right.meet(&f[b])) else {
                continue;
            };
            let Some(d) = unit(sub(p, q)) else { continue };
            let line = Line { p: q, d };
            // The miter line passes through the walls' meeting point.
            let miss = cross(sub(point, q), d).abs();
            if best.as_ref().is_none_or(|(m, _)| miss < *m) {
                best = Some((miss, line));
            }
        }
        return best
            .map(|(_, l)| (l, "miter"))
            .ok_or_else(|| "a miter of parallel walls".to_string());
    }
    if list.first().is_some_and(|j| j.element == wall.id) {
        // The first wall runs to the far face of the second (and stays at
        // the joint when the second runs on in line with it).
        let second = others[0];
        let f = faces(building, second.element, point)?;
        let far = f
            .iter()
            .filter_map(along)
            .max_by(|a, b| a.0.total_cmp(&b.0));
        Ok(far.map_or((square, "in line"), |far| (far.1, "extends")))
    } else {
        // The others stop at the first wall's near face; one in line with
        // the first stops where the first ends.
        let first = *list.first().ok_or("an empty join")?;
        let f = faces(building, first.element, point)?;
        if let Some(near) = f
            .iter()
            .filter_map(along)
            .min_by(|a, b| a.0.total_cmp(&b.0))
        {
            return Ok((near.1, "stops"));
        }
        let first_wall = &building.walls[&first.element];
        if first.end < 2
            && first_wall.joins[usize::from(first.end)]
                .first()
                .map(|j| j.element)
                == Some(first.element)
        {
            let (cut, _) = wall_end_cut(building, first_wall, usize::from(first.end))?;
            return Ok((cut, "stops at the first wall's end"));
        }
        Ok((square, "in line"))
    }
}

/// The cut line at one end of a straight wall.
fn wall_end_cut(
    building: &Building,
    wall: &Wall,
    end: usize,
) -> Result<(Line, &'static str), String> {
    let s = sides(building, wall).ok_or("its type has no layers with a width")?;
    let Curve::Line { start, end: stop } = &wall.curve else {
        return Err("an arc wall".into());
    };
    let (p0, p1) = (xy(*start), xy(*stop));
    let d = unit(sub(p1, p0)).ok_or("its location line has no length")?;
    let center = Line { p: p0, d };
    let (point, out) = if end == 0 {
        (p0, mul(d, -1.0))
    } else {
        (p1, d)
    };
    end_cut(building, wall, end, &center, s, point, out)
}

/// A wall's footprint (feet, the plane of its base) and what its ends do.
pub fn wall_footprint(building: &Building, wall: &Wall) -> Result<(Path, Vec<String>), String> {
    let s = sides(building, wall).ok_or("its type has no layers with a width")?;
    let h = (s[1] - s[0]) / 2.0;
    let mut notes = Vec::new();
    match &wall.curve {
        Curve::Line { start, end } => {
            let (p0, p1) = (xy(*start), xy(*end));
            let d = unit(sub(p1, p0)).ok_or("its location line has no length")?;
            let center = Line { p: p0, d };
            let (left, right) = (center.offset(s[1]), center.offset(s[0]));
            let mut ends = Vec::new();
            for (e, point, out) in [(0, p0, mul(d, -1.0)), (1, p1, d)] {
                let (cut, what) = end_cut(building, wall, e, &center, s, point, out)?;
                if what != "free" {
                    notes.push(format!(
                        "its {} {what}",
                        if e == 0 { "start" } else { "end" }
                    ));
                }
                let l = left.meet(&cut).ok_or("an end cut along the wall")?;
                let r = right.meet(&cut).ok_or("an end cut along the wall")?;
                ends.push((l, r));
            }
            let ((l0, r0), (l1, r1)) = (ends[0], ends[1]);
            // The cuts must not cross inside the wall.
            if dot(sub(r1, r0), d) <= 1e-6 || dot(sub(l1, l0), d) <= 1e-6 {
                return Err("its end cuts cross".into());
            }
            Ok((polygon(&[r0, r1, l1, l0]), notes))
        }
        arc @ Curve::Arc {
            center,
            x_axis,
            y_axis,
            radius,
            start,
            end,
        } => {
            if wall
                .joins
                .iter()
                .any(|j| j.iter().any(|e| e.element != wall.id))
            {
                notes.push("an arc wall: its ends stay square to its curve at its joins".into());
            }
            let normal = math::cross(*x_axis, *y_axis);
            if normal[2].abs() < 1.0 - 1e-6 || *radius <= h {
                return Err("an arc wall that is not level".into());
            }
            let (r_out, r_in) = (radius + h, radius - h);
            let frame = math::from_axes([center[0], center[1], 0.0], *x_axis, *y_axis, normal);
            let sweep = end - start;
            let at = |r: f64, t: f64| -> [f64; 3] {
                let p = add(
                    xy(*center),
                    add(mul(xy(*x_axis), r * t.cos()), mul(xy(*y_axis), r * t.sin())),
                );
                [p[0], p[1], 0.0]
            };
            let _ = arc;
            let segments = vec![
                Seg::Arc {
                    frame,
                    radius: r_out,
                    start: *start,
                    sweep,
                },
                Seg::Line {
                    start: at(r_out, *end),
                    end: at(r_in, *end),
                },
                Seg::Arc {
                    frame,
                    radius: r_in,
                    start: *end,
                    sweep: -sweep,
                },
                Seg::Line {
                    start: at(r_in, *start),
                    end: at(r_out, *start),
                },
            ];
            Ok((Path { segments }, notes))
        }
        Curve::Ellipse { .. } => Err("an elliptical wall".into()),
        Curve::Helix { .. } => Err("a helical wall".into()),
    }
}

/// A closed polygon's path (z = 0).
pub fn polygon(points: &[P2]) -> Path {
    let n = points.len();
    Path {
        segments: (0..n)
            .map(|i| Seg::Line {
                start: [points[i][0], points[i][1], 0.0],
                end: [points[(i + 1) % n][0], points[(i + 1) % n][1], 0.0],
            })
            .collect(),
    }
}

/// A curve as a piece of a path in the xy plane (its z dropped).
pub fn seg(curve: &Curve) -> Seg {
    match curve {
        Curve::Line { start, end } => Seg::Line {
            start: [start[0], start[1], 0.0],
            end: [end[0], end[1], 0.0],
        },
        Curve::Arc {
            center,
            x_axis,
            y_axis,
            radius,
            start,
            end,
        } => Seg::Arc {
            frame: math::from_axes(
                [center[0], center[1], 0.0],
                *x_axis,
                *y_axis,
                math::cross(*x_axis, *y_axis),
            ),
            radius: *radius,
            start: *start,
            sweep: end - start,
        },
        Curve::Ellipse {
            center,
            x_axis,
            y_axis,
            rx,
            ry,
            start,
            end,
        } => Seg::Ellipse {
            frame: math::from_axes(
                [center[0], center[1], 0.0],
                *x_axis,
                *y_axis,
                math::cross(*x_axis, *y_axis),
            ),
            a: *rx,
            b: *ry,
            start: *start,
            sweep: end - start,
        },
        // (A helix in plan: its circle seen along its axis.)
        Curve::Helix {
            center,
            x_axis,
            y_axis,
            radius,
            start,
            end,
            ..
        } => Seg::Arc {
            frame: math::from_axes(
                [center[0], center[1], 0.0],
                *x_axis,
                *y_axis,
                math::cross(*x_axis, *y_axis),
            ),
            radius: *radius,
            start: *start,
            sweep: end - start,
        },
    }
}

/// A loop of curves joined end to start (each turned as needed), as a
/// path in the xy plane; None when its curves do not close.
pub fn loop_path(curves: &[Curve], tolerance: f64) -> Option<Path> {
    let mut segs: Vec<Seg> = Vec::new();
    let mut left: Vec<Seg> = curves.iter().map(seg).collect();
    if left.is_empty() {
        return None;
    }
    segs.push(left.remove(0));
    let close = |a: [f64; 3], b: [f64; 3]| (a[0] - b[0]).hypot(a[1] - b[1]) <= tolerance;
    while !left.is_empty() {
        let at = segs.last().expect("one").end();
        let i = left
            .iter()
            .position(|s| close(s.start(), at) || close(s.end(), at))?;
        let s = left.remove(i);
        segs.push(if close(s.start(), at) {
            s
        } else {
            s.reversed()
        });
    }
    let first = segs[0].start();
    close(segs.last().expect("one").end(), first).then_some(Path { segments: segs })
}

/// A path's signed area (counter-clockwise positive), from its sampled
/// points.
pub fn signed_area(path: &Path) -> f64 {
    let pts: Vec<[f64; 3]> = path.segments.iter().flat_map(|s| s.sample(16)).collect();
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

fn inside(path: &Path, p: [f64; 3]) -> bool {
    let pts: Vec<[f64; 3]> = path.segments.iter().flat_map(|s| s.sample(16)).collect();
    let mut c = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                c = !c;
            }
        }
    }
    c
}

/// Whether a point (its x and y) lies in a closed path, or within
/// `tolerance` of it (mitcad#348: a wall under a roof's edge).
pub fn covers(path: &Path, p: P2, tolerance: f64) -> bool {
    let q = [p[0], p[1], 0.0];
    if inside(path, q) {
        return true;
    }
    let pts: Vec<[f64; 3]> = path.segments.iter().flat_map(|s| s.sample(16)).collect();
    let n = pts.len();
    (0..n).any(|i| {
        let (a, b) = (xy(pts[i]), xy(pts[(i + 1) % n]));
        let ab = sub(b, a);
        let l = dot(ab, ab);
        let t = if l > 1e-18 {
            (dot(sub(p, a), ab) / l).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = sub(p, add(a, mul(ab, t)));
        dot(d, d).sqrt() <= tolerance
    })
}

/// Loops as profile areas: each loop inside an odd number of others is a
/// hole of the smallest that holds it; outer loops counter-clockwise,
/// holes clockwise.
pub fn areas(loops: Vec<Path>) -> Vec<Area> {
    let n = loops.len();
    let areas: Vec<f64> = loops.iter().map(signed_area).collect();
    let probe = |p: &Path| {
        p.segments
            .first()
            .map(|s| s.sample(2)[1])
            .unwrap_or([0.0; 3])
    };
    // The loops holding each loop.
    let holders: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| {
                    j != i && areas[j].abs() > areas[i].abs() && inside(&loops[j], probe(&loops[i]))
                })
                .collect()
        })
        .collect();
    let orient = |p: &Path, a: f64, ccw: bool| {
        if (a > 0.0) == ccw {
            p.clone()
        } else {
            p.reversed()
        }
    };
    let mut out: Vec<(usize, Area)> = Vec::new();
    for i in 0..n {
        if holders[i].len().is_multiple_of(2) {
            out.push((
                i,
                Area {
                    outer: orient(&loops[i], areas[i], true),
                    holes: Vec::new(),
                },
            ));
        }
    }
    for i in 0..n {
        if !holders[i].len().is_multiple_of(2)
            && let Some(&owner) = holders[i]
                .iter()
                .filter(|j| holders[**j].len().is_multiple_of(2))
                .min_by(|a, b| areas[**a].abs().total_cmp(&areas[**b].abs()))
            && let Some((_, area)) = out.iter_mut().find(|(k, _)| *k == owner)
        {
            area.holes.push(orient(&loops[i], areas[i], false));
        }
    }
    out.into_iter().map(|(_, a)| a).collect()
}

/// Feet to millimetres, for a profile in the xy plane.
pub fn to_mm(path: &Path) -> Path {
    path.transformed(&scale_mm())
}

fn scale_mm() -> Matrix {
    [
        [FOOT_MM, 0.0, 0.0, 0.0],
        [0.0, FOOT_MM, 0.0, 0.0],
        [0.0, 0.0, FOOT_MM, 0.0],
    ]
}

/// A profile of areas (feet) in millimetres.
pub fn profile_mm(areas: Vec<Area>) -> Profile {
    Profile {
        kind: String::new(),
        name: String::new(),
        areas: areas
            .into_iter()
            .map(|a| Area {
                outer: to_mm(&a.outer),
                holes: a.holes.iter().map(to_mm).collect(),
            })
            .collect(),
    }
}

/// A circle of a radius about the origin (a whole turn).
pub fn circle(radius: f64) -> Path {
    Path {
        segments: vec![Seg::Arc {
            frame: math::IDENTITY,
            radius,
            start: 0.0,
            sweep: TAU,
        }],
    }
}

/// A lookup of walls for the cuts of other elements.
pub type Walls<'a> = &'a BTreeMap<i64, Wall>;

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_rvt::building::Layer;

    fn wall(id: i64, start: P2, end: P2, joins: [Vec<JoinedEnd>; 2]) -> Wall {
        Wall {
            id,
            class: "SWall".into(),
            type_id: Some(id * 100),
            curve: Curve::Line {
                start: [start[0], start[1], 0.0],
                end: [end[0], end[1], 0.0],
            },
            base: Some(0.0),
            height: Some(10.0),
            top_level: None,
            joins,
            join_types: [0, 0],
            no_join: [false, false],
            side_joins: Vec::new(),
            flipped: false,
            key_ref: 0,
            key_offset: 0.0,
            steps: Vec::new(),
            cut_by: Vec::new(),
            cutouts: Vec::new(),
            profile: Vec::new(),
            faces: None,
            attachments: Vec::new(),
        }
    }

    fn j(element: i64, end: u8) -> JoinedEnd {
        JoinedEnd { element, end }
    }

    fn bounds(path: &Path) -> [f64; 4] {
        let mut b = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for s in &path.segments {
            let p = s.start();
            b = [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ];
        }
        b
    }

    #[test]
    fn corner_joins_extend_the_first_wall() {
        // An L corner at (0, 0): wall 1 along x (width 2/3), listed first,
        // wall 2 along y (width 1/2); a third wall's end on wall 1's side.
        let mut b = Building::default();
        let layer = |w| {
            vec![Layer {
                width: w,
                material: None,
                function: 1,
            }]
        };
        b.structures.insert(100, layer(2.0 / 3.0));
        b.structures.insert(200, layer(0.5));
        b.structures.insert(300, layer(0.5));
        let corner = vec![j(1, 0), j(2, 0)];
        b.walls.insert(
            1,
            wall(1, [0.0, 0.0], [27.0, 0.0], [corner.clone(), vec![]]),
        );
        b.walls
            .insert(2, wall(2, [0.0, 0.0], [0.0, 23.0], [corner, vec![]]));
        b.walls.insert(
            3,
            wall(
                3,
                [10.0, 10.0],
                [10.0, 0.0],
                [vec![], vec![j(1, 3), j(3, 1)]],
            ),
        );
        let (p1, notes) = wall_footprint(&b, &b.walls[&1]).unwrap();
        assert_eq!(notes, vec!["its start extends"]);
        let x = bounds(&p1);
        assert!(
            (x[0] + 0.25).abs() < 1e-9 && (x[2] - 27.0).abs() < 1e-9,
            "{x:?}"
        );
        let (p2, _) = wall_footprint(&b, &b.walls[&2]).unwrap();
        let y = bounds(&p2);
        assert!((y[1] - 1.0 / 3.0).abs() < 1e-9, "{y:?}");
        // Wall 3 stops at wall 1's near face (y = 1/3).
        let (p3, notes) = wall_footprint(&b, &b.walls[&3]).unwrap();
        assert_eq!(notes, vec!["its end side"]);
        assert!((bounds(&p3)[1] - 1.0 / 3.0).abs() < 1e-9);
        assert!((signed_area(&p1) - 27.25 * 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn loops_become_areas_with_holes() {
        let square = |s: f64, o: f64| polygon(&[[o, o], [o + s, o], [o + s, o + s], [o, o + s]]);
        let a = areas(vec![square(2.0, 4.0), square(10.0, 0.0).reversed()]);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].holes.len(), 1);
        assert!(signed_area(&a[0].outer) > 0.0 && signed_area(&a[0].holes[0]) < 0.0);
        let curves = vec![
            Curve::Line {
                start: [0.0, 0.0, 0.0],
                end: [1.0, 0.0, 0.0],
            },
            Curve::Line {
                start: [0.0, 1.0, 0.0],
                end: [0.0, 0.0, 0.0],
            },
            Curve::Line {
                start: [1.0, 1.0, 0.0],
                end: [1.0, 0.0, 0.0],
            },
            Curve::Line {
                start: [1.0, 1.0, 0.0],
                end: [0.0, 1.0, 0.0],
            },
        ];
        let path = loop_path(&curves, 1e-6).unwrap();
        assert!((signed_area(&path).abs() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_wall_above_at_the_corner_shapes_no_end() {
        // A corner of two foundation walls (0 to 3 high), listed with the
        // two walls of the storey on them (3 to 13): those first in the
        // list, but they do not reach down to the foundation.
        let mut b = Building::default();
        let layer = |w| {
            vec![Layer {
                width: w,
                material: None,
                function: 1,
            }]
        };
        for t in [100, 200, 300, 400] {
            b.structures.insert(t, layer(0.5));
        }
        let corner = vec![j(3, 0), j(1, 0), j(2, 0), j(4, 0)];
        let low = |mut w: Wall| {
            w.height = Some(3.0);
            w
        };
        let high = |mut w: Wall| {
            w.base = Some(3.0);
            w
        };
        b.walls.insert(
            1,
            low(wall(1, [0.0, 0.0], [20.0, 0.0], [corner.clone(), vec![]])),
        );
        b.walls.insert(
            2,
            low(wall(2, [0.0, 0.0], [0.0, 20.0], [corner.clone(), vec![]])),
        );
        b.walls.insert(
            3,
            high(wall(3, [0.0, 0.0], [20.0, 0.0], [corner.clone(), vec![]])),
        );
        b.walls
            .insert(4, high(wall(4, [0.0, 0.0], [0.0, 20.0], [corner, vec![]])));
        // Of the foundation walls, wall 1 comes first: it runs on to the
        // far face of wall 2, which stops at its near face.
        let (p1, notes) = wall_footprint(&b, &b.walls[&1]).unwrap();
        assert_eq!(notes, vec!["its start extends"]);
        assert!((bounds(&p1)[0] + 0.25).abs() < 1e-9, "{:?}", bounds(&p1));
        let (p2, notes) = wall_footprint(&b, &b.walls[&2]).unwrap();
        assert_eq!(notes, vec!["its start stops"]);
        assert!((bounds(&p2)[1] - 0.25).abs() < 1e-9, "{:?}", bounds(&p2));
    }
}
