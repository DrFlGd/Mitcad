// SPDX-License-Identifier: MIT
//! Reverse Normal (mitcad#455): a surface body with faces turned over (every
//! face when none is named): their normals point the other way, which
//! decides the side Thicken and Offset Surface go to. The body and the
//! faces keep their names. A solid's faces are not turned (its normals
//! point out of its material).

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, check_faces, kernel_error, require_faces};
use super::mapping::map_params;
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::exchange::BodyKind;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::topo::FaceName;

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReverseNormalDef<P = ParamId> {
    pub body: BodyUid,
    /// The faces turned over; none: every face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<FaceName>,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

impl<P> ReverseNormalDef<P> {
    pub const TYPE: &'static str = "reverse_normal";
    pub const BASE_NAME: &'static str = "ReverseNormal";

    map_params! {
        ReverseNormalDef {
            body: copy,
            faces: clone,
            values: phantom,
        }
    }
}

impl FeatureInfo for ReverseNormalDef {
    fn references(&self) -> References {
        let mut references = References::default();
        references.body(self.body);
        add_faces(&mut references, &self.faces);
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        ctx.body(self.body)?;
        if !self.faces.is_empty() {
            check_faces(ctx, &self.faces, "faces")?;
        }
        Ok(())
    }
}

impl<K: Kernel> Evaluate<K> for ReverseNormalDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let shape = ctx.surface_body(self.body)?;
        if matches!(ctx.kernel.body_kind(&shape), Ok(BodyKind::Solid)) {
            return Err(format!(
                "body {} (from {}) is a solid; Reverse Normal turns faces of surface bodies",
                self.body,
                ctx.feature_name(self.body.feature)
            ));
        }
        let faces = if self.faces.is_empty() {
            Vec::new()
        } else {
            require_faces(ctx, self.body, &shape, &self.faces)?
        };
        let turned = ctx
            .kernel
            .reverse_faces(&shape, &faces)
            .map_err(kernel_error)?;
        Ok(FeatureOutput {
            changes: vec![BodyChange::Set(self.body, turned)],
            ..FeatureOutput::default()
        })
    }
}
