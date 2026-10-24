// SPDX-License-Identifier: MIT
//! Assembly constraints (mitcad#148): a `constraint_set` feature holds
//! rows, each a constraint between geometry of two occurrences of the
//! feature's component that takes away some of their motions (see
//! `crate::constraints` for what each type holds and what recompute does
//! with them). One row is a constraint; several are solved together, with
//! the joints and constraints before them.
//!
//! A row's references are joint origins (`features/joint.rs`): an
//! occurrence path from the feature's component and geometry of the
//! component it ends in.

use serde::{Deserialize, Serialize};

use super::joint::{JointOrigin, Limit};
use super::pattern::none;
use super::{
    CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References, is_false,
};
use crate::kernel::Kernel;
use crate::parameters::ParamId;

/// What a row holds (commands.md, "Assembly constraints").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
    /// Planes coplanar, points coincident, axes concentric (or a mix:
    /// a point on a plane or an axis, an axis on a plane).
    Align,
    /// The angle between two planes' normals or two axes.
    Angle,
    /// Side a's point centred between two faces of side b, or on one face's
    /// normal through its middle (an axis: on the axis).
    Center,
    /// Planar, cylindrical and spherical faces touching.
    Tangent,
    /// Two circles: their planes coincide and their centres meet.
    Insert,
    /// A circle on a cylinder's (a hole's) axis, depth free.
    InsertAxis,
}

impl ConstraintType {
    pub const ALL: [Self; 6] = [
        Self::Align,
        Self::Angle,
        Self::Center,
        Self::Tangent,
        Self::Insert,
        Self::InsertAxis,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Align => "align",
            Self::Angle => "angle",
            Self::Center => "center",
            Self::Tangent => "tangent",
            Self::Insert => "insert",
            Self::InsertAxis => "insert_axis",
        }
    }

    pub fn is_insert(self) -> bool {
        matches!(self, Self::Insert | Self::InsertAxis)
    }
}

impl std::fmt::Display for ConstraintType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One constraint of a set: geometry `a` of one occurrence held against
/// geometry `b` of another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintRow<P = ParamId> {
    #[serde(rename = "type")]
    pub kind: ConstraintType,
    pub a: JointOrigin,
    /// A center's second reference on a's side: a's point is between them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a2: Option<JointOrigin>,
    pub b: JointOrigin,
    /// A center's second face on b's side: a is centred between b and b2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b2: Option<JointOrigin>,
    /// An align's or insert's offset, mm.
    #[serde(default = "none", skip_serializing_if = "Option::is_none")]
    pub offset: Option<P>,
    /// An angle row's angle; a locked insert's turn about its axis.
    #[serde(default = "none", skip_serializing_if = "Option::is_none")]
    pub angle: Option<P>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip: bool,
    /// The offset (align) or angle (angle) free between limits instead,
    /// held at the rest value when there is one.
    #[serde(default = "none", skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limit<P>>,
    /// An insert holds its turn about the axis (`angle`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_rotation: bool,
}

