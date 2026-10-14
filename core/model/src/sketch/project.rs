// SPDX-License-Identifier: MIT
//! Projection of model geometry into a sketch (Project): the
//! kernel lists the curves of an edge, a face's boundary or a vertex
//! ([`crate::Kernel::curves_of`]); they are projected along the sketch
//! normal onto the plane and become fixed reference entities. Lines stay
//! lines, circles in parallel planes stay circles, other circles and
//! ellipses become ellipses (or lines seen edge on), B-splines keep their
//! knots with projected poles. Ends that meet share a point; a curve or
//! point the same as another of the source is projected once. A linked
//! projection is recorded ([`Projection`]) and its entities follow the
//! source at every evaluation, each the fresh curve of its kind most like
//! it (mitcad#232); an entity that finds none, and the entities of a lost
//! source, keep their last geometry, and the sketch warns about them. A
//! pairing that would tear an arc from the curves it shares points with
//! (the source changed shape) moves nothing: the whole projection keeps
//! its last geometry and warns (mitcad#249). Projections of earlier
//! versions and imports (no [`Projection::by_geometry`]) follow only a
//! source whose curves come back as they were listed, as there.

use std::collections::BTreeMap;

use super::geometry::{Curve2, P2, add, angle_after, dist, norm, scale, sub};
use super::solve::{SolveError, Solved};
use super::{Entity, EntityKind, Projection, Ref};
use crate::features::EvalContext;
use crate::features::sketch::SketchDef;
use crate::ids::EntityUid;
use crate::kernel::{Curve3, Kernel};
use crate::parameters::ParamId;
use crate::profile::{SketchFrame, cross, dot};
use crate::transform::Transform;

/// A projected primitive in sketch coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum Projected {
    Point(P2),
    Curve(Curve2),
}

fn to_sketch(frame: &SketchFrame, p: [f64; 3]) -> P2 {
    let d: [f64; 3] = std::array::from_fn(|i| p[i] - frame.origin[i]);
    [dot(d, frame.x_axis), dot(d, frame.y_axis)]
}

fn direction(frame: &SketchFrame, v: [f64; 3]) -> P2 {
    [dot(v, frame.x_axis), dot(v, frame.y_axis)]
}

/// The principal axes of the ellipse `M (cos t, sin t)` for `M = [a b]`:
/// semi-axes and the major axis direction.
fn principal_axes(a: P2, b: P2) -> (f64, f64, f64) {
    // M M^T = [[p, q], [q, r]].
    let p = a[0] * a[0] + b[0] * b[0];
    let q = a[0] * a[1] + b[0] * b[1];
    let r = a[1] * a[1] + b[1] * b[1];
    let mean = (p + r) / 2.0;
    let spread = (((p - r) / 2.0).powi(2) + q * q).sqrt();
    let major = (mean + spread).max(0.0).sqrt();
    let minor = (mean - spread).max(0.0).sqrt();
    let rotation = 0.5 * (2.0 * q).atan2(p - r);
    (major, minor, rotation)
}

/// Projects model curves onto the sketch plane.
pub fn project(curves: &[Curve3], frame: &SketchFrame) -> Vec<Projected> {
    let mut out = Vec::new();
    for curve in curves {
        match curve {
            Curve3::Point(p) => out.push(Projected::Point(to_sketch(frame, *p))),
            Curve3::Line { start, end } => {
                let (a, b) = (to_sketch(frame, *start), to_sketch(frame, *end));
                if dist(a, b) <= 1e-9 * (1.0 + norm(a)) {
                    out.push(Projected::Point(a));
                } else {
                    out.push(Projected::Curve(Curve2::Line { a, b }));
                }
            }
            Curve3::Conic {
                center,
                normal,
                x_axis,
                major,
                minor,
                start,
                end,
                closed,
            } => {
                let y_axis = cross(*normal, *x_axis);
                let c = to_sketch(frame, *center);
                let ax = scale(direction(frame, *x_axis), *major);
                let ay = scale(direction(frame, y_axis), *minor);
                let at = |t: f64| add(c, add(scale(ax, t.cos()), scale(ay, t.sin())));
                let (big, small, rotation) = principal_axes(ax, ay);
                let size = big.max(1e-300);
                if small <= 1e-9 * size {
                    // Seen edge on: the extreme points of the curve.
                    let ts: Vec<f64> = (0..=64)
                        .map(|i| start + (end - start) * i as f64 / 64.0)
                        .collect();
                    let u = [rotation.cos(), rotation.sin()];
                    let along = |t: &f64| dot2(sub(at(*t), c), u);
                    let lo = ts
                        .iter()
                        .copied()
                        .min_by(|a, b| along(a).total_cmp(&along(b)));
                    let hi = ts
                        .iter()
                        .copied()
                        .max_by(|a, b| along(a).total_cmp(&along(b)));
                    if let (Some(lo), Some(hi)) = (lo, hi) {
                        out.push(Projected::Curve(Curve2::Line {
                            a: at(lo),
                            b: at(hi),
                        }));
                    }
                    continue;
                }
                let round = (big - small).abs() <= 1e-9 * size;
                let ellipse = Curve2::Ellipse {
                    center: c,
                    major: big,
                    minor: small,
                    rotation,
                };
                let param = |p: P2| -> f64 {
                    if round {
                        let v = sub(p, c);
                        v[1].atan2(v[0])
                    } else {
                        ellipse.closest(p).0
                    }
                };
                if *closed {
                    out.push(Projected::Curve(if round {
                        Curve2::Circle {
                            center: c,
                            radius: big,
                            start: 0.0,
                        }
                    } else {
                        ellipse
                    }));
                    continue;
                }
                let (p0, p1, pm) = (at(*start), at(*end), at((start + end) / 2.0));
                let (mut t0, mut t1) = (param(p0), param(p1));
                let tm = param(pm);
                // Counter-clockwise from t0 through the middle to t1, or the
                // other way round.
                if angle_after(t0, tm) > angle_after(t0, t1) {
                    std::mem::swap(&mut t0, &mut t1);
                }
                let t1 = angle_after(t0, t1);
                out.push(Projected::Curve(if round {
                    Curve2::Arc {
                        center: c,
                        radius: big,
                        start: t0,
                        end: t1,
                    }
                } else {
                    Curve2::EllipticalArc {
                        center: c,
                        major: big,
                        minor: small,
                        rotation,
                        start: t0,
                        end: t1,
                    }
                }));
            }
            Curve3::BSpline {
                degree,
                poles,
                weights,
                knots,
            } => {
                out.push(Projected::Curve(Curve2::Nurbs(super::geometry::Nurbs {
                    degree: *degree as usize,
                    control: poles.iter().map(|p| to_sketch(frame, *p)).collect(),
                    weights: weights.clone(),
                    knots: knots.clone(),
                })));
            }
        }
    }
    out
}

