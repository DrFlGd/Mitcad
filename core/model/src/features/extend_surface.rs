// SPDX-License-Identifier: MIT
//! Extend (mitcad#455): a surface body's faces extended `distance` past
//! named edges of their open boundary: the surface continued (`natural`),
//! or leaving the edge tangent to it (`tangent`) or also with its
//! curvature (`curvature`). The body keeps its id and its faces their
//! names; each extended face stays one face.

use serde::{Deserialize, Serialize};

use super::face_refs::kernel_error;
use super::mapping::map_params;
use super::sheet_edges::{self, OpeningEdgesRef};
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::exchange::BodyKind;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::surface_modelling::ExtendMode;
use crate::topo::EdgeName;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtendSurfaceDef<P = ParamId> {
    /// A surface body.
    pub body: BodyUid,
    /// Named edges of its open boundary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<EdgeName>,
    /// Free edges of its openings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<OpeningEdgesRef>,
    /// How far past the edges (mm, more than 0), along the surface.
    pub distance: P,
    #[serde(default, skip_serializing_if = "ExtendMode::is_natural")]
    pub mode: ExtendMode,
}

impl<P> ExtendSurfaceDef<P> {
    pub const TYPE: &'static str = "extend_surface";
    pub const BASE_NAME: &'static str = "Extend";

    map_params! {
        ExtendSurfaceDef {
            body: copy,
            edges: clone,
            openings: clone,
            distance: param,
            mode: copy,
        }
    }
}

impl FeatureInfo for ExtendSurfaceDef {
    fn references(&self) -> References {
        let mut references = References::default();
        references.body(self.body);
        references.edges(&self.edges);
        sheet_edges::add_references(&mut references, &self.openings);
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        ctx.body(self.body)?;
        if self.edges.is_empty() && self.openings.is_empty() {
            return Err("no edges selected".to_owned());
        }
        if !self.edges.is_empty() {
            ctx.edges(&self.edges)?;
        }
        sheet_edges::check(ctx, &self.openings)
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        let distance = value(self.distance);
        if distance > 0.0 && distance.is_finite() {
            Ok(())
        } else {
            Err(format!(
                "the extension must be longer than zero, got {distance}"
            ))
        }
    }
}

impl<K: Kernel> Evaluate<K> for ExtendSurfaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let distance = ctx.positive(self.distance)?;
        let shape = ctx.surface_body(self.body)?;
        if matches!(ctx.kernel.body_kind(&shape), Ok(BodyKind::Solid)) {
            return Err(format!(
                "body {} (from {}) is a solid; Extend extends surface bodies",
                self.body,
                ctx.feature_name(self.body.feature)
            ));
        }
        let edges = if self.edges.is_empty() {
            Vec::new()
        } else {
            ctx.require_edges(self.body, &shape, &self.edges)?
        };
        let resolved = sheet_edges::resolve(ctx, self.body, &shape, &self.openings)?;
        let openings = sheet_edges::kernel_openings(&resolved);
        let extended = ctx
            .kernel
            .extend_surface(&shape, &edges, &openings, distance, self.mode)
            .map_err(kernel_error)?;
        Ok(FeatureOutput {
            changes: vec![BodyChange::Set(self.body, extended)],
            ..FeatureOutput::default()
        })
    }
}
