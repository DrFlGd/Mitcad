// SPDX-License-Identifier: MIT
//! Match Surface (mitcad#455): a face of a surface body rebuilt against
//! another body: the side of the face nearest a chain of the target's
//! edges (named, or free edges of openings) moves onto that chain and
//! meets the target's face beside it through the edges (G0), tangent (G1)
//! or also curved as it (G2), while the opposite side stays where it was.
//! The face is the body's only one unless `face` names it, and must be
//! bounded by its surface's parameter range alone (four sides, no holes).
//! The body keeps its id and the face its name.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, check_faces, kernel_error};
use super::mapping::map_params;
use super::sheet_edges::{self, OpeningEdgesRef};
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::exchange::BodyKind;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::surface_modelling::BodyEdges;
use crate::surfaces::PatchContinuity;
use crate::topo::{EdgeName, FaceName};

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchSurfaceDef<P = ParamId> {
    /// The surface body whose face is rebuilt.
    pub body: BodyUid,
    /// The face; none: the body's only face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceName>,
    /// The body matched to.
    pub target: BodyUid,
    /// Named edges of the target's chain (with those of `openings`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<EdgeName>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<OpeningEdgesRef>,
    /// The target's face beside the chain where an edge lies between two.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_face: Option<FaceName>,
    #[serde(default = "curvature", skip_serializing_if = "is_curvature")]
    pub continuity: PatchContinuity,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

fn curvature() -> PatchContinuity {
    PatchContinuity::G2
}

fn is_curvature(value: &PatchContinuity) -> bool {
    *value == PatchContinuity::G2
}

impl<P> MatchSurfaceDef<P> {
    pub const TYPE: &'static str = "match_surface";
    pub const BASE_NAME: &'static str = "MatchSurface";

    map_params! {
        MatchSurfaceDef {
            body: copy,
            face: clone,
            target: copy,
            edges: clone,
            openings: clone,
            target_face: clone,
            continuity: copy,
            values: phantom,
        }
    }
}

impl FeatureInfo for MatchSurfaceDef {
    fn references(&self) -> References {
        let mut references = References::default();
        references.body(self.body);
        add_faces(&mut references, &self.face);
        sheet_edges::add_chain_references(
            &mut references,
            self.target,
            &self.edges,
            &self.openings,
            &self.target_face,
        );
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        ctx.body(self.body)?;
        if let Some(face) = &self.face {
            check_faces(ctx, std::slice::from_ref(face), "face")?;
        }
        if self.target == self.body {
            return Err(format!("body {} cannot be matched to itself", self.body));
        }
        sheet_edges::check_chain(
            ctx,
            self.target,
            &self.edges,
            &self.openings,
            &self.target_face,
        )
    }
}

impl<K: Kernel> Evaluate<K> for MatchSurfaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let shape = ctx.surface_body(self.body)?;
        if matches!(ctx.kernel.body_kind(&shape), Ok(BodyKind::Solid)) {
            return Err(format!(
                "body {} (from {}) is a solid; Match Surface rebuilds a face of a surface body",
                self.body,
                ctx.feature_name(self.body.feature)
            ));
        }
        let face = match &self.face {
            Some(face) => Some(ctx.face_on(self.body, &shape, face)?),
            None => None,
        };
        let target = sheet_edges::resolve_chain(
            ctx,
            self.target,
            &self.edges,
            &self.openings,
            &self.target_face,
        )?;
        let openings = target.opening_edges();
        let edges = BodyEdges {
            body: &target.shape,
            edges: &target.edges,
            openings: &openings,
            face: target.face.as_ref(),
        };
        let matched = ctx
            .kernel
            .match_surface(&shape, face.as_ref(), &edges, self.continuity)
            .map_err(kernel_error)?;
        Ok(FeatureOutput {
            changes: vec![BodyChange::Set(self.body, matched)],
            ..FeatureOutput::default()
        })
    }
}
