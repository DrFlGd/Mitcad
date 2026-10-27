// SPDX-License-Identifier: MIT
//! A two-sided extrusion's distances in commands (mitcad#693, mitcad#765).
//! With `signed` (what the Extrude panel sends since mitcad#765) a side's
//! distance has a sign and a negative one ends on the other side's side, so
//! commands take it as it is. Without `signed` each side goes its own way by
//! its distance's size, so `add_feature`, `edit_feature` and their previews
//! refuse a negative side distance there instead of taking its size the
//! other way. The document's own API, which the imports add their features
//! through, still takes a side's size, as the stored designs measure it.

use super::ApiError;
use crate::document::{Document, split_named};
use crate::expr::{Dims, Expr, Unit};
use crate::features::{FeatureDef, ValueInput};
use crate::ids::FeatureUid;
use crate::kernel::Kernel;
use crate::parameters::{Parameters, unit_for};

impl<K: Kernel> Document<K> {
    /// Refuses a two-sided extrusion without `signed` with a negative side
    /// distance. An edit that keeps a side's negative distance as the
    /// feature has it (an imported feature's, while another of its inputs
    /// changes) passes.
    pub(super) fn check_two_side_distances(
        &self,
        def: &FeatureDef<ValueInput>,
        edited: Option<FeatureUid>,
    ) -> Result<(), ApiError> {
        let FeatureDef::Extrude(extrude) = def else {
            return Ok(());
        };
        // Signed distances (mitcad#765): the feature's check says whether
        // the two ends leave a length between them.
        if extrude.signed_two_sides() {
            return Ok(());
        }
        let params = self.parameters();
        let before = match edited
            .and_then(|uid| self.feature(uid))
            .map(|entry| &entry.def)
        {
            Some(FeatureDef::Extrude(old)) => old
                .two_side_distances()
                .map(|id| id.and_then(|id| params.value(*id))),
            _ => [None, None],
        };
        for (k, input) in extrude.two_side_distances().into_iter().enumerate() {
            let Some(value) = input.and_then(|input| input_value(params, input)) else {
                continue;
            };
            if value < 0.0 && before[k] != Some(value) {
                return Err(ApiError(format!(
                    "side {}'s distance is {value}: without \"signed\": true each side of a \
                     two-sided extrusion goes its own way from the start and takes no negative \
                     distance; with it a negative distance ends on the other side's side",
                    k + 1
                )));
            }
        }
        Ok(())
    }
}

/// The value a command's value slot will have: a number, or a parameter or
/// an expression evaluated as a length (`name = expression` by its
/// expression). None when it does not evaluate here: adding the feature
/// then says why.
fn input_value(params: &Parameters, input: &ValueInput) -> Option<f64> {
    let text = match input {
        ValueInput::Number(number) => return Some(*number),
        ValueInput::Name(text) => split_named(text).map_or(text.as_str(), |(_, rest)| rest),
    };
    let context = params.context();
    let unit = unit_for(Dims::LENGTH, &context).unwrap_or(Unit::MM);
    let parsed = Expr::parse(text).ok()?;
    parsed
        .eval_as(params.table(), &context, unit)
        .ok()
        .map(|quantity| quantity.value)
}
