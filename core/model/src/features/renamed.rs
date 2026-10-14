// SPDX-License-Identifier: MIT
//! Faces, edges and vertices found again after a sketch change renamed
//! them (mitcad#272).
//!
//! A side face is named by its sketch segment: the curve and the curves it
//! ends on (`F3:side(c1[c4,c2])`). When a sketch edit gives the segment
//! another neighbour, the face gets a new name although it is the same
//! face, and every reference to it would fail. Profile regions
//! (`sketch/regions.rs` `best_match`) and face appearances
//! (`document/bodies.rs`) already find such a face again; this module is
//! the one rule for features, used wherever a feature resolves a face,
//! edge or vertex name on a body (`docs/architecture.md`, "Topological
//! naming"):
//!
//! 1. A name that finds what it names is used as it is (a face name
//!    without `#k` finds every piece).
//! 2. A face name that finds no face finds the faces of the same feature
//!    and role whose key has most in common with it ([`likeness`]): a side
//!    face the same own curve and most of its end curves, a region most of
//!    its curves. A reference takes the best match only when it is the
//!    only one; a tie is an error that lists the candidates.
//! 3. An edge name that finds no edge finds its two faces each by rule 2
//!    and takes the edges between them: one with `#k`, all without. Of
//!    tied faces only those that meet the other face count.
//! 4. A vertex finds its faces each by rule 2.
//! 5. The stored name stays as it is; the feature warns that it found the
//!    face, edge or vertex under another name.

use std::str::FromStr;
use std::sync::Arc;

use super::face_refs::missing_face;
use super::{EvalContext, FeatureDef, FeatureEntry};
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::{Kernel, KernelError};
use crate::topo::{CurveId, EdgeName, FaceName, RoleKey, SegmentKey, TopoName, VertexName};

/// How much a face's name has in common with a reference that names no
/// face, best last: the curves the keys share and, as a tie-break, the
/// curves at the same end of a segment. None unless the feature and the
/// role are the same and the keys share a curve (a side face: its own
/// curve). Keys that hold names (a fillet's edge, a shell's face) compare
/// those names by the same rule.
pub(crate) fn likeness(reference: &FaceName, name: &FaceName) -> Option<(usize, usize)> {
    if reference.feature != name.feature || reference.role != name.role {
        return None;
    }
    key_likeness(&reference.key, &name.key)
}

fn key_likeness(reference: &Option<RoleKey>, name: &Option<RoleKey>) -> Option<(usize, usize)> {
    match (reference, name) {
        (None, None) => Some((1, 0)),
        (Some(RoleKey::Segment(a)), Some(RoleKey::Segment(b))) => {
            if a.curve != b.curve {
                return None;
            }
            let a_curves: Vec<CurveId> = a.curves().collect();
            let b_curves: Vec<CurveId> = b.curves().collect();
            Some((shared(&a_curves, &b_curves), ends_in_place(a, b)))
        }
        (Some(RoleKey::Region(a)), Some(RoleKey::Region(b))) => {
            let a: Vec<CurveId> = a.segments().flat_map(|s| s.curves()).collect();
            let b: Vec<CurveId> = b.segments().flat_map(|s| s.curves()).collect();
            let shared = shared(&a, &b);
            (shared > 0).then_some((shared, 0))
        }
        (Some(RoleKey::Face(a)), Some(RoleKey::Face(b))) => nested_likeness(a, b),
        (Some(RoleKey::Edge(a)), Some(RoleKey::Edge(b))) => edge_likeness(a, b),
        (Some(RoleKey::Vertex(a)), Some(RoleKey::Vertex(b))) => vertex_likeness(a, b),
        (Some(RoleKey::Index(a)), Some(RoleKey::Index(b))) if a == b => Some((1, 0)),
        _ => None,
    }
}

/// The curves of `a` that `b` has too, counted as `a` lists them.
fn shared(a: &[CurveId], b: &[CurveId]) -> usize {
    a.iter().filter(|c| b.contains(c)).count()
}

/// The curves two segments of one curve have at the same end.
fn ends_in_place(a: &SegmentKey, b: &SegmentKey) -> usize {
    match (&a.ends, &b.ends) {
        (Some(a), Some(b)) => shared(&a.0, &b.0) + shared(&a.1, &b.1),
        _ => 0,
    }
}

