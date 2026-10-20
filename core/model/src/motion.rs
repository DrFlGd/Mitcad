// SPDX-License-Identifier: MIT
//! Motion studies and contact sets of assemblies (mitcad#454).
//!
//! A **motion study** is a named timeline of `steps` steps along which
//! joints follow keys: per joint and free motion (a cylindrical joint's
//! `tz` and `rz` each), `(step, value)` points in millimetres or radians,
//! linear in between and held before the first and after the last point.
//! Playing a study drives the keyed joints to their values at a step
//! through the joint solver, as Drive Joint does (`document/motion.rs`).
//!
//! A **contact set** names occurrences (paths from the root) whose bodies
//! may touch but not pass through each other while joints move: driving a
//! joint, dragging, animating and playing a study stop at the first
//! contact (`crate::contact`). A suppressed set does nothing.
//!
//! Both are definition state (not timeline features): saved in the design
//! file, changed by undoable commands that recompute nothing. The shapes
//! follow the `.solidcraft` format's (`crates/doc/src/motion.rs` of
//! SolidCraft), with a joint's motion named instead of numbered.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::features::{OccurrencePath, is_false};
use crate::ids::FeatureUid;
use crate::joints::Motion;

/// The most steps a study may have.
pub const MAX_STEPS: u32 = 100_000;
/// The steps of a new study when none are given.
pub const DEFAULT_STEPS: u32 = 100;

/// A motion study: joints keyed along `steps` steps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionStudy {
    pub name: String,
    pub steps: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<StudyKey>,
}

/// One free motion of a joint along a study: `(step, value)` points sorted
/// by step, values in millimetres or radians.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyKey {
    pub joint: FeatureUid,
    pub motion: Motion,
    pub points: Vec<(f64, f64)>,
}

impl StudyKey {
    /// The value at a step: linear between the points around it, the
    /// first point's before the first, the last's after the last; None
    /// without points.
    pub fn value_at(&self, step: f64) -> Option<f64> {
        let first = self.points.first()?;
        let last = self.points.last()?;
        if step <= first.0 {
            return Some(first.1);
        }
        if step >= last.0 {
            return Some(last.1);
        }
        self.points.windows(2).find_map(|w| {
            let (a, b) = (w[0], w[1]);
            (step >= a.0 && step <= b.0).then(|| {
                if b.0 - a.0 < 1e-12 {
                    b.1
                } else {
                    a.1 + (b.1 - a.1) * (step - a.0) / (b.0 - a.0)
                }
            })
        })
    }

    /// Adds a point, or replaces the value of the point at the same step.
    pub fn set_point(&mut self, step: f64, value: f64) {
        match self
            .points
            .iter_mut()
            .find(|(s, _)| (*s - step).abs() < 1e-9)
        {
            Some(point) => point.1 = value,
            None => {
                self.points.push((step, value));
                self.points.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
        }
    }
}

impl MotionStudy {
    /// A new study without keys.
    pub fn new(name: impl Into<String>, steps: u32) -> Self {
        Self {
            name: name.into(),
            steps,
            keys: Vec::new(),
        }
    }

    /// Why the study cannot be kept, if it cannot (not its joints, which
    /// the document checks).
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("the motion study needs a name".to_owned());
        }
        if self.steps == 0 || self.steps > MAX_STEPS {
            return Err(format!("{}: the steps must be 1 to {MAX_STEPS}", self.name));
        }
        for (i, key) in self.keys.iter().enumerate() {
            if self.keys[..i]
                .iter()
                .any(|k| k.joint == key.joint && k.motion == key.motion)
            {
                return Err(format!(
                    "{}: {} {} is keyed more than once",
                    self.name, key.joint, key.motion
                ));
            }
            if key.points.is_empty() {
                return Err(format!(
                    "{}: the key of {} {} has no points",
                    self.name, key.joint, key.motion
                ));
            }
            if key
                .points
                .iter()
                .any(|(s, v)| !s.is_finite() || !v.is_finite())
            {
                return Err(format!(
                    "{}: the key of {} {} has a point that is not finite",
                    self.name, key.joint, key.motion
                ));
            }
            if key.points.windows(2).any(|w| w[1].0 < w[0].0) {
                return Err(format!(
                    "{}: the points of {} {} are not in the order of their steps",
                    self.name, key.joint, key.motion
                ));
            }
        }
        Ok(())
    }

    /// The keyed joints' values at a step, by joint and motion.
    pub fn values_at(&self, step: f64) -> BTreeMap<FeatureUid, BTreeMap<Motion, f64>> {
        let mut out: BTreeMap<FeatureUid, BTreeMap<Motion, f64>> = BTreeMap::new();
        for key in &self.keys {
            if let Some(v) = key.value_at(step) {
                out.entry(key.joint).or_default().insert(key.motion, v);
            }
        }
        out
    }

    /// The key of a joint's motion, when it has one.
    pub fn key_mut(&mut self, joint: FeatureUid, motion: Motion) -> Option<&mut StudyKey> {
        self.keys
            .iter_mut()
            .find(|k| k.joint == joint && k.motion == motion)
    }
}

/// Occurrences whose bodies may touch but not pass through each other
/// while joints move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactSet {
    pub name: String,
    /// Paths from the root; a body belongs to the innermost one its
    /// instance's path starts with.
    pub occurrences: Vec<OccurrencePath>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub suppressed: bool,
}

impl ContactSet {
    /// Why the set cannot be kept, if it cannot (not its occurrences, which
    /// the document checks).
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("the contact set needs a name".to_owned());
        }
        if self.occurrences.iter().any(OccurrencePath::is_empty) {
            return Err(format!(
                "{}: the root is not an occurrence of a contact set",
                self.name
            ));
        }
        for (i, path) in self.occurrences.iter().enumerate() {
            if self.occurrences[..i].contains(path) {
                return Err(format!("{}: {path} is listed more than once", self.name));
            }
        }
        Ok(())
    }
}