fn dot2(a: P2, b: P2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// Entities for projected primitives: fixed reference geometry, with
/// coinciding ends sharing a point. `next` gives fresh ids.
pub fn entities(projected: &[Projected], next: &mut dyn FnMut() -> EntityUid) -> Vec<Entity> {
    let mut out: Vec<Entity> = Vec::new();
    let mut points: Vec<(P2, EntityUid)> = Vec::new();
    let mut size = 1.0_f64;
    for p in projected {
        if let Projected::Curve(c) = p {
            let (lo, hi) = c.bounds();
            size = size.max(dist(lo, hi));
        }
    }
    let tol = 1e-9 * size;
    // Points are made through `point`, curve ids through `next`.
    let ids = std::cell::RefCell::new(next);
    let mut point = |out: &mut Vec<Entity>, at: P2, shared: bool| -> EntityUid {
        if shared && let Some((_, id)) = points.iter().find(|(p, _)| dist(*p, at) <= tol) {
            return *id;
        }
        let id = (ids.borrow_mut())();
        let mut e = Entity::point(id, at);
        e.fixed = true;
        e.reference = true;
        out.push(e);
        if shared {
            points.push((at, id));
        }
        id
    };
    for p in projected {
        let curve = match p {
            Projected::Point(at) => {
                point(&mut out, *at, true);
                continue;
            }
            Projected::Curve(c) => c,
        };
        let id = (ids.borrow_mut())();
        let kind = match curve {
            Curve2::Line { a, b } => EntityKind::Line {
                start: point(&mut out, *a, true),
                end: point(&mut out, *b, true),
                centerline: false,
            },
            Curve2::Circle { center, radius, .. } => EntityKind::Circle {
                center: point(&mut out, *center, false),
                radius: *radius,
                start: 0.0,
            },
            Curve2::Arc { center, .. } => {
                let (s, e) = curve.ends().expect("an arc has ends");
                EntityKind::Arc {
                    center: point(&mut out, *center, false),
                    start: point(&mut out, s, true),
                    end: point(&mut out, e, true),
                }
            }
            Curve2::Ellipse {
                center,
                major,
                minor,
                rotation,
            } => EntityKind::Ellipse {
                center: point(&mut out, *center, false),
                major: point(
                    &mut out,
                    add(*center, scale([rotation.cos(), rotation.sin()], *major)),
                    false,
                ),
                minor_radius: *minor,
            },
            Curve2::EllipticalArc {
                center,
                major,
                minor,
                rotation,
                ..
            } => {
                let (s, e) = curve.ends().expect("an arc has ends");
                EntityKind::EllipticalArc {
                    center: point(&mut out, *center, false),
                    major: point(
                        &mut out,
                        add(*center, scale([rotation.cos(), rotation.sin()], *major)),
                        false,
                    ),
                    minor_radius: *minor,
                    start: point(&mut out, s, true),
                    end: point(&mut out, e, true),
                }
            }
            Curve2::Nurbs(n) => {
                let last = n.control.len() - 1;
                let control = n
                    .control
                    .iter()
                    .enumerate()
                    .map(|(i, c)| point(&mut out, *c, i == 0 || i == last))
                    .collect();
                EntityKind::Spline {
                    degree: n.degree as u32,
                    control,
                    weights: n.weights.clone(),
                    knots: n.knots.clone(),
                }
            }
        };
        let mut e = Entity::new(id, kind);
        e.fixed = true;
        e.reference = true;
        out.push(e);
    }
    out
}

/// The kind of each entity, to tell whether new geometry fits old ids.
fn shape_of(entities: &[Entity]) -> Vec<(&'static str, Vec<usize>)> {
    let position: BTreeMap<EntityUid, usize> = entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id, i))
        .collect();
    entities
        .iter()
        .map(|e| {
            (
                e.kind.type_name(),
                e.kind
                    .points()
                    .iter()
                    .map(|p| position.get(p).copied().unwrap_or(usize::MAX))
                    .collect(),
            )
        })
        .collect()
}

