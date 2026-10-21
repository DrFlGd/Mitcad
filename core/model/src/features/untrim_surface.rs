// SPDX-License-Identifier: MIT
//! Untrim (mitcad#455): a face restored towards its surface: its holes
//! taken away (`holes`, the default) and its outer boundary put back at the
//! surface's own bounds (`outer`; a surface without bounds, such as a plane
//! or a cylinder, at the face's span). A new surface body of one face
//! `<feature>:untrim(<face>)`; with `in_place` the face of a surface body is
//! untrimmed where it is (the body and the face keep their names).

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, check_faces, kernel_error};
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
    is_false,
};
use crate::exchange::BodyKind;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::topo::FaceName;
use std::marker::PhantomData;

/// Has no values; `P` only matches the other feature types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UntrimSurfaceDef<P = ParamId> {
    pub body: BodyUid,
    pub face: FaceName,
    /// Takes the face's holes away.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub holes: bool,
    /// Puts the outer boundary back at the surface's bounds.
    #[serde(default, skip_serializing_if = "is_false")]
    pub outer: bool,
    /// Untrims the face of a surface body where it is.
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_place: bool,
    #[serde(skip)]
    pub values: PhantomData<P>,
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

impl<P> UntrimSurfaceDef<P> {
    pub const TYPE: &'static str = "untrim_surface";
    pub const BASE_NAME: &'static str = "Untrim";

    pub fn map_params<Q, E>(
        &self,
        _f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<UntrimSurfaceDef<Q>, E> {
        Ok(UntrimSurfaceDef {
            body: self.body,
            face: self.face.clone(),
            holes: self.holes,
            outer: self.outer,
            in_place: self.in_place,
            values: PhantomData,
        })
    }
}

impl FeatureInfo for UntrimSurfaceDef {
    fn references(&self) -> References {
        let mut references = References::default();
        references.body(self.body);
        add_faces(&mut references, std::slice::from_ref(&self.face));
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        ctx.body(self.body)?;
        check_faces(ctx, std::slice::from_ref(&self.face), "face")?;
        if !self.holes && !self.outer {
            return Err("untrim takes the holes away, the outer boundary out, or both".to_owned());
        }
        Ok(())
    }

    fn creates_bodies(&self) -> bool {
        !self.in_place
    }
}

impl<K: Kernel> Evaluate<K> for UntrimSurfaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let shape = ctx.surface_body(self.body)?;
        if self.in_place && matches!(ctx.kernel.body_kind(&shape), Ok(BodyKind::Solid)) {
            return Err(format!(
                "body {} (from {}) is a solid; a face is untrimmed in place on a surface body",
                self.body,
                ctx.feature_name(self.body.feature)
            ));
        }
        let face = ctx.face_on(self.body, &shape, &self.face)?;
        let untrimmed = ctx
            .kernel
            .untrim_surface(
                ctx.uid,
                &shape,
                &face,
                self.holes,
                self.outer,
                self.in_place,
            )
            .map_err(kernel_error)?;
        let target = if self.in_place {
            self.body
        } else {
            ctx.new_body()
        };
        Ok(FeatureOutput {
            changes: vec![BodyChange::Set(target, untrimmed)],
            ..FeatureOutput::default()
        })
    }
}