/// A name inside a key: the same piece of a face like it.
fn nested_likeness(a: &FaceName, b: &FaceName) -> Option<(usize, usize)> {
    if a.split != b.split {
        return None;
    }
    likeness(a, b)
}

fn add(a: (usize, usize), b: (usize, usize)) -> (usize, usize) {
    (a.0 + b.0, a.1 + b.1)
}

fn edge_likeness(a: &EdgeName, b: &EdgeName) -> Option<(usize, usize)> {
    if a.index != b.index {
        return None;
    }
    let [a0, a1] = a.faces();
    let [b0, b1] = b.faces();
    let pair = |x: &FaceName, y: &FaceName, z: &FaceName, w: &FaceName| {
        Some(add(nested_likeness(x, y)?, nested_likeness(z, w)?))
    };
    // Renaming may change the faces' order in the name.
    pair(a0, b0, a1, b1).max(pair(a0, b1, a1, b0))
}

fn vertex_likeness(a: &VertexName, b: &VertexName) -> Option<(usize, usize)> {
    if a.index != b.index || a.faces().len() != b.faces().len() {
        return None;
    }
    let mut total = (0, 0);
    for face in a.faces() {
        let best = b
            .faces()
            .iter()
            .filter_map(|other| nested_likeness(face, other))
            .max()?;
        total = add(total, best);
    }
    Some(total)
}

/// The faces `reference` finds among the faces (each face's names), as
/// face appearances take them: those it names (all pieces without `#k`),
/// else those of the same feature and role most like it, every one of
/// them (each face's index and the name of it that it finds).
pub(crate) fn faces_found<'a>(
    faces: &'a [Vec<FaceName>],
    reference: &FaceName,
) -> Vec<(usize, &'a FaceName)> {
    let exact: Vec<(usize, &FaceName)> = faces
        .iter()
        .enumerate()
        .filter_map(|(i, names)| names.iter().find(|n| n.matches(reference)).map(|n| (i, n)))
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    let scored: Vec<((usize, usize), usize, &FaceName)> = faces
        .iter()
        .enumerate()
        .filter_map(|(i, names)| {
            names
                .iter()
                .filter_map(|n| likeness(reference, n).map(|score| (score, i, n)))
                .max_by_key(|(score, _, _)| *score)
        })
        .collect();
    let Some(best) = scored.iter().map(|(score, _, _)| *score).max() else {
        return Vec::new();
    };
    scored
        .into_iter()
        .filter(|(score, _, _)| *score == best)
        .map(|(_, i, n)| (i, n))
        .collect()
}

/// The names a reference that names no face may stand for (rule 2): the
/// names, in text order, of the faces most like it, cut to the reference's
/// piece numbers (so that the pieces of one renamed face count once).
fn renamed_candidates(faces: &[Vec<FaceName>], reference: &FaceName) -> Vec<FaceName> {
    let mut best: Option<(usize, usize)> = None;
    let mut found: Vec<FaceName> = Vec::new();
    for name in faces.iter().flatten() {
        if !name.split.starts_with(&reference.split) {
            continue;
        }
        let Some(score) = likeness(reference, name) else {
            continue;
        };
        let mut base = name.clone();
        base.split.truncate(reference.split.len());
        match best {
            Some(b) if score < b => {}
            Some(b) if score == b => {
                if !found.contains(&base) {
                    found.push(base);
                }
            }
            _ => {
                best = Some(score);
                found = vec![base];
            }
        }
    }
    found.sort();
    found
}

/// The names of the shape's faces, each face's together; none when the
/// kernel cannot list them.
fn face_names<K: Kernel>(kernel: &K, shape: &K::Shape) -> Vec<Vec<FaceName>> {
    kernel
        .faces(shape)
        .unwrap_or_default()
        .into_iter()
        .map(|face| {
            face.names
                .iter()
                .filter_map(|n| FaceName::from_str(n).ok())
                .collect()
        })
        .collect()
}

/// Why a name finds nothing a feature can use.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Lost {
    /// Nothing by the name and nothing like it: the caller explains it, as
    /// before (`missing_face`, `missing_edge`).
    Gone,
    /// Several equally like it after a sketch change: the message lists
    /// them.
    Ambiguous(String),
    /// The kernel failed to look.
    Failed(String),
}

