// SPDX-License-Identifier: MIT
//! Helix: profiles of a sketch turned about an axis while they move along
//! it (a screw motion: FreeCAD's additive and subtractive helices,
//! mitcad#4), into a tool body with the operations of an extrusion. Every
//! section of the tool in a plane through the axis is the profile turned
//! there; the coil's sections are fixed shapes at right angles to its
//! helix instead. A growth also moves the profiles out as they turn, by
//! Mitcad's construction or FreeCAD's ([`HelixConstruction`], mitcad#83).

use serde::{Deserialize, Serialize};

use super::extrude::{
    Operation, ProfileRef, apply_operation, check_participants, check_profiles, participant_bodies,
    profile_references, profile_regions,
};
use super::geom_ref::{GeomRef, Want};
use super::{
    CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References, ToolUse, is_false,
};
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::sweeps::HelixSpec;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelixDef<P = ParamId> {
    /// Regions of one sketch, a set (see `extrude::selected_regions`).
    #[serde(deserialize_with = "super::extrude::profile_set")]
    pub profiles: Vec<ProfileRef>,
    /// A sketch line, an origin or construction axis, a straight edge or a
    /// fixed axis: the profiles move along its direction.
    pub axis: GeomRef,
    /// The rise per turn along the axis.
    pub pitch: P,
    /// The turns (need not be whole).
    pub revolutions: P,
    /// Turns left-handed about the direction of travel instead of
    /// right-handed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub left_handed: bool,
    /// Moves against the axis direction.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip: bool,
    /// How far the profiles move out from the axis per turn (mitcad#59),
    /// widening the helix (a cone's growth is the pitch times the tangent
    /// of its angle). Negative narrows it.
    #[serde(
        default = "super::pattern::none",
        skip_serializing_if = "Option::is_none"
    )]
    pub growth: Option<P>,
    /// How a growth is built (mitcad#83): Mitcad's own construction unless
    /// FreeCAD's ([`HelixConstruction`]). A command without it means
    /// Mitcad's; a stored growing helix always has it, except in files
    /// written before it existed, which mean FreeCAD's
    /// ([`HelixDef::settle_construction`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction: Option<HelixConstruction>,
    pub operation: Operation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
    /// How the helix starts and ends (mitcad#235): none for the pitch all
    /// the way (see [`HelixEnd`]).
    #[serde(
        default = "super::pattern::none",
        skip_serializing_if = "Option::is_none"
    )]
    pub start: Option<HelixEnd<P>>,
    #[serde(
        default = "super::pattern::none",
        skip_serializing_if = "Option::is_none"
    )]
    pub end: Option<HelixEnd<P>>,
}

/// A flat end of a helix (mitcad#235), as a coil's: over `transition_angle` (an
/// angle) the pitch changes linearly between the helix's and zero at the
/// outer end, and over `flat_angle` (an angle) the profiles turn without rising,
/// beyond the transition. Both come in addition to the helix's turns (its
/// `revolutions` are those at the full pitch). With ends a growth follows
/// the rise: the profiles move out by the growth for every pitch they rise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelixEnd<P = ParamId> {
    pub transition_angle: P,
    pub flat_angle: P,
}

impl<P> HelixEnd<P> {
    fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
        side: &str,
    ) -> Result<HelixEnd<Q>, E> {
        Ok(HelixEnd {
            transition_angle: f(&format!("{side}.transition_angle"), &self.transition_angle)?,
            flat_angle: f(&format!("{side}.flat_angle"), &self.flat_angle)?,
        })
    }
}

fn end_angle(value: f64, what: &str) -> Result<f64, String> {
    if value >= 0.0 && value.is_finite() {
        Ok(value)
    } else {
        Err(format!(
            "the helix's {what} must be an angle of zero or more, got {value}"
        ))
    }
}

/// How a helix with a growth moves its profiles (mitcad#83).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelixConstruction {
    /// Mitcad's: the screw motion with the profiles moved out along the
    /// direction from the axis towards them, in proportion to the turn.
    Mitcad,
    /// FreeCAD's construction of its conical and growing helices, for the
    /// FreeCAD import (the profiles follow the Frenet frame of a far
    /// spiral; a narrowing helix whose profiles sit at the axis' origin
    /// runs the other way).
    Freecad,
}

