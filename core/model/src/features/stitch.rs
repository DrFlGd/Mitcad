// SPDX-License-Identifier: MIT
//! Stitch (mitcad#456): bodies' faces sewn into one body where their edges
//! meet within a tolerance. The first body keeps its id and becomes the
//! sewn body, a solid where the faces close up, else a surface body; the
//! others are consumed. The faces keep their names.

use serde::{Deserialize, Serialize};

use super::face_refs::kernel_error;
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StitchDef<P = ParamId> {
    /// The bodies sewn; the first becomes the result.
    pub bodies: Vec<BodyUid>,
    /// Edges this close are sewn (mm); 0 for the kernel's own precision.
    pub tolerance: P,
}

impl<P> StitchDef<P> {
    pub const TYPE: &'static str = "stitch";
    pub const BASE_NAME: &'static str = "Stitch";

    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<StitchDef<Q>, E> {
        Ok(StitchDef {
            bodies: self.bodies.clone(),
            tolerance: f("tolerance", &self.tolerance)?,
        })
    }
}

fn check_tolerance(tolerance: f64) -> Result<(), String> {
    if tolerance >= 0.0 && tolerance.is_finite() {
        Ok(())
    } else {
        Err(format!(
            "the stitch tolerance must not be negative, got {tolerance}"
        ))
    }
}

impl FeatureInfo for StitchDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for body in &self.bodies {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        if self.bodies.is_empty() {
            return Err("no bodies to stitch selected".to_owned());
        }
        for (i, body) in self.bodies.iter().enumerate() {
            if self.bodies[..i].contains(body) {
                return Err(format!("body {body} is listed more than once"));
            }
            ctx.body(*body)?;
        }
        Ok(())
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        check_tolerance(value(self.tolerance))
    }
}

impl<K: Kernel> Evaluate<K> for StitchDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let Some(first) = self.bodies.first().copied() else {
            return Err("no bodies to stitch selected".to_owned());
        };
        let tolerance = ctx.param(self.tolerance)?;
        check_tolerance(tolerance)?;
        // Mesh bodies are refused (mitcad#399).
        let shapes = self
            .bodies
            .iter()
            .map(|body| ctx.surface_body(*body))
            .collect::<Result<Vec<_>, _>>()?;
        let refs: Vec<&K::Shape> = shapes.iter().collect();
        let sewn = ctx.kernel.stitch(&refs, tolerance).map_err(kernel_error)?;
        let mut changes = vec![BodyChange::Set(first, sewn)];
        changes.extend(self.bodies[1..].iter().map(|b| BodyChange::Remove(*b)));
        Ok(FeatureOutput {
            changes,
            ..FeatureOutput::default()
        })
    }
}
