// SPDX-License-Identifier: MIT
//! Sketches on geometry of other components, and linked curves as
//! projections (mitcad#225).
//!
//! A sketch may lie on a face of a body of another component, and its
//! linked curves follow edges of the model, its own component's or
//! another's. The model keeps both through occurrence links (mitcad#100,
//! `mitcad_model::links`): a sketch's `plane_link` and a projection's
//! `link`, each the occurrence path from the root to the geometry's
//! component and the one to the sketch's component.
//!
//! The file stores neither the face a sketch lies on nor the geometry a
//! linked curve follows in a form the decoder names (`core/import/README.md`,
//! *Sketch curve flags, spline kinds and sketches without a link*): both
//! are found in the replay by where they are. A face is the sketch's plane
//! when it is the only planar face of the other components' bodies, seen
//! through their occurrences, in the sketch's plane (looking along the
//! sketch's normal, else against it). A linked curve follows an edge when
//! that edge is the only one whose projection the curve lies on (the
//! edges in the sketch's plane first, as the file projects the face a
//! sketch lies on), its curve is the edge's only curve, and the curve is
//! where the edge projects (the snap of `snap.rs` put it there). Every
//! other linked curve stays fixed reference geometry, and the sketch's
//! import note says how many.

use std::collections::HashMap;

use mitcad_model::assembly::inverse;
use mitcad_model::datum::CurveGeometry;
use mitcad_model::features::SketchPlane;
use mitcad_model::sketch::EntityKind;
use mitcad_model::sketch::project::{entities, project, transform_curve};
use mitcad_model::{
    BodyUid, ComponentUid, Curve3, EntityUid, FaceName, OccurrenceUid, SketchFrame, TopoName,
    Transform,
};
use serde_json::{Value, json};

use crate::sketch::SketchParts;
use crate::{Importer, geom, refs, snap};

/// How far (mm) a linked curve's points and radius may lie from where its
/// source projects for the curve to be that projection: the snap put them
/// there, so only rounding is left.
const EXACT: f64 = 1e-6;

/// How far (mm) a face or an edge may lie off the sketch's plane to be in
/// it; the tolerance the sketch's own faces are found with. (A local
/// constant until the import's geometric tolerance option, mitcad#224,
/// replaces it.)
pub(crate) const IN_PLANE: f64 = 1e-4;

/// An occurrence path as the model writes it (`O1/O4`, empty for the
/// root).
pub(crate) fn path_text(path: &[OccurrenceUid]) -> String {
    path.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("/")
}

/// An occurrence link (`plane_link`, a projection's `link`).
pub(crate) fn link(source: &[OccurrenceUid], target: &[OccurrenceUid]) -> Value {
    let mut out = json!({});
    if !source.is_empty() {
        out["source"] = json!(path_text(source));
    }
    if !target.is_empty() {
        out["target"] = json!(path_text(target));
    }
    out
}

/// A planar face of another component's body, seen through one of its
/// occurrences, in the sketch's component's coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FaceCandidate {
    /// From the root to the body's component.
    pub path: Vec<OccurrenceUid>,
    pub body: BodyUid,
    pub face: FaceName,
    pub origin: [f64; 3],
    pub normal: [f64; 3],
}

/// The face a sketch with `frame` lies on among `candidates` (planar faces
/// of other components): those in the sketch's plane, looking along its
/// normal where any does; the only one, else the one that holds more of
/// the sketch's points than any other (`points_on` counts them), else
/// None.
pub(crate) fn plane_face(
    frame: &SketchFrame,
    candidates: Vec<FaceCandidate>,
    points_on: &mut dyn FnMut(&FaceCandidate) -> usize,
) -> Option<FaceCandidate> {
    let normal = geom::normal(frame);
    let mut on: Vec<FaceCandidate> = candidates
        .into_iter()
        .filter(|c| geom::coplanar(frame, c.origin, c.normal, IN_PLANE))
        .collect();
    if on.iter().any(|c| geom::dot(c.normal, normal) > 0.0) {
        on.retain(|c| geom::dot(c.normal, normal) > 0.0);
    }
    let mut on = unique(on, |a, b| {
        a.path == b.path && a.body == b.body && a.face == b.face
    });
    if on.len() == 1 {
        return on.pop();
    }
    let mut scored: Vec<(usize, FaceCandidate)> =
        on.into_iter().map(|c| (points_on(&c), c)).collect();
    scored.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    let best = matches!(scored.as_slice(),
        [(best, _), rest @ ..] if *best > 0 && rest.iter().all(|(n, _)| n < best));
    best.then(|| scored.swap_remove(0).1)
}