impl<P> HelixDef<P> {
    pub const TYPE: &'static str = "helix";
    pub const BASE_NAME: &'static str = "Helix";

    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<HelixDef<Q>, E> {
        Ok(HelixDef {
            profiles: self.profiles.clone(),
            axis: self.axis.clone(),
            pitch: f("pitch", &self.pitch)?,
            revolutions: f("revolutions", &self.revolutions)?,
            left_handed: self.left_handed,
            flip: self.flip,
            growth: self.growth.as_ref().map(|g| f("growth", g)).transpose()?,
            construction: self.construction,
            operation: self.operation,
            participants: self.participants.clone(),
            start: self
                .start
                .as_ref()
                .map(|e| e.map_params(f, "start"))
                .transpose()?,
            end: self
                .end
                .as_ref()
                .map(|e| e.map_params(f, "end"))
                .transpose()?,
        })
    }

    /// Gives a growing helix without a construction `construction`: Mitcad's
    /// for a command, FreeCAD's for a file written before mitcad#83 (its
    /// growing helices came from the FreeCAD import, built that way).
    pub(crate) fn settle_construction(&mut self, construction: HelixConstruction) {
        if self.growth.is_some() && self.construction.is_none() {
            self.construction = Some(construction);
        }
    }
}

fn positive(value: f64, what: &str) -> Result<f64, String> {
    if value > 0.0 && value.is_finite() {
        Ok(value)
    } else {
        Err(format!(
            "the helix's {what} must be greater than zero, got {value}"
        ))
    }
}

fn finite_growth(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!(
            "the helix's growth must be a finite length, got {value}"
        ))
    }
}

impl FeatureInfo for HelixDef {
    fn references(&self) -> References {
        let mut references = References::default();
        profile_references(&self.profiles, &mut references);
        self.axis.add_to(&mut references);
        for body in &self.participants {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        check_profiles(ctx, &self.profiles)?;
        check_participants(ctx, self.operation, &self.participants)?;
        self.axis.check(ctx, Want::Line)
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        positive(value(self.pitch), "pitch")?;
        positive(value(self.revolutions), "revolutions")?;
        if let Some(g) = self.growth {
            finite_growth(value(g))?;
        }
        for (side, end) in [("start", &self.start), ("end", &self.end)] {
            if let Some(e) = end {
                end_angle(value(e.transition_angle), &format!("{side} transition"))?;
                end_angle(value(e.flat_angle), &format!("{side} flat"))?;
            }
        }
        if self.construction == Some(HelixConstruction::Freecad)
            && (self.start.is_some() || self.end.is_some())
        {
            return Err("a helix with flat ends is built by Mitcad's construction".to_owned());
        }
        Ok(())
    }

    fn creates_bodies(&self) -> bool {
        true
    }

    fn profiles(&self) -> &[ProfileRef] {
        &self.profiles
    }

    fn new_component(&self) -> bool {
        self.operation == Operation::NewComponent
    }

    fn tool_use(&self) -> Option<ToolUse> {
        Some(ToolUse {
            operation: self.operation,
            participants: self.participants.clone(),
        })
    }
}

impl<K: Kernel> Evaluate<K> for HelixDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let (sketch, regions) = profile_regions(ctx, &self.profiles)?;
        let participants = participant_bodies(ctx, self.operation, &self.participants)?;
        let line = ctx.axis(&self.axis)?;
        let length = line.direction.iter().map(|v| v * v).sum::<f64>().sqrt();
        if length.is_nan() || length <= 1e-12 {
            return Err("the helix's axis has no direction".to_owned());
        }
        let sign = if self.flip { -1.0 } else { 1.0 };
        let direction = line.direction.map(|v| sign * v / length);
        let pitch = positive(ctx.param(self.pitch)?, "pitch")?;
        let revolutions = positive(ctx.param(self.revolutions)?, "revolutions")?;
        let growth = match self.growth {
            Some(g) => finite_growth(ctx.param(g)?)?,
            None => 0.0,
        };
        let bodies: Vec<K::Shape> = participants.iter().map(|(_, s)| s.clone()).collect();
        let mut ends = [0.0; 4];
        if let Some(e) = &self.start {
            ends[0] = end_angle(ctx.param(e.flat_angle)?, "start flat")?;
            ends[1] = end_angle(ctx.param(e.transition_angle)?, "start transition")?;
        }
        if let Some(e) = &self.end {
            ends[2] = end_angle(ctx.param(e.transition_angle)?, "end transition")?;
            ends[3] = end_angle(ctx.param(e.flat_angle)?, "end flat")?;
        }
        let spec = HelixSpec {
            feature: ctx.uid,
            frame: sketch.frame,
            regions: &regions,
            origin: line.origin,
            direction,
            pitch,
            revolutions,
            left_handed: self.left_handed,
            growth,
            flip: self.flip,
            freecad: self.construction == Some(HelixConstruction::Freecad),
            bodies: &bodies,
            ends,
        };
        let tool = ctx.kernel.helix(&spec).map_err(|e| e.to_string())?;
        apply_operation(ctx, self.operation, participants, tool, "helix")
    }
}