impl Lost {
    /// The message, with `gone` for a name that is gone.
    pub(crate) fn message(self, gone: impl FnOnce() -> String) -> String {
        match self {
            Self::Gone => gone(),
            Self::Ambiguous(message) | Self::Failed(message) => message,
        }
    }
}

fn ambiguous(name: &dyn std::fmt::Display, candidates: &[String]) -> Lost {
    Lost::Ambiguous(format!(
        "{name} is ambiguous after a sketch change: {}",
        candidates.join(", ")
    ))
}

/// The faces a face name of a feature reference can stand for on the
/// shape: itself when it finds a face (or the kernel cannot tell), else
/// the faces most like it.
struct Candidates {
    exact: bool,
    faces: Vec<FaceName>,
}

/// Looks the shape's faces up once, on the first name that needs them.
struct Faces<'a, K: Kernel> {
    kernel: &'a K,
    shape: &'a K::Shape,
    listed: Option<Vec<Vec<FaceName>>>,
}

impl<'a, K: Kernel> Faces<'a, K> {
    fn new(kernel: &'a K, shape: &'a K::Shape) -> Self {
        Self {
            kernel,
            shape,
            listed: None,
        }
    }

    fn candidates(&mut self, face: &FaceName) -> Result<Candidates, Lost> {
        match self.kernel.count_faces(self.shape, face) {
            Ok(0) => {}
            Ok(_) | Err(KernelError::Unsupported(_)) => {
                return Ok(Candidates {
                    exact: true,
                    faces: vec![face.clone()],
                });
            }
            Err(error) => return Err(Lost::Failed(error.to_string())),
        }
        let (kernel, shape) = (self.kernel, self.shape);
        let listed = self.listed.get_or_insert_with(|| face_names(kernel, shape));
        Ok(Candidates {
            exact: false,
            faces: renamed_candidates(listed, face),
        })
    }

    fn has_edge(&self, a: &FaceName, b: &FaceName, index: Option<u32>) -> Option<EdgeName> {
        let mut edge = EdgeName::new(a.clone(), b.clone());
        edge.index = index;
        (self.kernel.count_edges(self.shape, &edge) > 0).then_some(edge)
    }
}

