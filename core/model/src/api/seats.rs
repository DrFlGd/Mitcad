// SPDX-License-Identifier: MIT
//! Seats on shafts (mitcad#578): the `seat_frame` query, where a seat puts
//! a part at the timeline marker (the panels of seated features show the
//! shaft's diameter with it, and put a part that leaves its shaft where it
//! was). See `commands.md`, "Seats on shafts".

use serde::Deserialize;
use serde_json::Value;

use super::ApiError;
use crate::document::Document;
use crate::expr::Expr;
use crate::features::{Seat, ValueInput};
use crate::ids::ComponentUid;
use crate::kernel::Kernel;
use crate::parameters::{slot_unit, unit_for};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeatFrameQuery {
    /// A seat as a feature's `seat` takes it.
    seat: Seat<ValueInput>,
    /// The seated feature's component (the active one without it).
    #[serde(default)]
    component: Option<ComponentUid>,
}

impl<K: Kernel> Document<K> {
    pub(super) fn seat_frame_json(&self, query: &SeatFrameQuery) -> Result<Value, ApiError> {
        let params = self.parameters();
        let context = params.context();
        let seat = query
            .seat
            .map_params("seat.", &mut |slot, value| match value {
                ValueInput::Number(n) => Ok(*n),
                ValueInput::Name(text) => {
                    let unit = unit_for(slot_unit(slot).dims(), &context)
                        .unwrap_or(crate::expr::Unit::NONE);
                    let parsed = Expr::parse(text).map_err(|e| ApiError(format!("{slot}: {e}")))?;
                    parsed
                        .eval_as(params.table(), &context, unit)
                        .map(|v| v.value)
                        .map_err(|e| ApiError(format!("{slot}: {e}")))
                }
            })?;
        let component = query.component.unwrap_or(self.active_component());
        self.seat_frame(&seat, component)
            .map(|frame| frame.to_json())
            .map_err(ApiError)
    }
}