/// The model curves of a projection's source, in the sketch's component's
/// coordinates (moved there from another component's by its link,
/// mitcad#100).
fn source_curves<K: Kernel>(
    projection: &Projection,
    ctx: &mut EvalContext<'_, K>,
) -> Result<Vec<Curve3>, String> {
    let source = &projection.source;
    let (component, moved) = match &projection.link {
        Some(link) => {
            let (component, t) = ctx
                .linked(link)
                .map_err(|e| format!("projection of {source}: {e}"))?;
            (component, Some(t))
        }
        None => (ctx.component(), None),
    };
    let shapes = match projection.body {
        Some(body) => vec![(body, ctx.body_in(component, body)?)],
        None => ctx.bodies_in(component),
    };
    for (_, shape) in shapes {
        let mut curves = ctx
            .kernel
            .curves_of(&shape, source)
            .map_err(|e| format!("projection of {source}: {e}"))?;
        // A source a sketch change renamed is found again (mitcad#272).
        if curves.is_empty()
            && let Ok(Some((found, what, maker))) =
                crate::features::renamed::find_name(ctx.kernel, &shape, source)
        {
            curves = ctx
                .kernel
                .curves_of(&shape, &found)
                .map_err(|e| format!("projection of {source}: {e}"))?;
            if !curves.is_empty() {
                ctx.found_renamed(what, source, &found, maker);
            }
        }
        if !curves.is_empty() {
            return Ok(match moved {
                Some(t) => curves.iter().map(|c| transform_curve(&t, c)).collect(),
                None => curves,
            });
        }
    }
    Ok(Vec::new())
}

/// A model curve moved by a rigid transform.
pub fn transform_curve(t: &Transform, curve: &Curve3) -> Curve3 {
    match curve {
        Curve3::Point(p) => Curve3::Point(t.apply_point(*p)),
        Curve3::Line { start, end } => Curve3::Line {
            start: t.apply_point(*start),
            end: t.apply_point(*end),
        },
        Curve3::Conic {
            center,
            normal,
            x_axis,
            major,
            minor,
            start,
            end,
            closed,
        } => Curve3::Conic {
            center: t.apply_point(*center),
            normal: t.apply_vector(*normal),
            x_axis: t.apply_vector(*x_axis),
            major: *major,
            minor: *minor,
            start: *start,
            end: *end,
            closed: *closed,
        },
        Curve3::BSpline {
            degree,
            poles,
            weights,
            knots,
        } => Curve3::BSpline {
            degree: *degree,
            poles: poles.iter().map(|p| t.apply_point(*p)).collect(),
            weights: weights.clone(),
            knots: knots.clone(),
        },
    }
}

/// How a linked projection followed its source at an evaluation, when
/// anything of it did not (mitcad#232).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectionFollow {
    /// The projection's index in the definition's `projections`.
    pub projection: usize,
    /// Its curves and loose points that found nothing like them in the
    /// source: they keep their last geometry.
    pub unfollowed: Vec<EntityUid>,
    /// The source's curves and points no entity of the projection follows
    /// (a projection of some of a face's edges, an edge the face gained).
    pub unmatched: usize,
    /// The source has no curves (it is gone): everything kept.
    pub lost: bool,
    /// Its curves no longer match the source's (mitcad#249): following by
    /// geometry would tear its arcs from the curves they share points with,
    /// or a projection that follows by position finds the source's curves
    /// changed in kind, order or joints. Everything kept.
    pub kept: bool,
    /// Its curves that, where they are now, match none of the source's
    /// current curves (all of them for a lost source): stale curves Clean
    /// Up Stale Projections tidies (mitcad#252).
    pub stale: Vec<EntityUid>,
}

impl ProjectionFollow {
    /// The evaluation's warning about it; None when its entities all
    /// followed (source curves nothing follows are no warning: a
    /// projection may hold only some of its source's curves).
    pub fn warning(&self, projection: &Projection) -> Option<String> {
        if self.lost {
            return Some(format!(
                "the projection of {} does not follow it: it is gone, and its entities keep \
                 their last geometry",
                projection.source
            ));
        }
        if self.kept {
            return Some(format!(
                "the projection of {} does not follow it: its curves no longer match the \
                 source's, and they keep their last geometry",
                projection.source
            ));
        }
        if self.unfollowed.is_empty() {
            return None;
        }
        let names: Vec<String> = self
            .unfollowed
            .iter()
            .map(|id| entity_name(projection, *id))
            .collect();
        let one = names.len() == 1;
        Some(format!(
            "the projection of {} does not follow it with {}: nothing like {} is in it now, \
             so {} {} last geometry",
            projection.source,
            names.join(", "),
            if one { "it" } else { "them" },
            if one { "it keeps" } else { "they keep" },
            if one { "its" } else { "their" },
        ))
    }
}

/// `p7` or `c9`, as the projection lists it.
fn entity_name(projection: &Projection, id: EntityUid) -> String {
    projection
        .entities
        .iter()
        .find(|r| r.uid() == id)
        .map_or_else(|| format!("c{}", id.0), ToString::to_string)
}

/// What [`follow_links`] did.
#[derive(Debug, Clone, Default)]
pub struct Followed {
    /// The definition with linked projections moved to their sources'
    /// current geometry, or None when nothing moved.
    pub def: Option<SketchDef>,
    /// The projections that did not follow all the way.
    pub reports: Vec<ProjectionFollow>,
}