/// `items` without repeats (by `same`), in their order.
fn unique<T>(items: Vec<T>, same: impl Fn(&T, &T) -> bool) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    for item in items {
        if !out.iter().any(|o| same(o, &item)) {
            out.push(item);
        }
    }
    out
}

/// An edge of the replay a linked curve may follow.
#[derive(Debug, Clone)]
pub(crate) struct EdgeSource {
    pub body: BodyUid,
    /// The edge's name, as the kernel lists it (empty for an edge without
    /// one: no source, but the snap may still put a curve on it).
    pub edge: String,
    /// From the root to the body's component; None for the sketch's own
    /// component (no link).
    pub path: Option<Vec<OccurrenceUid>>,
    /// The edge's curve in the sketch's component's coordinates.
    pub curve: CurveGeometry,
}

/// Linked curves made projections.
#[derive(Debug, Default)]
pub(crate) struct Resolved {
    /// The sketch definition's `projections`.
    pub projections: Vec<Value>,
    /// Of them, those of other components (with a link).
    pub other: usize,
}

/// The import notes on a sketch's `linked` curves: those `resolved` as
/// projections where the sketch came in with them (`used`), and the rest
/// kept fixed, with the reason where their sources were looked for
/// (`looked`), how many the snap put on the model's edges and how many it
/// `left` where the file stored them (a snap would change the regions).
pub(crate) fn notes(
    linked: usize,
    resolved: &Resolved,
    used: bool,
    looked: bool,
    snapped: usize,
    left: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    let made = if used { resolved.projections.len() } else { 0 };
    if made > 0 {
        out.push(format!(
            "{made} linked curves as projections{}",
            if resolved.other > 0 {
                format!(" ({} of other components)", resolved.other)
            } else {
                String::new()
            }
        ));
    }
    let rest = linked.saturating_sub(made);
    if rest > 0 {
        let mut why = Vec::new();
        if looked && !used && !resolved.projections.is_empty() {
            why.push("the sketch does not solve with them as projections".to_owned());
        } else if looked {
            why.push("no single edge of the replay they follow".to_owned());
        }
        if snapped > 0 {
            why.push(format!("{snapped} on the model's edges"));
        }
        if left > 0 {
            why.push(format!(
                "{left} left where the file stored them: a snap onto the model's edge would change the sketch's regions"
            ));
        }
        out.push(format!(
            "{rest} linked curves kept as fixed reference geometry{}",
            if why.is_empty() {
                String::new()
            } else {
                format!(" ({})", why.join("; "))
            }
        ));
    }
    out
}

/// The box of a sketch's point positions (sketch mm); None without points.
pub(crate) fn points_box(parts: &SketchParts) -> Option<([f64; 2], [f64; 2])> {
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for (_, p) in &parts.positions {
        for k in 0..2 {
            low[k] = low[k].min(p[k]);
            high[k] = high[k].max(p[k]);
        }
    }
    (low[0].is_finite() && high[0].is_finite()).then_some((low, high))
}

/// Whether an edge lies in the sketch's plane.
fn in_plane(curve: &CurveGeometry, frame: &SketchFrame) -> bool {
    let n = geom::normal(frame);
    let on = |p: [f64; 3]| geom::dot(geom::sub(p, frame.origin), n).abs() <= IN_PLANE;
    match curve {
        CurveGeometry::Line { start, end } => on(*start) && on(*end),
        CurveGeometry::Circle { center, normal, .. } => {
            on(*center) && geom::norm(geom::cross(*normal, n)) < 1e-6
        }
        CurveGeometry::Other { .. } => false,
    }
}