fn texts<T: ToString>(names: &[T]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

/// Rule 2 for a face: None when the name finds its face (or the kernel
/// cannot tell), else the one face most like it.
pub(crate) fn find_face<K: Kernel>(
    kernel: &K,
    shape: &K::Shape,
    face: &FaceName,
) -> Result<Option<FaceName>, Lost> {
    let found = Faces::new(kernel, shape).candidates(face)?;
    if found.exact {
        return Ok(None);
    }
    match found.faces.as_slice() {
        [] => Err(Lost::Gone),
        [one] => Ok(Some(one.clone())),
        several => Err(ambiguous(face, &texts(several))),
    }
}

/// Rule 3 for an edge: None when the name finds its edges, else the edge
/// name between the faces its faces find.
pub(crate) fn find_edge<K: Kernel>(
    kernel: &K,
    shape: &K::Shape,
    edge: &EdgeName,
) -> Result<Option<EdgeName>, Lost> {
    if kernel.count_edges(shape, edge) > 0 {
        return Ok(None);
    }
    let mut faces = Faces::new(kernel, shape);
    let [a, b] = edge.faces();
    let a = faces.candidates(a)?;
    let b = faces.candidates(b)?;
    if a.exact && b.exact {
        return Err(Lost::Gone);
    }
    let mut found: Vec<EdgeName> = Vec::new();
    for x in &a.faces {
        for y in &b.faces {
            if let Some(between) = faces.has_edge(x, y, edge.index)
                && !found.contains(&between)
            {
                found.push(between);
            }
        }
    }
    found.sort();
    match found.as_slice() {
        [] => Err(Lost::Gone),
        [one] => Ok(Some(one.clone())),
        several => Err(ambiguous(edge, &texts(several))),
    }
}

/// Rule 4 for a vertex: None when every face of its name finds its face,
/// else the vertex of the faces found. Of tied faces only the one that
/// meets another face of the vertex counts.
pub(crate) fn find_vertex<K: Kernel>(
    kernel: &K,
    shape: &K::Shape,
    vertex: &VertexName,
) -> Result<Option<VertexName>, Lost> {
    let mut faces = Faces::new(kernel, shape);
    let mut found = Vec::with_capacity(vertex.faces().len());
    for face in vertex.faces() {
        found.push(faces.candidates(face)?);
    }
    if found.iter().all(|c| c.exact) {
        return Ok(None);
    }
    let settled: Vec<FaceName> = found
        .iter()
        .filter(|c| c.faces.len() == 1)
        .map(|c| c.faces[0].clone())
        .collect();
    let mut chosen = Vec::with_capacity(found.len());
    for (face, candidates) in vertex.faces().iter().zip(&found) {
        match candidates.faces.as_slice() {
            [] => return Err(Lost::Gone),
            [one] => chosen.push(one.clone()),
            several => {
                let near: Vec<&FaceName> = several
                    .iter()
                    .filter(|x| settled.iter().any(|s| faces.has_edge(x, s, None).is_some()))
                    .collect();
                match near.as_slice() {
                    [one] => chosen.push((*one).clone()),
                    _ => return Err(ambiguous(face, &texts(several))),
                }
            }
        }
    }
    let mut renamed = VertexName::new(chosen).ok_or(Lost::Gone)?;
    renamed.index = vertex.index;
    Ok(Some(renamed))
}

/// Rules 2 to 4 for any name: None when it finds what it names, else the
/// name it is found as, with what it is (`face`, `edge`, `vertex`) and
/// the feature that made the renamed face.
pub(crate) fn find_name<K: Kernel>(
    kernel: &K,
    shape: &K::Shape,
    name: &TopoName,
) -> Result<Option<(TopoName, &'static str, FeatureUid)>, Lost> {
    Ok(match name {
        TopoName::Face(face) => find_face(kernel, shape, face)?
            .map(|found| (TopoName::Face(found), "face", face.feature)),
        TopoName::Edge(edge) => find_edge(kernel, shape, edge)?.map(|found| {
            let maker =
                renamed_maker(edge.faces(), found.faces()).unwrap_or(edge.faces()[0].feature);
            (TopoName::Edge(found), "edge", maker)
        }),
        TopoName::Vertex(vertex) => find_vertex(kernel, shape, vertex)?.map(|found| {
            let maker =
                renamed_maker(vertex.faces(), found.faces()).unwrap_or(vertex.faces()[0].feature);
            (TopoName::Vertex(found), "vertex", maker)
        }),
    })
}

/// The feature that made the first face of `stored` that `found` renamed
/// (the face whose sketch changed).
fn renamed_maker<'a>(
    stored: impl IntoIterator<Item = &'a FaceName>,
    found: &[FaceName],
) -> Option<FeatureUid> {
    stored
        .into_iter()
        .find(|face| !found.contains(face))
        .map(|face| face.feature)
}

/// The sketch whose change renamed faces of `maker`: the sketch of its
/// profiles, else a sketch it refers to; else `maker` itself (by name).
fn changed_sketch(features: &[Arc<FeatureEntry>], maker: FeatureUid) -> String {
    let name = |uid: FeatureUid| {
        features
            .iter()
            .find(|f| f.uid == uid)
            .map_or_else(|| uid.to_string(), |f| f.name.clone())
    };
    let Some(entry) = features.iter().find(|f| f.uid == maker) else {
        return name(maker);
    };
    let info = entry.def.info();
    let sketch = info.profiles().first().map(|p| p.sketch).or_else(|| {
        info.references().features.into_iter().find(|uid| {
            features
                .iter()
                .any(|f| f.uid == *uid && matches!(f.def, FeatureDef::Sketch(_)))
        })
    });
    name(sketch.unwrap_or(maker))
}

/// The warning of a feature that found `stored` as `found` (rule 5), e.g.
/// `edge E{…} found as E{…} after a change in Sketch10`; the timeline
/// shows it after the feature's name.
pub(crate) fn warning(
    features: &[Arc<FeatureEntry>],
    what: &str,
    stored: &dyn std::fmt::Display,
    found: &dyn std::fmt::Display,
    maker: FeatureUid,
) -> String {
    let sketch = changed_sketch(features, maker);
    format!("{what} {stored} found as {found} after a change in {sketch}")
}

