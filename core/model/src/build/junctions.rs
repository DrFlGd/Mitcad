// SPDX-License-Identifier: MIT
//! Where profiles meet (`build_junctions`): from the derived parts' frames
//! and boxes at the timeline marker. Two profiles meet when their boxes
//! touch or overlap within the model's tolerance; then an end on a side
//! face is `T`, or `L` within the section's size of that side's end; two
//! ends at one point are `L` (axes at an angle) or `inline` (on one axis);
//! two sides crossing are `cross`.

use super::BuildRole;
use super::System;
use super::geometry::{
    V3, angle_between, dot, in_box, inverse, moved_box, norm, parts, scale, sub, z_axis,
};
use super::parts::PartInfo;
use crate::document::Document;
use crate::ids::{ComponentUid, OccurrenceUid};
use crate::kernel::{BoundingBox, Kernel};
use crate::transform::Transform;

/// Distances within this meet, mm.
const TOLERANCE: f64 = 1e-3;

/// One part of a junction.
#[derive(Debug, Clone, PartialEq)]
pub struct JunctionSide {
    pub component: ComponentUid,
    /// The path from the BUILD host.
    pub occurrence: Vec<OccurrenceUid>,
    /// `end_a`, `end_b`, a slot frame, or `side` (a closed side).
    pub at: String,
    /// That frame in the design's coordinates.
    pub frame: Transform,
}

/// Where two profiles meet.
#[derive(Debug, Clone, PartialEq)]
pub struct Junction {
    pub id: String,
    /// `L`, `T`, `cross` or `inline`.
    pub kind: &'static str,
    /// The part whose end meets (for `T` and `L`), and the other.
    pub a: JunctionSide,
    pub b: JunctionSide,
    /// Between the axes, radians.
    pub angle: f64,
    pub systems: [Option<System>; 2],
    /// The free room in the inner corner: along a, along b and across.
    pub space: [f64; 3],
    /// Connectors already there.
    pub connectors: Vec<ComponentUid>,
    /// b's axis from the junction into b (design coordinates).
    pub b_axis: V3,
}

/// A profile as the search sees it.
struct Placed {
    info: PartInfo,
    path: Vec<OccurrenceUid>,
    world: Transform,
    bounds: BoundingBox,
    world_box: BoundingBox,
    length: f64,
}

impl Placed {
    fn frame(&self, name: &str) -> Option<Transform> {
        Some(self.world.after(self.info.mount(name)?))
    }

    /// The axis from end A to end B in the design.
    fn axis(&self) -> V3 {
        self.world.apply_vector([0.0, 0.0, 1.0])
    }

    fn side(&self, at: String, frame: Transform) -> JunctionSide {
        JunctionSide {
            component: self.info.component,
            occurrence: self.path.clone(),
            at,
            frame,
        }
    }

    /// The section's size across the axis.
    fn section(&self) -> f64 {
        let b = &self.bounds;
        (b.max[0] - b.min[0]).max(b.max[1] - b.min[1])
    }
}

impl<K: Kernel> Document<K> {
    /// The junctions of the design's profiles (of `only` when given).
    pub fn build_junctions(&self, only: Option<&[ComponentUid]>) -> Vec<Junction> {
        let active = self.host_placement();
        let placed: Vec<Placed> = self
            .build_parts_info()
            .into_iter()
            .filter(|i| i.meta.role == BuildRole::Profile || i.is_cut_to_length())
            .filter(|i| i.meta.role != BuildRole::Panel)
            .filter_map(|info| {
                let path = self.placed_in_host(info.component)?;
                let world = active.after(&self.path_transform(&path));
                let bounds = self.part_bounds(info.component)?;
                let length = info
                    .length
                    .map_or(bounds.max[2] - bounds.min[2], |(_, l)| l);
                Some(Placed {
                    world_box: moved_box(&bounds, &world),
                    info,
                    path,
                    world,
                    bounds,
                    length,
                })
            })
            .collect();
        let connectors: Vec<(ComponentUid, String)> = self
            .assembly()
            .components
            .iter()
            .filter_map(|c| Some((c.uid, c.build.as_ref()?.junction.clone()?)))
            .collect();
        let mut out = Vec::new();
        for (i, p) in placed.iter().enumerate() {
            for q in &placed[i + 1..] {
                if let Some(only) = only
                    && !only.contains(&p.info.component)
                    && !only.contains(&q.info.component)
                {
                    continue;
                }
                if !p.world_box.overlaps(&q.world_box, TOLERANCE) {
                    continue;
                }
                // The newer part is a when both could be.
                let (p, q) = if p.info.component > q.info.component {
                    (p, q)
                } else {
                    (q, p)
                };
                if let Some(mut junction) = meet(p, q).or_else(|| meet(q, p)) {
                    junction.connectors = connectors
                        .iter()
                        .filter(|(_, id)| *id == junction.id)
                        .map(|(c, _)| *c)
                        .collect();
                    out.push(junction);
                }
            }
        }
        out
    }
}

