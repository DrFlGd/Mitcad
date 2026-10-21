// SPDX-License-Identifier: MIT
//! Offset Surface (mitcad#455): faces of a body copied out along their
//! normals as a new surface body, `<feature>:offset(<face>)` each. Every
//! face of the body when none is named; with `chain` also the faces joined
//! to the named ones through smooth edges. The body is not changed.

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, kernel_error, require_faces};
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
    is_false,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::topo::FaceName;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OffsetSurfaceDef<P = ParamId> {
    pub body: BodyUid,
    /// The faces offset; none: every face of the body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<FaceName>,
    /// Along the faces' normals (mm), against them when negative; 0 copies
    /// the faces.
    pub distance: P,
    /// Also the faces joined to the named ones through smooth edges.
    #[serde(default, skip_serializing_if = "is_false")]
    pub chain: bool,
}

impl<P> OffsetSurfaceDef<P> {
    pub const TYPE: &'static str = "offset_surface";
    pub const BASE_NAME: &'static str = "OffsetSurface";

    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<OffsetSurfaceDef<Q>, E> {
        Ok(OffsetSurfaceDef {
            body: self.body,
            faces: self.faces.clone(),
            distance: f("distance", &self.distance)?,
            chain: self.chain,
        })
    }
}

impl FeatureInfo for OffsetSurfaceDef {
    fn references(&self) -> References {
        let mut references = References::default();
        references.body(self.body);
        add_faces(&mut references, &self.faces);
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        ctx.body(self.body)?;
        if !self.faces.is_empty() {
            super::face_refs::check_faces(ctx, &self.faces, "faces")?;
        }
        Ok(())
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        let distance = value(self.distance);
        if distance.is_finite() {
            Ok(())
        } else {
            Err(format!(
                "the offset distance must be a number, got {distance}"
            ))
        }
    }

    fn creates_bodies(&self) -> bool {
        true
    }
}

impl<K: Kernel> Evaluate<K> for OffsetSurfaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let distance = ctx.param(self.distance)?;
        let shape = ctx.surface_body(self.body)?;
        let faces = if self.faces.is_empty() {
            Vec::new()
        } else {
            require_faces(ctx, self.body, &shape, &self.faces)?
        };
        let offset = ctx
            .kernel
            .offset_surface(ctx.uid, &shape, &faces, distance, self.chain)
            .map_err(kernel_error)?;
        let mut output = FeatureOutput {
            tool: Some(offset.clone()),
            ..FeatureOutput::default()
        };
        output.changes.push(BodyChange::Set(ctx.new_body(), offset));
        Ok(output)
    }
}
