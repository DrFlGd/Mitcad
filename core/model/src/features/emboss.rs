// SPDX-License-Identifier: MIT
//! Emboss (mitcad#458): profiles of a sketch, texts too, raised from a face
//! or sunk into it by a depth. On a planar face, or the sketch plane when
//! no face is named, the profiles are extruded from the face's plane out of
//! the material (raise, joined) or into it (sink, cut). On a cylindrical
//! face they are wrapped onto the cylinder: laid out by arc length around
//! it, not projected, and raised or sunk radially, so that a profile keeps
//! its widths on the face and its walls stand at right angles to it.

use serde::{Deserialize, Serialize};

use super::extrude::{
    Operation, ProfileRef, apply_operation, check_participants, check_profiles, participant_bodies,
    profile_references, profile_regions, profile_set,
};
use super::mapping::map_params;
use super::reference::FaceRef;
use super::{CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References, ToolUse};
use crate::ids::BodyUid;
use crate::kernel::{
    Axis, Bound, EmbossWrapSpec, ExtrudeFeatureSpec, ExtrudeSide, ExtrudeStart, Kernel, Plane,
    SideEnd, Target,
};
use crate::parameters::ParamId;
use crate::profile::{ProfileRegion, SketchFrame, dot};

/// Whether the profiles stand up from the face or go down into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbossMode {
    /// Out of the material, joined to the body.
    #[default]
    Raise,
    /// Into the material, cut from the body.
    Sink,
}

impl EmbossMode {
    fn is_raise(&self) -> bool {
        *self == Self::Raise
    }