/// The definition with linked projections moved to their sources' current
/// geometry (None when there are none or nothing moved), and what of them
/// could not follow. Each entity follows the fresh curve of its kind most
/// like it ([`pair`]); one that finds none keeps its last geometry.
pub fn follow_links<K: Kernel>(
    def: &SketchDef,
    frame: &SketchFrame,
    ctx: &mut EvalContext<'_, K>,
) -> Result<Followed, String> {
    let mut out = Followed::default();
    if def.projections.is_empty() {
        return Ok(out);
    }
    let mut updated = def.clone();
    let mut changed = false;
    let index = super::EntityIndex::new(&def.entities);
    let old_at = |id: EntityUid| index.point(id);
    for (k, projection) in def.projections.iter().enumerate() {
        let curves = source_curves(projection, ctx)?;
        if curves.is_empty() {
            // A lost projection keeps its last geometry.
            out.reports.push(ProjectionFollow {
                projection: k,
                lost: true,
                stale: linked_entities(def, projection)
                    .iter()
                    .filter(|e| !e.is_point())
                    .map(|e| e.id)
                    .collect(),
                ..ProjectionFollow::default()
            });
            continue;
        }
        // Fresh ids above the sketch's: the old ids replace them one at a
        // time, so no old id may also be a fresh one still to replace.
        let mut counter = def.entities.iter().map(|e| e.id.0).max().unwrap_or(0);
        let mut fresh_id = || {
            counter += 1;
            EntityUid(counter)
        };
        let old: Vec<Entity> = linked_entities(def, projection);
        if !projection.by_geometry {
            // A projection of an earlier version or an import follows as
            // there: only a source whose curves come back as they were
            // listed, of the same kinds and joined the same way, entity by
            // entity (mitcad#249).
            let fresh = entities(&project(&curves, frame), &mut fresh_id);
            if shape_of(&fresh) != shape_of(&old) {
                out.reports.push(ProjectionFollow {
                    projection: k,
                    kept: true,
                    stale: stale_curves(&updated, projection, &fresh),
                    ..ProjectionFollow::default()
                });
                continue;
            }
            for (new, old) in fresh.iter().zip(&old) {
                if let Some(target) = updated.entity_mut(old.id) {
                    let mut kind = new.kind.clone();
                    // Keep the old point ids.
                    for (from, to) in new.kind.points().iter().zip(old.kind.points()) {
                        kind.replace_point(*from, to);
                    }
                    // Where a circle starts is no geometry of its source.
                    if let (
                        EntityKind::Circle { start, .. },
                        EntityKind::Circle {
                            start: old_start, ..
                        },
                    ) = (&mut kind, &old.kind)
                    {
                        *start = *old_start;
                    }
                    if target.kind != kind {
                        target.kind = kind;
                        changed = true;
                    }
                }
            }
            continue;
        }
        let fresh = entities(&distinct(project(&curves, frame)), &mut fresh_id);
        let pairing = pair(&old, &old_at, &fresh);
        if pairing.kept {
            // A pairing that tears the projection's joints moves nothing
            // (mitcad#249).
            out.reports.push(ProjectionFollow {
                projection: k,
                kept: true,
                stale: stale_curves(&updated, projection, &fresh),
                ..ProjectionFollow::default()
            });
            continue;
        }
        for (id, kind) in pairing.kinds {
            if let Some(target) = updated.entity_mut(id)
                && target.kind != kind
            {
                target.kind = kind;
                changed = true;
            }
        }
        if !pairing.unfollowed.is_empty() || pairing.unmatched > 0 {
            // Where its entities are now: those that followed match.
            let stale = if pairing.unfollowed.is_empty() {
                Vec::new()
            } else {
                stale_curves(&updated, projection, &fresh)
            };
            out.reports.push(ProjectionFollow {
                projection: k,
                unfollowed: pairing.unfollowed,
                unmatched: pairing.unmatched,
                stale,
                ..ProjectionFollow::default()
            });
        }
    }
    out.def = changed.then_some(updated);
    Ok(out)
}

/// How far apart, relative to the size of the geometry, two curves or
/// points may be and still be the same.
pub(crate) const SAME: f64 = 1e-7;

/// How far apart, relative to the size of the geometry, a projected curve
/// and a curve of its source may be and still match: looser than [`SAME`],
/// as an imported projection's numbers come from the file.
const MATCH: f64 = 1e-6;

/// The curves of a projection that match none of `fresh` (entities made
/// from its source's current curves), where `def` has them (mitcad#252).
fn stale_curves(def: &SketchDef, projection: &Projection, fresh: &[Entity]) -> Vec<EntityUid> {
    let index = super::EntityIndex::new(&def.entities);
    let at = |id: EntityUid| index.point(id);
    let fresh_points: BTreeMap<EntityUid, P2> = fresh
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Point { at } => Some((e.id, at)),
            _ => None,
        })
        .collect();
    let fresh_at = |id: EntityUid| fresh_points.get(&id).copied();
    let old = linked_entities(def, projection);
    let mut size = 1.0_f64;
    for p in fresh_points.values() {
        size = size.max(norm(*p));
    }
    for e in &old {
        if let EntityKind::Point { at } = e.kind {
            size = size.max(norm(at));
        }
    }
    let tol = MATCH * size;
    old.iter()
        .filter(|e| !e.is_point())
        .filter(|e| {
            !fresh.iter().any(|f| {
                !f.is_point()
                    && unlikeness(&e.kind, &at, &f.kind, &fresh_at).is_some_and(|(c, _)| c <= tol)
            })
        })
        .map(|e| e.id)
        .collect()
}

