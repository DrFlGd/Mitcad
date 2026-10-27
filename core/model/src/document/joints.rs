// SPDX-License-Identifier: MIT
//! Commands and views of joints (mitcad#55; the kinematics are in
//! `crate::joints`, the features in `features/joint.rs`): joints, as-built
//! joints that record where their occurrences are, rigid groups, the
//! joints' state and degrees of freedom at the marker, driving their
//! motions and dragging occurrences while they hold.

use std::collections::{BTreeMap, BTreeSet};

use super::{Added, Document, ModelError, invalid};
use crate::assembly::inverse;
use crate::contact::{ContactHit, ContactPairs, sweep};
use crate::datum::Vec3;
use crate::features::{AsBuiltJointDef, FeatureDef, ValueInput};
use crate::ids::{ComponentUid, FeatureUid, OccurrenceUid};
use crate::joints::{DofReport, Dragged, JointResult, Motion, dof_report, drag, path_component};
use crate::kernel::Kernel;
// The joint origins and constraint rows before they are added (mitcad#778).
use crate::constraints::{
    Geo, RowCheck, RowReference, constraint_fits, fitting_types, measures, resolve_geo,
};
use crate::features::{ConstraintType, FrameOverride, GeomRef, JointOrigin, OccurrencePath};
use crate::joints::{OriginFrame, apply_override, origin_frame};

/// The largest step (mm) of the grabbed point between two samples of a
/// drag checked against the contact sets, and the most samples.
const DRAG_STEP: f64 = 2.0;
const DRAG_SAMPLES: usize = 16;

/// What [`Document::drive_joint`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct Driven {
    /// The parameters it created.
    pub parameters: Vec<String>,
    /// The contact that stopped the joint short of the values (mitcad#454)
    /// and how far it went toward them, 0 to 1.
    pub contact: Option<(ContactHit, f64)>,
}

impl<K: Kernel> Document<K> {
    /// What a joint or as-built joint did in the last recompute; None when
    /// it failed, is suppressed or after the marker.
    pub fn joint_result(&self, uid: FeatureUid) -> Option<&JointResult> {
        self.result.joints.joints.get(&uid)
    }

    /// What a constraint set did in the last recompute (mitcad#148); None
    /// when it failed, is suppressed or after the marker.
    pub fn constraint_result(
        &self,
        uid: FeatureUid,
    ) -> Option<&crate::constraints::ConstraintSetResult> {
        self.result.joints.constraints.get(&uid)
    }

    /// The rigid groups in effect at the marker: feature, component and
    /// occurrences.
    pub fn rigid_groups(&self) -> &[(FeatureUid, ComponentUid, Vec<OccurrenceUid>)] {
        &self.result.joints.groups
    }

    /// The remaining degrees of freedom of a component's top-level
    /// occurrences at the marker (the rank of the joint equations; see
    /// [`DofReport`]).
    pub fn joint_dof(&self, component: ComponentUid) -> DofReport {
        dof_report(
            &self.state.assembly,
            &self.result.joints,
            &self.result.placements,
            component,
        )
    }

    /// Adds an as-built joint to `component` (the active one when None):
    /// with `relative` left out, it records where `a` is in `b`'s
    /// coordinates at the marker (paths from `component`: the product of
    /// their placements is in its coordinates).
    pub fn add_as_built_joint(
        &mut self,
        def: &AsBuiltJointDef<ValueInput>,
        name: Option<&str>,
        component: Option<ComponentUid>,
    ) -> Result<Added, ModelError> {
        let component = component.unwrap_or(self.state.assembly.active);
        let a = &self.state.assembly;
        for (label, path) in [("a", &def.a.0), ("b", &def.b.0)] {
            path_component(a, component, path).map_err(|e| invalid(format!("{label}: {e}")))?;
        }
        let mut def = def.clone();
        if def.relative.is_none() {
            let pa = self.path_transform(&def.a.0);
            let pb = self.path_transform(&def.b.0);
            def.relative = Some(inverse(&pb).after(&pa));
        }
        self.add_feature_to(&FeatureDef::AsBuiltJoint(def), name, Some(component))
    }