    /// The operation the tool takes part in.
    pub fn operation(self) -> Operation {
        match self {
            Self::Raise => Operation::Join,
            Self::Sink => Operation::Cut,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbossDef<P = ParamId> {
    /// Regions of one sketch, a set (as an extrusion's): the glyphs of its
    /// texts bound regions too.
    #[serde(deserialize_with = "profile_set")]
    pub profiles: Vec<ProfileRef>,
    /// The face the profiles are raised from or sunk into: a planar face,
    /// or a cylindrical face they are wrapped onto. None: the sketch plane,
    /// raised along the sketch normal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceRef>,
    pub depth: P,
    #[serde(default, skip_serializing_if = "EmbossMode::is_raise")]
    pub mode: EmbossMode,
    /// The bodies it joins or cuts; empty for the face's body, or all
    /// bodies without a face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
}

impl<P> EmbossDef<P> {
    pub const TYPE: &'static str = "emboss";
    pub const BASE_NAME: &'static str = "Emboss";

    map_params! {
        EmbossDef {
            profiles: clone,
            face: clone,
            depth: param,
            mode: copy,
            participants: clone,
        }
    }

    /// The bodies it works on when none are listed: the face's.
    fn default_participants(&self) -> Vec<BodyUid> {
        if self.participants.is_empty() {
            self.face.iter().map(|f| f.body).collect()
        } else {
            self.participants.clone()
        }
    }
}

impl FeatureInfo for EmbossDef {
    fn references(&self) -> References {
        let mut references = References::default();
        profile_references(&self.profiles, &mut references);
        if let Some(face) = &self.face {
            face.add_references(&mut references);
        }
        for body in &self.participants {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        check_profiles(ctx, &self.profiles)?;
        if let Some(face) = &self.face {
            face.check(ctx)?;
        }
        check_participants(ctx, self.mode.operation(), &self.participants)
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        let depth = value(self.depth);
        if depth > 0.0 && depth.is_finite() {
            Ok(())
        } else {
            Err(format!(
                "the emboss depth must be greater than zero, got {depth}"
            ))
        }
    }

    fn creates_bodies(&self) -> bool {
        // A sink can cut a body into pieces.
        true
    }

    fn tool_use(&self) -> Option<ToolUse> {
        Some(ToolUse {
            operation: self.mode.operation(),
            participants: self.default_participants(),
        })
    }

    fn profiles(&self) -> &[ProfileRef] {
        &self.profiles
    }
}

impl<K: Kernel> Evaluate<K> for EmbossDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let depth = ctx.param(self.depth)?;
        self.check_values(&|_| depth)?;
        let (sketch, regions) = profile_regions(ctx, &self.profiles)?;
        let operation = self.mode.operation();
        let participants = participant_bodies(ctx, operation, &self.default_participants())?;
        let tool = self.tool(ctx, sketch.frame, &regions, depth)?;
        apply_operation(ctx, operation, participants, tool, "emboss")
    }
}

/// Sketch normals within this of a plane's normal count as parallel, and
/// within this of right angles to a cylinder's axis as along it.
const PARALLEL: f64 = 1e-9;
const ALONG_AXIS: f64 = 1e-6;

/// The tool of profiles wrapped onto a cylindrical face by arc length, or
/// onto a conical one by its development (mitcad#473), raised out of the
/// material by `depth` or sunk into it; None for a face of another kind.
/// Emboss and Extrude's `wrap` build it alike.
pub(crate) fn wrapped_tool<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    frame: SketchFrame,
    regions: &[ProfileRegion],
    face: &FaceRef,
    depth: f64,
    raise: bool,
) -> Result<Option<K::Shape>, String> {
    let normal = frame.normal();
    let shape = ctx.surface_body(face.body)?;
    let found = ctx.face_on(face.body, &shape, &face.face)?;
    if let Ok(cylinder) = ctx.kernel.face_cylinder(&shape, &found) {
        let axis = cylinder.axis;
        if dot(normal, axis.direction).abs() > ALONG_AXIS {
            return Err(format!(
                "to wrap onto face {}, the sketch plane must be parallel to the \
                 cylinder's axis",
                face.face
            ));
        }
        // Out of the material: away from the axis on an outer face,
        // towards it on a bore's wall.
        let outwards = if cylinder.internal { -1.0 } else { 1.0 };
        let along = if raise { outwards } else { -outwards };
        let to_radius = cylinder.radius + along * depth;
        if to_radius <= cylinder.radius * 1e-6 {
            return Err(format!(
                "the depth {depth} mm reaches the axis of face {} (radius {} mm)",
                face.face, cylinder.radius
            ));
        }
        let spec = EmbossWrapSpec {
            feature: ctx.uid,
            frame,
            regions,
            axis,
            radius: cylinder.radius,
            to_radius,
            half_angle: 0.0,
        };
        return ctx
            .kernel
            .wrapped_emboss(&spec)
            .map(Some)
            .map_err(|e| e.to_string());
    }
    if let Ok(cone) = ctx.kernel.face_cone(&shape, &found) {
        if dot(normal, cone.axis).abs() > ALONG_AXIS {
            return Err(format!(
                "to wrap onto face {}, the sketch plane must be parallel to the cone's axis",
                face.face
            ));
        }
        // The cone's radius at the height of the sketch's origin.
        let from: [f64; 3] = std::array::from_fn(|i| frame.origin[i] - cone.apex[i]);
        let height = dot(from, cone.axis);
        let radius = height * cone.half_angle.tan();
        if radius.is_nan() || radius <= 0.0 {
            return Err(format!(
                "to wrap onto face {}, the sketch's origin must lie beside the cone, not \
                 beyond its apex",
                face.face
            ));
        }
        let outwards = if cone.internal { -1.0 } else { 1.0 };
        let offset = if raise { outwards } else { -outwards } * depth;
        if radius + offset * cone.half_angle.cos() <= radius * 1e-6 {
            return Err(format!(
                "the depth {depth} mm reaches the axis of face {}",
                face.face
            ));
        }
        let spec = EmbossWrapSpec {
            feature: ctx.uid,
            frame,
            regions,
            axis: Axis {
                origin: cone.apex,
                direction: cone.axis,
            },
            radius,
            to_radius: radius + offset,
            half_angle: cone.half_angle,
        };
        return ctx
            .kernel
            .wrapped_emboss(&spec)
            .map(Some)
            .map_err(|e| e.to_string());
    }
    Ok(None)
}

impl EmbossDef {
    /// The tool body: the profiles extruded from a plane, or wrapped onto
    /// a cylinder and grown radially; faces named after the feature.
    fn tool<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
        frame: SketchFrame,
        regions: &[ProfileRegion],
        depth: f64,
    ) -> Result<K::Shape, String> {
        let normal = frame.normal();
        let raise = self.mode == EmbossMode::Raise;
        let Some(face) = &self.face else {
            // From the sketch plane, raised along its normal.
            let direction = if raise { normal } else { normal.map(|v| -v) };
            return self.extrude(
                ctx,
                frame,
                regions,
                direction,
                ExtrudeStart::Offset(0.0),
                depth,
            );
        };
        if let Some(tool) = wrapped_tool(ctx, frame, regions, face, depth, raise)? {
            return Ok(tool);
        }
        let shape = ctx.surface_body(face.body)?;
        let found = ctx.face_on(face.body, &shape, &face.face)?;
        let plane = ctx.kernel.face_plane(&shape, &found).map_err(|e| {
            format!(
                "face {} is neither planar, cylindrical nor conical, which an emboss needs: {e}",
                face.face
            )
        })?;
        // Along the sketch normal, out of the material or into it.
        let facing = dot(normal, plane.normal);
        if facing.abs() < ALONG_AXIS {
            return Err(format!(
                "the sketch plane stands at right angles to face {}",
                face.face
            ));
        }
        let out = if facing > 0.0 {
            normal
        } else {
            normal.map(|v| -v)
        };
        let direction = if raise { out } else { out.map(|v| -v) };
        let start = if facing.abs() > 1.0 - PARALLEL {
            let offset: [f64; 3] = std::array::from_fn(|i| plane.origin[i] - frame.origin[i]);
            ExtrudeStart::Offset(dot(offset, normal) + 0.0)
        } else {
            // The profiles projected along the sketch normal onto the
            // face's plane.
            ExtrudeStart::Object(Bound {
                target: Target::Plane(Plane {
                    origin: plane.origin,
                    normal: plane.normal,
                }),
                offset: 0.0,
            })
        };
        self.extrude(ctx, frame, regions, direction, start, depth)
    }

    fn extrude<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
        frame: SketchFrame,
        regions: &[ProfileRegion],
        direction: [f64; 3],
        start: ExtrudeStart<K::Shape>,
        depth: f64,
    ) -> Result<K::Shape, String> {
        let spec = ExtrudeFeatureSpec {
            feature: ctx.uid,
            frame,
            regions,
            direction,
            start,
            side1: ExtrudeSide {
                end: SideEnd::Distance(depth),
                taper: 0.0,
                thin: None,
            },
            side2: None,
        };
        ctx.kernel.extrude_feature(&spec).map_err(|e| e.to_string())
    }
}