/// The projected primitives without repeats: a curve or point the same as
/// an earlier one (a seam edge a face lists twice, edges seen edge on that
/// project onto one line) is left out (mitcad#232).
pub fn distinct(projected: Vec<Projected>) -> Vec<Projected> {
    let mut size = 1.0_f64;
    for p in &projected {
        if let Projected::Curve(c) = p {
            let (lo, hi) = c.bounds();
            size = size.max(norm(lo)).max(norm(hi));
        }
    }
    let tol = SAME * size;
    let mut out: Vec<Projected> = Vec::new();
    for p in projected {
        if !out.iter().any(|o| same_projected(o, &p, tol)) {
            out.push(p);
        }
    }
    out
}

fn same_projected(a: &Projected, b: &Projected, tol: f64) -> bool {
    let near = |p: P2, q: P2| dist(p, q) <= tol;
    let close = |x: f64, y: f64| (x - y).abs() <= tol;
    let ends = |a: &Curve2, b: &Curve2| {
        a.ends()
            .zip(b.ends())
            .is_some_and(|(x, y)| near(x.0, y.0) && near(x.1, y.1))
    };
    let middle = |c: &Curve2| {
        let (t0, t1) = c.domain();
        c.point((t0 + t1) / 2.0)
    };
    let (a, b) = match (a, b) {
        (Projected::Point(p), Projected::Point(q)) => return near(*p, *q),
        (Projected::Curve(a), Projected::Curve(b)) => (a, b),
        _ => return false,
    };
    match (a, b) {
        (Curve2::Line { a: a0, b: a1 }, Curve2::Line { a: b0, b: b1 }) => {
            (near(*a0, *b0) && near(*a1, *b1)) || (near(*a0, *b1) && near(*a1, *b0))
        }
        (
            Curve2::Circle {
                center: c0,
                radius: r0,
                ..
            },
            Curve2::Circle {
                center: c1,
                radius: r1,
                ..
            },
        ) => near(*c0, *c1) && close(*r0, *r1),
        (Curve2::Arc { .. }, Curve2::Arc { .. })
        | (Curve2::EllipticalArc { .. }, Curve2::EllipticalArc { .. }) => {
            ends(a, b) && near(middle(a), middle(b))
        }
        (
            Curve2::Ellipse {
                center: c0,
                major: m0,
                minor: n0,
                rotation: t0,
            },
            Curve2::Ellipse {
                center: c1,
                major: m1,
                minor: n1,
                rotation: t1,
            },
        ) => {
            // The major axis either way round.
            let turn = (t0 - t1).rem_euclid(std::f64::consts::PI);
            let turn = turn.min(std::f64::consts::PI - turn);
            near(*c0, *c1) && close(*m0, *m1) && close(*n0, *n1) && turn * m0.max(1.0) <= tol
        }
        (Curve2::Nurbs(x), Curve2::Nurbs(y)) => {
            x.degree == y.degree
                && x.control.len() == y.control.len()
                && x.weights.len() == y.weights.len()
                && x.knots.len() == y.knots.len()
                && (x.control.iter().zip(&y.control).all(|(p, q)| near(*p, *q))
                    || x.control
                        .iter()
                        .zip(y.control.iter().rev())
                        .all(|(p, q)| near(*p, *q)))
        }
        _ => false,
    }
}

/// An entity's geometry to compare with another of its kind: its points'
/// positions and its sizes; None for a kind projections do not make or a
/// point that is missing.
fn anchors(kind: &EntityKind, at: &dyn Fn(EntityUid) -> Option<P2>) -> Option<(Vec<P2>, Vec<f64>)> {
    let points = |ids: &[EntityUid]| ids.iter().map(|id| at(*id)).collect::<Option<Vec<P2>>>();
    Some(match kind {
        EntityKind::Point { at } => (vec![*at], Vec::new()),
        EntityKind::Circle { center, radius, .. } => (points(&[*center])?, vec![*radius]),
        EntityKind::Ellipse { minor_radius, .. }
        | EntityKind::EllipticalArc { minor_radius, .. } => {
            (points(&kind.points())?, vec![*minor_radius])
        }
        EntityKind::Line { .. } | EntityKind::Arc { .. } => (points(&kind.points())?, Vec::new()),
        EntityKind::Spline {
            degree,
            control,
            weights,
            knots,
        } => {
            let mut sizes = vec![f64::from(*degree), knots.len() as f64];
            sizes.extend(weights);
            (points(control)?, sizes)
        }
        EntityKind::FittedSpline { .. } => return None,
    })
}

/// How unlike two entities' geometry is (the distances of their points
/// plus the differences of their sizes), and whether the second runs the
/// other way round (lines and splines); None when they are not of one kind
/// and form.
pub(crate) fn unlikeness(
    a: &EntityKind,
    a_at: &dyn Fn(EntityUid) -> Option<P2>,
    b: &EntityKind,
    b_at: &dyn Fn(EntityUid) -> Option<P2>,
) -> Option<(f64, bool)> {
    if a.type_name() != b.type_name() {
        return None;
    }
    let (pa, sa) = anchors(a, a_at)?;
    let (pb, sb) = anchors(b, b_at)?;
    if pa.len() != pb.len() || sa.len() != sb.len() {
        return None;
    }
    let sizes: f64 = sa.iter().zip(&sb).map(|(x, y)| (x - y).abs()).sum();
    let ahead: f64 = pa.iter().zip(&pb).map(|(p, q)| dist(*p, *q)).sum();
    if matches!(a, EntityKind::Line { .. } | EntityKind::Spline { .. }) {
        let back: f64 = pa
            .iter()
            .zip(pb.iter().rev())
            .map(|(p, q)| dist(*p, *q))
            .sum();
        if back < ahead {
            return Some((back + sizes, true));
        }
    }
    Some((ahead + sizes, false))
}