    /// A joint feature's definition with its parameters by name, to edit.
    fn joint_input(&self, uid: FeatureUid) -> Result<FeatureDef<ValueInput>, ModelError> {
        super::motion::joint_def(&self.state, uid)
    }

    /// Drives a joint's free motions: each value (mm, angles; parameters and
    /// expressions allowed) is stored as the joint's position and the
    /// motion is held there; None takes the position away (the rest value,
    /// else free). With contact sets (mitcad#454) the joint moves from
    /// where it is toward the values and stops at the first contact: the
    /// positions are then where it stopped (numbers). One undo step.
    pub fn drive_joint(
        &mut self,
        uid: FeatureUid,
        values: &BTreeMap<Motion, Option<ValueInput>>,
    ) -> Result<Driven, ModelError> {
        let mut def = self.joint_input(uid)?;
        // A cam's follower follows the cam (mitcad#466).
        for (motion, value) in values {
            if value.is_none() {
                continue;
            }
            for f in self.state.features.iter().filter(|f| !f.suppressed) {
                if let FeatureDef::MotionLink(l) = &f.def
                    && matches!(l.coupling, crate::features::Coupling::Cam { .. })
                    && l.b.joint == uid
                    && l.b.motion == *motion
                {
                    let cam = self
                        .state
                        .entry(l.a.joint)
                        .map_or_else(|| l.a.joint.to_string(), |e| e.name.clone());
                    return Err(invalid(format!(
                        "{motion} follows the cam of {}: drive {cam} instead",
                        f.name
                    )));
                }
            }
        }
        // Where the given values take it.
        let mut to = BTreeMap::new();
        for (motion, value) in values {
            if let Some(v) = value {
                to.insert(*motion, self.motion_value(v, *motion)?);
            }
        }
        let mut contact = None;
        if !to.is_empty() && self.has_contact_pairs() {
            let outcome = self.drive_to(&BTreeMap::from([(uid, to)]))?;
            if outcome.contact.is_some() {
                contact = outcome.contact.map(|hit| (hit, outcome.fraction));
                let reached = outcome.values.get(&uid).cloned().unwrap_or_default();
                let position = match &mut def {
                    FeatureDef::Joint(j) => &mut j.position,
                    FeatureDef::AsBuiltJoint(j) => &mut j.position,
                    _ => unreachable!("a joint"),
                };
                for (motion, v) in reached {
                    position.insert(motion, ValueInput::Number(v));
                }
            }
        }
        let position = match &mut def {
            FeatureDef::Joint(j) => &mut j.position,
            FeatureDef::AsBuiltJoint(j) => &mut j.position,
            _ => unreachable!("a joint"),
        };
        for (motion, value) in values {
            match value {
                Some(v) if contact.is_none() => {
                    position.insert(*motion, v.clone());
                }
                Some(_) => {}
                None => {
                    position.remove(motion);
                }
            };
        }
        let entry = self.state.entry(uid).expect("found");
        self.state.editable(entry.component)?;
        let name = entry.name.clone();
        // The motions linked to the driven ones follow them (mitcad#466).
        let driven: Vec<(FeatureUid, Motion)> = values
            .iter()
            .filter(|(_, v)| v.is_some())
            .map(|(m, _)| (uid, *m))
            .collect();
        let parameters = self.apply(|state| {
            let created = state.edit_feature(uid, &def)?;
            state.release_linked(&driven)?;
            Ok((format!("Drive {name}"), created))
        })?;
        Ok(Driven {
            parameters,
            contact,
        })
    }