impl<P> ConstraintRow<P> {
    // Written by hand: the slots take the row's index (`rows.0.offset`).
    fn map_params<Q, E>(
        &self,
        index: usize,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<ConstraintRow<Q>, E> {
        // An angle row's limits are angles (`rows.0.limits.min_angle`).
        let suffix = if self.kind == ConstraintType::Angle {
            "_angle"
        } else {
            ""
        };
        let mut slot = |name: &str, value: &Option<P>| -> Result<Option<Q>, E> {
            value
                .as_ref()
                .map(|v| f(&format!("rows.{index}.{name}"), v))
                .transpose()
        };
        let limits = match &self.limits {
            Some(l) => Some(Limit {
                min: slot(&format!("limits.min{suffix}"), &l.min)?,
                max: slot(&format!("limits.max{suffix}"), &l.max)?,
                rest: slot(&format!("limits.rest{suffix}"), &l.rest)?,
            }),
            None => None,
        };
        Ok(ConstraintRow {
            kind: self.kind,
            a: self.a.clone(),
            a2: self.a2.clone(),
            b: self.b.clone(),
            b2: self.b2.clone(),
            offset: slot("offset", &self.offset)?,
            angle: slot("angle", &self.angle)?,
            flip: self.flip,
            limits,
            lock_rotation: self.lock_rotation,
        })
    }

    /// Whether min or max is given: the value is free between them.
    pub fn limited(&self) -> bool {
        self.limits
            .as_ref()
            .is_some_and(|l| l.min.is_some() || l.max.is_some())
    }

    /// The references in order: a, a2, b, b2.
    pub fn origins(&self) -> impl Iterator<Item = (&'static str, &JointOrigin)> {
        [
            ("a", Some(&self.a)),
            ("a2", self.a2.as_ref()),
            ("b", Some(&self.b)),
            ("b2", self.b2.as_ref()),
        ]
        .into_iter()
        .filter_map(|(k, o)| Some((k, o?)))
    }

    /// The row's static checks (geometry types are checked at recompute).
    fn check_fields(&self) -> Result<(), String> {
        use ConstraintType as T;
        let kind = self.kind;
        if (self.a2.is_some() || self.b2.is_some()) && kind != T::Center {
            return Err("a2 and b2 are for center rows".to_owned());
        }
        if self.lock_rotation && !kind.is_insert() {
            return Err("lock_rotation is for insert rows".to_owned());
        }
        if self.limits.is_some() && !matches!(kind, T::Align | T::Angle) {
            return Err("limits are for align and angle rows".to_owned());
        }
        if self.offset.is_some() && !matches!(kind, T::Align | T::Insert) {
            return Err("offset is for align and insert rows".to_owned());
        }
        if self.angle.is_some() && !(kind == T::Angle || (kind.is_insert() && self.lock_rotation)) {
            return Err("angle is for angle rows and inserts with lock_rotation".to_owned());
        }
        if self.limited() && (self.offset.is_some() || self.angle.is_some()) {
            return Err(
                "with limits the value is free between them: give a rest value instead of \
                 an offset or angle"
                    .to_owned(),
            );
        }
        if self.flip && matches!(kind, T::Center | T::InsertAxis) {
            return Err(format!("a {kind} row has no flip"));
        }
        Ok(())
    }

    fn check_values(&self, value: &dyn Fn(&P) -> f64) -> Result<(), String> {
        let angle_row = self.kind == ConstraintType::Angle;
        let in_range = |v: f64| !angle_row || (-1e-12..=std::f64::consts::PI + 1e-12).contains(&v);
        if let Some(angle) = &self.angle
            && angle_row
            && !in_range(value(angle))
        {
            return Err("the angle must be between 0 and 180 degrees".to_owned());
        }
        if let Some(l) = &self.limits {
            let (min, max, rest) = (
                l.min.as_ref().map(value),
                l.max.as_ref().map(value),
                l.rest.as_ref().map(value),
            );
            if let (Some(lo), Some(hi)) = (min, max)
                && lo > hi
            {
                return Err("limits: the minimum is greater than the maximum".to_owned());
            }
            let outside = |v: f64| min.is_some_and(|m| v < m) || max.is_some_and(|m| v > m);
            if rest.is_some_and(outside) {
                return Err("limits: the rest value is outside the limits".to_owned());
            }
            if [min, max, rest].into_iter().flatten().any(|v| !in_range(v)) {
                return Err("an angle's limits must be between 0 and 180 degrees".to_owned());
            }
        }
        Ok(())
    }
}

/// The name of a row in messages: `row 2 (insert)`.
pub fn row_label(index: usize, kind: ConstraintType) -> String {
    format!("row {} ({kind})", index + 1)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintSetDef<P = ParamId> {
    pub rows: Vec<ConstraintRow<P>>,
}

impl<P> ConstraintSetDef<P> {
    pub const TYPE: &'static str = "constraint_set";
    pub const BASE_NAME: &'static str = "Constraint";

    // Written by hand: each row maps under its index.
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<ConstraintSetDef<Q>, E> {
        Ok(ConstraintSetDef {
            rows: self
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| row.map_params(i, f))
                .collect::<Result<_, _>>()?,
        })
    }
}

impl FeatureInfo for ConstraintSetDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for row in &self.rows {
            for (_, origin) in row.origins() {
                origin.add_references(&mut references);
            }
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        if self.rows.is_empty() {
            return Err("a constraint set has at least one row".to_owned());
        }
        for (i, row) in self.rows.iter().enumerate() {
            let label = row_label(i, row.kind);
            row.check_fields().map_err(|e| format!("{label}: {e}"))?;
            for (key, origin) in row.origins() {
                origin
                    .check(ctx)
                    .map_err(|e| format!("{label}: {key}: {e}"))?;
            }
            if row.a.occurrence.is_empty() && row.b.occurrence.is_empty() {
                return Err(format!(
                    "{label}: a constraint relates occurrences: a and b are both the \
                     component's own geometry"
                ));
            }
            for (key, second, first) in [("a2", &row.a2, &row.a), ("b2", &row.b2, &row.b)] {
                if second
                    .as_ref()
                    .is_some_and(|o| o.occurrence != first.occurrence)
                {
                    return Err(format!(
                        "{label}: {key} is on another occurrence than {}",
                        &key[..1]
                    ));
                }
            }
            if row.a.occurrence == row.b.occurrence {
                return Err(format!(
                    "{label}: a and b are on the same occurrence ({})",
                    row.a.occurrence
                ));
            }
        }
        Ok(())
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        for (i, row) in self.rows.iter().enumerate() {
            row.check_values(&|id: &ParamId| value(*id))
                .map_err(|e| format!("{}: {e}", row_label(i, row.kind)))?;
        }
        Ok(())
    }
}

impl<K: Kernel> Evaluate<K> for ConstraintSetDef {
    /// Reads the rows' values, so the cache keys on them, and checks them;
    /// recompute then solves the rows (`crate::constraints`).
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        for (i, row) in self.rows.iter().enumerate() {
            let ids: Vec<ParamId> = row
                .offset
                .iter()
                .chain(&row.angle)
                .chain(
                    row.limits
                        .iter()
                        .flat_map(|l| [&l.min, &l.max, &l.rest])
                        .flatten(),
                )
                .copied()
                .collect();
            let mut values = std::collections::BTreeMap::new();
            for id in ids {
                values.insert(id, ctx.param(id)?);
            }
            row.check_values(&|id: &ParamId| values.get(id).copied().unwrap_or(0.0))
                .map_err(|e| format!("{}: {e}", row_label(i, row.kind)))?;
        }
        Ok(FeatureOutput::default())
    }
}