/// The point ids of a sketch entity (JSON) in the order of
/// [`EntityKind::points`] for its type; None for other types.
fn point_keys(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "line" => &["start", "end"],
        "circle" => &["center"],
        "arc" => &["center", "start", "end"],
        "ellipse" => &["center", "major"],
        "elliptical_arc" => &["center", "major", "start", "end"],
        _ => return None,
    })
}

/// The position of point `id` among `entities`.
fn point_at(entities: &[Value], id: &str) -> Option<[f64; 2]> {
    let e = entities.iter().find(|e| e["id"] == id)?;
    let at = e["at"].as_array()?;
    Some([at.first()?.as_f64()?, at.get(1)?.as_f64()?])
}

/// The entity ids of a projection of `curves` (the source's, in the
/// sketch's component's coordinates) that the linked curve `entities[i]`
/// already is, in the order the model makes the projection's entities;
/// None unless the curve and its points are where the projection puts
/// them. A line the other way round is turned (`swap`).
fn as_projection(
    entities_json: &[Value],
    i: usize,
    curves: &[Curve3],
    frame: &SketchFrame,
) -> Option<(Vec<String>, bool)> {
    let e = &entities_json[i];
    let kind = e["type"].as_str()?;
    let keys = point_keys(kind)?;
    let mut counter = 0;
    let fresh = entities(&project(curves, frame), &mut || {
        counter += 1;
        EntityUid(counter)
    });
    let curve = fresh.last()?;
    // One curve and its own points.
    if fresh.len() != curve.kind.points().len() + 1 || curve.kind.type_name() != kind {
        return None;
    }
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) <= EXACT;
    let radius_ok = match &curve.kind {
        EntityKind::Circle { radius, .. } => e["radius"]
            .as_f64()
            .is_some_and(|r| (r - radius).abs() <= EXACT),
        EntityKind::Ellipse { minor_radius, .. }
        | EntityKind::EllipticalArc { minor_radius, .. } => e["minor_radius"]
            .as_f64()
            .is_some_and(|r| (r - minor_radius).abs() <= EXACT),
        _ => true,
    };
    if !radius_ok {
        return None;
    }
    let fresh_at = |id: EntityUid| {
        fresh.iter().find_map(|f| match f.kind {
            EntityKind::Point { at } if f.id == id => Some(at),
            _ => None,
        })
    };
    let points = curve.kind.points();
    let try_keys = |keys: &[&str]| -> Option<Vec<String>> {
        let mut map: Vec<(EntityUid, String)> = Vec::new();
        for (p, key) in points.iter().zip(keys) {
            let id = e[*key].as_str()?;
            if !near(fresh_at(*p)?, point_at(entities_json, id)?) {
                return None;
            }
            map.push((*p, id.to_owned()));
        }
        let mut refs = Vec::new();
        for f in &fresh {
            if f.id == curve.id {
                refs.push(e["id"].as_str()?.to_owned());
            } else {
                refs.push(map.iter().find(|(p, _)| *p == f.id)?.1.clone());
            }
        }
        Some(refs)
    };
    if let Some(refs) = try_keys(keys) {
        return Some((refs, false));
    }
    // A line the projection draws the other way round.
    if kind == "line" {
        let refs = try_keys(&["end", "start"])?;
        return Some((refs, true));
    }
    None
}

/// What the projection rule decides for a linked curve (`entity`, an
/// index into the sketch's entities): the distinct edges (indices into the
/// edge sources) whose projection it lies on, those in the sketch's plane
/// only where there are any; the curve follows the edge when there is one.
/// The snap takes the first of them too (`snap.rs`), so that the two agree
/// on the edge (mitcad#227).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    pub entity: usize,
    pub candidates: Vec<usize>,
}

impl Choice {
    /// The edge the curve follows, when the candidates name one.
    pub fn chosen(&self) -> Option<usize> {
        match self.candidates[..] {
            [k] => Some(k),
            _ => None,
        }
    }
}

