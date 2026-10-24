// SPDX-License-Identifier: MIT
//! Fair Surface (mitcad#455): B-spline faces of a body smoothed: their
//! inner control points move towards a fairer (less bending) net,
//! `strength` of a full step at a time for `iterations` steps, none more
//! than `tolerance` from where it was. The boundary row of control points
//! stays (the faces keep meeting their neighbours), with `keep_edges` also
//! the row next to it (they keep their tangents there). Faces of other
//! surface types are fair already and stay as they are. Every face of the
//! body when none is named; a faired face must be bounded by its surface's
//! parameter range alone. The body keeps its id and the faces their names.

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, check_faces, kernel_error, require_faces};
use super::mapping::map_params;
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::surface_modelling::Fairing;
use crate::topo::FaceName;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FairSurfaceDef<P = ParamId> {
    pub body: BodyUid,
    /// The faces; none: every face of the body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<FaceName>,
    /// The share of a full smoothing step each iteration takes (more than
    /// 0, at most 1).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub strength: f64,
    #[serde(default = "one_step", skip_serializing_if = "is_one_step")]
    pub iterations: u32,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub keep_edges: bool,
    /// The farthest a control point moves (mm, more than 0).
    pub tolerance: P,
}

fn one() -> f64 {
    1.0
}

fn is_one(value: &f64) -> bool {
    *value == 1.0
}

fn one_step() -> u32 {
    1
}

fn is_one_step(value: &u32) -> bool {
    *value == 1
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Fair Surface takes at most this many iterations.
const MOST_ITERATIONS: u32 = 1000;

impl<P> FairSurfaceDef<P> {
    pub const TYPE: &'static str = "fair_surface";
    pub const BASE_NAME: &'static str = "FairSurface";

    map_params! {
        FairSurfaceDef {
            body: copy,
            faces: clone,
            strength: copy,
            iterations: copy,
            keep_edges: copy,
            tolerance: param,
        }
    }
}

impl FeatureInfo for FairSurfaceDef {
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
        if !(self.strength > 0.0 && self.strength <= 1.0) {
            return Err(format!(
                "the fairing strength must be more than 0 and at most 1, got {}",
                self.strength
            ));
        }
        if self.iterations == 0 || self.iterations > MOST_ITERATIONS {
            return Err(format!(
                "the fairing takes 1 to {MOST_ITERATIONS} iterations, got {}",
                self.iterations
            ));
        }
        Ok(())
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        let tolerance = value(self.tolerance);
        if !(tolerance > 0.0 && tolerance.is_finite()) {
            return Err(format!(
                "the fairing tolerance must be greater than zero, got {tolerance}"
            ));
        }
        Ok(())
    }
}

impl<K: Kernel> Evaluate<K> for FairSurfaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let tolerance = ctx.positive(self.tolerance)?;
        let shape = ctx.surface_body(self.body)?;
        let faces = if self.faces.is_empty() {
            Vec::new()
        } else {
            require_faces(ctx, self.body, &shape, &self.faces)?
        };
        let fairing = Fairing {
            strength: self.strength,
            iterations: self.iterations,
            keep_edges: self.keep_edges,
            tolerance,
        };
        let faired = ctx
            .kernel
            .fair_surface(&shape, &faces, fairing)
            .map_err(kernel_error)?;
        Ok(FeatureOutput {
            changes: vec![BodyChange::Set(self.body, faired)],
            ..FeatureOutput::default()
        })
    }
}
