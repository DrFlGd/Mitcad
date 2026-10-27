// SPDX-License-Identifier: MIT
//! Pulleys (mitcad#562): the `pulley_values` query, the derived values a
//! Pulley panel shows (diameters, and with `belt_with` the belt's length,
//! its nearest catalogue length and the teeth in mesh). See `commands.md`,
//! "Pulleys".

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::prelude::*;
use mitcad_model::Document;
use mitcad_model::expr::Expr;
use mitcad_model::features::pulley::{self, PulleyDef};
use mitcad_model::features::{FeatureDef, ValueInput};
use mitcad_model::ids::FeatureUid;
use mitcad_model::kernel::Kernel;
use mitcad_model::parameters::{slot_unit, unit_for};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PulleyValuesQuery {
    /// A pulley feature of the timeline.
    #[serde(default)]
    uid: Option<FeatureUid>,
    /// Or a definition as `add_feature` takes it (a panel's).
    #[serde(default)]
    def: Option<Box<PulleyDef<ValueInput>>>,
    /// Catalogue belt lengths (mm): the nearest to the belt's is given.
    #[serde(default)]
    belt_lengths: Option<Vec<f64>>,
}

/// The length of `lengths` nearest to `length`.
pub(crate) fn nearest_length(lengths: &[f64], length: f64) -> Option<f64> {
    lengths
        .iter()
        .copied()
        .filter(|l| l.is_finite())
        .min_by(|a, b| (a - length).abs().total_cmp(&(b - length).abs()))
}

/// The document's methods of this module used across the model, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(crate) trait PulleysCrateOps<K: Kernel> {
    /// A pulley feature's definition with the document's values.
    fn pulley_values_of(&self, uid: FeatureUid) -> Result<PulleyDef<f64>, ApiError>;
}

impl<K: Kernel> PulleysCrateOps<K> for Document<K> {
    fn pulley_values_of(&self, uid: FeatureUid) -> Result<PulleyDef<f64>, ApiError> {
        let entry = self
            .feature(uid)
            .ok_or_else(|| ApiError(format!("feature {uid} does not exist")))?;
        let FeatureDef::Pulley(def) = &entry.def else {
            return Err(ApiError(format!("{} is not a pulley", entry.name)));
        };
        let params = self.parameters();
        Ok(pulley::evaluated(def, &|id| {
            params.value(id).unwrap_or(f64::NAN)
        }))
    }
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait PulleysOps<K: Kernel> {
    fn pulley_values_json(&self, query: &PulleyValuesQuery) -> Result<Value, ApiError>;
}

impl<K: Kernel> PulleysOps<K> for Document<K> {
    fn pulley_values_json(&self, query: &PulleyValuesQuery) -> Result<Value, ApiError> {
        let (mut values, component) = match (&query.uid, &query.def) {
            (Some(uid), None) => (
                self.pulley_values_of(*uid)?,
                self.feature(*uid)
                    .map_or(self.active_component(), |f| f.component),
            ),
            (None, Some(def)) => (self.pulley_values_in(def)?, self.active_component()),
            _ => {
                return Err(ApiError(
                    "pulley_values takes a pulley feature's uid or a definition".to_owned(),
                ));
            }
        };
        // On a shaft (mitcad#578): the bore the shaft gives at the marker.
        let seat = values
            .seat
            .as_ref()
            .map(|seat| self.seat_frame(seat, component));
        if let Some(Ok(frame)) = &seat {
            values.seat_bore(frame.diameter);
        }
        // Placed by picked geometry (mitcad#656): a shaft gives the bore
        // as a seat does.
        let placed = values.at.clone().map(|at| self.place_frame(&at));
        if let Some(Ok(placed)) = &placed
            && let Some(frame) = placed.round()
        {
            values.seat_bore(frame.diameter);
        }
        let mut result = pulley::derived_values(&values);
        if let Err(error) = values.check() {
            result["error"] = json!(error);
        }
        match seat {
            Some(Ok(frame)) => {
                result["shaft_diameter"] = json!(frame.diameter);
                result["seat"] = frame.to_json();
            }
            Some(Err(error)) => result["error"] = json!(format!("seat: {error}")),
            None => {}
        }
        match placed {
            Some(Ok(placed)) => {
                if let Some(frame) = placed.round() {
                    result["shaft_diameter"] = json!(frame.diameter);
                }
                if let Some(frame) = &placed.seat {
                    result["seat"] = frame.to_json();
                }
            }
            Some(Err(error)) => result["error"] = json!(format!("at: {error}")),
            None => {}
        }
        // A belt kept checked with an earlier pulley (mitcad#656), on the
        // pulleys as built.
        if let (Some(check), Some(uid)) = (&values.belt_check, query.uid) {
            result["belt_check"] = match self.belt_check_value(
                check.pulley,
                uid,
                query.belt_lengths.as_deref(),
                false,
                3,
            ) {
                Ok(mut value) => {
                    value["pulley"] = json!(check.pulley);
                    value
                }
                Err(error) => json!({"pulley": check.pulley, "error": error.0}),
            };
        }
        if let Some(belt) = &values.belt_with {
            let mate = self.pulley_values_of(belt.pulley)?;
            match pulley::belt_drive(&values, &mate, belt.center_distance) {
                Ok(drive) => {
                    result["center_distance"] = json!(drive.center_distance);
                    result["mate"] = json!(belt.pulley);
                    result["belt_length"] = json!(drive.belt_length);
                    result["wrap_angles"] = json!([drive.wrap.0, drive.wrap.1]);
                    if let Some(teeth) = drive.belt_teeth {
                        result["belt_teeth"] = json!(teeth);
                    }
                    if let Some(teeth) = drive.teeth_in_mesh {
                        result["teeth_in_mesh"] = json!(teeth);
                    }
                    if let Some(length) = query
                        .belt_lengths
                        .as_deref()
                        .and_then(|lengths| nearest_length(lengths, drive.belt_length))
                    {
                        result["catalogue_length"] = json!(length);
                    }
                }
                Err(error) => result["error"] = json!(error),
            }
        }
        Ok(result)
    }
}

/// The document's methods used only in this module, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
trait PulleysLocal<K: Kernel> {
    /// A definition's values: numbers as they are (mm, radians), names and
    /// expressions evaluated in the slot's unit.
    fn pulley_values_in(&self, def: &PulleyDef<ValueInput>) -> Result<PulleyDef<f64>, ApiError>;
}

impl<K: Kernel> PulleysLocal<K> for Document<K> {
    fn pulley_values_in(&self, def: &PulleyDef<ValueInput>) -> Result<PulleyDef<f64>, ApiError> {
        let params = mitcad_model::internal::print_fits::evaluation_parameters(self.parameters())
            .map_err(|e| ApiError(e.to_string()))?;
        let context = params.context();
        def.map_params(&mut |slot, value| match value {
            ValueInput::Number(n) => Ok(*n),
            ValueInput::Name(text) => {
                let unit = unit_for(slot_unit(slot).dims(), &context)
                    .unwrap_or(mitcad_model::expr::Unit::NONE);
                let parsed = Expr::parse(text).map_err(|e| ApiError(format!("{slot}: {e}")))?;
                parsed
                    .eval_as(params.table(), &context, unit)
                    .map(|v| v.value)
                    .map_err(|e| ApiError(format!("{slot}: {e}")))
            }
        })
    }
}