/// The projection rule's choices for the linked curves of `parts` among
/// `edges` (in the space of the sketch's `frame`): curves no edge projects
/// onto are left out.
pub(crate) fn choices(
    parts: &SketchParts,
    frame: &SketchFrame,
    edges: &[EdgeSource],
) -> Vec<Choice> {
    let geometry: Vec<CurveGeometry> = edges.iter().map(|e| e.curve.clone()).collect();
    snap::sources(parts, frame, &geometry)
        .into_iter()
        .map(|(entity, found)| {
            let planar: Vec<usize> = found
                .iter()
                .copied()
                .filter(|k| in_plane(&edges[*k].curve, frame))
                .collect();
            let candidates = unique(if planar.is_empty() { found } else { planar }, |a, b| {
                let (a, b) = (&edges[*a], &edges[*b]);
                a.body == b.body && a.edge == b.edge && a.path == b.path
            });
            Choice { entity, candidates }
        })
        .collect()
}

/// For the snap: each linked curve with the edge it is to be put on first,
/// the first of the projection rule's candidates.
pub(crate) fn preferred(choices: &[Choice]) -> Vec<(usize, usize)> {
    choices
        .iter()
        .filter_map(|c| Some((c.entity, *c.candidates.first()?)))
        .collect()
}

/// The linked curves of `parts` that follow an edge of `edges` (in the
/// space of the sketch's `frame`), as projections (see the module
/// documentation), by the projection rule's `choices`. `target` is the
/// occurrence path the sketch's component is seen at; `curves_of` gives
/// an edge's curves as the model's projection reads them
/// (`Kernel::curves_of`, moved into the sketch's component's
/// coordinates), None where the name does not resolve. Lines the
/// projection draws the other way round are turned in `parts`.
pub(crate) fn resolve(
    parts: &mut SketchParts,
    frame: &SketchFrame,
    edges: &[EdgeSource],
    choices: &[Choice],
    target: &[OccurrenceUid],
    curves_of: &mut dyn FnMut(&EdgeSource) -> Option<Vec<Curve3>>,
) -> Resolved {
    let mut out = Resolved::default();
    for choice in choices {
        let i = choice.entity;
        let Some(k) = choice.chosen() else {
            if crate::tracing() {
                eprintln!(
                    "import:   linked {} follows {} edges: kept fixed",
                    parts.entities[i]["id"],
                    choice.candidates.len()
                );
            }
            continue;
        };
        let source = &edges[k];
        let Some(curves) = curves_of(source) else {
            continue;
        };
        let Some((refs, swap)) = as_projection(&parts.entities, i, &curves, frame) else {
            if crate::tracing() {
                eprintln!(
                    "import:   linked {} is not where {} projects: kept fixed",
                    parts.entities[i]["id"], source.edge
                );
            }
            continue;
        };
        if swap {
            let e = &mut parts.entities[i];
            let (start, end) = (e["start"].clone(), e["end"].clone());
            e["start"] = end;
            e["end"] = start;
        }
        let mut projection = json!({"source": source.edge, "body": source.body.to_string(),
                                    "entities": refs});
        if let Some(path) = &source.path {
            projection["link"] = link(path, target);
            out.other += 1;
        }
        out.projections.push(projection);
    }
    out
}

/// A curve of an edge moved by a rigid transform (lines and circles; the
/// rest is no source [`snap::sources`] matches).
fn moved(t: &Transform, curve: &CurveGeometry) -> Option<CurveGeometry> {
    Some(match curve {
        CurveGeometry::Line { start, end } => CurveGeometry::Line {
            start: t.apply_point(*start),
            end: t.apply_point(*end),
        },
        CurveGeometry::Circle {
            center,
            normal,
            radius,
            start,
            end,
        } => CurveGeometry::Circle {
            center: t.apply_point(*center),
            normal: t.apply_vector(*normal),
            radius: *radius,
            start: t.apply_point(*start),
            end: t.apply_point(*end),
        },
        CurveGeometry::Other { .. } => return None,
    })
}