/// Names of features as features resolve them: what a feature reads is
/// recorded, and a name found by the fallback leaves a warning.
impl<K: Kernel> EvalContext<'_, K> {
    /// Warns that `stored` was found as `found` (rule 5).
    pub(crate) fn found_renamed(
        &mut self,
        what: &str,
        stored: &dyn std::fmt::Display,
        found: &dyn std::fmt::Display,
        maker: FeatureUid,
    ) {
        let message = warning(self.env.features, what, stored, found, maker);
        self.warn(message);
    }

    /// The name the kernel gets for a face of the body (rules 1, 2 and 5);
    /// a face that is gone is explained as before.
    pub fn face_on(
        &mut self,
        body: BodyUid,
        shape: &K::Shape,
        face: &FaceName,
    ) -> Result<FaceName, String> {
        match find_face(self.kernel, shape, face) {
            Ok(None) => Ok(face.clone()),
            Ok(Some(found)) => {
                self.found_renamed("face", face, &found, face.feature);
                Ok(found)
            }
            Err(lost) => Err(lost.message(|| missing_face(&*self, body, face))),
        }
    }

    /// [`EvalContext::face_on`] with the caller's message for a face that
    /// is gone.
    pub(crate) fn face_or(
        &mut self,
        shape: &K::Shape,
        face: &FaceName,
        gone: impl FnOnce() -> String,
    ) -> Result<FaceName, String> {
        match find_face(self.kernel, shape, face) {
            Ok(None) => Ok(face.clone()),
            Ok(Some(found)) => {
                self.found_renamed("face", face, &found, face.feature);
                Ok(found)
            }
            Err(lost) => Err(lost.message(gone)),
        }
    }

    /// [`EvalContext::face_on`] for each face.
    pub fn faces_on(
        &mut self,
        body: BodyUid,
        shape: &K::Shape,
        faces: &[FaceName],
    ) -> Result<Vec<FaceName>, String> {
        faces
            .iter()
            .map(|face| self.face_on(body, shape, face))
            .collect()
    }

    /// The name the kernel gets for an edge of the body (rules 1, 3 and 5);
    /// an edge that is gone is explained as before.
    pub fn edge_on(
        &mut self,
        body: BodyUid,
        shape: &K::Shape,
        edge: &EdgeName,
    ) -> Result<EdgeName, String> {
        match find_edge(self.kernel, shape, edge) {
            Ok(None) => Ok(edge.clone()),
            Ok(Some(found)) => {
                let maker =
                    renamed_maker(edge.faces(), found.faces()).unwrap_or(edge.faces()[0].feature);
                self.found_renamed("edge", edge, &found, maker);
                Ok(found)
            }
            Err(lost) => Err(lost.message(|| self.missing_edge(body, edge))),
        }
    }

    /// [`EvalContext::edge_on`] for each edge.
    pub fn edges_on(
        &mut self,
        body: BodyUid,
        shape: &K::Shape,
        edges: &[EdgeName],
    ) -> Result<Vec<EdgeName>, String> {
        edges
            .iter()
            .map(|edge| self.edge_on(body, shape, edge))
            .collect()
    }

    /// The name the kernel gets for a vertex of the body (rules 1, 4 and
    /// 5); a vertex whose faces are gone is left to the kernel, as before.
    pub fn vertex_on(
        &mut self,
        shape: &K::Shape,
        vertex: &VertexName,
    ) -> Result<VertexName, String> {
        match find_vertex(self.kernel, shape, vertex) {
            Ok(None) | Err(Lost::Gone) => Ok(vertex.clone()),
            Ok(Some(found)) => {
                let maker = renamed_maker(vertex.faces(), found.faces())
                    .unwrap_or(vertex.faces()[0].feature);
                self.found_renamed("vertex", vertex, &found, maker);
                Ok(found)
            }
            Err(lost) => Err(lost.message(String::new)),
        }
    }
}

/// The same rules where a reference resolves through
/// [`super::geom_ref::Resolver`] (planes, axes and points on faces, edges
/// and vertices): a name that is gone goes to the kernel as it is, whose
/// error says so.
pub(crate) fn face_for<R: super::geom_ref::Resolver>(
    r: &mut R,
    shape: &<R::Kernel as Kernel>::Shape,
    face: &FaceName,
) -> Result<FaceName, String> {
    match find_face(r.kernel(), shape, face) {
        Ok(None) | Err(Lost::Gone) => Ok(face.clone()),
        Ok(Some(found)) => {
            r.renamed("face", &face.to_string(), &found.to_string(), face.feature);
            Ok(found)
        }
        Err(lost) => Err(lost.message(String::new)),
    }
}

