// SPDX-License-Identifier: MIT
//! Seats on shafts (mitcad#578): the `seat_frame` query, where a seat puts
//! a part at the timeline marker (the panels of seated features show the
//! shaft's diameter with it, and put a part that leaves its shaft where it
//! was). See `commands.md`, "Seats on shafts".

use serde::Deserialize;
use serde_json::Value;

use super::ApiError;
use mitcad_model::Document;
use mitcad_model::expr::Expr;
use mitcad_model::features::{Seat, ValueInput};
use mitcad_model::ids::ComponentUid;
use mitcad_model::kernel::Kernel;
use mitcad_model::parameters::{slot_unit, unit_for};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeatFrameQuery {
    /// A seat as a feature's `seat` takes it.
    seat: Seat<ValueInput>,
    /// The seated feature's component (the active one without it).
    #[serde(default)]
    component: Option<ComponentUid>,
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait SeatsOps<K: Kernel> {
    fn seat_frame_json(&self, query: &SeatFrameQuery) -> Result<Value, ApiError>;

    /// A seat's values: numbers as they are (mm), names and expressions
    /// evaluated in the slot's unit (also Gear Pair's and Belt Drive's
    /// queries, mitcad#754).
    fn seat_values(&self, seat: &Seat<ValueInput>) -> Result<Seat<f64>, ApiError>;
}

impl<K: Kernel> SeatsOps<K> for Document<K> {
    fn seat_frame_json(&self, query: &SeatFrameQuery) -> Result<Value, ApiError> {
        let seat = self.seat_values(&query.seat)?;
        let component = query.component.unwrap_or(self.active_component());
        self.seat_frame(&seat, component)
            .map(|frame| frame.to_json())
            .map_err(ApiError)
    }

    fn seat_values(&self, seat: &Seat<ValueInput>) -> Result<Seat<f64>, ApiError> {
        let params = self.parameters();
        let context = params.context();
        seat.map_params("seat.", &mut |slot, value| match value {
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