impl<K: crate::ImportKernel> Importer<'_, K> {
    /// The occurrence path the item's component is seen at: the first of
    /// its paths (the original the file's features are made in, as
    /// [`Importer::original_placement_at`]); None where it is placed
    /// nowhere.
    pub(crate) fn sketch_target(&self) -> Option<Vec<OccurrenceUid>> {
        self.doc
            .assembly()
            .paths_to(self.component)
            .into_iter()
            .min()
    }

    /// From the coordinates of the occurrence path `path`'s component into
    /// the item's component's, seen at `target`, by the placements at the
    /// marker (those the model moves linked geometry by).
    fn to_target(&self, path: &[OccurrenceUid], target: &[OccurrenceUid]) -> Transform {
        inverse(&self.doc.path_transform(target)).after(&self.doc.path_transform(path))
    }

    /// The planar face of another component's body, seen through one of
    /// its occurrences, that a sketch of the item's component with `frame`
    /// lies on (see the module documentation): the definition's `plane`
    /// and `plane_link`, the face's frame in the sketch's component and
    /// an import note.
    pub(crate) fn linked_face_plane(
        &self,
        frame: &SketchFrame,
        points: &[(u32, [f64; 2])],
    ) -> Option<(Value, Value, SketchFrame, String)> {
        let target = self.sketch_target()?;
        let into = inverse(&self.doc.path_transform(&target));
        let mut planes: HashMap<BodyUid, Vec<(FaceName, mitcad_model::Plane)>> = HashMap::new();
        for (body, face, plane) in refs::planar_faces(self.doc) {
            planes.entry(body).or_default().push((face, plane));
        }
        let mut candidates = Vec::new();
        for instance in self.doc.instances() {
            if instance.component == self.component {
                continue;
            }
            let Some(faces) = planes.get(&instance.body) else {
                continue;
            };
            let t = into.after(&instance.transform);
            for (face, plane) in faces {
                let (origin, normal) = (t.apply_point(plane.origin), t.apply_vector(plane.normal));
                if geom::coplanar(frame, origin, normal, IN_PLANE) {
                    candidates.push(FaceCandidate {
                        path: instance.path.clone(),
                        body: instance.body,
                        face: face.clone(),
                        origin,
                        normal,
                    });
                }
            }
        }
        if crate::tracing() && !candidates.is_empty() {
            let n = geom::normal(frame);
            eprintln!(
                "import:   {} faces of other components in the sketch's plane",
                candidates.len()
            );
            for c in candidates.iter().filter(|c| geom::dot(c.normal, n) > 0.0) {
                eprintln!(
                    "import:     along its normal: {} {} at {}",
                    c.body,
                    c.face,
                    self.doc.assembly().path_name(&c.path)
                );
            }
        }
        // The sketch's points (at most 256 of them) in a candidate's body.
        let step = points.len().div_ceil(256).max(1);
        let kernel = self.doc.kernel();
        let mut points_on = |c: &FaceCandidate| -> usize {
            let Some(shape) = self.doc.body_shape(c.body) else {
                return 0;
            };
            let back = inverse(&into.after(&self.doc.path_transform(&c.path)));
            points
                .iter()
                .step_by(step)
                .filter(|(_, p)| {
                    let at = back.apply_point(frame.point(*p));
                    crate::tick();
                    kernel
                        .face_point_normal(shape, &c.face, at)
                        .is_ok_and(|near| geom::distance(near.point, at) <= IN_PLANE)
                })
                .count()
        };
        let c = plane_face(frame, candidates, &mut points_on)?;
        let f = SketchPlane::face_frame(c.origin, c.normal);
        let note = format!(
            "on a face of {} (its occurrence {})",
            self.doc.assembly().name(
                self.doc
                    .body_component(c.body)
                    .unwrap_or(ComponentUid::ROOT)
            ),
            self.doc.assembly().path_name(&c.path)
        );
        Some((
            json!({"face": c.face.to_string(), "body": c.body.to_string()}),
            link(&c.path, &target),
            f,
            note,
        ))
    }

    /// The edges a linked curve of a sketch of the item's component seen at
    /// `target` may follow: its own component's, and those of the other
    /// components' bodies through each of their occurrences, moved into
    /// its coordinates; bodies whose box does not come near `near` (the
    /// box of the sketch's points in `frame`, mm) are left out.
    pub(crate) fn edge_sources(
        &self,
        target: &[OccurrenceUid],
        frame: &SketchFrame,
        near: Option<([f64; 2], [f64; 2])>,
    ) -> Vec<EdgeSource> {
        let kernel = self.doc.kernel();
        let into = inverse(&self.doc.path_transform(target));
        let mut out = Vec::new();
        let mut edges_of: HashMap<BodyUid, Vec<(Option<String>, CurveGeometry)>> = HashMap::new();
        let mut read = |body: BodyUid| -> Option<Vec<(Option<String>, CurveGeometry)>> {
            if let Some(edges) = edges_of.get(&body) {
                return Some(edges.clone());
            }
            crate::tick();
            let edges = kernel.edge_geometries(self.doc.body_shape(body)?).ok()?;
            edges_of.insert(body, edges.clone());
            Some(edges)
        };
        for body in self.doc.component_bodies(self.component) {
            // (Unnamed edges too: the snap puts curves on them.)
            for (name, curve) in read(body).unwrap_or_default() {
                out.push(EdgeSource {
                    body,
                    edge: name.unwrap_or_default(),
                    path: None,
                    curve,
                });
            }
        }
        let comes_near = |shape: &K::Shape, t: &Transform| -> bool {
            let Some((low, high)) = near else {
                return true;
            };
            let Ok(Some(b)) = kernel.bounding_box(shape) else {
                return true;
            };
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for i in 0..8 {
                let corner =
                    std::array::from_fn(|k| if i >> k & 1 == 0 { b.min[k] } else { b.max[k] });
                let p = geom::to_sketch(frame, t.apply_point(corner));
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let margin = 1.0;
            (0..2).all(|k| hi[k] >= low[k] - margin && lo[k] <= high[k] + margin)
        };
        for instance in self.doc.instances() {
            if instance.component == self.component {
                continue;
            }
            let t = into.after(&instance.transform);
            let Some(shape) = self.doc.body_shape(instance.body) else {
                continue;
            };
            if !comes_near(shape, &t) {
                continue;
            }
            for (name, curve) in read(instance.body).unwrap_or_default() {
                if let Some(curve) = moved(&t, &curve) {
                    out.push(EdgeSource {
                        body: instance.body,
                        edge: name.unwrap_or_default(),
                        path: Some(instance.path.clone()),
                        curve,
                    });
                }
            }
        }
        out
    }

    /// The curves of an edge as the model's projection reads them, in the
    /// item's component's coordinates seen at `target`: one curve, else
    /// None.
    pub(crate) fn edge_curves(
        &self,
        source: &EdgeSource,
        target: &[OccurrenceUid],
    ) -> Option<Vec<Curve3>> {
        let name: TopoName = source.edge.parse().ok()?;
        let shape = self.doc.body_shape(source.body)?;
        let curves = self.doc.kernel().curves_of(shape, &name).ok()?;
        if curves.len() != 1 {
            return None;
        }
        Some(match &source.path {
            Some(path) => {
                let t = self.to_target(path, target);
                curves.iter().map(|c| transform_curve(&t, c)).collect()
            }
            None => curves,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(n: u64) -> BodyUid {
        format!("F{n}.b0").parse().unwrap()
    }

    fn face(name: &str) -> FaceName {
        name.parse().unwrap()
    }

    fn candidate(path: &[u32], z: f64, nz: f64) -> FaceCandidate {
        FaceCandidate {
            path: path.iter().map(|o| OccurrenceUid(*o)).collect(),
            body: body(2),
            face: face(if nz > 0.0 { "F2:end" } else { "F2:start" }),
            origin: [0.0, 0.0, z],
            normal: [0.0, 0.0, nz],
        }
    }

    fn at(z: f64) -> SketchFrame {
        SketchFrame {
            origin: [0.0, 0.0, z],
            ..SketchFrame::XY
        }
    }

    #[test]
    fn the_one_face_in_the_plane_looking_along_the_sketch() {
        let mut none = |_: &FaceCandidate| 0;
        // The board's top face (looking up) and a part's bottom face on it
        // (looking down): the top one.
        let up = candidate(&[1], 1.6, 1.0);
        let down = candidate(&[2], 1.6, -1.0);
        let below = candidate(&[1], 0.0, -1.0);
        assert_eq!(
            plane_face(
                &at(1.6),
                vec![down.clone(), up.clone(), below.clone()],
                &mut none
            ),
            Some(up.clone())
        );
        // Only one against the normal: that one.
        assert_eq!(
            plane_face(&at(0.0), vec![up.clone(), below.clone()], &mut none),
            Some(below)
        );
        // Two faces of different occurrences in the plane: the one holding
        // more of the sketch's points, none when neither holds more.
        let pad = candidate(&[3], 1.6, 1.0);
        let both = vec![up.clone(), pad.clone()];
        let mut board_points = |c: &FaceCandidate| if c.path == up.path { 40 } else { 2 };
        assert_eq!(
            plane_face(&at(1.6), both.clone(), &mut board_points),
            Some(up.clone())
        );
        let mut same = |_: &FaceCandidate| 3;
        assert_eq!(plane_face(&at(1.6), both.clone(), &mut same), None);
        assert_eq!(plane_face(&at(1.6), both, &mut none), None);
        assert_eq!(plane_face(&at(5.0), vec![up], &mut none), None);
    }

    #[test]
    fn links_name_both_paths() {
        assert_eq!(
            link(&[OccurrenceUid(1), OccurrenceUid(4)], &[]),
            json!({"source": "O1/O4"})
        );
        assert_eq!(
            link(&[OccurrenceUid(2)], &[OccurrenceUid(3)]),
            json!({"source": "O2", "target": "O3"})
        );
    }

    fn parts(entities: Vec<Value>) -> SketchParts {
        SketchParts {
            entities,
            linked: 3,
            ..SketchParts::default()
        }
    }

    fn line_edge(
        body_n: u64,
        name: &str,
        a: [f64; 3],
        b: [f64; 3],
        path: Option<Vec<u32>>,
    ) -> EdgeSource {
        EdgeSource {
            body: body(body_n),
            edge: name.to_owned(),
            path: path.map(|p| p.into_iter().map(OccurrenceUid).collect()),
            curve: CurveGeometry::Line { start: a, end: b },
        }
    }

    fn curve_of(edge: &EdgeSource) -> Option<Vec<Curve3>> {
        match &edge.curve {
            CurveGeometry::Line { start, end } => Some(vec![Curve3::Line {
                start: *start,
                end: *end,
            }]),
            CurveGeometry::Circle {
                center,
                normal,
                radius,
                ..
            } => Some(vec![Curve3::Conic {
                center: *center,
                normal: *normal,
                x_axis: [1.0, 0.0, 0.0],
                major: *radius,
                minor: *radius,
                start: 0.0,
                end: std::f64::consts::TAU,
                closed: true,
            }]),
            CurveGeometry::Other { .. } => None,
        }
    }

    #[test]
    fn linked_curves_on_one_edge_each_become_projections() {
        let point = |id: &str, x: f64, y: f64| json!({"id": id, "type": "point", "at": [x, y], "fixed": true, "reference": true});
        let mut p = parts(vec![
            point("p1", 0.0, 0.0),
            point("p2", 10.0, 0.0),
            point("p3", 10.0, 5.0),
            point("p4", 20.0, 20.0),
            json!({"id": "c5", "type": "line", "start": "p1", "end": "p2",
                   "fixed": true, "reference": true}),
            // Drawn from (10, 5) to (10, 0); the edge runs up.
            json!({"id": "c6", "type": "line", "start": "p3", "end": "p2",
                   "fixed": true, "reference": true}),
            json!({"id": "c7", "type": "circle", "center": "p4", "radius": 3.0,
                   "fixed": true, "reference": true}),
            // Two edges of other bodies project onto it: kept fixed.
            point("p8", 0.0, 30.0),
            point("p9", 5.0, 30.0),
            json!({"id": "c10", "type": "line", "start": "p8", "end": "p9",
                   "fixed": true, "reference": true}),
        ]);
        let frame = at(1.6);
        let edges = vec![
            // The sketch's own face's edge, and one under it (a wall's
            // bottom edge) that projects onto the same line.
            line_edge(1, "E1", [0.0, 0.0, 1.6], [10.0, 0.0, 1.6], None),
            line_edge(1, "E2", [0.0, 0.0, 0.0], [10.0, 0.0, 0.0], None),
            // Another component's edge, running up.
            line_edge(3, "E3", [10.0, 0.0, 1.6], [10.0, 5.0, 1.6], Some(vec![2])),
            EdgeSource {
                body: body(3),
                edge: "E4".to_owned(),
                path: Some(vec![OccurrenceUid(2)]),
                curve: CurveGeometry::Circle {
                    center: [20.0, 20.0, 1.6],
                    normal: [0.0, 0.0, 1.0],
                    radius: 3.0,
                    start: [23.0, 20.0, 1.6],
                    end: [23.0, 20.0, 1.6],
                },
            },
            line_edge(4, "E5", [0.0, 30.0, 0.0], [5.0, 30.0, 0.0], None),
            line_edge(5, "E6", [0.0, 30.0, 9.0], [5.0, 30.0, 9.0], None),
        ];
        let c = choices(&p, &frame, &edges);
        let r = resolve(&mut p, &frame, &edges, &c, &[], &mut curve_of);
        assert_eq!(r.projections.len(), 3, "{:?}", r.projections);
        assert_eq!(r.other, 2);
        assert_eq!(
            r.projections[0],
            json!({"source": "E1", "body": "F1.b0", "entities": ["p1", "p2", "c5"]})
        );
        // The line is turned to run as its edge.
        assert_eq!(
            r.projections[1],
            json!({"source": "E3", "body": "F3.b0", "entities": ["p2", "p3", "c6"],
                   "link": {"source": "O2"}})
        );
        assert_eq!(
            (&p.entities[5]["start"], &p.entities[5]["end"]),
            (&json!("p2"), &json!("p3"))
        );
        assert_eq!(
            r.projections[2],
            json!({"source": "E4", "body": "F3.b0", "entities": ["p4", "c7"],
                   "link": {"source": "O2"}})
        );
    }

    #[test]
    fn a_curve_off_its_edge_stays_fixed() {
        let mut p = parts(vec![
            json!({"id": "p1", "type": "point", "at": [0.0, 0.001], "reference": true}),
            json!({"id": "p2", "type": "point", "at": [10.0, 0.0], "reference": true}),
            json!({"id": "c3", "type": "line", "start": "p1", "end": "p2", "reference": true}),
        ]);
        let edges = vec![line_edge(1, "E1", [0.0, 0.0, 0.0], [10.0, 0.0, 0.0], None)];
        // Within the snap's reach but not on the projection: not snapped,
        // so no projection.
        let c = choices(&p, &SketchFrame::XY, &edges);
        let r = resolve(&mut p, &SketchFrame::XY, &edges, &c, &[], &mut curve_of);
        assert!(r.projections.is_empty());
    }

    #[test]
    fn the_note_says_what_became_projections_and_why_the_rest_did_not() {
        let resolved = Resolved {
            projections: vec![json!({}); 3],
            other: 2,
        };
        assert_eq!(
            notes(5, &resolved, true, true, 4, 0),
            [
                "3 linked curves as projections (2 of other components)",
                "2 linked curves kept as fixed reference geometry (no single edge of the replay \
                 they follow; 4 on the model's edges)"
            ]
        );
        assert_eq!(
            notes(5, &resolved, true, true, 3, 1),
            [
                "3 linked curves as projections (2 of other components)",
                "2 linked curves kept as fixed reference geometry (no single edge of the replay \
                 they follow; 3 on the model's edges; 1 left where the file stored them: a snap \
                 onto the model's edge would change the sketch's regions)"
            ]
        );
        assert_eq!(
            notes(5, &resolved, false, true, 0, 0),
            [
                "5 linked curves kept as fixed reference geometry (the sketch does not solve with \
              them as projections)"
            ]
        );
        // A copy in another component: not looked for.
        assert_eq!(
            notes(2, &Resolved::default(), false, false, 0, 0),
            ["2 linked curves kept as fixed reference geometry"]
        );
    }
}
