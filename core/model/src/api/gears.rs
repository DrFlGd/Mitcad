// SPDX-License-Identifier: MIT
//! Gears (mitcad#472): the `gear_values` query (the derived values a Gear
//! panel shows: diameters, pitch, the mesh's centre distance) and the
//! `link_gears` command (the motion link between two meshed gears'
//! joints). See `commands.md`, "Gears".

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use crate::document::Document;
use crate::expr::Expr;
use crate::features::gear::{self, GearDef, GearKind};
use crate::features::{FeatureDef, ValueInput};
use crate::ids::FeatureUid;
use crate::kernel::Kernel;
use crate::parameters::{slot_unit, unit_for};

/// The commands' names, to send them here.
pub(crate) const COMMANDS: [&str; 1] = ["link_gears"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GearValuesQuery {
    /// A gear feature of the timeline.
    #[serde(default)]
    uid: Option<FeatureUid>,
    /// Or a definition as `add_feature` takes it (a panel's).
    #[serde(default)]
    def: Option<Box<GearDef<ValueInput>>>,
}

impl<K: Kernel> Document<K> {
    /// A gear feature's definition with the document's values.
    pub(crate) fn gear_values_of(&self, uid: FeatureUid) -> Result<GearDef<f64>, ApiError> {
        let entry = self
            .feature(uid)
            .ok_or_else(|| ApiError(format!("feature {uid} does not exist")))?;
        let FeatureDef::Gear(def) = &entry.def else {
            return Err(ApiError(format!("{} is not a gear", entry.name)));
        };
        let params = self.parameters();
        Ok(gear::evaluated(def, &|id| {
            params.value(id).unwrap_or(f64::NAN)
        }))
    }

    /// A definition's values: numbers as they are (mm, radians), names and
    /// expressions evaluated in the slot's unit.
    fn gear_values_in(&self, def: &GearDef<ValueInput>) -> Result<GearDef<f64>, ApiError> {
        let params = self.parameters();
        let context = params.context();
        def.map_params(&mut |slot, value| match value {
            ValueInput::Number(n) => Ok(*n),
            ValueInput::Name(text) => {
                let unit =
                    unit_for(slot_unit(slot).dims(), &context).unwrap_or(crate::expr::Unit::NONE);
                let parsed = Expr::parse(text).map_err(|e| ApiError(format!("{slot}: {e}")))?;
                parsed
                    .eval_as(params.table(), &context, unit)
                    .map(|v| v.value)
                    .map_err(|e| ApiError(format!("{slot}: {e}")))
            }
        })
    }

    pub(super) fn gear_values_json(&self, query: &GearValuesQuery) -> Result<Value, ApiError> {
        let values = match (&query.uid, &query.def) {
            (Some(uid), None) => self.gear_values_of(*uid)?,
            (None, Some(def)) => self.gear_values_in(def)?,
            _ => {
                return Err(ApiError(
                    "gear_values takes a gear feature's uid or a definition".to_owned(),
                ));
            }
        };
        let mut result = gear::derived_values(&values);
        match values.check() {
            Ok(()) => {}
            Err(error) => result["error"] = json!(error),
        }
        if let Some(mesh) = &values.mesh_with {
            let mate = self.gear_values_of(mesh.gear)?;
            match gear::mesh_values(&values, &mate) {
                Ok((distance, ra, rb)) => {
                    result["centre_distance"] = json!(distance);
                    result["mate"] = json!(mesh.gear);
                    if values.kind != GearKind::Rack && mate.kind != GearKind::Rack {
                        result["ratio"] = json!(mate.teeth / values.teeth);
                    }
                    result["pitch_radii"] = json!([ra, rb]);
                }
                Err(error) => result["error"] = json!(error),
            }
        }
        Ok(result)
    }
}

impl<K: Kernel> Document<K> {
    /// The `add_motion_link` command `link_gears` stands for: the gear of
    /// `gear` and the gear it meshes with, each on a joint of its
    /// component's occurrence (a turn of a gear, a slide of a rack), coupled
    /// as gears (internal with a ring) or a rack and pinion by their pitch
    /// radii, the mate driving.
    pub(super) fn link_gears_json(&self, value: &Value) -> Result<Value, ApiError> {
        let gear: FeatureUid = value
            .get("gear")
            .cloned()
            .ok_or_else(|| ApiError("link_gears needs a gear".to_owned()))
            .and_then(|v| serde_json::from_value(v).map_err(|e| ApiError(format!("gear: {e}"))))?;
        if let Some(key) = value.as_object().and_then(|o| {
            o.keys()
                .find(|k| !["cmd", "gear", "name"].contains(&k.as_str()))
        }) {
            return Err(ApiError(format!("link_gears has no field {key}")));
        }
        let values = self.gear_values_of(gear)?;
        let mesh = values.mesh_with.as_ref().ok_or_else(|| {
            ApiError(format!(
                "{} meshes with no gear",
                self.feature(gear).map_or(String::new(), |f| f.name.clone())
            ))
        })?;
        let mate_uid = mesh.gear;
        let mate = self.gear_values_of(mate_uid)?;
        gear::mesh_values(&values, &mate).map_err(ApiError)?;
        let (a, b) = (
            self.gear_joint(mate_uid, mate.kind)?,
            self.gear_joint(gear, values.kind)?,
        );
        // The joints' motions against the gears' own: a link's relation
        // holds as it is when both run alike.
        let reverse = a.2 * b.2 < 0.0;
        // One link a mesh.
        for entry in self.features() {
            if let FeatureDef::MotionLink(link) = &entry.def {
                let joints = [link.a.joint, link.b.joint];
                if joints.contains(&a.0) && joints.contains(&b.0) {
                    return Err(ApiError(format!(
                        "the gears are linked already ({})",
                        entry.name
                    )));
                }
            }
        }
        let (ra, rb) = (mate.tooth().pitch_radius(), values.tooth().pitch_radius());
        let coupling = match (mate.kind, values.kind) {
            (GearKind::Rack, _) => json!({"type": "rack_pinion", "radius": rb}),
            (_, GearKind::Rack) => json!({"type": "rack_pinion", "radius": ra}),
            (m, n) => json!({"type": "gear", "ra": ra, "rb": rb,
                             "internal": m == GearKind::Ring || n == GearKind::Ring}),
        };
        let mut link = json!({
            "cmd": "add_motion_link",
            "a": {"joint": a.0.to_string(), "motion": a.1},
            "b": {"joint": b.0.to_string(), "motion": b.1},
            "coupling": coupling,
            "reverse": reverse,
        });
        if let Some(name) = value.get("name") {
            link["name"] = name.clone();
        }
        Ok(link)
    }

    /// The joint a gear feature's body moves on: a joint of an occurrence
    /// of the body's component that lets it turn (a rack: slide), and that
    /// motion.
    fn gear_joint(
        &self,
        uid: FeatureUid,
        kind: GearKind,
    ) -> Result<(FeatureUid, crate::joints::Motion, f64), ApiError> {
        use crate::joints::{JointKind, Motion};
        let name = self
            .feature(uid)
            .map_or(uid.to_string(), |f| f.name.clone());
        let component = self
            .bodies()
            .iter()
            .filter(|b| b.uid.feature == uid)
            .find_map(|b| self.body_component(b.uid))
            .ok_or_else(|| ApiError(format!("{name} has no body")))?;
        let occurrences: Vec<_> = self
            .assembly()
            .occurrences
            .iter()
            .filter(|o| o.component == component)
            .map(|o| o.uid)
            .collect();
        for entry in self.features() {
            let FeatureDef::Joint(joint) = &entry.def else {
                continue;
            };
            if self.status(entry.uid).is_none_or(|s| !s.is_ok()) {
                continue;
            }
            let on = [&joint.a, &joint.b].iter().any(|o| {
                o.occurrence
                    .0
                    .last()
                    .is_some_and(|u| occurrences.contains(u))
            });
            if !on {
                continue;
            }
            let motions = joint.kind.motions(joint.slide_axis);
            let motion = if kind == GearKind::Rack {
                motions.iter().copied().find(|m| {
                    matches!(m, Motion::Tx | Motion::Ty | Motion::Tz)
                        && (joint.kind != JointKind::Cylindrical || *m == Motion::Tz)
                })
            } else {
                motions.iter().copied().find(|m| *m == Motion::Rz)
            };
            if let Some(motion) = motion {
                let gear_on_b = joint
                    .b
                    .occurrence
                    .0
                    .last()
                    .is_some_and(|u| occurrences.contains(u));
                return Ok((
                    entry.uid,
                    motion,
                    self.gear_sense(uid, kind, entry.uid, motion, gear_on_b)?,
                ));
            }
        }
        Err(ApiError(format!(
            "{name} has no joint to {}: add a {} joint to its component's occurrence first",
            if kind == GearKind::Rack {
                "slide on"
            } else {
                "turn on"
            },
            if kind == GearKind::Rack {
                "slider"
            } else {
                "revolute"
            }
        )))
    }
}

impl<K: Kernel> Document<K> {
    /// +1 when the joint's motion runs as the gear's own (a turn
    /// counter-clockwise about its axis, a rack's slide along its x axis),
    /// -1 when against it.
    fn gear_sense(
        &self,
        gear: FeatureUid,
        kind: GearKind,
        joint: FeatureUid,
        motion: crate::joints::Motion,
        gear_on_b: bool,
    ) -> Result<f64, ApiError> {
        use crate::joints::Motion;
        let frame = self
            .last_result()
            .result(gear)
            .and_then(|r| r.output.as_ref())
            .and_then(|o| o.mounts.as_ref())
            .and_then(|m| m.get(gear::MOUNT).copied())
            .ok_or_else(|| ApiError(format!("{gear} has no result")))?;
        let result = self
            .joint_result(joint)
            .ok_or_else(|| ApiError(format!("joint {joint} is not in effect")))?;
        let column = |t: &crate::transform::Transform, c: usize| -> [f64; 3] {
            std::array::from_fn(|r| t.linear[r][c])
        };
        let own = column(&frame, if kind == GearKind::Rack { 0 } else { 2 });
        let index = match motion {
            Motion::Tx | Motion::Rx => 0,
            Motion::Ty | Motion::Ry => 1,
            Motion::Tz | Motion::Rz => 2,
        };
        let along = column(&result.frame_a, index);
        let dot: f64 = own.iter().zip(&along).map(|(a, b)| a * b).sum();
        if dot.abs() < 0.999 {
            return Err(ApiError(format!(
                "joint {joint}'s axis is not along {}'s {}",
                self.feature(gear)
                    .map_or(gear.to_string(), |f| f.name.clone()),
                if kind == GearKind::Rack {
                    "length"
                } else {
                    "axis"
                }
            )));
        }
        Ok(dot.signum() * if gear_on_b { 1.0 } else { -1.0 })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GearLinkQuery {
    gear: FeatureUid,
}

impl<K: Kernel> Document<K> {
    /// The `gear_link` query: the `add_motion_link` command `link_gears`
    /// would run for the gear, or the reason it cannot (a panel links the
    /// gears it meshes when it can).
    pub(super) fn gear_link_json(&self, query: &GearLinkQuery) -> Value {
        match self.link_gears_json(&json!({"cmd": "link_gears", "gear": query.gear})) {
            Ok(command) => json!({"command": command}),
            Err(error) => json!({"reason": error.0}),
        }
    }
}