    /// Where dragging `occurrence` would put the occurrences of its parent
    /// component: `grab` (in the parent's coordinates, the occurrence's
    /// origin when None) pulled toward `target` while the joints in effect
    /// at the marker hold, the other occurrences moving as little as
    /// possible. Driven motions follow like free ones. Changes nothing.
    pub fn joint_drag(
        &self,
        occurrence: OccurrenceUid,
        grab: Option<Vec3>,
        target: Vec3,
    ) -> Result<Dragged, ModelError> {
        self.state.require_occurrence(occurrence)?;
        let grab = grab.unwrap_or_else(|| {
            self.placement(occurrence)
                .map_or([0.0; 3], |p| p.translation)
        });
        if !grab.iter().chain(&target).all(|v| v.is_finite()) {
            return Err(invalid("the points must be finite"));
        }
        let drag_to = |t: f64| {
            let at: Vec3 = std::array::from_fn(|i| grab[i] + (target[i] - grab[i]) * t);
            drag(
                &self.state.assembly,
                &self.result.joints,
                &self.result.placements,
                occurrence,
                grab,
                at,
            )
            .map_err(invalid)
        };
        // Contact sets (mitcad#454): the drag stops where bodies touch.
        let pairs = ContactPairs::new(&self.kernel, &self.state, &self.result);
        if pairs.is_empty() {
            return drag_to(1.0);
        }
        let failed = |e: crate::kernel::KernelError| invalid(format!("contact check: {e}"));
        let skip: BTreeSet<usize> = pairs
            .overlapping(&self.kernel, &self.result.placements, &BTreeSet::new())
            .map_err(failed)?
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        let distance = (0..3)
            .map(|i| (target[i] - grab[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        let samples = ((distance / DRAG_STEP).ceil() as usize).clamp(1, DRAG_SAMPLES);
        let stop = sweep(
            samples,
            |t| -> Result<(Dragged, Option<ContactHit>), ModelError> {
                let dragged = drag_to(t)?;
                let mut placements = self.result.placements.clone();
                placements.extend(dragged.placements.iter().copied());
                let deepest = pairs
                    .overlapping(&self.kernel, &placements, &skip)
                    .map_err(failed)?
                    .into_iter()
                    .max_by(|a, b| a.1.total_cmp(&b.1));
                Ok((dragged, deepest.map(|(p, v)| pairs.hit(p, v))))
            },
        )?;
        let mut dragged = stop.pose;
        dragged.contact = stop.hit;
        Ok(dragged)
    }

    /// Drags `occurrence` (see [`Document::joint_drag`]) and keeps the
    /// result: the moved occurrences' own placements become where the drag
    /// put them, and the joints' driven motions are driven to their new
    /// values. One undo step.
    pub fn drag_occurrence(
        &mut self,
        occurrence: OccurrenceUid,
        grab: Option<Vec3>,
        target: Vec3,
    ) -> Result<Dragged, ModelError> {
        let dragged = self.joint_drag(occurrence, grab, target)?;
        let mut edits = Vec::new();
        for joint in &self.result.joints.active {
            let Some(values) = dragged.values.get(&joint.uid) else {
                continue;
            };
            let moved: BTreeMap<Motion, Option<ValueInput>> = joint
                .motions
                .iter()
                .filter_map(|spec| {
                    let target = spec.target?;
                    let v = *values.get(&spec.motion)?;
                    ((v - target).abs() > 1e-12 * (1.0 + v.abs()))
                        .then_some((spec.motion, Some(ValueInput::Number(v))))
                })
                .collect();
            if moved.is_empty() {
                continue;
            }
            let mut def = self.joint_input(joint.uid)?;
            let position = match &mut def {
                FeatureDef::Joint(j) => &mut j.position,
                FeatureDef::AsBuiltJoint(j) => &mut j.position,
                _ => unreachable!("a joint"),
            };
            for (m, v) in moved {
                position.insert(m, v.expect("a value"));
            }
            edits.push((joint.uid, def));
        }
        let name = self.state.assembly.occurrence_name(occurrence);
        let placements = dragged.placements.clone();
        self.apply(|state| {
            for (o, transform) in &placements {
                let by = state.positioning_features(*o);
                if !by.is_empty() {
                    return Err(invalid(format!(
                        "{} is placed by {} in the timeline; capture the position instead",
                        state.assembly.occurrence_name(*o),
                        by.join(", ")
                    )));
                }
                state.assembly.occurrence_mut(*o).expect("exists").transform = *transform;
            }
            for (uid, def) in &edits {
                state.edit_feature(*uid, def)?;
            }
            Ok((format!("Drag {name}"), ()))
        })?;
        Ok(dragged)
    }
}

// Joint origins and constraint rows before they are added (mitcad#778: the
// api's `joint_frame` and `constraint_row` queries answer with these, and
// the import calls them without the api).

impl<K: Kernel> Document<K> {
    /// An occurrence path from `component`: uids (`O1/O4`), names
    /// (`Arm:1/Pin:2`), or empty for the component's own geometry.
    pub fn occurrence_path_from(
        &self,
        component: ComponentUid,
        text: &str,
    ) -> Result<Vec<OccurrenceUid>, String> {
        let a = self.assembly();
        crate::joints::find_path_from(a, component, text)
            .ok_or_else(|| format!("no occurrence {text} in {}", a.name(component)))
    }

    /// The frame a joint origin gives at the marker, as a joint would
    /// resolve it: `geometry` on the occurrence path `occurrence` from
    /// `from` ([`Document::occurrence_path_from`]), with `frame_override`,
    /// placed by the path's occurrences into the coordinates of `from`.
    /// The geometry must be the component's the path ends in.
    pub fn joint_frame(
        &self,
        from: ComponentUid,
        occurrence: &str,
        geometry: &GeomRef,
        frame_override: Option<&FrameOverride>,
    ) -> Result<OriginFrame, String> {
        let a = self.assembly();
        let path = self.occurrence_path_from(from, occurrence)?;
        let at = path_component(a, from, &path)?;
        let owner = match geometry {
            GeomRef::Face { body, .. }
            | GeomRef::Edge { body, .. }
            | GeomRef::Vertex { body, .. }
            | GeomRef::Body(body) => Some(
                self.body_component(*body)
                    .ok_or_else(|| format!("body {body} does not exist at the marker"))?,
            ),
            GeomRef::Datum(uid)
            | GeomRef::SketchCurves { sketch: uid, .. }
            | GeomRef::SketchPoint { sketch: uid, .. }
            | GeomRef::Mount { feature: uid, .. } => Some(
                self.feature(*uid)
                    .ok_or_else(|| format!("feature {uid} does not exist"))?
                    .component,
            ),
            _ => None,
        };
        if let Some(owner) = owner
            && owner != at
        {
            return Err(format!(
                "{} is in {}, not in {}",
                geometry,
                a.name(owner),
                a.name(at)
            ));
        }
        let mut local = origin_frame(&mut self.resolver(), geometry)?;
        if let Some(o) = frame_override {
            local = apply_override(&local, o)?;
        }
        Ok(OriginFrame {
            component: at,
            frame: self.path_transform(&path).after(&local),
            local,
        })
    }

    /// A reference of a constraint row at the marker, its frame placed by
    /// its path into `component`'s coordinates.
    fn row_geo(
        &self,
        component: ComponentUid,
        reference: &RowReference<'_>,
    ) -> Result<Geo, String> {
        let path = self.occurrence_path_from(component, reference.occurrence)?;
        let origin = JointOrigin {
            occurrence: OccurrencePath(path.clone()),
            geometry: reference.geometry.clone(),
            frame_override: reference.frame_override,
        };
        let mut geo = resolve_geo(&mut self.resolver(), &origin)?;
        geo.frame = self.path_transform(&path).after(&geo.frame);
        Ok(geo)
    }

    /// A constraint row's references before the row is added, in
    /// `component` (paths from it): their kinds, the types they fit (the
    /// likeliest first), what they measure where they are (an align's
    /// `offset`, the `angle` between their z axes, an insert's `twist`: the
    /// turn a locked insert records), and for a `kind` whether they fit
    /// it. A reference that does not resolve fails with its key (`a: …`).
    pub fn constraint_row(
        &self,
        component: ComponentUid,
        a: &RowReference<'_>,
        b: &RowReference<'_>,
        a2: Option<&RowReference<'_>>,
        b2: Option<&RowReference<'_>>,
        kind: Option<ConstraintType>,
    ) -> Result<RowCheck, String> {
        let geo = |key: &str, reference: &RowReference<'_>| {
            self.row_geo(component, reference)
                .map_err(|e| format!("{key}: {e}"))
        };
        let a = geo("a", a)?;
        let b = geo("b", b)?;
        let a2 = a2.map(|r| geo("a2", r)).transpose()?;
        let b2 = b2.map(|r| geo("b2", r)).transpose()?;
        Ok(RowCheck {
            types: fitting_types(&a, &b),
            measures: measures(&a, &b),
            fits: kind.map(|kind| constraint_fits(kind, &a, a2.as_ref(), &b, b2.as_ref())),
            a,
            b,
        })
    }
}