/// How `p` meets `q`: an end of p on q's end or side, or crossing.
fn meet(p: &Placed, q: &Placed) -> Option<Junction> {
    let ends = ["end_a", "end_b"];
    // Two ends at one point.
    for e in ends {
        let Some(pe) = p.frame(e) else { continue };
        for f in ends {
            let Some(qf) = q.frame(f) else { continue };
            if norm(sub(pe.translation, qf.translation)) <= TOLERANCE {
                let angle = angle_between(scale(z_axis(&pe), -1.0), z_axis(&qf));
                let kind = if dot(z_axis(&pe), z_axis(&qf)) < -1.0 + 1e-6 {
                    "inline"
                } else {
                    "L"
                };
                return Some(junction(
                    kind,
                    p.side(e.to_owned(), pe),
                    q.side(f.to_owned(), qf),
                    angle,
                    p,
                    q,
                    scale(z_axis(&qf), -1.0),
                ));
            }
        }
    }
    // An end of p on a side of q.
    let to_q = inverse(&q.world);
    for e in ends {
        let Some(pe) = p.frame(e) else { continue };
        let point = to_q.apply_point(pe.translation);
        let out_of_p = to_q.apply_vector(z_axis(&pe));
        if !in_box(point, &q.bounds, TOLERANCE) {
            continue;
        }
        // The side face it stands on: the slot frame whose plane holds the
        // point and faces it, else a side of the box.
        let mut at = None;
        for (name, slot) in q.info.slots() {
            let [o, _, _, z] = parts(slot);
            if dot(sub(point, o), z).abs() <= TOLERANCE && dot(out_of_p, z) < -1.0 + 1e-6 {
                at = Some((name.clone(), q.world.after(slot)));
                break;
            }
        }
        let at = match at {
            Some(at) => at,
            None => {
                let b = &q.bounds;
                let on_side = (0..2).any(|k| {
                    (point[k] - b.min[k]).abs() <= TOLERANCE
                        || (point[k] - b.max[k]).abs() <= TOLERANCE
                });
                if !on_side || out_of_p[2].abs() > 1e-6 {
                    continue;
                }
                (
                    "side".to_owned(),
                    q.world.after(&Transform::translation(point)),
                )
            }
        };
        // Within the section's size of q's end: a corner.
        let from_end = point[2].min(q.length - point[2]);
        let kind = if from_end <= q.section() + TOLERANCE {
            "L"
        } else {
            "T"
        };
        let angle = angle_between(scale(z_axis(&pe), -1.0), q.axis());
        let into_q = q.axis();
        let mut j = junction(
            kind,
            p.side(e.to_owned(), pe),
            q.side(at.0, at.1),
            angle,
            p,
            q,
            into_q,
        );
        // Along b: the room beside a, to b's farther end.
        j.space[1] = point[2].max(q.length - point[2]);
        return Some(j);
    }
    // Two sides crossing.
    if p.info.component > q.info.component {
        let pa = p.axis();
        let qa = q.axis();
        if dot(pa, qa).abs() < 1.0 - 1e-6 {
            let towards = |from: &Placed, to: &Placed| -> (String, Transform) {
                let centre = |b: &BoundingBox| -> V3 {
                    std::array::from_fn(|i| (b.min[i] + b.max[i]) / 2.0)
                };
                let target = centre(&to.world_box);
                from.info
                    .slots()
                    .into_iter()
                    .map(|(n, t)| (n.clone(), from.world.after(t)))
                    .max_by(|x, y| {
                        let d = |t: &Transform| dot(z_axis(t), sub(target, t.translation));
                        d(&x.1).total_cmp(&d(&y.1))
                    })
                    .unwrap_or_else(|| ("side".to_owned(), from.world))
            };
            let (pa_name, pa_frame) = towards(p, q);
            let (qa_name, qa_frame) = towards(q, p);
            return Some(junction(
                "cross",
                p.side(pa_name, pa_frame),
                q.side(qa_name, qa_frame),
                angle_between(pa, qa),
                p,
                q,
                qa,
            ));
        }
    }
    None
}

fn junction(
    kind: &'static str,
    a: JunctionSide,
    b: JunctionSide,
    angle: f64,
    p: &Placed,
    q: &Placed,
    b_axis: V3,
) -> Junction {
    let id = format!("{}:{}|{}:{}", a.component, a.at, b.component, b.at);
    Junction {
        id,
        kind,
        a,
        b,
        angle,
        systems: [p.info.meta.system.clone(), q.info.meta.system.clone()],
        space: [p.length, q.length, p.section().min(q.section())],
        connectors: Vec::new(),
        b_axis,
    }
}
