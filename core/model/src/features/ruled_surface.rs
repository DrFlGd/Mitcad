// SPDX-License-Identifier: MIT
//! Ruled Surface (mitcad#455): a strip `distance` wide hanging off a chain
//! of a body's edges, as a new surface body: along the normal of the face
//! beside each edge, continuing the face, at an angle from its normal (or
//! from a direction), or along a fixed direction; `flip` the other way.
//! Neighbouring strips meet at sharp corners mitred or by a round sector.
//! Faces `<feature>:ruled(<edge>)` and `<feature>:corner(<vertex>)`. The
//! body is not changed.

use serde::{Deserialize, Serialize};

use super::face_refs::{add_faces, kernel_error};
use super::sheet_edges::{self, OpeningEdgesRef};
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
    is_false,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::surface_modelling::{RuledCorners, RuledMode, RuledSpec};
use crate::topo::{EdgeName, FaceName};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuledSurfaceDef<P = ParamId> {
    pub body: BodyUid,
    /// Edges of the body that join into chains (with those of
    /// `openings`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<EdgeName>,
    /// Free edges of the body, of its openings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<OpeningEdgesRef>,
    /// The face beside the chain where an edge lies between two faces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceName>,
    /// The strip's width (mm, more than 0).
    pub distance: P,
    #[serde(default, skip_serializing_if = "RuledMode::is_normal")]
    pub mode: RuledMode,
    /// The angle mode's angle (radians, from the normal or `direction`).
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub angle: Option<P>,
    /// The direction mode's direction, or the angle mode's reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip: bool,
    #[serde(default, skip_serializing_if = "RuledCorners::is_mitre")]
    pub corners: RuledCorners,
}

impl<P> RuledSurfaceDef<P> {
    pub const TYPE: &'static str = "ruled_surface";
    pub const BASE_NAME: &'static str = "RuledSurface";

    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<RuledSurfaceDef<Q>, E> {
        Ok(RuledSurfaceDef {
            body: self.body,
            edges: self.edges.clone(),
            openings: self.openings.clone(),
            face: self.face.clone(),
            distance: f("distance", &self.distance)?,
            mode: self.mode,
            angle: self.angle.as_ref().map(|a| f("angle", a)).transpose()?,
            direction: self.direction,
            flip: self.flip,
            corners: self.corners,
        })
    }
}

/// The unit vector of a direction of some length.
fn unit(direction: [f64; 3]) -> Result<[f64; 3], String> {
    let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || length < 1e-12 {
        return Err(format!(
            "the direction must have some length, got {direction:?}"
        ));
    }
    Ok(direction.map(|v| v / length))
}

impl FeatureInfo for RuledSurfaceDef {
    fn references(&self) -> References {
        let mut references = References::default();
        references.body(self.body);
        references.edges(&self.edges);
        sheet_edges::add_references(&mut references, &self.openings);
        add_faces(&mut references, &self.face);
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
        sheet_edges::check(ctx, &self.openings)?;
        if let Some(face) = &self.face {
            super::face_refs::check_faces(ctx, std::slice::from_ref(face), "face")?;
        }
        match self.mode {
            RuledMode::Direction if self.direction.is_none() => {
                return Err("a ruled surface along a direction needs the direction".to_owned());
            }
            RuledMode::Angle if self.angle.is_none() => {
                return Err("a ruled surface at an angle needs the angle".to_owned());
            }
            _ => {}
        }
        if let Some(direction) = self.direction {
            unit(direction)?;
        }
        Ok(())
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        let distance = value(self.distance);
        if !(distance > 0.0 && distance.is_finite()) {
            return Err(format!(
                "the ruled surface's distance must be greater than zero, got {distance}"
            ));
        }
        if let Some(angle) = self.angle
            && !value(angle).is_finite()
        {
            return Err("the ruled surface's angle must be a number".to_owned());
        }
        Ok(())
    }

    fn creates_bodies(&self) -> bool {
        true
    }
}

impl<K: Kernel> Evaluate<K> for RuledSurfaceDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let distance = ctx.positive(self.distance)?;
        let angle = match self.angle {
            Some(id) => ctx.param(id)?,
            None => 0.0,
        };
        let shape = ctx.surface_body(self.body)?;
        let edges = if self.edges.is_empty() {
            Vec::new()
        } else {
            ctx.require_edges(self.body, &shape, &self.edges)?
        };
        let resolved = sheet_edges::resolve(ctx, self.body, &shape, &self.openings)?;
        let openings = sheet_edges::kernel_openings(&resolved);
        let face = match &self.face {
            Some(face) => Some(ctx.face_on(self.body, &shape, face)?),
            None => None,
        };
        let spec = RuledSpec {
            feature: ctx.uid,
            body: &shape,
            edges: &edges,
            openings: &openings,
            face: face.as_ref(),
            distance,
            mode: self.mode,
            angle,
            direction: self.direction.map(unit).transpose()?,
            flip: self.flip,
            corners: self.corners,
        };
        let strip = ctx.kernel.ruled_surface(&spec).map_err(kernel_error)?;
        let mut output = FeatureOutput {
            tool: Some(strip.clone()),
            ..FeatureOutput::default()
        };
        output.changes.push(BodyChange::Set(ctx.new_body(), strip));
        Ok(output)
    }
}