/// How a projection's entities follow fresh ones made from its source.
#[derive(Debug, Default)]
struct Pairing {
    /// The new kinds of the old entities that follow, points included.
    kinds: Vec<(EntityUid, EntityKind)>,
    unfollowed: Vec<EntityUid>,
    unmatched: usize,
    /// The pairs would tear points the projection's curves share: nothing
    /// follows (mitcad#249).
    kept: bool,
}

/// A pair of an old and a fresh entity (indices), how unlike they are and
/// whether the fresh one runs the other way round.
type Pair = (usize, usize, f64, bool);

/// Pairs a projection's entities (`old`, their points' positions from
/// `old_at`) with `fresh` ones made from its source's current curves: each
/// with the fresh one of its kind most like it, in the order of the lists
/// when that is as good (the kernel lists a source's curves the same way
/// each time) and nearest first otherwise, so that a projection of some of
/// a source's curves, or of curves listed in another order or from another
/// start, follows. An entity the same as another of the projection (a
/// curve projected twice) follows the same fresh one.
fn pair(old: &[Entity], old_at: &dyn Fn(EntityUid) -> Option<P2>, fresh: &[Entity]) -> Pairing {
    let fresh_points: BTreeMap<EntityUid, P2> = fresh
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Point { at } => Some((e.id, at)),
            _ => None,
        })
        .collect();
    let fresh_at = |id: EntityUid| fresh_points.get(&id).copied();
    // Curves, then the points no curve of the list uses.
    let items = |list: &[Entity]| -> Vec<Entity> {
        let used: std::collections::BTreeSet<EntityUid> =
            list.iter().flat_map(|e| e.kind.points()).collect();
        let curves = list.iter().filter(|e| !e.is_point());
        let loose = list
            .iter()
            .filter(|e| e.is_point() && !used.contains(&e.id));
        curves.chain(loose).cloned().collect()
    };
    let (olds, news) = (items(old), items(fresh));
    let cost = |i: usize, j: usize| unlikeness(&olds[i].kind, old_at, &news[j].kind, &fresh_at);
    // In the order of the lists.
    let in_order: Option<Vec<Pair>> = (olds.len() == news.len()
        && shape_of(old) == shape_of(fresh))
    .then(|| {
        (0..olds.len())
            .map(|i| cost(i, i).map(|(c, back)| (i, i, c, back)))
            .collect::<Option<Vec<_>>>()
    })
    .flatten();
    // Nearest first.
    let mut candidates: Vec<Pair> = Vec::new();
    for i in 0..olds.len() {
        for j in 0..news.len() {
            if let Some((c, back)) = cost(i, j) {
                candidates.push((i, j, c, back));
            }
        }
    }
    candidates.sort_by(|a, b| a.2.total_cmp(&b.2));
    let (mut old_taken, mut new_taken) = (vec![false; olds.len()], vec![false; news.len()]);
    let mut nearest: Vec<Pair> = Vec::new();
    for (i, j, c, back) in candidates {
        if !old_taken[i] && !new_taken[j] {
            old_taken[i] = true;
            new_taken[j] = true;
            nearest.push((i, j, c, back));
        }
    }
    let total = |pairs: &[Pair]| pairs.iter().map(|p| p.2).sum::<f64>();
    let mut pairs = match in_order {
        Some(in_order)
            if in_order.len() > nearest.len()
                || total(&in_order) <= total(&nearest) * (1.0 + 1e-9) + 1e-12 =>
        {
            in_order
        }
        _ => nearest,
    };
    // What counts as the same, by the size of the projection's geometry.
    let mut size = 1.0_f64;
    for e in &olds {
        for p in anchors(&e.kind, old_at).map(|a| a.0).unwrap_or_default() {
            size = size.max(norm(p));
        }
    }
    // Repeats of a paired entity follow its fresh one.
    let paired: Vec<(usize, usize)> = pairs.iter().map(|p| (p.0, p.1)).collect();
    for i in 0..olds.len() {
        if paired.iter().any(|p| p.0 == i) {
            continue;
        }
        let twin = paired.iter().find(|(k, _)| {
            unlikeness(&olds[i].kind, old_at, &olds[*k].kind, old_at)
                .is_some_and(|(c, _)| c <= SAME * size)
        });
        if let Some(&(_, j)) = twin
            && let Some((c, back)) = cost(i, j)
        {
            pairs.push((i, j, c, back));
        }
    }
    let mut out = Pairing::default();
    let mut moved: BTreeMap<EntityUid, P2> = BTreeMap::new();
    // A point of an arc the pairs would put in two places tears the
    // projection's joints: the arc's centre and ends would no longer fit
    // (mitcad#249). Lines take either place and stay lines.
    let tol = SAME * size;
    let bound = |kind: &EntityKind| {
        matches!(
            kind,
            EntityKind::Arc { .. } | EntityKind::EllipticalArc { .. }
        )
    };
    let arc_points: std::collections::BTreeSet<EntityUid> = olds
        .iter()
        .filter(|e| bound(&e.kind))
        .flat_map(|e| e.kind.points())
        .collect();
    let mut torn = false;
    for &(i, j, _, back) in &pairs {
        let (o, f) = (&olds[i], &news[j]);
        if let (EntityKind::Point { .. }, EntityKind::Point { at }) = (&o.kind, &f.kind) {
            moved.entry(o.id).or_insert(*at);
            continue;
        }
        let (op, fp) = (o.kind.points(), f.kind.points());
        let n = op.len();
        for (k, id) in op.iter().enumerate() {
            let from = if back { fp[n - 1 - k] } else { fp[k] };
            if let Some(at) = fresh_at(from) {
                let to = *moved.entry(*id).or_insert(at);
                torn |= arc_points.contains(id) && dist(to, at) > tol;
            }
        }
        let kind = match (&o.kind, &f.kind) {
            // A line is its points (and the user's centre line flag).
            (EntityKind::Line { .. }, _) => o.kind.clone(),
            (
                EntityKind::Spline { .. },
                EntityKind::Spline {
                    degree,
                    weights,
                    knots,
                    ..
                },
            ) if back => {
                let (lo, hi) = (
                    knots.first().copied().unwrap_or(0.0),
                    knots.last().copied().unwrap_or(1.0),
                );
                EntityKind::Spline {
                    degree: *degree,
                    control: op.clone(),
                    weights: weights.iter().rev().copied().collect(),
                    knots: knots.iter().rev().map(|k| lo + hi - k).collect(),
                }
            }
            _ => {
                let mut kind = f.kind.clone();
                // Keep the old point ids.
                for (from, to) in fp.iter().zip(&op) {
                    kind.replace_point(*from, *to);
                }
                // Where a circle starts (its seam, an import's) is no
                // geometry of its source: kept.
                if let (
                    EntityKind::Circle { start, .. },
                    EntityKind::Circle {
                        start: old_start, ..
                    },
                ) = (&mut kind, &o.kind)
                {
                    *start = *old_start;
                }
                kind
            }
        };
        out.kinds.push((o.id, kind));
    }
    out.unfollowed = (0..olds.len())
        .filter(|i| !pairs.iter().any(|p| p.0 == *i))
        .map(|i| olds[i].id)
        .collect();
    // An arc that keeps its geometry while a point of it moves with
    // another curve tears too.
    torn |= olds
        .iter()
        .filter(|e| bound(&e.kind) && out.unfollowed.contains(&e.id))
        .flat_map(|e| e.kind.points())
        .any(|id| {
            moved
                .get(&id)
                .zip(old_at(id))
                .is_some_and(|(to, from)| dist(*to, from) > tol)
        });
    if torn {
        return Pairing {
            kept: true,
            ..Pairing::default()
        };
    }
    for (id, at) in moved {
        out.kinds.push((id, EntityKind::Point { at }));
    }
    out.unmatched = (0..news.len())
        .filter(|j| !pairs.iter().any(|p| p.1 == *j))
        .count();
    out
}

