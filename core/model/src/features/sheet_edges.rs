// SPDX-License-Identifier: MIT
//! Free edges of surface bodies in the definitions of surface features
//! (mitcad#455). A surface body's open boundary is made of free edges
//! (edges of one face only), which have no names of their own: a
//! definition picks them as an opening (a loop of free edges, found by the
//! faces it runs along, `features::patch::Opening`; every opening when it
//! names none) and, when it wants only some of them, the faces those edges
//! bound.

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, check_faces, kernel_error};
use super::patch::{Opening, find_opening};
use super::{CheckContext, EvalContext, References};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::surface_modelling::OpeningEdges;
use crate::topo::{EdgeName, FaceName};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningEdgesRef {
    /// The opening; none: every opening of the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<Opening>,
    /// Only the free edges that bound these faces; none: all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<FaceName>,
    /// Of those, only the edge nearest each point (mm, the component's
    /// coordinates); none: all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub at: Vec<[f64; 3]>,
}

/// An opening resolved: its index among the body's open boundaries, the
/// faces and the points that narrow its edges down.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedOpening {
    pub index: usize,
    pub faces: Vec<FaceName>,
    pub at: Vec<[f64; 3]>,
}

/// Resolved openings as the kernel takes them.
pub(crate) fn kernel_openings(resolved: &[ResolvedOpening]) -> Vec<OpeningEdges<'_>> {
    resolved
        .iter()
        .map(|o| OpeningEdges {
            index: o.index,
            faces: &o.faces,
            at: &o.at,
        })
        .collect()
}

pub(crate) fn add_references(references: &mut References, openings: &[OpeningEdgesRef]) {
    for o in openings {
        if let Some(opening) = &o.opening {
            add_faces(references, &opening.faces);
        }
        add_faces(references, &o.faces);
    }
}

pub(crate) fn check(ctx: &CheckContext<'_>, openings: &[OpeningEdgesRef]) -> Result<(), String> {
    for o in openings {
        if let Some(opening) = &o.opening {
            check_faces(ctx, &opening.faces, "faces of the opening")?;
        }
        if !o.faces.is_empty() {
            check_faces(ctx, &o.faces, "faces of the free edges")?;
        }
        if !o.at.is_empty() && o.opening.is_none() {
            return Err("points pick free edges of one opening; name the opening".to_owned());
        }
        if o.at.iter().flatten().any(|v| !v.is_finite()) {
            return Err("a point picking a free edge is not finite".to_owned());
        }
    }
    Ok(())
}

/// A chain of a body's edges as Blend, Boundary and Match Surface take it
/// (their definitions' `body`, `edges`, `openings` and `face`), resolved
/// against the body's shape.
pub(crate) struct ResolvedChain<S> {
    pub shape: S,
    pub edges: Vec<EdgeName>,
    pub openings: Vec<ResolvedOpening>,
    pub face: Option<FaceName>,
}

impl<S> ResolvedChain<S> {
    /// The openings as the kernel takes them.
    pub fn opening_edges(&self) -> Vec<OpeningEdges<'_>> {
        kernel_openings(&self.openings)
    }
}

/// The references of a chain of edges.
pub(crate) fn add_chain_references(
    references: &mut References,
    body: BodyUid,
    edges: &[EdgeName],
    openings: &[OpeningEdgesRef],
    face: &Option<FaceName>,
) {
    references.body(body);
    references.edges(edges);
    add_references(references, openings);
    add_faces(references, face);
}

/// Checks a chain of edges: some edges, named or of openings.
pub(crate) fn check_chain(
    ctx: &CheckContext<'_>,
    body: BodyUid,
    edges: &[EdgeName],
    openings: &[OpeningEdgesRef],
    face: &Option<FaceName>,
) -> Result<(), String> {
    ctx.body(body)?;
    if edges.is_empty() && openings.is_empty() {
        return Err("no edges selected".to_owned());
    }
    if !edges.is_empty() {
        ctx.edges(edges)?;
    }
    check(ctx, openings)?;
    if let Some(face) = face {
        check_faces(ctx, std::slice::from_ref(face), "face")?;
    }
    Ok(())
}

/// A chain of edges resolved against its body (a surface body or a solid).
pub(crate) fn resolve_chain<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    body: BodyUid,
    edges: &[EdgeName],
    openings: &[OpeningEdgesRef],
    face: &Option<FaceName>,
) -> Result<ResolvedChain<K::Shape>, String> {
    let shape = ctx.surface_body(body)?;
    let edges = if edges.is_empty() {
        Vec::new()
    } else {
        ctx.require_edges(body, &shape, edges)?
    };
    let openings = resolve(ctx, body, &shape, openings)?;
    let face = match face {
        Some(face) => Some(ctx.face_on(body, &shape, face)?),
        None => None,
    };
    Ok(ResolvedChain {
        shape,
        edges,
        openings,
        face,
    })
}

/// Each opening's index among the body's open boundaries, with its faces
/// and points.
pub(crate) fn resolve<K: Kernel>(
    ctx: &EvalContext<'_, K>,
    body: BodyUid,
    shape: &K::Shape,
    openings: &[OpeningEdgesRef],
) -> Result<Vec<ResolvedOpening>, String> {
    let mut resolved = Vec::new();
    for o in openings {
        match &o.opening {
            Some(opening) => resolved.push(ResolvedOpening {
                index: find_opening(ctx, body, shape, opening)?,
                faces: o.faces.clone(),
                at: o.at.clone(),
            }),
            None => {
                let count = ctx
                    .kernel
                    .open_boundaries(shape)
                    .map_err(kernel_error)?
                    .len();
                if count == 0 {
                    return Err(format!(
                        "body {body} (from {}) has no open edges",
                        ctx.feature_name(body.feature)
                    ));
                }
                resolved.extend((0..count).map(|index| ResolvedOpening {
                    index,
                    faces: o.faces.clone(),
                    at: Vec::new(),
                }));
            }
        }
    }
    Ok(resolved)
}
