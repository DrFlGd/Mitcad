// SPDX-License-Identifier: MIT
//! The planar faces of the bodies at the marker, for sketch and
//! construction planes, and the planar faces and edges an import keeps by
//! body version while it runs ([`KeptPlanes`]).

use std::sync::Arc;

use mitcad_model::{BodyUid, Document, EdgeName, FaceName, Kernel, Plane};
use mitcad_vec3::Vec3;

/// Every planar face of the bodies at the marker with its plane (outward
/// normal). Each body's are kept by its version ([`planes_of`]).
pub fn planar_faces<K: Kernel>(doc: &Document<K>) -> Vec<(BodyUid, FaceName, Plane)> {
    let kernel = doc.kernel();
    let mut out = Vec::new();
    for body in crate::body_scope::bodies(doc) {
        // Large bodies have thousands of faces, a kernel call each
        // (mitcad#82: the watchdog's progress).
        crate::tick();
        let planes = match doc.body_version(body.uid) {
            Some(version) => planes_of(version, || body_planes(kernel, body.shape)),
            None => Arc::new(body_planes(kernel, body.shape)),
        };
        out.extend(
            planes
                .iter()
                .map(|(name, plane)| (body.uid, name.clone(), *plane)),
        );
    }
    out
}

// Planar faces kept by the body's version (mitcad#103).

/// The planar faces of a body's shape with their planes.
fn body_planes<K: Kernel>(kernel: &K, shape: &K::Shape) -> Vec<(FaceName, Plane)> {
    let Ok(faces) = kernel.faces(shape) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for face in faces.iter().filter(|f| f.surface == "plane") {
        let Some(name) = face.names.first().and_then(|n| n.parse::<FaceName>().ok()) else {
            continue;
        };
        crate::tick();
        if let Ok(plane) = kernel.face_plane(shape, &name) {
            out.push((name, plane));
        }
    }
    out
}

/// How many bodies' planar faces an import keeps ([`planes_of`]).
const PLANES_KEPT: usize = 64;

pub type Planes = Arc<Vec<(FaceName, Plane)>>;

/// How many edges of bodies an import keeps ([`edges_of`]), the oldest
/// bodies' going first: a large body's edges take megabytes, and each
/// lookup reads the edges of every body (with fewer kept than a design's
/// bodies, each lookup found every body's edges again).
const EDGES_KEPT: usize = 2_000_000;

/// A named edge of a body with its middle, unit tangent there, length and
/// ends where known (`mitcad_import`'s `refs::ReplayEdges`).
pub type BodyEdge = (EdgeName, Vec3, Vec3, f64, Option<[Vec3; 2]>);

/// A body's named edges.
pub type BodyEdges = Arc<Vec<BodyEdge>>;

std::thread_local! {
    /// The planar faces found by the import running on this thread, by
    /// body version, the latest last ([`KeptPlanes`]).
    static KEPT: std::cell::RefCell<Option<Vec<(u128, Planes)>>> =
        const { std::cell::RefCell::new(None) };
    /// The bodies' edges likewise (mitcad#119).
    static KEPT_EDGES: std::cell::RefCell<Option<Vec<(u128, BodyEdges)>>> =
        const { std::cell::RefCell::new(None) };
}

/// Keeps the planar faces and the edges found on this thread until
/// dropped (`mitcad_import::import_design`): every sketch looks for its plane
/// among the faces of all bodies, and on bodies of thousands of faces that
/// took seconds a sketch while the bodies stayed the same (mitcad#103);
/// every rounding found its edges among those of all bodies, which on a
/// design with bodies of 60 000 edges took a third of a second each time
/// (mitcad#119). A version names one shape of the document (its feature,
/// definition and inputs).
pub struct KeptPlanes(bool);

impl KeptPlanes {
    // A guard, made where the keeping starts: no `Default`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let outer = KEPT.with(|k| k.borrow_mut().replace(Vec::new()).is_none());
        if outer {
            KEPT_EDGES.with(|k| k.borrow_mut().replace(Vec::new()));
        }
        KeptPlanes(outer)
    }
}

impl Drop for KeptPlanes {
    fn drop(&mut self) {
        // (An import inside an import keeps the outer one's.)
        if self.0 {
            KEPT.with(|k| k.borrow_mut().take());
            KEPT_EDGES.with(|k| k.borrow_mut().take());
        }
    }
}

/// Drops the edges kept on this thread (when memory gets tight); they are
/// found again when asked for.
pub fn release_kept_edges() {
    KEPT_EDGES.with(|k| {
        if let Some(kept) = k.borrow_mut().as_mut() {
            kept.clear();
        }
    });
}

/// The edges of the body whose shape has `version`: kept ones, or found by
/// `find` (and kept while [`KeptPlanes`] is).
pub fn edges_of(version: u128, find: impl FnOnce() -> Vec<BodyEdge>) -> BodyEdges {
    let kept = KEPT_EDGES.with(|k| {
        let mut k = k.borrow_mut();
        let kept = k.as_mut()?;
        let i = kept.iter().position(|(v, _)| *v == version)?;
        let entry = kept.remove(i);
        let edges = Arc::clone(&entry.1);
        kept.push(entry);
        Some(edges)
    });
    if let Some(edges) = kept {
        return edges;
    }
    let edges = Arc::new(find());
    KEPT_EDGES.with(|k| {
        if let Some(kept) = k.borrow_mut().as_mut() {
            kept.push((version, Arc::clone(&edges)));
            while kept.len() > 1 && kept.iter().map(|(_, e)| e.len()).sum::<usize>() > EDGES_KEPT {
                kept.remove(0);
            }
        }
    });
    edges
}

/// The planar faces of the body whose shape has `version`: kept ones, or
/// found by `find` (and kept while [`KeptPlanes`] is).
pub fn planes_of(version: u128, find: impl FnOnce() -> Vec<(FaceName, Plane)>) -> Planes {
    let kept = KEPT.with(|k| {
        let mut k = k.borrow_mut();
        let kept = k.as_mut()?;
        let i = kept.iter().position(|(v, _)| *v == version)?;
        let entry = kept.remove(i);
        let planes = Arc::clone(&entry.1);
        kept.push(entry);
        Some(planes)
    });
    if let Some(planes) = kept {
        return planes;
    }
    let planes = Arc::new(find());
    KEPT.with(|k| {
        if let Some(kept) = k.borrow_mut().as_mut() {
            if kept.len() >= PLANES_KEPT {
                kept.remove(0);
            }
            kept.push((version, Arc::clone(&planes)));
        }
    });
    planes
}