pub(crate) fn edge_for<R: super::geom_ref::Resolver>(
    r: &mut R,
    shape: &<R::Kernel as Kernel>::Shape,
    edge: &EdgeName,
) -> Result<EdgeName, String> {
    match find_edge(r.kernel(), shape, edge) {
        Ok(None) | Err(Lost::Gone) => Ok(edge.clone()),
        Ok(Some(found)) => {
            let maker =
                renamed_maker(edge.faces(), found.faces()).unwrap_or(edge.faces()[0].feature);
            r.renamed("edge", &edge.to_string(), &found.to_string(), maker);
            Ok(found)
        }
        Err(lost) => Err(lost.message(String::new)),
    }
}

pub(crate) fn vertex_for<R: super::geom_ref::Resolver>(
    r: &mut R,
    shape: &<R::Kernel as Kernel>::Shape,
    vertex: &VertexName,
) -> Result<VertexName, String> {
    match find_vertex(r.kernel(), shape, vertex) {
        Ok(None) | Err(Lost::Gone) => Ok(vertex.clone()),
        Ok(Some(found)) => {
            let maker =
                renamed_maker(vertex.faces(), found.faces()).unwrap_or(vertex.faces()[0].feature);
            r.renamed("vertex", &vertex.to_string(), &found.to_string(), maker);
            Ok(found)
        }
        Err(lost) => Err(lost.message(String::new)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(text: &str) -> FaceName {
        text.parse().unwrap()
    }

    #[test]
    fn a_side_face_needs_its_own_curve_and_prefers_ends_in_place() {
        let reference = face("F3:side(c71[c104,c72])");
        assert_eq!(likeness(&reference, &face("F3:side(c72[c71,c73])")), None);
        assert_eq!(likeness(&reference, &face("F4:side(c71[c104,c72])")), None);
        assert_eq!(likeness(&reference, &face("F3:end(r{c71})")), None);
        // c71 and c104 shared; c104 at the start in both.
        assert_eq!(
            likeness(&reference, &face("F3:side(c71[c104,c103])")),
            Some((2, 1))
        );
        // The same curves, c104 at the other end.
        assert_eq!(
            likeness(&reference, &face("F3:side(c71[c103,c104])")),
            Some((2, 0))
        );
    }

    #[test]
    fn names_in_keys_follow_the_same_rule() {
        let reference = face("F5:fillet(E{F3:side(c1[c4,c2])|F3:side(c2[c1,c3])})");
        let renamed = face("F5:fillet(E{F3:side(c1[c4,c9])|F3:side(c2[c9,c3])})");
        assert!(likeness(&reference, &renamed).is_some());
        let other = face("F5:fillet(E{F3:side(c3[c2,c4])|F3:side(c4[c3,c1])})");
        assert_eq!(likeness(&reference, &other), None);
        let shell = face("F6:offset(F3:side(c1[c4,c2]))");
        assert!(likeness(&shell, &face("F6:offset(F3:side(c1[c4,c9]))")).is_some());
        assert_eq!(
            likeness(&shell, &face("F6:offset(F3:side(c1[c4,c9])#1)")),
            None
        );
    }

    #[test]
    fn candidates_are_unique_faces_most_like_the_reference() {
        let faces = |names: &[&str]| -> Vec<Vec<FaceName>> {
            names.iter().map(|n| vec![face(n)]).collect()
        };
        let reference = face("F3:side(c1[c4,c2])");
        // The pieces of one renamed face count once.
        let pieces = faces(&[
            "F3:side(c1[c4,c9])#0",
            "F3:side(c1[c4,c9])#1",
            "F3:end(r{c1})",
        ]);
        assert_eq!(
            renamed_candidates(&pieces, &reference),
            vec![face("F3:side(c1[c4,c9])")]
        );
        // Two pieces of the line, each as like it: both listed.
        let split = faces(&["F3:side(c1[c5,c7])", "F3:side(c1[c7,c6])"]);
        assert_eq!(renamed_candidates(&split, &reference).len(), 2);
        // A piece number of the reference must be kept.
        let piece = face("F3:side(c1[c4,c2])#1");
        assert!(renamed_candidates(&pieces, &piece).len() == 1);
        assert!(renamed_candidates(&faces(&["F3:side(c1[c4,c9])#0"]), &piece).is_empty());
    }
}
