// SPDX-License-Identifier: MIT
//! Thicken (mitcad#456): surface bodies made solid by a wall along their
//! faces' normals (against them for a negative thickness), or half of it to
//! each side when symmetric, with the operations of an extrusion: new
//! bodies, or joined with, cut from or intersected with other bodies (all
//! but the thickened ones without participants). The surface bodies stay
//! as they are. The wall's faces are `<feature>:front(<face>)` on the side
//! a face's normal points to, `<feature>:back(<face>)` on the other and
//! `<feature>:wall(<face>)` along the face's free edges.

use serde::{Deserialize, Serialize};

use super::extrude::{Operation, apply_operation, check_participants, participant_bodies};
use super::face_refs::kernel_error;
use super::mapping::map_params;
use super::{
    CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References, is_false,
};
use crate::exchange::BodyKind;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThickenDef<P = ParamId> {
    /// The surface bodies thickened, each into solids of its own.
    pub bodies: Vec<BodyUid>,
    /// The wall (mm, not 0): along the faces' normals, against them when
    /// negative.
    pub thickness: P,
    /// Half of the thickness to each side of the faces.
    #[serde(default, skip_serializing_if = "is_false")]
    pub symmetric: bool,
    pub operation: Operation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
}

impl<P> ThickenDef<P> {
    pub const TYPE: &'static str = "thicken";
    pub const BASE_NAME: &'static str = "Thicken";

    map_params! {
        ThickenDef {
            bodies: clone,
            thickness: param,
            symmetric: copy,
            operation: copy,
            participants: clone,
        }
    }
}

fn check_thickness(thickness: f64) -> Result<(), String> {
    if thickness != 0.0 && thickness.is_finite() {
        Ok(())
    } else {
        Err(format!("the thickness must not be zero, got {thickness}"))
    }
}

impl FeatureInfo for ThickenDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for body in self.bodies.iter().chain(&self.participants) {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        if self.bodies.is_empty() {
            return Err("no surface bodies to thicken selected".to_owned());
        }
        for (i, body) in self.bodies.iter().enumerate() {
            if self.bodies[..i].contains(body) {
                return Err(format!("body {body} is listed more than once"));
            }
            ctx.body(*body)?;
        }
        check_participants(ctx, self.operation, &self.participants)?;
        if let Some(body) = self.participants.iter().find(|b| self.bodies.contains(b)) {
            return Err(format!(
                "body {body} is thickened; it cannot also be a participant"
            ));
        }
        Ok(())
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        check_thickness(value(self.thickness))
    }

    fn creates_bodies(&self) -> bool {
        true
    }

    fn new_component(&self) -> bool {
        self.operation == Operation::NewComponent
    }
}

impl<K: Kernel> Evaluate<K> for ThickenDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let thickness = ctx.param(self.thickness)?;
        check_thickness(thickness)?;
        // Without participants a join or cut works on every body but the
        // thickened ones (mesh bodies are left out, mitcad#399).
        let participants: Vec<(BodyUid, K::Shape)> =
            if !self.operation.makes_bodies() && self.participants.is_empty() {
                ctx.surface_bodies()
                    .into_iter()
                    .filter(|(uid, _)| !self.bodies.contains(uid))
                    .collect()
            } else {
                participant_bodies(ctx, self.operation, &self.participants)?
            };
        let mut solids = Vec::with_capacity(self.bodies.len());
        for body in &self.bodies {
            let shape = ctx.surface_body(*body)?;
            if matches!(ctx.kernel.body_kind(&shape), Ok(BodyKind::Solid)) {
                return Err(format!(
                    "body {body} (from {}) is a solid; Thicken makes surface bodies solid",
                    ctx.feature_name(body.feature)
                ));
            }
            let solid = ctx
                .kernel
                .thicken(ctx.uid, &shape, thickness, self.symmetric)
                .map_err(|e| format!("body {body}: {}", kernel_error(e)))?;
            solids.push(solid);
        }
        let tool = if solids.len() == 1 {
            solids.remove(0)
        } else {
            ctx.kernel.compound(&solids).map_err(kernel_error)?
        };
        apply_operation(ctx, self.operation, participants, tool, "thickened surface")
    }
}
