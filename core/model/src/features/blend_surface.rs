// SPDX-License-Identifier: MIT
//! Blend Surface (mitcad#455): a surface bridging two chains of edges (of
//! one body or two), each end leaving its chain repeating the face beside
//! it up to its continuity (G0 through the edges, G1 tangent, G2 also
//! curved as the face) and as full as its bulge asks. A new surface body
//! of one face `<feature>:blend`; with `stitch` the blend is sewn onto the
//! two bodies where the edges meet, into the first body (the second is
//! consumed).

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::face_refs::kernel_error;
use super::sheet_edges::{self, OpeningEdgesRef};
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
    is_false,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::surface_modelling::{BlendEnd, BlendSpec, BodyEdges, OpeningEdges};
use crate::surfaces::PatchContinuity;
use crate::topo::{EdgeName, FaceName};

/// Has no values (its bulges are fixed numbers); `P` only matches the
/// other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlendSurfaceDef<P = ParamId> {
    pub from: BlendSide,
    pub to: BlendSide,
    /// Sews the blend onto its two bodies.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stitch: bool,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

/// One end of a blend: a chain of a body's edges and how the blend leaves
/// it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlendSide {
    pub body: BodyUid,
    /// Named edges of the chain (with those of `openings`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<EdgeName>,
    /// Free edges of the body, of its openings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<OpeningEdgesRef>,
    /// The face beside the chain where an edge lies between two faces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceName>,
    #[serde(default = "tangent", skip_serializing_if = "is_tangent")]
    pub continuity: PatchContinuity,
    /// 0 the plain bridge; more fuller, less (down to -0.9) flatter.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bulge: f64,
}

fn tangent() -> PatchContinuity {
    PatchContinuity::G1
}

fn is_tangent(value: &PatchContinuity) -> bool {
    *value == PatchContinuity::G1
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

impl<P> BlendSurfaceDef<P> {
    pub const TYPE: &'static str = "blend_surface";
    pub const BASE_NAME: &'static str = "BlendSurface";

    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<BlendSurfaceDef<Q>, E> {
        Ok(BlendSurfaceDef {
            from: self.from.clone(),
            to: self.to.clone(),
            stitch: self.stitch,
            values: PhantomData,
        })
    }
}

impl BlendSide {
    fn check(&self, ctx: &CheckContext<'_>, which: &str) -> Result<(), String> {
        sheet_edges::check_chain(ctx, self.body, &self.edges, &self.openings, &self.face)
            .map_err(|e| format!("the blend's {which} side: {e}"))?;
        if !(self.bulge.is_finite() && self.bulge >= -0.9) {
            return Err(format!(
                "the blend's {which} bulge must be -0.9 or more, got {}",
                self.bulge
            ));
        }
        Ok(())
    }
}

impl FeatureInfo for BlendSurfaceDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for side in [&self.from, &self.to] {
            sheet_edges::add_chain_references(
                &mut references,
                side.body,
                &side.edges,
                &side.openings,
                &side.face,
            );
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        self.from.check(ctx, "first")?;
        self.to.check(ctx, "second")
    }

    fn creates_bodies(&self) -> bool {
        !self.stitch
    }
}

/// The blend's edges are fitted through points along the chains: they are
/// sewn onto the bodies within this (mm).
const STITCH_TOLERANCE: f64 = 0.01;

fn blend_end<'a, S>(
    side: &BlendSide,
    chain: &'a sheet_edges::ResolvedChain<S>,
    openings: &'a [OpeningEdges<'a>],
) -> BlendEnd<'a, S> {
    BlendEnd {
        edges: BodyEdges {
            body: &chain.shape,
            edges: &chain.edges,
            openings,
            face: chain.face.as_ref(),
        },
        continuity: side.continuity,
        bulge: side.bulge,
    }
}

impl<K: Kernel> Evaluate<K> for BlendSurfaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let resolve = |ctx: &mut EvalContext<'_, K>, side: &BlendSide| {
            sheet_edges::resolve_chain(ctx, side.body, &side.edges, &side.openings, &side.face)
        };
        let from = resolve(ctx, &self.from)?;
        let to = resolve(ctx, &self.to)?;
        let from_openings = from.opening_edges();
        let to_openings = to.opening_edges();
        let spec = BlendSpec {
            feature: ctx.uid,
            from: blend_end(&self.from, &from, &from_openings),
            to: blend_end(&self.to, &to, &to_openings),
        };
        let blend = ctx.kernel.blend_surface(&spec).map_err(kernel_error)?;
        let mut output = FeatureOutput {
            tool: Some(blend.clone()),
            ..FeatureOutput::default()
        };
        if !self.stitch {
            output.changes.push(BodyChange::Set(ctx.new_body(), blend));
            return Ok(output);
        }
        let mut shapes = vec![&from.shape, &blend];
        if self.to.body != self.from.body {
            shapes.push(&to.shape);
        }
        let sewn = ctx
            .kernel
            .stitch(&shapes, STITCH_TOLERANCE)
            .map_err(kernel_error)?;
        output.changes.push(BodyChange::Set(self.from.body, sewn));
        if self.to.body != self.from.body {
            output.changes.push(BodyChange::Remove(self.to.body));
        }
        Ok(output)
    }
}
