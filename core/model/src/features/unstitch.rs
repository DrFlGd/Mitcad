// SPDX-License-Identifier: MIT
//! Unstitch (mitcad#455): a body taken apart into surface bodies: each
//! named face its own (new bodies, in the order named), the faces left
//! one surface body that keeps the body's id. Without named faces every
//! face comes apart: the first stays the body, each other one a new body.
//! A solid comes apart into surfaces. The faces keep their names.

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, check_faces, kernel_error, require_faces};
use super::mapping::map_params;
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::topo::FaceName;

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnstitchDef<P = ParamId> {
    pub body: BodyUid,
    /// The faces taken out; none: every face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<FaceName>,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

impl<P> UnstitchDef<P> {
    pub const TYPE: &'static str = "unstitch";
    pub const BASE_NAME: &'static str = "Unstitch";

    map_params! {
        UnstitchDef {
            body: copy,
            faces: clone,
            values: phantom,
        }
    }
}

impl FeatureInfo for UnstitchDef {
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

    fn creates_bodies(&self) -> bool {
        true
    }
}

impl<K: Kernel> Evaluate<K> for UnstitchDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let shape = ctx.surface_body(self.body)?;
        let faces = if self.faces.is_empty() {
            Vec::new()
        } else {
            require_faces(ctx, self.body, &shape, &self.faces)?
        };
        let (rest, apart) = ctx.kernel.unstitch(&shape, &faces).map_err(kernel_error)?;
        let mut apart = apart.into_iter();
        // The body stays: what is left, or the first face apart.
        let kept = match rest {
            Some(rest) => rest,
            None => apart
                .next()
                .ok_or_else(|| format!("body {} has no faces", self.body))?,
        };
        let mut changes = vec![BodyChange::Set(self.body, kept)];
        for piece in apart {
            changes.push(BodyChange::Set(ctx.new_body(), piece));
        }
        Ok(FeatureOutput {
            changes,
            ..FeatureOutput::default()
        })
    }
}