/// Solves a definition whose linked projections [`follow_links`] moved
/// (`followed`) as if they were dragged there from where `def` has them:
/// in steps, each starting from the last one's solution, so that geometry
/// constrained to them keeps its side, as with a dimension change (the
/// solver's continuation). One solve when they moved little or a step
/// does not solve.
pub fn solve_followed(
    def: &SketchDef,
    followed: &SketchDef,
    value: &mut dyn FnMut(ParamId) -> Result<f64, String>,
    name: &dyn Fn(ParamId) -> String,
) -> Result<Solved, SolveError> {
    let steps = follow_steps(def, followed);
    if steps > 1
        && let Some(solved) = solve_in_steps(def, followed, steps, value, name)
    {
        return Ok(solved);
    }
    followed.solve_with(value, name)
}

/// The steps of [`solve_followed`]: one for each tenth of the sketch's
/// size the farthest point moves, at most 16.
fn follow_steps(def: &SketchDef, followed: &SketchDef) -> usize {
    let at = |e: &Entity| match e.kind {
        EntityKind::Point { at } => Some(at),
        _ => None,
    };
    let (mut low, mut high) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for p in def.entities.iter().chain(&followed.entities).filter_map(at) {
        for i in 0..2 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    let size = dist(low, high);
    let farthest = def
        .entities
        .iter()
        .zip(&followed.entities)
        .filter_map(|(old, new)| Some(dist(at(old)?, at(new)?)))
        .fold(0.0, f64::max);
    if !(size.is_finite() && size > 0.0) {
        return 1;
    }
    ((farthest / (0.1 * size)).ceil() as usize).clamp(1, 16)
}

fn solve_in_steps(
    def: &SketchDef,
    followed: &SketchDef,
    steps: usize,
    value: &mut dyn FnMut(ParamId) -> Result<f64, String>,
    name: &dyn Fn(ParamId) -> String,
) -> Option<Solved> {
    // From the definition solved where it was.
    let mut current = def.clone();
    current
        .solve_with(value, name)
        .ok()?
        .store(&mut current.entities);
    for k in 1..=steps {
        let t = k as f64 / steps as f64;
        for ((entity, old), new) in current
            .entities
            .iter_mut()
            .zip(&def.entities)
            .zip(&followed.entities)
        {
            if old.kind != new.kind {
                entity.kind = between(&old.kind, &new.kind, t);
            }
        }
        let solved = current.solve_with(value, name).ok()?;
        if k == steps {
            return Some(solved);
        }
        solved.store(&mut current.entities);
    }
    None
}

/// An entity's kind the part `t` of the way from `old` to `new`: points
/// and radii in between, the rest as `new`.
fn between(old: &EntityKind, new: &EntityKind, t: f64) -> EntityKind {
    let mix = |a: f64, b: f64| if t >= 1.0 { b } else { a + (b - a) * t };
    let mut kind = new.clone();
    match (old, &mut kind) {
        (EntityKind::Point { at: a }, EntityKind::Point { at: b }) => {
            *b = [mix(a[0], b[0]), mix(a[1], b[1])];
        }
        (EntityKind::Circle { radius: a, .. }, EntityKind::Circle { radius: b, .. })
        | (
            EntityKind::Ellipse {
                minor_radius: a, ..
            },
            EntityKind::Ellipse {
                minor_radius: b, ..
            },
        )
        | (
            EntityKind::EllipticalArc {
                minor_radius: a, ..
            },
            EntityKind::EllipticalArc {
                minor_radius: b, ..
            },
        ) => *b = mix(*a, *b),
        _ => {}
    }
    kind
}

/// The entities of `solution` (the definition as it evaluated, solved)
/// that differ from `def`'s: a sketch output keeps them.
pub fn moved_entities(def: &SketchDef, solution: &SketchDef) -> Vec<Entity> {
    solution
        .entities
        .iter()
        .zip(&def.entities)
        .filter(|(new, old)| new.id == old.id && new.kind != old.kind)
        .map(|(new, _)| new.clone())
        .collect()
}

/// The definition with the entities the last evaluation moved (a sketch
/// output's `moved`) in place, or None when none applies. The stored
/// definition keeps linked projections, and what is constrained to them,
/// where the last edit left them; this is the sketch as it evaluated,
/// which queries show and edits start from. Moved entities the definition
/// no longer has as the same kind of entity on the same points, and fixed
/// ones outside linked projections, are left out.
pub fn with_moved(def: &SketchDef, moved: &[Entity]) -> Option<SketchDef> {
    let mut out: Option<SketchDef> = None;
    for entity in moved {
        let Some(old) = def.entity(entity.id) else {
            continue;
        };
        let linked = || {
            def.projections
                .iter()
                .any(|p| p.entities.iter().any(|r| r.uid() == entity.id))
        };
        if old.kind == entity.kind
            || old.kind.type_name() != entity.kind.type_name()
            || old.kind.points() != entity.kind.points()
            || (old.fixed && !linked())
        {
            continue;
        }
        if let Some(target) = out.get_or_insert_with(|| def.clone()).entity_mut(entity.id) {
            target.kind = entity.kind.clone();
        }
    }
    out
}

/// The entities of a projection with the points they use, in order:
/// each curve after its points, as [`entities`] makes them.
pub fn linked_entities(def: &SketchDef, projection: &Projection) -> Vec<Entity> {
    let mut out: Vec<Entity> = Vec::new();
    for r in &projection.entities {
        let Some(e) = def.entity(r.uid()) else {
            continue;
        };
        if !out.iter().any(|o| o.id == e.id) {
            out.push(e.clone());
        }
    }
    out
}

/// The refs of entities, points included, for a projection record.
pub fn refs(entities: &[Entity]) -> Vec<Ref> {
    entities.iter().map(Entity::as_ref).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_xz() -> SketchFrame {
        SketchFrame {
            origin: [0.0; 3],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 0.0, -1.0],
        }
    }

    #[test]
    fn conics_project_to_circles_ellipses_or_lines() {
        // A circle in the XY plane seen from XY: a circle.
        let circle = Curve3::Conic {
            center: [1.0, 2.0, 5.0],
            normal: [0.0, 0.0, 1.0],
            x_axis: [1.0, 0.0, 0.0],
            major: 3.0,
            minor: 3.0,
            start: 0.0,
            end: std::f64::consts::TAU,
            closed: true,
        };
        let out = project(std::slice::from_ref(&circle), &SketchFrame::XY);
        assert_eq!(
            out,
            vec![Projected::Curve(Curve2::Circle {
                center: [1.0, 2.0],
                radius: 3.0,
                start: 0.0
            })]
        );
        // Seen edge on from XZ: a line across its diameter.
        let out = project(std::slice::from_ref(&circle), &frame_xz());
        let Projected::Curve(Curve2::Line { a, b }) = &out[0] else {
            panic!("{out:?}");
        };
        assert!((dist(*a, *b) - 6.0).abs() < 1e-9);
        // Tilted 60 degrees: an ellipse with half the minor radius.
        let tilted = Curve3::Conic {
            center: [1.0, 2.0, 5.0],
            normal: [0.0, (60f64).to_radians().sin(), (60f64).to_radians().cos()],
            x_axis: [1.0, 0.0, 0.0],
            major: 3.0,
            minor: 3.0,
            start: 0.0,
            end: std::f64::consts::TAU,
            closed: true,
        };
        let Projected::Curve(Curve2::Ellipse { major, minor, .. }) =
            &project(&[tilted], &SketchFrame::XY)[0]
        else {
            panic!("an ellipse");
        };
        assert!((major - 3.0).abs() < 1e-9 && (minor - 1.5).abs() < 1e-9);
    }

    #[test]
    fn projected_ends_share_points() {
        let square: Vec<Curve3> = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]
            .windows(2)
            .map(|w| Curve3::Line {
                start: [w[0][0], w[0][1], 3.0],
                end: [w[1][0], w[1][1], 3.0],
            })
            .chain([Curve3::Line {
                start: [0.0, 10.0, 3.0],
                end: [0.0, 0.0, 3.0],
            }])
            .collect();
        let mut n = 0;
        let entities = entities(&project(&square, &SketchFrame::XY), &mut || {
            n += 1;
            EntityUid(n)
        });
        assert_eq!(entities.iter().filter(|e| e.is_point()).count(), 4);
        assert_eq!(entities.iter().filter(|e| !e.is_point()).count(), 4);
        assert!(entities.iter().all(|e| e.fixed && e.reference));
    }
}
